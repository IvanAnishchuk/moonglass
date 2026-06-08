//! Validator registry updates: activations, exits, pending deposits.

use crate::constants::{
    EFFECTIVE_BALANCE_INCREMENT, EJECTION_BALANCE, FAR_FUTURE_EPOCH,
    HYSTERESIS_DOWNWARD_MULTIPLIER, HYSTERESIS_QUOTIENT, HYSTERESIS_UPWARD_MULTIPLIER,
    MAX_PENDING_DEPOSITS_PER_EPOCH, MIN_ACTIVATION_BALANCE,
};
use crate::containers::{BeaconState, PendingConsolidation, PendingDeposit};
use crate::error::TransitionError;
use crate::primitives::{Gwei, ValidatorIndex};
use crate::state_transition::compute_activation_exit_epoch;

impl BeaconState {
    /// Move eligible queue entries into the active set, eject underbalanced
    /// validators, and consume activation-churn budget.
    ///
    /// Spec: `process_registry_updates`
    pub fn process_registry_updates(&mut self) -> Result<(), TransitionError> {
        let current = self.slot.epoch();
        let len = self.validators.len();
        let mut to_exit: Vec<ValidatorIndex> = Vec::new();
        for i in 0..len {
            let v = &self.validators[i];
            if v.activation_eligibility_epoch == FAR_FUTURE_EPOCH
                && v.effective_balance >= MIN_ACTIVATION_BALANCE
            {
                self.validators[i].activation_eligibility_epoch = current.saturating_add(1);
            }
            let v = &self.validators[i];
            if v.is_active_at(current) && v.effective_balance <= EJECTION_BALANCE {
                to_exit.push(ValidatorIndex(i as u64));
            }
        }
        for vi in to_exit {
            self.initiate_validator_exit(vi)?;
        }

        let activation_epoch = compute_activation_exit_epoch(current);
        let finalized_epoch = self.finalized_checkpoint.epoch.as_u64();
        for i in 0..self.validators.len() {
            let v = &self.validators[i];
            if v.activation_epoch == FAR_FUTURE_EPOCH
                && v.activation_eligibility_epoch.as_u64() <= finalized_epoch
            {
                self.validators[i].activation_epoch = activation_epoch;
            }
        }
        Ok(())
    }

    /// Drain pending deposits into the registry. Walks the queue in order,
    /// stopping when the deposit is not yet finalized, the per-epoch deposit
    /// count cap is reached, or the next not-yet-exited validator's deposit
    /// would exceed the activation churn budget. Already-withdrawn validators
    /// take their deposit without consuming churn. Exiting validators have
    /// their deposit moved to a postponed tail so it is reconsidered after
    /// the withdrawable epoch.
    ///
    /// Spec: `process_pending_deposits`
    pub fn process_pending_deposits(&mut self) -> Result<(), TransitionError> {
        let next_epoch = self.slot.epoch().saturating_add(1);
        let available_for_processing = self
            .deposit_balance_to_consume
            .as_u64()
            .saturating_add(self.activation_churn_limit().as_u64());
        let mut processed_amount: u64 = 0;
        let mut next_deposit_index: u64 = 0;
        let mut deposits_to_postpone: Vec<PendingDeposit> = Vec::new();
        let mut is_churn_limit_reached = false;
        let finalized_slot = self.finalized_checkpoint.epoch.start_slot();

        let queue: Vec<PendingDeposit> = self.pending_deposits.iter().copied().collect();
        for deposit in &queue {
            if deposit.slot > finalized_slot {
                break;
            }
            if next_deposit_index >= MAX_PENDING_DEPOSITS_PER_EPOCH {
                break;
            }

            let mut is_validator_exited = false;
            let mut is_validator_withdrawn = false;
            if let Some(v) = self.validators.iter().find(|v| v.pubkey == deposit.pubkey) {
                is_validator_exited = v.exit_epoch < crate::constants::FAR_FUTURE_EPOCH;
                is_validator_withdrawn = v.withdrawable_epoch < next_epoch;
            }

            if is_validator_withdrawn {
                self.apply_pending_deposit(deposit)?;
            } else if is_validator_exited {
                deposits_to_postpone.push(*deposit);
            } else {
                if processed_amount.saturating_add(deposit.amount.as_u64())
                    > available_for_processing
                {
                    is_churn_limit_reached = true;
                    break;
                }
                processed_amount = processed_amount.saturating_add(deposit.amount.as_u64());
                self.apply_pending_deposit(deposit)?;
            }

            next_deposit_index = next_deposit_index.saturating_add(1);
        }

        let consumed_count = next_deposit_index as usize;
        let mut new_queue: Vec<PendingDeposit> = queue.into_iter().skip(consumed_count).collect();
        new_queue.extend(deposits_to_postpone);
        self.keep_pending_deposits(new_queue);

        self.deposit_balance_to_consume = if is_churn_limit_reached {
            Gwei(available_for_processing.saturating_sub(processed_amount))
        } else {
            Gwei::ZERO
        };
        Ok(())
    }

