//! Consensus state transition.
//!
//! The transition moves a [`BeaconState`] from one accepted block to the next.
//! It advances empty slots, verifies the proposer's domain-separated signature,
//! applies the block-processing phases, and finally checks that the computed
//! post-state root matches the root claimed by the block.
//!
//! Execution payload envelopes are verified against the bid and expected
//! consensus-side commitments. Execution-engine validity and blob/data
//! availability are not yet implemented.
//!
//! # Reading routes
//!
//! - Block transition: [`BeaconState::apply_signed_block`] clones the pre-state,
//!   runs [`BeaconState::process_slots`], verifies the proposer signature,
//!   applies [`BeaconState::process_block`], checks the claimed post-state root,
//!   then commits the clone back to `self`.
//! - Slot transition: [`BeaconState::process_slots`] repeatedly calls
//!   [`BeaconState::process_slot`], which records recent roots, advances the
//!   clock, clears the payload-availability bit for the next slot, and runs
//!   [`BeaconState::process_epoch`] at epoch boundaries.
//! - Block body transition: [`BeaconState::process_block`] handles the previous
//!   slot's parent payload commitment before accepting the current slot's
//!   builder bid and operations.
//! - Operation transition: [`BeaconState::process_operations`] runs slashings,
//!   beacon attestations, exits, credential changes, and payload
//!   attestations in consensus order.
//!
//! The main functions keep spec shape. Smaller helpers may be named around the
//! state handoff they make visible.

mod balance;
mod block;
mod builder;
mod committee;
mod epoch;
mod operations;
mod signing;
mod slot;
mod validator;
mod withdrawal;

pub use committee::*;
pub use operations::*;
pub use signing::*;
pub use validator::*;

use crate::constants::{
    DOMAIN_BEACON_BUILDER, DOMAIN_BEACON_PROPOSER, GENESIS_SLOT, SLOT_DURATION_MS,
};
use crate::containers::{
    BeaconState, ExecutionPayloadEnvelope, SignedBeaconBlock, SignedExecutionPayloadEnvelope,
};
use crate::error::{BlockError, MerkleError, SignatureError, TransitionError};
use crate::primitives::{BLSPubkey, Root};

#[doc(hidden)]
pub(crate) trait TreeRootExt {
    /// Compute the SSZ tree root, emitting `on_fail` on merkleization failure.
    fn tree_root(&mut self, on_fail: MerkleError) -> Result<Root, TransitionError>;
}

impl<T> TreeRootExt for T
where
    T: ssz_rs::Merkleized,
{
    fn tree_root(&mut self, on_fail: MerkleError) -> Result<Root, TransitionError> {
        ssz_rs::Merkleized::hash_tree_root(self)
            .map(Root::from)
            .map_err(|_| on_fail.into())
    }
}

impl BeaconState {
    /// Apply `signed_block` and update this state in place to the post-state.
    ///
    /// The transition runs on a clone so a rejected block leaves `self`
    /// untouched. The clone advances empty slots up to the block slot with
    /// [`BeaconState::process_slots`], verifies the proposer's domain-separated
    /// signature, runs the block-processing phases with
    /// [`BeaconState::process_block`], and confirms the computed root matches
    /// `signed_block.message.state_root`. Only after the post-state root agrees
    /// does the clone replace `self`. A root disagreement raises
    /// [`TransitionError::StateRootMismatch`], so a forged or stale state root
    /// can never be committed.
    /// Spec: `state_transition`
    pub fn apply_signed_block(
        &mut self,
        signed_block: &SignedBeaconBlock,
    ) -> Result<(), TransitionError> {
        let mut next = self.clone();
        next.process_slots(signed_block.message.slot)?;
        next.verify_block_signature(signed_block)?;
        next.process_block(&signed_block.message)?;
        next.expect_post_state_root(signed_block.message.state_root)?;
        *self = next;
        Ok(())
    }

    /// Verify a delivered execution payload envelope for the current block.
    ///
    /// The envelope must name the current block through `beacon_block_root` and
    /// the parent through `parent_beacon_block_root`, then the required
    /// domain-separated signature is checked and the covered committed fields are
    /// matched against the bid accepted into
    /// [`BeaconState::latest_execution_payload_bid`]. Builder index, RANDAO, gas
    /// limit, block hash, requests root, slot, parent hash, timestamp, and
    /// withdrawals must all line up, and any mismatch raises a [`BlockError`].
    /// This is a consensus-side validation step only: it does not mutate durable
    /// `BeaconState`, does not run the execution engine, and does not check blob
    /// or data availability. Fork choice records the checked envelope in
    /// [`crate::fork_choice::Store::payloads`], and the committed requests are
    /// applied later by a child block through
    /// [`BeaconState::accept_parent_payload_commitment`].
    pub fn verify_execution_payload_envelope(
        &mut self,
        signed_envelope: &SignedExecutionPayloadEnvelope,
    ) -> Result<(), TransitionError> {
        let envelope = &signed_envelope.message;
        if envelope.beacon_block_root != self.current_block_root()? {
            return Err(BlockError::EnvelopeBlockRootMismatch.into());
        }
        if envelope.parent_beacon_block_root != self.latest_block_header.parent_root {
            return Err(BlockError::EnvelopeParentMismatch.into());
        }

        self.verify_execution_payload_envelope_signature(signed_envelope)?;
        self.validate_execution_payload_envelope(envelope)
    }

    /// Compute the block root represented by the state's latest block header
    /// after filling in the current state root.
    fn current_block_root(&mut self) -> Result<Root, TransitionError> {
        let state_root = self.tree_root(MerkleError::BeaconState)?;
        let mut header = self.latest_block_header.with_state_root(state_root);
        header.tree_root(MerkleError::BeaconBlockHeader)
    }

