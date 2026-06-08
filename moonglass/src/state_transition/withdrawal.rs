//! Withdrawal-sweep transition phases.
//!
//! Computes the per-slot expected withdrawals (queued builder withdrawals
//! first, then pending partial withdrawals, then a builder sweep, then a
//! validator sweep) and stores them in `state.payload_expected_withdrawals`
//! for the execution-payload path to verify. Validator and builder balances
//! move here.

// Safe: spec-bounded `usize`<->`u64` casts; the withdrawal-sweep function
// transcribes the spec ladder linearly and benefits from staying in one body.
#![allow(clippy::cast_possible_truncation, clippy::too_many_lines)]

use crate::constants::{
    FAR_FUTURE_EPOCH, MAX_BUILDERS_PER_WITHDRAWALS_SWEEP,
    MAX_PENDING_PARTIALS_PER_WITHDRAWALS_SWEEP, MAX_VALIDATORS_PER_WITHDRAWALS_SWEEP,
    MAX_WITHDRAWALS_PER_PAYLOAD, MIN_ACTIVATION_BALANCE,
};
use crate::containers::{BeaconState, Withdrawal};
use crate::error::{RegistryError, TransitionError};
use crate::primitives::{BuilderIndex, ExecutionAddress, Gwei, ValidatorIndex, WithdrawalIndex};

/// Per-sweep accounting that flows from `expected_withdrawals` into the
/// post-state update step.
pub(crate) struct ExpectedWithdrawals {
    withdrawals: Vec<Withdrawal>,
    processed_builder_withdrawals_count: u64,
    processed_partial_withdrawals_count: u64,
    processed_builders_sweep_count: u64,
}

fn withdrawal_address_from_credentials(credentials: &[u8; 32]) -> ExecutionAddress {
    let mut address = [0u8; 20];
    address.copy_from_slice(&credentials[12..]);
    ExecutionAddress(address)
}

fn builder_index_to_validator_index(idx: BuilderIndex) -> ValidatorIndex {
    idx.to_validator_index()
        .expect("builder index fits builder-index encoding")
}

impl BeaconState {
    /// Sum of in-flight withdrawals already chosen for `validator_index` during
    /// the current sweep. Lets later steps in the same sweep observe the
    /// post-withdrawal balance without mutating state mid-sweep.
    fn balance_after_withdrawals(
        &self,
        validator_index: ValidatorIndex,
        prior: &[Withdrawal],
    ) -> Gwei {
        let starting = self
            .balances
            .get(validator_index.as_usize())
            .copied()
            .unwrap_or(Gwei::ZERO);
        let withdrawn: u64 = prior
            .iter()
            .filter(|w| w.validator_index == validator_index)
            .map(|w| w.amount.as_u64())
            .sum();
        Gwei(starting.as_u64().saturating_sub(withdrawn))
    }

    fn get_builder_withdrawals(
        &self,
        withdrawal_index: WithdrawalIndex,
    ) -> Result<(Vec<Withdrawal>, u64), TransitionError> {
        let withdrawals_limit = MAX_WITHDRAWALS_PER_PAYLOAD.saturating_sub(1);
        let mut withdrawals: Vec<Withdrawal> = Vec::new();
        let mut next_index = withdrawal_index;
        let mut processed: u64 = 0;
        for entry in self.builder_pending_withdrawals.iter() {
            if withdrawals.len() >= withdrawals_limit {
                break;
            }
            if entry.builder_index.as_usize() >= self.builders.len() {
                return Err(
                    RegistryError::BuilderIndexOutOfRange(entry.builder_index.as_u64()).into(),
                );
            }
            withdrawals.push(Withdrawal {
                index: next_index,
                validator_index: builder_index_to_validator_index(entry.builder_index),
                address: entry.fee_recipient,
                amount: entry.amount,
            });
            next_index = WithdrawalIndex(next_index.as_u64().saturating_add(1));
            processed = processed.saturating_add(1);
        }
        Ok((withdrawals, processed))
    }

