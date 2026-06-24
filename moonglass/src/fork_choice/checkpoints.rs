//! Fork-choice [checkpoint](crate::glossary#checkpoint) caches and pulled-up
//! checkpoint evidence.
//!
//! Block processing can reveal
//! [justification](crate::glossary#justification-and-finalization)/finalization
//! information before the store's realized checkpoints are advanced by time.
//! These methods keep both views: realized checkpoints used by fork choice now,
//! and unrealized checkpoints pulled from block post-states, promoted to
//! realized at the next [epoch](crate::glossary#epoch) boundary.
//!
//! The admission handlers write these caches. The justified and finalized
//! checkpoints they track are then read by [`filter`](super::filter) and
//! [`weight`](super::weight).

use crate::containers::Checkpoint;
use crate::error::ForkChoiceError;
use crate::primitives::Root;

use super::store::Store;

impl Store {
    /// Move the store's confirmed justified and finalized checkpoints forward.
    ///
    /// Justified and finalized are the two stages by which consensus commits to a
    /// block: justified means the chain has voted for it this round, finalized
    /// means reverting it would need a slashable safety violation. This raises the
    /// store's record of each, but only when a newer one is supplied, never
    /// backward.
    pub fn update_checkpoints(&mut self, justified: Checkpoint, finalized: Checkpoint) {
        if justified.epoch > self.justified_checkpoint.epoch {
            self.justified_checkpoint = justified;
        }
        if finalized.epoch > self.finalized_checkpoint.epoch {
            self.finalized_checkpoint = finalized;
        }
    }

    /// Make sure we have the beacon state at an attestation's target checkpoint.
    ///
    /// To validate an attestation, fork choice needs the state as of the
    /// checkpoint the vote targets, so it can look up the committees and check
    /// signatures. If that state is not cached yet, this takes the target block's
    /// state and fast-forwards it through any empty slots up to the checkpoint,
    /// then caches the result for reuse.
    pub fn store_target_checkpoint_state(
        &mut self,
        target: Checkpoint,
    ) -> Result<(), ForkChoiceError> {
        if self.checkpoint_states.contains_key(&target) {
            return Ok(());
        }
        let mut state = self
            .block_states
            .get(&target.root)
            .ok_or(ForkChoiceError::UnknownBlock(target.root))?
            .clone();
        let target_slot = target.epoch.start_slot();
        if state.slot < target_slot {
            state.process_slots(target_slot)?;
        }
        self.checkpoint_states.insert(target, state);
        Ok(())
    }

    /// Move the store's *unrealized* justified and finalized checkpoints forward.
    ///
    /// A block's state can already imply newer justification than the store has
    /// formally adopted. We track that as "unrealized": running ahead of the
    /// confirmed checkpoints, and promoted to confirmed when the clock next
    /// crosses an [epoch](crate::glossary#epoch) boundary, in [`Store::on_tick`].
    /// (Checkpoints implied by a block already in a past epoch are the exception:
    /// [`Self::compute_pulled_up_tip`] adopts those at once.) This raises the
    /// unrealized records when a newer one arrives.
    pub fn update_unrealized_checkpoints(
        &mut self,
        unrealized_justified: Checkpoint,
        unrealized_finalized: Checkpoint,
    ) {
        if unrealized_justified.epoch > self.unrealized_justified_checkpoint.epoch {
            self.unrealized_justified_checkpoint = unrealized_justified;
        }
        if unrealized_finalized.epoch > self.unrealized_finalized_checkpoint.epoch {
            self.unrealized_finalized_checkpoint = unrealized_finalized;
        }
    }

    /// Work out the justification a block implies, and record it as unrealized.
    ///
    /// After importing a block, its state may already justify or finalize more
    /// than the store has adopted. To find out, this replays justification and
    /// finalization on a copy of the block's state (a copy, so the stored state is
    /// left untouched) and saves the result as the block's unrealized
    /// justification. If the block is from a past epoch, that result is also
    /// adopted as confirmed straight away.
    pub fn compute_pulled_up_tip(&mut self, block_root: Root) -> Result<(), ForkChoiceError> {
        // All fallible reads first, so an error never leaves the store maps
        // half-updated.
        let mut pulled_up_state = self
            .block_states
            .get(&block_root)
            .ok_or(ForkChoiceError::UnknownBlock(block_root))?
            .clone();
        pulled_up_state.process_justification_and_finalization()?;
        let block_epoch = self
            .blocks
            .get(&block_root)
            .ok_or(ForkChoiceError::UnknownBlock(block_root))?
            .slot
            .epoch();
        let current_epoch = self.get_current_store_epoch();

        // Then the store mutations.
        self.unrealized_justifications
            .insert(block_root, pulled_up_state.current_justified_checkpoint);
        self.update_unrealized_checkpoints(
            pulled_up_state.current_justified_checkpoint,
            pulled_up_state.finalized_checkpoint,
        );
        if block_epoch < current_epoch {
            self.update_checkpoints(
                pulled_up_state.current_justified_checkpoint,
                pulled_up_state.finalized_checkpoint,
            );
        }
        Ok(())
    }
}
