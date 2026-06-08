//! Execution-layer requests processing (deposits, withdrawals, consolidations).

use crate::constants::{
    FAR_FUTURE_EPOCH, FULL_EXIT_REQUEST_AMOUNT, MIN_ACTIVATION_BALANCE,
    MIN_VALIDATOR_WITHDRAWABILITY_DELAY, PENDING_CONSOLIDATIONS_LIMIT,
    PENDING_PARTIAL_WITHDRAWALS_LIMIT, SHARD_COMMITTEE_PERIOD,
};
use crate::containers::{
    BeaconState, ConsolidationRequest, DepositRequest, PendingConsolidation, PendingDeposit,
    PendingPartialWithdrawal, WithdrawalRequest,
};
use crate::error::TransitionError;
use crate::primitives::Gwei;
use crate::state_transition::BeaconStateLookup;

impl BeaconState {
    /// Route a deposit-request payload. Existing-builder pubkeys (or a new
    /// pubkey with builder credentials that is not already a validator or
    /// queued) flow into the builder registry via `apply_deposit_for_builder`.
    /// Everything else queues onto `pending_deposits` for the activation churn
    /// path in epoch processing.
    ///
    /// Spec: `process_deposit_request`
    pub fn process_deposit_request(
        &mut self,
        request: &DepositRequest,
    ) -> Result<(), TransitionError> {
        let is_builder = self.builders.iter().any(|b| b.pubkey == request.pubkey);
        let is_validator = self.validators.iter().any(|v| v.pubkey == request.pubkey);
        let is_pending = self.is_pending_validator_deposit(&request.pubkey)?;
        let creds_are_builder =
            Self::is_builder_withdrawal_credential(&request.withdrawal_credentials);
        if is_builder || (creds_are_builder && !is_validator && !is_pending) {
            return self.apply_deposit_for_builder(
                request.pubkey,
                request.withdrawal_credentials,
                request.amount,
                request.signature,
            );
        }
        // Validator deposit requests queue with `slot = state.slot` so
        // `process_pending_deposits` can distinguish them from Eth1-bridge
        // deposits (which queue with `slot = GENESIS_SLOT` via `apply_deposit`).
        self.pending_deposits.push(PendingDeposit {
            pubkey: request.pubkey,
            withdrawal_credentials: request.withdrawal_credentials,
            amount: request.amount,
            signature: request.signature,
            slot: self.slot,
        });
        Ok(())
    }

    /// Apply a withdrawal-request payload by either initiating exit or queueing a
    /// partial withdrawal.
    ///
    /// Full-exit requests (`amount == FULL_EXIT_REQUEST_AMOUNT`) require an
    /// active, not-yet-exiting validator with execution-layer withdrawal
    /// credentials, no pending partial withdrawal, and an eligibility wait of
    /// `SHARD_COMMITTEE_PERIOD` past activation. Partial requests require
    /// compounding credentials, sufficient effective balance, and excess
    /// balance over `MIN_ACTIVATION_BALANCE` net of already-queued partials;
    /// the actual amount queued consumes exit churn and is clamped to that
    /// excess.
    ///
    /// Spec: `process_withdrawal_request`
    pub fn process_withdrawal_request(
        &mut self,
        request: &WithdrawalRequest,
    ) -> Result<(), TransitionError> {
        // Skip partial-withdrawal requests when the queue is at its hard cap.
        // Full-exit requests still flow through because they don't enqueue a
        // partial withdrawal.
        let is_full_exit_request = request.amount == FULL_EXIT_REQUEST_AMOUNT;
        if !is_full_exit_request
            && self.pending_partial_withdrawals.len() == PENDING_PARTIAL_WITHDRAWALS_LIMIT
        {
            return Ok(());
        }
        let Some(validator_index) = self.validator_index(&request.validator_pubkey) else {
            return Ok(());
        };
        let validator = self.validator(validator_index)?;
        if !validator.has_execution_withdrawal_credential() {
            return Ok(());
        }
        let creds = validator.withdrawal_credentials;
        if creds[12..] != request.source_address.0[..] {
            return Ok(());
        }
        if !validator.is_active_at(self.slot.epoch()) {
            return Ok(());
        }
        if validator.exit_epoch != FAR_FUTURE_EPOCH {
            return Ok(());
        }
        let current = self.slot.epoch();
        if current
            < validator
                .activation_epoch
                .saturating_add(SHARD_COMMITTEE_PERIOD)
        {
            return Ok(());
        }

        let pending = self.pending_balance_to_withdraw(validator_index);

        if is_full_exit_request {
            if pending != Gwei::ZERO {
                return Ok(());
            }
            self.initiate_validator_exit(validator_index)?;
            return Ok(());
        }

        // Partial withdrawal path: compounding credentials only.
        let validator = self.validator(validator_index)?;
        if !validator.has_compounding_withdrawal_credential() {
            return Ok(());
        }
        let effective_balance = validator.effective_balance;
        let balance = self.balances[validator_index.as_usize()];
        let has_sufficient_effective_balance = effective_balance >= MIN_ACTIVATION_BALANCE;
        let floor = MIN_ACTIVATION_BALANCE.saturating_add(pending);
        let has_excess_balance = balance > floor;
        if !has_sufficient_effective_balance || !has_excess_balance {
            return Ok(());
        }
        let max_excess = balance.as_u64() - floor.as_u64();
        let to_withdraw = Gwei(max_excess.min(request.amount.as_u64()));
        let exit_queue_epoch = self.consume_exit_churn(to_withdraw);
        let withdrawable_epoch =
            exit_queue_epoch.saturating_add(MIN_VALIDATOR_WITHDRAWABILITY_DELAY);
        self.pending_partial_withdrawals
            .push(PendingPartialWithdrawal {
                validator_index,
                amount: to_withdraw,
                withdrawable_epoch,
            });
        Ok(())
    }