    fn get_pending_partial_withdrawals(
        &self,
        mut withdrawal_index: WithdrawalIndex,
        prior_withdrawals: &[Withdrawal],
    ) -> Result<(Vec<Withdrawal>, u64), TransitionError> {
        let epoch = self.slot.epoch();
        let withdrawals_limit = prior_withdrawals
            .len()
            .saturating_add(MAX_PENDING_PARTIALS_PER_WITHDRAWALS_SWEEP as usize)
            .min(MAX_WITHDRAWALS_PER_PAYLOAD.saturating_sub(1));
        let mut withdrawals: Vec<Withdrawal> = Vec::new();
        let mut processed: u64 = 0;
        for entry in self.pending_partial_withdrawals.iter() {
            let all_count = prior_withdrawals.len() + withdrawals.len();
            if entry.withdrawable_epoch > epoch || all_count >= withdrawals_limit {
                break;
            }
            if entry.validator_index.as_usize() >= self.validators.len() {
                return Err(RegistryError::ValidatorIndexOutOfRange(
                    entry.validator_index.as_u64(),
                )
                .into());
            }
            let validator = &self.validators[entry.validator_index.as_usize()];
            let combined: Vec<Withdrawal> = prior_withdrawals
                .iter()
                .copied()
                .chain(withdrawals.iter().copied())
                .collect();
            let balance = self.balance_after_withdrawals(entry.validator_index, &combined);
            // `is_eligible_for_partial_withdrawals`: validator not yet
            // exiting, effective balance at or above the activation floor,
            // and a strict excess over `MIN_ACTIVATION_BALANCE`. Queue entries
            // that fail eligibility are still consumed so the queue can drain.
            let not_exiting = validator.exit_epoch == FAR_FUTURE_EPOCH;
            let has_sufficient_effective = validator.effective_balance >= MIN_ACTIVATION_BALANCE;
            let has_excess = balance > MIN_ACTIVATION_BALANCE;
            if not_exiting && has_sufficient_effective && has_excess {
                let max_withdraw = balance
                    .as_u64()
                    .saturating_sub(MIN_ACTIVATION_BALANCE.as_u64());
                let amount = Gwei(entry.amount.as_u64().min(max_withdraw));
                withdrawals.push(Withdrawal {
                    index: withdrawal_index,
                    validator_index: entry.validator_index,
                    address: withdrawal_address_from_credentials(&validator.withdrawal_credentials),
                    amount,
                });
                withdrawal_index = WithdrawalIndex(withdrawal_index.as_u64().saturating_add(1));
            }
            processed = processed.saturating_add(1);
        }
        Ok((withdrawals, processed))
    }

    fn get_builders_sweep_withdrawals(
        &self,
        mut withdrawal_index: WithdrawalIndex,
        prior_withdrawals: &[Withdrawal],
    ) -> Result<(Vec<Withdrawal>, u64), TransitionError> {
        let epoch = self.slot.epoch();
        let builder_len = self.builders.len();
        if builder_len == 0 {
            return Ok((Vec::new(), 0));
        }
        if self.next_withdrawal_builder_index.as_usize() >= builder_len {
            return Err(RegistryError::BuilderIndexOutOfRange(
                self.next_withdrawal_builder_index.as_u64(),
            )
            .into());
        }
        let builders_limit = (MAX_BUILDERS_PER_WITHDRAWALS_SWEEP as usize).min(builder_len);
        let withdrawals_limit = MAX_WITHDRAWALS_PER_PAYLOAD.saturating_sub(1);

        let mut withdrawals: Vec<Withdrawal> = Vec::new();
        let mut processed: u64 = 0;
        let mut cursor = self.next_withdrawal_builder_index.as_usize();
        for _ in 0..builders_limit {
            let all_count = prior_withdrawals.len() + withdrawals.len();
            if all_count >= withdrawals_limit {
                break;
            }
            let builder = &self.builders[cursor];
            if builder.withdrawable_epoch <= epoch && builder.balance.as_u64() > 0 {
                withdrawals.push(Withdrawal {
                    index: withdrawal_index,
                    validator_index: builder_index_to_validator_index(BuilderIndex(cursor as u64)),
                    address: builder.execution_address,
                    amount: builder.balance,
                });
                withdrawal_index = WithdrawalIndex(withdrawal_index.as_u64().saturating_add(1));
            }
            cursor = (cursor + 1) % builder_len;
            processed = processed.saturating_add(1);
        }
        Ok((withdrawals, processed))
    }

