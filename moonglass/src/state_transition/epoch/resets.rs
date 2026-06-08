//! End-of-epoch resets: eth1 votes, slashings buffer, randao, historical summaries.

use crate::constants::{
    EPOCHS_PER_ETH1_VOTING_PERIOD, EPOCHS_PER_HISTORICAL_VECTOR, EPOCHS_PER_SLASHINGS_VECTOR,
    SLOTS_PER_EPOCH, SLOTS_PER_HISTORICAL_ROOT,
};
use crate::containers::{BeaconState, HistoricalSummary};
use crate::error::{MerkleError, TransitionError};
use crate::primitives::{Gwei, ParticipationFlags};
use crate::state_transition::TreeRootExt;

impl BeaconState {
    /// Clear the deposit-vote bag at the end of each voting period.
    ///
    /// Spec: `process_eth1_data_reset`
    pub fn process_eth1_data_reset(&mut self) -> Result<(), TransitionError> {
        let next_epoch = self.slot.epoch().as_u64() + 1;
        if next_epoch.is_multiple_of(EPOCHS_PER_ETH1_VOTING_PERIOD as u64) {
            self.eth1_data_votes = ssz_rs::List::default();
        }
        Ok(())
    }

    /// Clear the next-epoch slot of the slashings ring buffer.
    ///
    /// Spec: `process_slashings_reset`
    pub fn process_slashings_reset(&mut self) -> Result<(), TransitionError> {
        let next_epoch = self.slot.epoch().as_u64() + 1;
        let idx = (next_epoch as usize) % EPOCHS_PER_SLASHINGS_VECTOR;
        self.slashings[idx] = Gwei::ZERO;
        Ok(())
    }

    /// Copy the current epoch's randao mix into the next epoch's ring-buffer slot.
    ///
    /// Spec: `process_randao_mixes_reset`
    pub fn process_randao_mixes_reset(&mut self) -> Result<(), TransitionError> {
        let current_epoch = self.slot.epoch();
        let next_idx = (current_epoch.as_u64() + 1) as usize % EPOCHS_PER_HISTORICAL_VECTOR;
        let current_idx = current_epoch.as_usize() % EPOCHS_PER_HISTORICAL_VECTOR;
        self.randao_mixes[next_idx] = self.randao_mixes[current_idx];
        Ok(())
    }

    /// Append a historical-summary record at the historical-root boundary.
    ///
    /// Spec: `process_historical_summaries_update`
    pub fn process_historical_summaries_update(&mut self) -> Result<(), TransitionError> {
        let next_epoch = self.slot.epoch().as_u64() + 1;
        let boundary = (SLOTS_PER_HISTORICAL_ROOT / SLOTS_PER_EPOCH) as u64;
        if !next_epoch.is_multiple_of(boundary) {
            return Ok(());
        }
        let mut block_summary = self.block_roots.clone();
        let mut state_summary = self.state_roots.clone();
        let block_summary_root = block_summary.tree_root(MerkleError::BlockRoots)?;
        let state_summary_root = state_summary.tree_root(MerkleError::StateRoots)?;
        self.historical_summaries.push(HistoricalSummary {
            block_summary_root,
            state_summary_root,
        });
        Ok(())
    }

    /// Rotate the per-validator participation flags: current becomes previous,
    /// current is zeroed at the new validator-set length.
    ///
    /// Spec: `process_participation_flag_updates`
    pub fn process_participation_flag_updates(&mut self) -> Result<(), TransitionError> {
        self.previous_epoch_participation = self.current_epoch_participation.clone();
        let len = self.validators.len();
        self.current_epoch_participation = ssz_rs::List::default();
        for _ in 0..len {
            self.current_epoch_participation
                .push(ParticipationFlags::NONE);
        }
        Ok(())
    }
}