    /// Verify the builder or self-build signature on an execution payload envelope.
    fn verify_execution_payload_envelope_signature(
        &self,
        signed_envelope: &SignedExecutionPayloadEnvelope,
    ) -> Result<(), TransitionError> {
        let envelope = &signed_envelope.message;
        let signer_pubkey = self.execution_payload_envelope_signer(envelope.builder_index)?;
        let mut envelope_msg = envelope.clone();
        let signing_root = self.signing_root_for(
            &mut envelope_msg,
            DOMAIN_BEACON_BUILDER,
            self.slot.epoch(),
            MerkleError::ExecutionPayloadEnvelope,
        )?;
        verify_signature(
            &signer_pubkey,
            signing_root,
            &signed_envelope.signature,
            SignatureError::ExecutionPayloadEnvelope(envelope.builder_index),
        )
    }

    /// Return the public key that must have signed the payload envelope.
    ///
    /// Self-build envelopes are signed by the beacon proposer. Non-self-build
    /// envelopes are signed by the registered builder named in the envelope.
    fn execution_payload_envelope_signer(
        &self,
        builder_index: crate::primitives::BuilderIndex,
    ) -> Result<BLSPubkey, TransitionError> {
        if builder_index == crate::constants::BUILDER_INDEX_SELF_BUILD {
            let proposer = self.beacon_proposer_index()?;
            return Ok(self.validator(proposer)?.pubkey);
        }
        Ok(self.builder(builder_index)?.pubkey)
    }

    /// Check an envelope against the latest accepted execution payload bid.
    ///
    /// This is the consensus-side boundary: it validates committed fields and
    /// expected withdrawals, but does not run execution-engine validity or blob
    /// data-availability verification.
    fn validate_execution_payload_envelope(
        &self,
        envelope: &ExecutionPayloadEnvelope,
    ) -> Result<(), TransitionError> {
        let bid = &self.latest_execution_payload_bid;
        if envelope.builder_index != bid.builder_index {
            return Err(BlockError::EnvelopeBuilderMismatch.into());
        }
        if envelope.payload.prev_randao != bid.prev_randao {
            return Err(BlockError::EnvelopeRandaoMismatch.into());
        }
        if envelope.payload.gas_limit != bid.gas_limit {
            return Err(BlockError::EnvelopeGasLimitMismatch.into());
        }
        if envelope.payload.block_hash != bid.block_hash {
            return Err(BlockError::EnvelopePayloadHashMismatch.into());
        }
        let mut requests = envelope.execution_requests.clone();
        let requests_root = requests.tree_root(MerkleError::ExecutionRequests)?;
        if requests_root != bid.execution_requests_root {
            return Err(BlockError::EnvelopeRequestsRootMismatch.into());
        }
        if envelope.payload.slot_number != self.slot.as_u64() {
            return Err(BlockError::EnvelopeSlotMismatch.into());
        }
        if envelope.payload.parent_hash != self.latest_block_hash {
            return Err(BlockError::EnvelopeParentHashMismatch {
                got: envelope.payload.parent_hash,
                want: self.latest_block_hash,
            }
            .into());
        }
        let expected_timestamp = self.expected_execution_payload_timestamp()?;
        if envelope.payload.timestamp != expected_timestamp {
            return Err(BlockError::EnvelopeTimestampMismatch {
                got: envelope.payload.timestamp,
                want: expected_timestamp,
            }
            .into());
        }
        if envelope.payload.withdrawals != self.payload_expected_withdrawals {
            return Err(BlockError::WithdrawalsRootMismatch.into());
        }
        Ok(())
    }

    /// Expected execution payload timestamp for the state's current slot.
    fn expected_execution_payload_timestamp(&self) -> Result<u64, TransitionError> {
        // Spec: `compute_time_at_slot`. Multiply slot * SLOT_DURATION_MS first
        // and divide by 1000 last so the result is exact when SLOT_DURATION_MS
        // is not a multiple of 1000.
        let slots_since_genesis = self.slot.as_u64().saturating_sub(GENESIS_SLOT.as_u64());
        slots_since_genesis
            .checked_mul(SLOT_DURATION_MS)
            .map(|ms| ms / 1_000)
            .and_then(|seconds_offset| self.genesis_time.checked_add(seconds_offset))
            .ok_or_else(|| BlockError::EnvelopeTimestampOverflow.into())
    }

    /// Verify the proposer's signature over `signed_block.message`.
    ///
    /// The expected signer is the validator at `signed_block.message.proposer_index`,
    /// and the signing root combines the block root with the proposer domain at
    /// the state's current epoch. A signature that does not verify raises a
    /// [`SignatureError::BlockProposer`], which keeps a block whose body was
    /// authored by anyone other than the slot's proposer out of the transition.
    pub fn verify_block_signature(
        &self,
        signed_block: &SignedBeaconBlock,
    ) -> Result<(), TransitionError> {
        let proposer_index = signed_block.message.proposer_index;
        let pubkey = self.validator(proposer_index)?.pubkey;
        let domain = self.domain_for(DOMAIN_BEACON_PROPOSER, self.slot.epoch())?;
        let mut block = signed_block.message.clone();
        let signing_root = compute_signing_root(&mut block, domain, MerkleError::BeaconBlock)?;
        verify_signature(
            &pubkey,
            signing_root,
            &signed_block.signature,
            SignatureError::BlockProposer(proposer_index),
        )
    }

    /// Compare the computed post-state root against the block's claimed root.
    fn expect_post_state_root(&mut self, expected: Root) -> Result<(), TransitionError> {
        let post_root = self.tree_root(MerkleError::BeaconState)?;
        if expected != post_root {
            return Err(TransitionError::StateRootMismatch {
                got: expected,
                want: post_root,
            });
        }
        Ok(())
    }
}