    fn get_validators_sweep_withdrawals(
        &self,
        mut withdrawal_index: WithdrawalIndex,
        prior_withdrawals: &[Withdrawal],
    ) -> Result<Vec<Withdrawal>, TransitionError> {
        let epoch = self.slot.epoch();
        let registry_len = self.validators.len();
        if registry_len == 0 {
            return Ok(Vec::new());
        }
        if self.next_withdrawal_validator_index.as_usize() >= registry_len {
            return Err(RegistryError::ValidatorIndexOutOfRange(
                self.next_withdrawal_validator_index.as_u64(),
            )
            .into());
        }
        let validators_limit = (MAX_VALIDATORS_PER_WITHDRAWALS_SWEEP as usize).min(registry_len);
        let withdrawals_limit = MAX_WITHDRAWALS_PER_PAYLOAD;

        let mut withdrawals: Vec<Withdrawal> = Vec::new();
        let mut cursor = self.next_withdrawal_validator_index.as_usize();
        for _ in 0..validators_limit {
            let all_count = prior_withdrawals.len() + withdrawals.len();
            if all_count >= withdrawals_limit {
                break;
            }
            let validator = &self.validators[cursor];
            let combined: Vec<Withdrawal> = prior_withdrawals
                .iter()
                .copied()
                .chain(withdrawals.iter().copied())
                .collect();
            let validator_index = ValidatorIndex(cursor as u64);
            let balance = self.balance_after_withdrawals(validator_index, &combined);
            let address = withdrawal_address_from_credentials(&validator.withdrawal_credentials);
            if validator.is_fully_withdrawable_at(balance, epoch) {
                withdrawals.push(Withdrawal {
                    index: withdrawal_index,
                    validator_index,
                    address,
                    amount: balance,
                });
                withdrawal_index = WithdrawalIndex(withdrawal_index.as_u64().saturating_add(1));
            } else if validator.is_partially_withdrawable(balance) {
                let max = validator.max_effective_balance();
                let amount = balance.saturating_sub(max);
                if amount.as_u64() > 0 {
                    withdrawals.push(Withdrawal {
                        index: withdrawal_index,
                        validator_index,
                        address,
                        amount,
                    });
                    withdrawal_index = WithdrawalIndex(withdrawal_index.as_u64().saturating_add(1));
                }
            }
            cursor = (cursor + 1) % registry_len;
        }
        Ok(withdrawals)
    }

    fn expected_withdrawals(&self) -> Result<ExpectedWithdrawals, TransitionError> {
        let mut withdrawals: Vec<Withdrawal> = Vec::new();
        let mut next_index = self.next_withdrawal_index;

        let (builder_withdrawals, processed_builder_withdrawals_count) =
            self.get_builder_withdrawals(next_index)?;
        if let Some(last) = builder_withdrawals.last() {
            next_index = WithdrawalIndex(last.index.as_u64().saturating_add(1));
        }
        withdrawals.extend(builder_withdrawals);

        let (partial_withdrawals, processed_partial_withdrawals_count) =
            self.get_pending_partial_withdrawals(next_index, &withdrawals)?;
        if let Some(last) = partial_withdrawals.last() {
            next_index = WithdrawalIndex(last.index.as_u64().saturating_add(1));
        }
        withdrawals.extend(partial_withdrawals);

        let (builders_sweep_withdrawals, processed_builders_sweep_count) =
            self.get_builders_sweep_withdrawals(next_index, &withdrawals)?;
        if let Some(last) = builders_sweep_withdrawals.last() {
            next_index = WithdrawalIndex(last.index.as_u64().saturating_add(1));
        }
        withdrawals.extend(builders_sweep_withdrawals);

        let validators_sweep_withdrawals =
            self.get_validators_sweep_withdrawals(next_index, &withdrawals)?;
        withdrawals.extend(validators_sweep_withdrawals);

        Ok(ExpectedWithdrawals {
            withdrawals,
            processed_builder_withdrawals_count,
            processed_partial_withdrawals_count,
            processed_builders_sweep_count,
        })
    }

