//! Balance behavior used by rewards and validator exits.
//!
//! Covers saturating balance mutation, total active balance, base-reward math,
//! per-flag reward and penalty distribution, the inactivity-leak penalty, and
//! the slashing mutation.

use crate::constants::{
    BASE_REWARD_FACTOR, EFFECTIVE_BALANCE_INCREMENT, EPOCHS_PER_SLASHINGS_VECTOR, GENESIS_EPOCH,
    INACTIVITY_PENALTY_QUOTIENT, INACTIVITY_SCORE_BIAS, MIN_EPOCHS_TO_INACTIVITY_PENALTY,
    MIN_SLASHING_PENALTY_QUOTIENT, PARTICIPATION_FLAG_WEIGHTS, PROPOSER_WEIGHT,
    TIMELY_HEAD_FLAG_INDEX, VALIDATOR_REGISTRY_LIMIT, WEIGHT_DENOMINATOR,
    WHISTLEBLOWER_REWARD_QUOTIENT,
};
use crate::containers::BeaconState;
use crate::error::{RegistryError, TransitionError};
use crate::primitives::{Epoch, Gwei, ValidatorIndex};
use crate::state_transition::BeaconStateLookup;

/// Per-validator balance changes produced by an epoch accounting phase.
pub(crate) struct BalanceDeltas {
    rewards: Vec<Gwei>,
    penalties: Vec<Gwei>,
}

impl BalanceDeltas {
    fn zeroed(validator_count: usize) -> Self {
        Self {
            rewards: vec![Gwei::ZERO; validator_count],
            penalties: vec![Gwei::ZERO; validator_count],
        }
    }

    /// Saturating-apply the accumulated rewards then penalties to `balances`.
    pub(crate) fn apply_to(self, balances: &mut ssz_rs::List<Gwei, VALIDATOR_REGISTRY_LIMIT>) {
        for (i, reward) in self.rewards.iter().enumerate() {
            if i < balances.len() {
                balances[i] = balances[i].saturating_add(*reward);
            }
        }
        for (i, penalty) in self.penalties.iter().enumerate() {
            if i < balances.len() {
                balances[i] = balances[i].saturating_sub(*penalty);
            }
        }
    }
}

impl BeaconState {
    /// Add `delta` gwei to `index`'s balance. Saturates at `Gwei::MAX`.
    pub fn increase_balance(
        &mut self,
        index: ValidatorIndex,
        delta: Gwei,
    ) -> Result<(), TransitionError> {
        let slot = self.balance_mut(index)?;
        *slot = slot.saturating_add(delta);
        Ok(())
    }

    /// Subtract `delta` gwei from `index`'s balance. Saturates at `Gwei(0)`.
    pub fn decrease_balance(
        &mut self,
        index: ValidatorIndex,
        delta: Gwei,
    ) -> Result<(), TransitionError> {
        let slot = self.balance_mut(index)?;
        *slot = slot.saturating_sub(delta);
        Ok(())
    }

    fn balance_mut(&mut self, index: ValidatorIndex) -> Result<&mut Gwei, TransitionError> {
        // Existence check: errors if `index` is out of bounds for the validator registry.
        let _ = self.validator(index)?;
        if index.as_usize() >= self.balances.len() {
            return Err(RegistryError::ValidatorIndexOutOfRange(index.as_u64()).into());
        }
        Ok(&mut self.balances[index.as_usize()])
    }

    /// Sum of effective balances over the active validator set, floored at
    /// `EFFECTIVE_BALANCE_INCREMENT` so downstream divisions stay safe.
    #[must_use]
    pub fn total_active_balance(&self) -> Gwei {
        let epoch = self.slot.epoch();
        let total: Gwei = self
            .validators
            .iter()
            .filter(|v| v.is_active_at(epoch))
            .map(|v| v.effective_balance)
            .fold(Gwei(0), Gwei::saturating_add);
        total.max(EFFECTIVE_BALANCE_INCREMENT)
    }

