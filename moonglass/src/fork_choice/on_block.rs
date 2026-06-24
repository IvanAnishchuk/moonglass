//! Taking in a new [block](crate::glossary#beacon-block).
//!
//! [`Store::on_block`] is the doorway from the state transition into fork choice.
//! Given a signed block whose parent we already know, it runs the block through
//! the state transition and, if that succeeds, files the block and its resulting
//! state in the store, sets up its empty vote vectors, and records its
//! timeliness, proposer boost, and [checkpoints](crate::glossary#checkpoint).
//! The state transition itself checks and applies the
//! [attestations](crate::glossary#attestation) and slashings carried inside the
//! block. Turning those into fork-choice votes is left to the other handlers,
//! wired up by [`Store::on_block_with_embedded_messages`] for the reference tests.

use crate::constants::PTC_SIZE;
use crate::containers::{
    BeaconState, PayloadAttestation, PayloadAttestationMessage, SignedBeaconBlock,
};
use crate::error::{ForkChoiceError, MerkleError};
use crate::primitives::BLSSignature;
use crate::state_transition::TreeRootExt as _;

use super::store::Store;

impl Store {
    /// Validate a new block, compute its state, and file both in the store.
    ///
    /// This is the main entry for accepting a block. It first refuses the block
    /// outright if the parent is unknown, if the block claims a full parent whose
    /// payload we have not seen, if it is from the future, or if it does not
    /// descend from what we have already finalized. It then runs the state
    /// transition on a copy of the parent's state to derive the new block's state.
    /// On success it records the block and state, seeds empty payload-vote
    /// vectors, replays the payload-attestation votes carried in the block, and
    /// updates timeliness, proposer boost, and checkpoints. All of this is local
    /// fork-choice bookkeeping, not new chain state.
    ///
    /// Runs on a scratch copy and commits only on success, so a rejected block
    /// leaves the store unchanged.
    pub fn on_block(&mut self, signed_block: &SignedBeaconBlock) -> Result<(), ForkChoiceError> {
        let mut scratch = self.clone();
        scratch.on_block_inner(signed_block)?;
        *self = scratch;
        Ok(())
    }

    /// Non-transactional body of [`Self::on_block`], used by
    /// [`Self::on_block_with_embedded_messages`] inside its own transaction.
    pub fn on_block_inner(
        &mut self,
        signed_block: &SignedBeaconBlock,
    ) -> Result<(), ForkChoiceError> {
        let block = &signed_block.message;

        // Phase 1: pre-import validation. Each check rejects the block outright.
        // Bind the parent post-state up front, which both proves the parent is
        // known and gives phase 2 the state to copy without a second lookup.
        let parent_state = self
            .block_states
            .get(&block.parent_root)
            .ok_or(ForkChoiceError::UnknownParent(block.parent_root))?;
        if self.is_parent_node_full(block)? && !self.is_payload_verified(block.parent_root) {
            return Err(ForkChoiceError::PayloadParentEnvelopeNotRecorded(
                block.parent_root,
            ));
        }
        let current = self.get_current_slot();
        if current < block.slot {
            return Err(ForkChoiceError::BlockFromFuture {
                block_slot: block.slot,
                current_slot: current,
            });
        }
        let finalized_slot = self.finalized_checkpoint.epoch.start_slot();
        if block.slot <= finalized_slot {
            return Err(ForkChoiceError::BlockBeforeFinalizedSlot {
                block_slot: block.slot,
                finalized_slot,
            });
        }
        let finalized_checkpoint_block =
            self.get_checkpoint_block(block.parent_root, self.finalized_checkpoint.epoch)?;
        if self.finalized_checkpoint.root != finalized_checkpoint_block {
            return Err(ForkChoiceError::BlockNotDescendedFromFinalized);
        }

        // Phase 2: run the state transition to derive the block's post-state.
        let mut state = parent_state.clone();
        let mut block_clone = block.clone();
        let block_root = block_clone.tree_root(MerkleError::BeaconBlock)?;
        state.apply_signed_block(signed_block)?;

        // The head as of before this block is imported, used by proposer-boost
        // selection in phase 5.
        let pre_import_head = self.get_head()?;

        // Phase 3: record the block, its post-state, and empty PTC vote vectors.
        self.blocks.insert(block_root, block.clone());
        self.block_states.insert(block_root, state.clone());
        self.payload_timeliness_vote
            .insert(block_root, vec![None; PTC_SIZE]);
        self.payload_data_availability_vote
            .insert(block_root, vec![None; PTC_SIZE]);

        // Phase 4: replay the payload-attestation votes carried in the block.
        self.notify_ptc_messages(&state, &block.body.payload_attestations)?;

        // Phase 5: record timeliness, then update proposer boost and checkpoints.
        self.record_block_timeliness(block_root)?;
        self.update_proposer_boost_root(pre_import_head.root, block_root)?;
        self.update_checkpoints(
            state.current_justified_checkpoint,
            state.finalized_checkpoint,
        );
        self.compute_pulled_up_tip(block_root)?;

        Ok(())
    }

    /// Import a block, then replay the fork-choice votes carried in it.
    ///
    /// This is the one-block-at-a-time shape the reference tests drive, mirroring
    /// the harness's block step. It imports the block with [`Self::on_block`], then
    /// replays the block's beacon attestations through [`Self::on_attestation`] and
    /// its attester slashings through [`Self::on_attester_slashing`], each marked
    /// as arriving inside a block. The block's payload attestations are already
    /// folded in by [`Self::on_block`] itself. Every step must succeed, so a
    /// failure anywhere is the block step's verdict, matching the reference
    /// harness, which replays these as required steps rather than optional ones.
    /// The work runs on a copy of the store and is committed only once all steps
    /// succeed, leaving the caller's store untouched on any error.
    pub fn on_block_with_embedded_messages(
        &mut self,
        signed_block: &SignedBeaconBlock,
    ) -> Result<(), ForkChoiceError> {
        let mut updated = self.clone();
        updated.on_block_inner(signed_block)?;
        for attestation in signed_block.message.body.attestations.iter() {
            updated.on_attestation_inner(attestation, true)?;
        }
        for slashing in signed_block.message.body.attester_slashings.iter() {
            updated.on_attester_slashing(slashing)?;
        }
        *self = updated;
        Ok(())
    }

    /// Fold a block's bundled payload-attestation votes into the store.
    ///
    /// A block carries the previous slot's committee votes about its parent's
    /// payload. The state transition has already checked those as a group, so this
    /// expands each into individual validators and records their votes through the
    /// same path gossip votes use ([`Self::on_payload_attestation_message`]). The
    /// genesis block has no previous slot to vote on, so it is skipped.
    pub fn notify_ptc_messages(
        &mut self,
        state: &BeaconState,
        payload_attestations: &[PayloadAttestation],
    ) -> Result<(), ForkChoiceError> {
        // A genesis block has no previous slot to attest.
        if state.slot.as_u64() == 0 {
            return Ok(());
        }
        // Expand each block aggregate into its attesting validators using the
        // block's committee, then feed each validator through the same handler the
        // gossip path uses. The from-block path skips the current-slot check and
        // signature the block already verified, and records under the aggregate's
        // target root, not the containing block root.
        for attestation in payload_attestations {
            let indexed = state.indexed_payload_attestation(attestation.data.slot, attestation)?;
            for validator_index in indexed.attesting_indices.iter() {
                let message = PayloadAttestationMessage {
                    validator_index: *validator_index,
                    data: attestation.data,
                    signature: BLSSignature::default(),
                };
                self.on_payload_attestation_message_inner(&message, true)?;
            }
        }
        Ok(())
    }
}
