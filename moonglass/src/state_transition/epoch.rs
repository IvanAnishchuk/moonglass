//! Epoch-boundary processing.
//!
//! Runs the epoch-boundary phases that update finality, inactivity, rewards,
//! registry churn, slashings, lifecycle queues, effective balances, historical
//! accumulators, sync committees, proposer lookahead, builder-payment windows,
//! and payload-timeliness committee assignments.

// Safe: spec-bounded `usize`<->`u64` casts on iteration counters and indices.
#![allow(clippy::cast_possible_truncation)]

mod accounting;
mod finality;
mod registry;
mod resets;
mod sync_committees;
mod windows;

use crate::containers::BeaconState;
use crate::error::TransitionError;

impl BeaconState {
    /// Run all epoch sub-phases in consensus order.
    ///
    /// Spec: `process_epoch`
    pub fn process_epoch(&mut self) -> Result<(), TransitionError> {
        self.process_justification_and_finalization()?;
        self.process_inactivity_updates()?;
        self.process_rewards_and_penalties()?;
        self.process_registry_updates()?;
        self.process_slashings()?;
        self.process_eth1_data_reset()?;
        self.process_pending_deposits()?;
        self.process_pending_consolidations()?;
        self.process_builder_pending_payments()?;
        self.process_effective_balance_updates()?;
        self.process_slashings_reset()?;
        self.process_randao_mixes_reset()?;
        self.process_historical_summaries_update()?;
        self.process_participation_flag_updates()?;
        self.process_sync_committee_updates()?;
        self.process_proposer_lookahead()?;
        self.process_ptc_window()?;
        Ok(())
    }
}
