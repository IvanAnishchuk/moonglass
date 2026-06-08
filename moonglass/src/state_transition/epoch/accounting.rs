//! Per-epoch reward and penalty accounting.

use crate::constants::{
    EFFECTIVE_BALANCE_INCREMENT, EPOCHS_PER_SLASHINGS_VECTOR, GENESIS_EPOCH, INACTIVITY_SCORE_BIAS,
    INACTIVITY_SCORE_RECOVERY_RATE, PARTICIPATION_FLAG_WEIGHTS, PROPORTIONAL_SLASHING_MULTIPLIER,
    TIMELY_TARGET_FLAG_INDEX,
};
use crate::containers::BeaconState;
use crate::error::TransitionError;
use crate::primitives::{Gwei, ValidatorIndex};

impl BeaconState {
    /// Bump per-validator inactivity scores for validators that missed the
    /// timely-target flag in the previous epoch, and decay scores when not leaking.
    ///
    /// Spec: `process_inactivity_updates`
    pub fn process_inactivity_updates(&mut self) -> Result<(), TransitionError> {
        if self.slot.epoch() == GENESIS_EPOCH {
            return Ok(());
        }
        let previous = self.previous_epoch();
        let matching: std::collections::HashSet<u64> = self
            .unslashed_participating_indices(TIMELY_TARGET_FLAG_INDEX, previous)?
            .iter()
            .map(|i| i.as_u64())
            .collect();
        let in_leak = self.is_in_inactivity_leak();
        let eligible = self.eligible_validator_indices();
        for vi in eligible {
            let idx = vi.as_usize();
            if idx >= self.inactivity_scores.len() {
                continue;
            }
            if matching.contains(&vi.as_u64()) {
                let s = self.inactivity_scores[idx];
                self.inactivity_scores[idx] = s.saturating_sub(1.min(s));
            } else {
                self.inactivity_scores[idx] =
                    self.inactivity_scores[idx].saturating_add(INACTIVITY_SCORE_BIAS);
            }
            if !in_leak {
                let s = self.inactivity_scores[idx];
                self.inactivity_scores[idx] =
                    s.saturating_sub(INACTIVITY_SCORE_RECOVERY_RATE.min(s));
            }
        }
        Ok(())
    }

    /// Apply per-flag rewards and penalties plus the inactivity-leak deltas.
    ///
    /// Spec: `process_rewards_and_penalties`
    pub fn process_rewards_and_penalties(&mut self) -> Result<(), TransitionError> {
        if self.slot.epoch() == GENESIS_EPOCH {
            return Ok(());
        }
        let mut all_deltas = Vec::new();
        for fi in 0..PARTICIPATION_FLAG_WEIGHTS.len() {
            all_deltas.push(self.participation_flag_deltas(fi)?);
        }
        all_deltas.push(self.inactivity_penalty_deltas()?);
        for deltas in all_deltas {
            deltas.apply_to(&mut self.balances);
        }
        Ok(())
    }

    /// Apply the proportional slashings sweep across all validators in their
    /// slashing window.
    ///
    /// Spec: `process_slashings`
    pub fn process_slashings(&mut self) -> Result<(), TransitionError> {
        let epoch = self.slot.epoch();
        let total_balance = self.total_active_balance();
        let sum_slashings: u64 = self.slashings.iter().map(|g| g.as_u64()).sum();
        let adjusted = sum_slashings
            .saturating_mul(PROPORTIONAL_SLASHING_MULTIPLIER)
            .min(total_balance.as_u64());
        let increment = EFFECTIVE_BALANCE_INCREMENT.as_u64();
        // Factor `increment` out of `total_balance` first so the per-increment
        // penalty is computed against the small denominator. Multiplying it back
        // by each validator's increment count avoids the u64 overflow risk that
        // a naive `adjusted * effective_balance / total_balance` would hit.
        let total_increments = total_balance.as_u64() / increment;
        // `total_active_balance` is floored at `EFFECTIVE_BALANCE_INCREMENT`,
        // so `total_increments >= 1` and the divisor below is always nonzero.
        debug_assert!(total_increments >= 1);
        let penalty_per_increment = adjusted.checked_div(total_increments).unwrap_or(0);
        let half = (EPOCHS_PER_SLASHINGS_VECTOR as u64) / 2;
        let len = self.validators.len();
        for i in 0..len {
            let v = &self.validators[i];
            if v.slashed && epoch.as_u64() + half == v.withdrawable_epoch.as_u64() {
                let effective_balance_increments = v.effective_balance.as_u64() / increment;
                let penalty = penalty_per_increment.saturating_mul(effective_balance_increments);
                self.decrease_balance(ValidatorIndex(i as u64), Gwei(penalty))?;
            }
        }
        Ok(())
    }
}
