//! Per-slot processing.
//!
//! Empty slots still change state: the transition records the previous state
//! root and block root in historical ring buffers, clears the next
//! payload-availability bit, and runs epoch-boundary processing when the next
//! slot starts a new epoch.

use crate::constants::{SLOTS_PER_EPOCH, SLOTS_PER_HISTORICAL_ROOT};
use crate::containers::BeaconState;
use crate::error::{MerkleError, SlotError, TransitionError};
use crate::primitives::{Root, Slot};
use crate::state_transition::TreeRootExt;

/// True if incrementing `slot` by one lands on the first slot of a new epoch.
fn next_slot_starts_epoch(slot: Slot) -> bool {
    (slot.as_u64() + 1).is_multiple_of(SLOTS_PER_EPOCH as u64)
}

impl BeaconState {
    /// Advance this state one slot at a time up to `target_slot`.
    ///
    /// Spec: `process_slots`
    pub fn process_slots(&mut self, target_slot: Slot) -> Result<(), TransitionError> {
        if self.slot >= target_slot {
            return Err(SlotError::NotAfter {
                current: self.slot,
                target: target_slot,
            }
            .into());
        }
        while self.slot < target_slot {
            self.process_slot()?;
            // Run epoch processing on the last slot of the ending epoch so that
            // `self.slot` inside `process_epoch` refers to that ending epoch.
            if next_slot_starts_epoch(self.slot) {
                self.process_epoch()?;
            }
            self.slot += 1;
        }
        Ok(())
    }

    /// Cache the previous state root and previous block root into ring buffers.
    ///
    /// Spec: `process_slot`
    pub fn process_slot(&mut self) -> Result<(), TransitionError> {
        let previous_state_root = self.tree_root(MerkleError::BeaconState)?;

        let index = self.slot % SLOTS_PER_HISTORICAL_ROOT;
        self.state_roots[index] = previous_state_root;

        // Backfill the header's state_root if it was left zero by process_block_header.
        if self.latest_block_header.state_root == Root::ZERO {
            self.latest_block_header = self
                .latest_block_header
                .with_state_root(previous_state_root);
        }

        let previous_block_root = self
            .latest_block_header
            .tree_root(MerkleError::BeaconBlockHeader)?;
        self.block_roots[index] = previous_block_root;

        self.clear_next_payload_availability();

        Ok(())
    }

    /// Reset the payload-availability bit for the next slot.
    fn clear_next_payload_availability(&mut self) {
        let next_index = (self.slot + 1) % SLOTS_PER_HISTORICAL_ROOT;
        self.execution_payload_availability.set(next_index, false);
    }
}
