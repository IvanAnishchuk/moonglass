//! Advancing the clock.
//!
//! Fork choice runs on the store's own clock, and moving it forward has side
//! effects. The store's time is local, not chain state, but it controls which
//! messages are admitted and when pending
//! [justification](crate::glossary#justification-and-finalization) becomes
//! official. Crossing into a new [slot](crate::glossary#slot) clears the proposer
//! boost. Crossing into a new [epoch](crate::glossary#epoch) promotes the
//! [checkpoints](crate::glossary#checkpoint) that block processing had lined up.

use crate::error::ForkChoiceError;
use crate::primitives::Root;

use super::helpers::compute_slots_since_epoch_start;
use super::store::Store;

impl Store {
    /// Move the store's clock forward to `time`.
    ///
    /// So that no slot's side effects are skipped when time jumps ahead, this
    /// steps through each slot boundary in between, applying the per-slot effects
    /// at each, before finally settling on `time`. Moving backwards is rejected
    /// with [`ForkChoiceError::TickWentBackwards`].
    pub fn on_tick(&mut self, time: u64) -> Result<(), ForkChoiceError> {
        if time < self.time {
            return Err(ForkChoiceError::TickWentBackwards {
                from: self.time,
                to: time,
            });
        }
        let tick_slot = self.slot_at_time(time);
        while self.get_slots_since_genesis() < tick_slot {
            // `previous_time` is the start time of the next slot boundary stepped
            // through on the way to `time`.
            let next_slot = self.get_current_slot().saturating_add(1).as_u64();
            let previous_time = self.slot_start_time(next_slot);
            self.on_tick_per_slot(previous_time);
        }
        self.on_tick_per_slot(time);
        Ok(())
    }

    /// Apply one clock step and the boundary effects it triggers.
    ///
    /// Sets the time, then checks two boundaries. If a new slot began, it clears
    /// the proposer boost, since a fresh slot starts with no boosted block. If
    /// that new slot also begins an epoch, it promotes the unrealized justified
    /// and finalized checkpoints to confirmed.
    pub fn on_tick_per_slot(&mut self, time: u64) {
        let previous_slot = self.get_current_slot();
        self.time = time;
        let current = self.get_current_slot();
        let advanced_slot = current > previous_slot;

        if advanced_slot {
            self.proposer_boost_root = Root::ZERO;
        }

        if advanced_slot && compute_slots_since_epoch_start(current) == 0 {
            self.update_checkpoints(
                self.unrealized_justified_checkpoint,
                self.unrealized_finalized_checkpoint,
            );
        }
    }
}
