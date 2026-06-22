//! Per-block processing phases.
//!
//! Block processing applies the parent payload's committed requests, validates
//! the block identity, applies withdrawals and the accepted builder bid, then
//! processes randomness, deposit-chain votes, operations, payload-timeliness
//! votes, and sync-committee participation.
//!
//! The first phase is intentionally cross-slot: a child block settles the parent
//! block's payload effects before its own current-slot bid is accepted. That
//! order is the main payload handoff to watch when tracing state writes.

mod parent_payload;
mod sync_aggregate;

use sha2::{Digest, Sha256};

use crate::constants::{DOMAIN_RANDAO, EPOCHS_PER_HISTORICAL_VECTOR, ETH1_DATA_VOTES_LEN};
use crate::containers::{BeaconBlock, BeaconBlockBody, BeaconState};
use crate::error::{BlockError, MerkleError, SignatureError, TransitionError};
use crate::primitives::Root;
use crate::state_transition::{BeaconStateLookup, TreeRootExt, verify_signature};

impl BeaconState {
    /// Apply the per-block sub-phases of `block` in consensus order.
    ///
    /// The first phase, [`BeaconState::accept_parent_payload_commitment`],
    /// settles the parent block's delivered payload by applying its execution
    /// requests, releasing the parent builder payment, and marking the parent
    /// payload available, all before the current slot's own identity and bid are
    /// touched. The remaining phases then validate and cache the block header,
    /// apply withdrawals, accept the current builder bid, mix the RANDAO reveal,
    /// record the deposit-chain vote, process the body operations, and reward
    /// sync-committee participation. When this runs through
    /// [`BeaconState::apply_signed_block`], a failure in any phase aborts the
    /// cloned transition before it replaces the caller's state.
    /// Spec: `process_block`
    pub fn process_block(&mut self, block: &BeaconBlock) -> Result<(), TransitionError> {
        // Previous-slot payload handoff: settle the parent payload before this
        // slot records its own payload commitment.
        self.accept_parent_payload_commitment(block)?;

        // Current-slot block identity and body processing.
        self.process_block_header(block)?;
        self.process_withdrawals()?;
        self.process_execution_payload_bid(&block.body.signed_execution_payload_bid)?;
        self.process_randao(&block.body)?;
        self.process_eth1_data(&block.body)?;
        self.process_operations(&block.body)?;
        self.process_sync_aggregate(&block.body.sync_aggregate)?;
        Ok(())
    }

    /// Validate the block's identity fields and cache its header.
    ///
    /// The block's `slot` must equal the state slot and lie strictly after the
    /// parent header's slot, the `proposer_index` must match the slot's expected
    /// proposer, the `parent_root` must hash-match the cached parent header, and
    /// that proposer must not already be slashed. When all hold, the block's
    /// header is stored as `latest_block_header` with a zero state root, which a
    /// later [`BeaconState::process_slot`] backfills. Any mismatch raises a
    /// [`BlockError`], rejecting a block that claims the wrong slot, proposer, or
    /// parent.
    /// Spec: `process_block_header`
    pub fn process_block_header(&mut self, block: &BeaconBlock) -> Result<(), TransitionError> {
        if block.slot != self.slot {
            return Err(BlockError::BlockSlotMismatch {
                block: block.slot,
                state: self.slot,
            }
            .into());
        }
        if block.slot <= self.latest_block_header.slot {
            return Err(BlockError::SlotNotAfterParent {
                block: block.slot,
                parent: self.latest_block_header.slot,
            }
            .into());
        }
        let expected_proposer = self.beacon_proposer_index()?;
        if block.proposer_index != expected_proposer {
            return Err(BlockError::ProposerIndexMismatch {
                got: block.proposer_index,
                want: expected_proposer,
            }
            .into());
        }
        let parent_root = self
            .latest_block_header
            .tree_root(MerkleError::BeaconBlockHeader)?;
        if block.parent_root != parent_root {
            return Err(BlockError::ParentRootMismatch {
                got: block.parent_root,
                want: parent_root,
            }
            .into());
        }
        let body_root = block.body.clone().tree_root(MerkleError::BeaconBlockBody)?;
        if self.validator(block.proposer_index)?.slashed {
            return Err(BlockError::ProposerSlashed(block.proposer_index).into());
        }
        self.latest_block_header = block.header(body_root, Root::ZERO);
        Ok(())
    }

    /// Verify the proposer's RANDAO reveal and fold it into the mix.
    ///
    /// The reveal is checked as the proposer's signature over the current epoch
    /// under the RANDAO domain, rejecting a forged reveal with a
    /// [`SignatureError::RandaoReveal`]. On success its hash is mixed by
    /// exclusive-or into the current epoch's slot of `randao_mixes`, advancing
    /// the chain randomness that later seeds committee, proposer, and
    /// sync-committee sampling.
    /// Spec: `process_randao`
    pub fn process_randao(&mut self, body: &BeaconBlockBody) -> Result<(), TransitionError> {
        let epoch = self.slot.epoch();
        let proposer_index = self.beacon_proposer_index()?;
        let pubkey = self.validator(proposer_index)?.pubkey;
        let mut epoch_object = epoch;
        let signing_root =
            self.signing_root_for(&mut epoch_object, DOMAIN_RANDAO, epoch, MerkleError::Epoch)?;
        verify_signature(
            &pubkey,
            signing_root,
            &body.randao_reveal,
            SignatureError::RandaoReveal,
        )?;

        let mix_index = epoch % EPOCHS_PER_HISTORICAL_VECTOR;
        let reveal_hash: [u8; 32] = Sha256::digest(body.randao_reveal.0).into();
        let mut mix = self.randao_mixes[mix_index];
        for (m, h) in mix.iter_mut().zip(reveal_hash) {
            *m ^= h;
        }
        self.randao_mixes[mix_index] = mix;
        Ok(())
    }

    /// Record the proposer's deposit-chain vote and promote it on majority.
    ///
    /// The vote in `body.eth1_data` is appended to `eth1_data_votes`, which is
    /// rejected with [`BlockError::Eth1VotesFull`] once the period's bag is
    /// already full. When more than half the period's slots have now voted for
    /// the same data, it is promoted into `eth1_data` as the deposit source the
    /// next deposit proofs verify against. The bag itself is cleared only later,
    /// at the voting-period boundary.
    /// Spec: `process_eth1_data`
    pub fn process_eth1_data(&mut self, body: &BeaconBlockBody) -> Result<(), TransitionError> {
        if self.eth1_data_votes.len() >= ETH1_DATA_VOTES_LEN {
            return Err(BlockError::Eth1VotesFull.into());
        }
        self.eth1_data_votes.push(body.eth1_data);
        let votes_for = self
            .eth1_data_votes
            .iter()
            .filter(|v| **v == body.eth1_data)
            .count();
        if votes_for * 2 > ETH1_DATA_VOTES_LEN {
            self.eth1_data = body.eth1_data;
        }
        Ok(())
    }
}