    /// Apply a consolidation-request payload. Routes a same-key request through
    /// the switch-to-compounding helper. Otherwise validates the full set of
    /// preconditions and, if all pass, schedules the source exit via the
    /// consolidation-churn cursor and appends a pending consolidation.
    ///
    /// Spec: `process_consolidation_request`
    pub fn process_consolidation_request(
        &mut self,
        request: &ConsolidationRequest,
    ) -> Result<(), TransitionError> {
        if self.is_valid_switch_to_compounding_request(request) {
            if let Some(source_index) = self.validator_index(&request.source_pubkey) {
                self.switch_to_compounding_validator(source_index)?;
            }
            return Ok(());
        }

        // Source must not equal target so a consolidation cannot impersonate
        // an exit on a validator that lacks one.
        if request.source_pubkey == request.target_pubkey {
            return Ok(());
        }
        if self.pending_consolidations.len() == PENDING_CONSOLIDATIONS_LIMIT {
            return Ok(());
        }
        if self.consolidation_churn_limit() <= MIN_ACTIVATION_BALANCE {
            return Ok(());
        }

        let Some(source_index) = self.validator_index(&request.source_pubkey) else {
            return Ok(());
        };
        let Some(target_index) = self.validator_index(&request.target_pubkey) else {
            return Ok(());
        };
        let source = self.validator(source_index)?;
        if !source.has_execution_withdrawal_credential() {
            return Ok(());
        }
        if source.withdrawal_credentials[12..] != request.source_address.0[..] {
            return Ok(());
        }
        let target = self.validator(target_index)?;
        if !target.has_compounding_withdrawal_credential() {
            return Ok(());
        }
        let current_epoch = self.slot.epoch();
        if !source.is_active_at(current_epoch) || !target.is_active_at(current_epoch) {
            return Ok(());
        }
        if source.exit_epoch != FAR_FUTURE_EPOCH || target.exit_epoch != FAR_FUTURE_EPOCH {
            return Ok(());
        }
        if current_epoch
            < source
                .activation_epoch
                .saturating_add(SHARD_COMMITTEE_PERIOD)
        {
            return Ok(());
        }
        if self.pending_balance_to_withdraw(source_index) > Gwei::ZERO {
            return Ok(());
        }

        let source_effective_balance = source.effective_balance;
        let exit_epoch = self.consume_consolidation_churn(source_effective_balance);
        let withdrawable_epoch = exit_epoch.saturating_add(MIN_VALIDATOR_WITHDRAWABILITY_DELAY);
        let v = &mut self.validators[source_index.as_usize()];
        v.exit_epoch = exit_epoch;
        v.withdrawable_epoch = withdrawable_epoch;

        self.pending_consolidations.push(PendingConsolidation {
            source_index,
            target_index,
        });
        Ok(())
    }

    fn is_valid_switch_to_compounding_request(&self, request: &ConsolidationRequest) -> bool {
        if request.source_pubkey != request.target_pubkey {
            return false;
        }
        let Some(source_index) = self.validator_index(&request.source_pubkey) else {
            return false;
        };
        let Ok(source) = self.validator(source_index) else {
            return false;
        };
        if source.withdrawal_credentials[12..] != request.source_address.0[..] {
            return false;
        }
        if !source.has_eth1_withdrawal_credential() {
            return false;
        }
        let current_epoch = self.slot.epoch();
        source.is_active_at(current_epoch) && source.exit_epoch == FAR_FUTURE_EPOCH
    }
}