    /// Sum of effective balances over `indices`, floored at
    /// `EFFECTIVE_BALANCE_INCREMENT`.
    #[must_use]
    pub fn total_balance(&self, indices: &[ValidatorIndex]) -> Gwei {
        let total = indices
            .iter()
            .filter_map(|i| self.validators.get(i.as_usize()))
            .map(|v| v.effective_balance)
            .fold(Gwei(0), Gwei::saturating_add);
        total.max(EFFECTIVE_BALANCE_INCREMENT)
    }

    /// Base reward issued per `EFFECTIVE_BALANCE_INCREMENT` of effective balance
    /// per epoch. The reciprocal scale knob for total issuance.
    #[must_use]
    pub fn base_reward_per_increment(&self) -> Gwei {
        let total = self.total_active_balance().as_u64();
        let sqrt = isqrt_u64(total).max(1);
        Gwei(EFFECTIVE_BALANCE_INCREMENT.as_u64() * BASE_REWARD_FACTOR / sqrt)
    }

    /// Base reward for the validator at `index`. Errors if `index` is out of range.
    pub fn base_reward(&self, index: ValidatorIndex) -> Result<Gwei, TransitionError> {
        let v = self.validator(index)?;
        let increments = v.effective_balance.as_u64() / EFFECTIVE_BALANCE_INCREMENT.as_u64();
        Ok(Gwei(increments * self.base_reward_per_increment().as_u64()))
    }

    /// Total active-balance, expressed in `EFFECTIVE_BALANCE_INCREMENT` units.
    #[must_use]
    pub fn active_increments(&self) -> u64 {
        self.total_active_balance().as_u64() / EFFECTIVE_BALANCE_INCREMENT.as_u64()
    }

    /// Previous epoch from the state's current slot. Saturates at `GENESIS_EPOCH`.
    #[must_use]
    pub fn previous_epoch(&self) -> Epoch {
        let current = self.slot.epoch();
        if current == GENESIS_EPOCH {
            GENESIS_EPOCH
        } else {
            current.saturating_sub(1)
        }
    }

    /// True if finality has fallen far enough behind to trigger the inactivity leak.
    #[must_use]
    pub fn is_in_inactivity_leak(&self) -> bool {
        let delay = self
            .previous_epoch()
            .as_u64()
            .saturating_sub(self.finalized_checkpoint.epoch.as_u64());
        delay > MIN_EPOCHS_TO_INACTIVITY_PENALTY
    }

    /// Validators eligible for per-epoch rewards or penalties.
    #[must_use]
    pub fn eligible_validator_indices(&self) -> Vec<ValidatorIndex> {
        let previous = self.previous_epoch();
        self.validators
            .iter()
            .enumerate()
            .filter_map(|(i, v)| {
                let active = v.is_active_at(previous);
                let post_slash = v.slashed && previous.saturating_add(1) < v.withdrawable_epoch;
                (active || post_slash).then_some(ValidatorIndex(i as u64))
            })
            .collect()
    }

    /// Indices of validators that were active in `epoch`, not slashed, and earned
    /// the participation flag at `flag_index`.
    pub fn unslashed_participating_indices(
        &self,
        flag_index: usize,
        epoch: Epoch,
    ) -> Result<Vec<ValidatorIndex>, TransitionError> {
        let current = self.slot.epoch();
        let previous = self.previous_epoch();
        let participation = if epoch == current {
            &self.current_epoch_participation
        } else if epoch == previous {
            &self.previous_epoch_participation
        } else {
            return Ok(Vec::new());
        };
        let mut out = Vec::new();
        for (i, v) in self.validators.iter().enumerate() {
            if !v.is_active_at(epoch) || v.slashed {
                continue;
            }
            let flags = participation.get(i).copied().unwrap_or_default();
            if flags.has_flag(flag_index)? {
                out.push(ValidatorIndex(i as u64));
            }
        }
        Ok(out)
    }

