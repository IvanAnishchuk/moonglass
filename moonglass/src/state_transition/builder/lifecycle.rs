//! Builder registry lifecycle: activations, exits, slashings.

use crate::constants::{
    EPOCHS_PER_SLASHINGS_VECTOR, FAR_FUTURE_EPOCH, MIN_BUILDER_WITHDRAWABILITY_DELAY,
    MIN_SLASHING_PENALTY_QUOTIENT, PROPOSER_WEIGHT, WEIGHT_DENOMINATOR,
    WHISTLEBLOWER_REWARD_QUOTIENT,
};
use crate::containers::BeaconState;
use crate::error::TransitionError;
use crate::primitives::{BuilderIndex, Gwei, ValidatorIndex};
use crate::state_transition::BeaconStateLookup;

impl BeaconState {
    /// True if the builder at `builder_index` has a finalized placement and has
    /// not yet initiated exit.
    ///
    /// Spec: `is_active_builder`.
    pub fn is_active_builder(&self, builder_index: BuilderIndex) -> Result<bool, TransitionError> {
        let builder = self.builder(builder_index)?;
        Ok(builder.deposit_epoch < self.finalized_checkpoint.epoch
            && builder.withdrawable_epoch == FAR_FUTURE_EPOCH)
    }

    /// Schedule a builder to exit the active builder set if not already scheduled.
    pub fn initiate_builder_exit(
        &mut self,
        builder_index: BuilderIndex,
    ) -> Result<(), TransitionError> {
        let builder = self.builder(builder_index)?;
        if builder.withdrawable_epoch != FAR_FUTURE_EPOCH {
            return Ok(());
        }
        let current_epoch = self.slot.epoch();
        let withdrawable = current_epoch.saturating_add(MIN_BUILDER_WITHDRAWABILITY_DELAY);
        self.builders[builder_index.as_usize()].withdrawable_epoch = withdrawable;
        Ok(())
    }

    /// Penalize a builder and pay the whistleblower and proposer their fractions.
    pub fn slash_builder(
        &mut self,
        builder_index: BuilderIndex,
        whistleblower_index: Option<ValidatorIndex>,
    ) -> Result<(), TransitionError> {
        let current_epoch = self.slot.epoch();
        let proposer_index = self.beacon_proposer_index()?;
        let whistleblower = whistleblower_index.unwrap_or(proposer_index);
        // Existence checks: error if either index is out of bounds.
        let _ = self.validator(proposer_index)?;
        let _ = self.validator(whistleblower)?;
        let balance = self.builder(builder_index)?.balance;

        self.initiate_builder_exit(builder_index)?;
        let penalty = Gwei(balance.as_u64() / MIN_SLASHING_PENALTY_QUOTIENT);
        let builder = &mut self.builders[builder_index.as_usize()];
        builder.balance = builder.balance.saturating_sub(penalty);
        let extended = current_epoch.saturating_add(EPOCHS_PER_SLASHINGS_VECTOR as u64);
        if extended > builder.withdrawable_epoch {
            builder.withdrawable_epoch = extended;
        }

        let whistleblower_reward = Gwei(balance.as_u64() / WHISTLEBLOWER_REWARD_QUOTIENT);
        let proposer_reward =
            Gwei(whistleblower_reward.as_u64() * PROPOSER_WEIGHT / WEIGHT_DENOMINATOR);
        self.increase_balance(proposer_index, proposer_reward)?;
        self.increase_balance(
            whistleblower,
            Gwei(whistleblower_reward.as_u64() - proposer_reward.as_u64()),
        )?;
        Ok(())
    }
}