    /// Compute expected withdrawals for the current slot and apply them. Drains
    /// the builder pending-withdrawals queue and partial-withdrawals queue
    /// against their per-sweep limits, then rotates the builder and validator
    /// sweep cursors.
    ///
    /// # Panics
    ///
    /// Panics on the invariant that any `validator_index` already tagged with
    /// `BUILDER_INDEX_FLAG` can be decoded back into a `BuilderIndex`.
    ///
    /// Spec: `process_withdrawals`
    pub fn process_withdrawals(&mut self) -> Result<(), TransitionError> {
        if self.latest_block_hash != self.latest_execution_payload_bid.block_hash {
            return Ok(());
        }

        let expected = self.expected_withdrawals()?;

        // Apply balance changes.
        for w in &expected.withdrawals {
            if w.validator_index.is_builder_index() {
                let builder_index = w
                    .validator_index
                    .to_builder_index()
                    .expect("builder-index flag set");
                let idx = builder_index.as_usize();
                if idx < self.builders.len() {
                    let current = self.builders[idx].balance;
                    let drop = Gwei(w.amount.as_u64().min(current.as_u64()));
                    self.builders[idx].balance = current.saturating_sub(drop);
                }
            } else {
                self.decrease_balance(w.validator_index, w.amount)?;
            }
        }

        // Update next_withdrawal_index.
        if let Some(last) = expected.withdrawals.last() {
            self.next_withdrawal_index = WithdrawalIndex(last.index.as_u64().saturating_add(1));
        }

        // Mirror the chosen withdrawals onto the payload-expected queue.
        self.payload_expected_withdrawals = ssz_rs::List::default();
        for w in &expected.withdrawals {
            self.payload_expected_withdrawals.push(*w);
        }

        // Drain the consumed prefix of the builder pending withdrawals queue.
        if expected.processed_builder_withdrawals_count > 0 {
            let remaining: Vec<_> = self
                .builder_pending_withdrawals
                .iter()
                .skip(expected.processed_builder_withdrawals_count as usize)
                .copied()
                .collect();
            self.builder_pending_withdrawals = ssz_rs::List::default();
            for item in remaining {
                self.builder_pending_withdrawals.push(item);
            }
        }

        // Drain the consumed prefix of the pending-partial-withdrawals queue.
        if expected.processed_partial_withdrawals_count > 0 {
            let remaining: Vec<_> = self
                .pending_partial_withdrawals
                .iter()
                .skip(expected.processed_partial_withdrawals_count as usize)
                .copied()
                .collect();
            self.pending_partial_withdrawals = ssz_rs::List::default();
            for item in remaining {
                self.pending_partial_withdrawals.push(item);
            }
        }

        // Rotate the builder sweep cursor.
        let builder_len = self.builders.len();
        if builder_len > 0 {
            let next = self
                .next_withdrawal_builder_index
                .as_u64()
                .saturating_add(expected.processed_builders_sweep_count)
                % builder_len as u64;
            self.next_withdrawal_builder_index = BuilderIndex(next);
        }

        // Rotate the validator sweep cursor.
        //
        // Spec: `update_next_withdrawal_validator_index`. The mod takes the
        // last withdrawal's `validator_index` as-is, even when the last entry
        // came from the builder sweep with the `BUILDER_INDEX_FLAG` bit set.
        let registry_len = self.validators.len();
        if registry_len > 0 {
            if expected.withdrawals.len() == MAX_WITHDRAWALS_PER_PAYLOAD {
                if let Some(last) = expected.withdrawals.last() {
                    let next = (last.validator_index.as_u64() + 1) % registry_len as u64;
                    self.next_withdrawal_validator_index = ValidatorIndex(next);
                }
            } else {
                let advance = self.next_withdrawal_validator_index.as_u64()
                    + MAX_VALIDATORS_PER_WITHDRAWALS_SWEEP;
                self.next_withdrawal_validator_index =
                    ValidatorIndex(advance % registry_len as u64);
            }
        }

        Ok(())
    }
}