    fn apply_pending_deposit(&mut self, deposit: &PendingDeposit) -> Result<(), TransitionError> {
        let existing = self
            .validators
            .iter()
            .position(|v| v.pubkey == deposit.pubkey);
        if let Some(idx) = existing {
            if idx < self.balances.len() {
                self.balances[idx] = self.balances[idx].saturating_add(deposit.amount);
            }
            return Ok(());
        }
        // Verify proof-of-possession for new validators. Invalid deposits are
        // dropped silently per spec, not propagated as transition errors.
        if !self.is_valid_deposit_signature(
            &deposit.pubkey,
            deposit.withdrawal_credentials,
            deposit.amount,
            &deposit.signature,
        )? {
            return Ok(());
        }
        self.add_validator_to_registry(
            deposit.pubkey,
            deposit.withdrawal_credentials,
            deposit.amount,
        )
    }

    fn keep_pending_deposits(&mut self, kept: Vec<PendingDeposit>) {
        self.pending_deposits = ssz_rs::List::default();
        for d in kept {
            self.pending_deposits.push(d);
        }
    }

    /// Drain consolidations whose source validator is withdrawable next epoch,
    /// moving each source's effective balance into the target. Slashed sources
    /// drop without moving balance. The walk halts at the first entry whose
    /// source is not yet withdrawable, leaving the rest queued for later epochs.
    ///
    /// Spec: `process_pending_consolidations`
    pub fn process_pending_consolidations(&mut self) -> Result<(), TransitionError> {
        let next_epoch = self.slot.epoch().saturating_add(1);
        let queue: Vec<PendingConsolidation> =
            self.pending_consolidations.iter().copied().collect();
        let mut consumed = 0usize;
        for entry in &queue {
            let Some(source) = self.validators.get(entry.source_index.as_usize()) else {
                consumed += 1;
                continue;
            };
            if source.slashed {
                consumed += 1;
                continue;
            }
            if source.withdrawable_epoch > next_epoch {
                break;
            }
            let source_effective_balance = self
                .balances
                .get(entry.source_index.as_usize())
                .copied()
                .unwrap_or(Gwei::ZERO)
                .min(source.effective_balance);
            self.decrease_balance(entry.source_index, source_effective_balance)?;
            self.increase_balance(entry.target_index, source_effective_balance)?;
            consumed += 1;
        }
        let remaining: Vec<PendingConsolidation> = queue.into_iter().skip(consumed).collect();
        self.keep_pending_consolidations(remaining);
        Ok(())
    }

    fn keep_pending_consolidations(&mut self, kept: Vec<PendingConsolidation>) {
        self.pending_consolidations = ssz_rs::List::default();
        for c in kept {
            self.pending_consolidations.push(c);
        }
    }

    /// Round each validator's effective balance toward its actual balance,
    /// gated by a hysteresis band so it does not oscillate per slot.
    ///
    /// Spec: `process_effective_balance_updates`
    pub fn process_effective_balance_updates(&mut self) -> Result<(), TransitionError> {
        let hysteresis_increment =
            EFFECTIVE_BALANCE_INCREMENT.as_u64() / HYSTERESIS_QUOTIENT.max(1);
        let downward = hysteresis_increment.saturating_mul(HYSTERESIS_DOWNWARD_MULTIPLIER);
        let upward = hysteresis_increment.saturating_mul(HYSTERESIS_UPWARD_MULTIPLIER);
        let len = self.validators.len();
        for i in 0..len {
            let balance = self.balances.get(i).copied().unwrap_or(Gwei::ZERO).as_u64();
            let v = &mut self.validators[i];
            let max = v.max_effective_balance().as_u64();
            let eff = v.effective_balance.as_u64();
            if balance.saturating_add(downward) < eff || eff.saturating_add(upward) < balance {
                let rounded = balance - balance % EFFECTIVE_BALANCE_INCREMENT.as_u64();
                v.effective_balance = Gwei(rounded.min(max));
            }
        }
        Ok(())
    }
}
