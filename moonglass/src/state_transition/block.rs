//! Per-block processing phases.
//!
//! Block processing applies the parent payload's committed requests, validates
//! the block identity, applies withdrawals and the accepted builder bid, then
//! processes randomness, deposit-chain votes, operations, payload-timeliness
//! votes, and sync-committee participation.

mod parent_payload;
mod sync_aggregate;

use sha2::{Digest, Sha256};

use crate::constants::{DOMAIN_RANDAO, EPOCHS_PER_HISTORICAL_VECTOR, ETH1_DATA_VOTES_LEN};
use crate::containers::{BeaconBlock, BeaconBlockBody, BeaconState};
use crate::error::{BlockError, MerkleError, SignatureError, TransitionError};
use crate::primitives::Root;
use crate::state_transition::{BeaconStateLookup, TreeRootExt, verify_signature};

impl BeaconState {
    /// Apply the per-block sub-phases of `block`.
    ///
    /// Spec: `process_block`
    pub fn process_block(&mut self, block: &BeaconBlock) -> Result<(), TransitionError> {
        self.accept_parent_payload_commitment(block)?;
        self.process_block_header(block)?;
        self.process_withdrawals()?;
        self.process_execution_payload_bid(block)?;
        self.process_randao(&block.body)?;
        self.process_eth1_data(&block.body)?;
        self.process_operations(&block.body)?;
        self.process_sync_aggregate(&block.body.sync_aggregate)?;
        Ok(())
    }

    /// Validate the block's identity fields and cache its header.
    ///
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
        self.latest_block_header = block.header(body_root, Root::ZERO);
        if self.validator(block.proposer_index)?.slashed {
            return Err(BlockError::ProposerSlashed(block.proposer_index).into());
        }
        Ok(())
    }

    /// Verify and mix the proposer's RANDAO reveal.
    ///
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

    /// Append the proposer's deposit-chain vote and promote it on majority.
    ///
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