    /// Per-validator reward and penalty deltas for participation flag `flag_index`.
    pub(crate) fn participation_flag_deltas(
        &self,
        flag_index: usize,
    ) -> Result<BalanceDeltas, TransitionError> {
        let len = self.validators.len();
        let mut deltas = BalanceDeltas::zeroed(len);
        let previous = self.previous_epoch();
        let participating = self.unslashed_participating_indices(flag_index, previous)?;
        let participating_set: std::collections::HashSet<u64> =
            participating.iter().map(|i| i.as_u64()).collect();
        let weight = PARTICIPATION_FLAG_WEIGHTS
            .get(flag_index)
            .copied()
            .unwrap_or(0);
        let participating_increments =
            self.total_balance(&participating).as_u64() / EFFECTIVE_BALANCE_INCREMENT.as_u64();
        let active_increments = self.active_increments().max(1);
        let in_leak = self.is_in_inactivity_leak();
        for index in self.eligible_validator_indices() {
            let base = self.base_reward(index)?.as_u64();
            if participating_set.contains(&index.as_u64()) {
                if !in_leak {
                    let numerator = base
                        .saturating_mul(weight)
                        .saturating_mul(participating_increments);
                    let denom = active_increments.saturating_mul(WEIGHT_DENOMINATOR);
                    deltas.rewards[index.as_usize()] = deltas.rewards[index.as_usize()]
                        .saturating_add(Gwei(numerator / denom.max(1)));
                }
            } else if flag_index != TIMELY_HEAD_FLAG_INDEX {
                let penalty = base.saturating_mul(weight) / WEIGHT_DENOMINATOR.max(1);
                deltas.penalties[index.as_usize()] =
                    deltas.penalties[index.as_usize()].saturating_add(Gwei(penalty));
            }
        }
        Ok(deltas)
    }

    /// Per-validator inactivity-leak deltas.
    pub(crate) fn inactivity_penalty_deltas(&self) -> Result<BalanceDeltas, TransitionError> {
        let len = self.validators.len();
        let mut deltas = BalanceDeltas::zeroed(len);
        let previous = self.previous_epoch();
        let matching = self.unslashed_participating_indices(
            crate::constants::TIMELY_TARGET_FLAG_INDEX,
            previous,
        )?;
        let matching_set: std::collections::HashSet<u64> =
            matching.iter().map(|i| i.as_u64()).collect();
        let denominator = INACTIVITY_SCORE_BIAS.saturating_mul(INACTIVITY_PENALTY_QUOTIENT);
        for index in self.eligible_validator_indices() {
            if matching_set.contains(&index.as_u64()) {
                continue;
            }
            let v = &self.validators[index.as_usize()];
            let score = self
                .inactivity_scores
                .get(index.as_usize())
                .copied()
                .unwrap_or(0);
            let numerator = v.effective_balance.as_u64().saturating_mul(score);
            deltas.penalties[index.as_usize()] = Gwei(numerator / denominator.max(1));
        }
        Ok(deltas)
    }

    /// Apply the slashing mutation to `slashed_index`.
    pub fn slash_validator(
        &mut self,
        slashed_index: ValidatorIndex,
        whistleblower_index: Option<ValidatorIndex>,
    ) -> Result<(), TransitionError> {
        let current_epoch = self.slot.epoch();
        self.initiate_validator_exit(slashed_index)?;

        let effective_balance = self.validator(slashed_index)?.effective_balance;
        let v = &mut self.validators[slashed_index.as_usize()];
        v.slashed = true;
        let extended = current_epoch.saturating_add(EPOCHS_PER_SLASHINGS_VECTOR as u64);
        if extended > v.withdrawable_epoch {
            v.withdrawable_epoch = extended;
        }

        let slashings_slot = current_epoch % EPOCHS_PER_SLASHINGS_VECTOR;
        let bucket = &mut self.slashings[slashings_slot];
        *bucket = bucket.saturating_add(effective_balance);

        self.decrease_balance(
            slashed_index,
            Gwei(effective_balance.as_u64() / MIN_SLASHING_PENALTY_QUOTIENT),
        )?;

        let proposer_index = self.beacon_proposer_index()?;
        let whistleblower = whistleblower_index.unwrap_or(proposer_index);
        let whistleblower_reward = Gwei(effective_balance.as_u64() / WHISTLEBLOWER_REWARD_QUOTIENT);
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

/// Integer square root via Newton's method.
///
/// Returns the largest `n` with `n*n <= x`.
pub(crate) fn isqrt_u64(x: u64) -> u64 {
    if x < 2 {
        return x;
    }
    let mut n = x;
    let mut next = u64::midpoint(n, x / n);
    while next < n {
        n = next;
        next = u64::midpoint(n, x / n);
    }
    n
}
