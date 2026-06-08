//! Validator predicates, registry lookups, and lifecycle scheduling.
//!
//! A validator enters through a deposit path, waits for activation, performs
//! duties while active, may change withdrawal credentials, may request exit,
//! waits through churn scheduling, and then becomes withdrawable. Effective
//! balance controls voting and reward weight. Actual balance controls available
//! withdrawal amount.

use crate::constants::{
    BLS_WITHDRAWAL_PREFIX, CHURN_LIMIT_QUOTIENT, COMPOUNDING_WITHDRAWAL_PREFIX,
    CONSOLIDATION_CHURN_LIMIT_QUOTIENT, EFFECTIVE_BALANCE_INCREMENT,
    ETH1_ADDRESS_WITHDRAWAL_PREFIX, FAR_FUTURE_EPOCH, MAX_EFFECTIVE_BALANCE,
    MAX_PER_EPOCH_ACTIVATION_CHURN_LIMIT, MAX_SEED_LOOKAHEAD, MIN_ACTIVATION_BALANCE,
    MIN_PER_EPOCH_CHURN_LIMIT, MIN_VALIDATOR_WITHDRAWABILITY_DELAY,
};
use crate::containers::{BeaconState, Builder, Validator};
use crate::error::{RegistryError, TransitionError};
use crate::primitives::{BLSPubkey, BuilderIndex, Epoch, Gwei, ValidatorIndex};

impl Validator {
    /// True if this validator is in the active set during `epoch`.
    #[must_use]
    pub fn is_active_at(&self, epoch: Epoch) -> bool {
        self.activation_epoch <= epoch && epoch < self.exit_epoch
    }

    /// True if this validator is slashable at `epoch`.
    #[must_use]
    pub fn is_slashable_at(&self, epoch: Epoch) -> bool {
        !self.slashed && self.activation_epoch <= epoch && epoch < self.withdrawable_epoch
    }

    /// True if this validator has Eth1-address withdrawal credentials.
    #[must_use]
    pub fn has_eth1_withdrawal_credential(&self) -> bool {
        self.withdrawal_credentials[0] == ETH1_ADDRESS_WITHDRAWAL_PREFIX
    }

    /// True if this validator has compounding withdrawal credentials.
    #[must_use]
    pub fn has_compounding_withdrawal_credential(&self) -> bool {
        self.withdrawal_credentials[0] == COMPOUNDING_WITHDRAWAL_PREFIX
    }

    /// True if this validator has execution-address or compounding credentials.
    #[must_use]
    pub fn has_execution_withdrawal_credential(&self) -> bool {
        self.has_eth1_withdrawal_credential() || self.has_compounding_withdrawal_credential()
    }

    /// True if this validator still has bare BLS withdrawal credentials.
    #[must_use]
    pub fn has_bls_withdrawal_credential(&self) -> bool {
        self.withdrawal_credentials[0] == BLS_WITHDRAWAL_PREFIX
    }

    /// True if this validator is fully withdrawable at `epoch`.
    #[must_use]
    pub fn is_fully_withdrawable_at(&self, balance: Gwei, epoch: Epoch) -> bool {
        self.has_execution_withdrawal_credential()
            && self.withdrawable_epoch <= epoch
            && balance.as_u64() > 0
    }

    /// True if this validator has effective-balance excess that can be swept
    /// as a partial withdrawal.
    ///
    /// Spec: `is_partially_withdrawable_validator`.
    #[must_use]
    pub fn is_partially_withdrawable(&self, balance: Gwei) -> bool {
        let max = self.max_effective_balance();
        let has_max_effective_balance = self.effective_balance == max;
        let has_excess_balance = balance > max;
        self.has_execution_withdrawal_credential()
            && has_max_effective_balance
            && has_excess_balance
    }

    /// Effective-balance cap: 2048 ETH if compounding, else 32 ETH.
    #[must_use]
    pub fn max_effective_balance(&self) -> Gwei {
        if self.has_compounding_withdrawal_credential() {
            MAX_EFFECTIVE_BALANCE
        } else {
            MIN_ACTIVATION_BALANCE
        }
    }
}

/// Checked indexed and keyed access into the validator and builder registries.
///
/// Out-of-range index access becomes a typed transition error. Pubkey lookups
/// return `Option` because a missing pubkey is a documented drop case for
/// several spec paths, not an error.
pub trait BeaconStateLookup {
    /// Borrow the validator at `index`, returning a typed out-of-range error.
    fn validator(&self, index: ValidatorIndex) -> Result<&Validator, TransitionError>;

    /// Borrow the builder at `index`, returning a typed out-of-range error.
    fn builder(&self, index: BuilderIndex) -> Result<&Builder, TransitionError>;

    /// Locate the validator index for `pubkey`. Linear scan. `None` if absent.
    fn validator_index(&self, pubkey: &BLSPubkey) -> Option<ValidatorIndex>;

    /// Locate the builder index for `pubkey`. Linear scan. `None` if absent.
    fn builder_index(&self, pubkey: &BLSPubkey) -> Option<BuilderIndex>;
}

impl BeaconStateLookup for BeaconState {
    fn validator(&self, index: ValidatorIndex) -> Result<&Validator, TransitionError> {
        self.validators
            .get(index.as_usize())
            .ok_or_else(|| RegistryError::ValidatorIndexOutOfRange(index.as_u64()).into())
    }

    fn builder(&self, index: BuilderIndex) -> Result<&Builder, TransitionError> {
        self.builders
            .get(index.as_usize())
            .ok_or_else(|| RegistryError::BuilderIndexOutOfRange(index.as_u64()).into())
    }

    fn validator_index(&self, pubkey: &BLSPubkey) -> Option<ValidatorIndex> {
        self.validators
            .iter()
            .position(|v| v.pubkey == *pubkey)
            .map(|i| ValidatorIndex(i as u64))
    }

    fn builder_index(&self, pubkey: &BLSPubkey) -> Option<BuilderIndex> {
        self.builders
            .iter()
            .position(|b| b.pubkey == *pubkey)
            .map(|i| BuilderIndex(i as u64))
    }
}

impl BeaconState {
    /// True if `pubkey` already has an entry in the validator registry or in the
    /// pending-deposit queue.
    #[must_use]
    pub fn is_pending_validator(&self, pubkey: &BLSPubkey) -> bool {
        self.validators.iter().any(|v| v.pubkey == *pubkey)
            || self.pending_deposits.iter().any(|d| d.pubkey == *pubkey)
    }

    /// Sum of scheduled partial withdrawals queued against `index`.
    #[must_use]
    pub fn pending_balance_to_withdraw(&self, index: ValidatorIndex) -> Gwei {
        self.pending_partial_withdrawals
            .iter()
            .filter(|w| w.validator_index == index)
            .map(|w| w.amount)
            .fold(Gwei::ZERO, Gwei::saturating_add)
    }

    /// Total gwei the chain is willing to move into or out of the active set per epoch,
    /// before any activation or consolidation specific caps are applied.
    #[must_use]
    pub fn balance_churn_limit(&self) -> Gwei {
        let stake_scaled = Gwei(self.total_active_balance().as_u64() / CHURN_LIMIT_QUOTIENT);
        let churn = MIN_PER_EPOCH_CHURN_LIMIT.max(stake_scaled);
        let remainder = churn.as_u64() % EFFECTIVE_BALANCE_INCREMENT.as_u64();
        Gwei(churn.as_u64() - remainder)
    }

    /// Per-epoch churn budget for activations, capped at
    /// `MAX_PER_EPOCH_ACTIVATION_CHURN_LIMIT`.
    #[must_use]
    pub fn activation_churn_limit(&self) -> Gwei {
        MAX_PER_EPOCH_ACTIVATION_CHURN_LIMIT.min(self.balance_churn_limit())
    }

    /// Per-epoch churn budget for exits. Equal to `balance_churn_limit`, with no
    /// activation cap applied: the exit pipeline is independent of activation.
    #[must_use]
    pub fn exit_churn_limit(&self) -> Gwei {
        self.balance_churn_limit()
    }

    /// Per-epoch churn budget specifically for consolidations. Derived directly
    /// from `total_active_balance / CONSOLIDATION_CHURN_LIMIT_QUOTIENT` and
    /// rounded down to `EFFECTIVE_BALANCE_INCREMENT`.
    #[must_use]
    pub fn consolidation_churn_limit(&self) -> Gwei {
        let raw = self.total_active_balance().as_u64() / CONSOLIDATION_CHURN_LIMIT_QUOTIENT;
        let remainder = raw % EFFECTIVE_BALANCE_INCREMENT.as_u64();
        Gwei(raw - remainder)
    }

    /// Assign an exit epoch to `exit_balance` worth of departing stake.
    pub fn consume_exit_churn(&mut self, exit_balance: Gwei) -> Epoch {
        let current = self.slot.epoch();
        let per_epoch_churn = self.exit_churn_limit();
        let (exit_epoch, remaining) = consume_churn_budget(
            current,
            self.earliest_exit_epoch,
            self.exit_balance_to_consume,
            exit_balance,
            per_epoch_churn,
        );
        self.exit_balance_to_consume = remaining;
        self.earliest_exit_epoch = exit_epoch;
        exit_epoch
    }

    /// Assign a consolidation epoch to `consolidation_balance` worth of stake.
    pub fn consume_consolidation_churn(&mut self, consolidation_balance: Gwei) -> Epoch {
        let current = self.slot.epoch();
        let per_epoch_churn = self.consolidation_churn_limit();
        let (consolidation_epoch, remaining) = consume_churn_budget(
            current,
            self.earliest_consolidation_epoch,
            self.consolidation_balance_to_consume,
            consolidation_balance,
            per_epoch_churn,
        );
        self.consolidation_balance_to_consume = remaining;
        self.earliest_consolidation_epoch = consolidation_epoch;
        consolidation_epoch
    }

    /// Schedule `index` to exit the active set.
    pub fn initiate_validator_exit(
        &mut self,
        index: ValidatorIndex,
    ) -> Result<(), TransitionError> {
        let validator = self.validator(index)?;
        if validator.exit_epoch != FAR_FUTURE_EPOCH {
            return Ok(());
        }
        let effective_balance = validator.effective_balance;
        let exit_epoch = self.consume_exit_churn(effective_balance);
        let withdrawable = exit_epoch
            .as_u64()
            .checked_add(MIN_VALIDATOR_WITHDRAWABILITY_DELAY)
            .map(crate::primitives::Epoch)
            .ok_or(crate::error::OperationError::WithdrawableEpochOverflow(
                index,
            ))?;
        let v = &mut self.validators[index.as_usize()];
        v.exit_epoch = exit_epoch;
        v.withdrawable_epoch = withdrawable;
        Ok(())
    }

    /// Switch `index`'s credential prefix to compounding.
    pub fn switch_to_compounding_validator(
        &mut self,
        index: ValidatorIndex,
    ) -> Result<(), TransitionError> {
        // Existence check: errors if `index` is out of bounds for the validator registry.
        let _ = self.validator(index)?;
        let v = &mut self.validators[index.as_usize()];
        if !v.has_eth1_withdrawal_credential() {
            return Ok(());
        }
        v.withdrawal_credentials[0] = COMPOUNDING_WITHDRAWAL_PREFIX;
        self.queue_excess_active_balance(index)?;
        Ok(())
    }

    /// Move any balance above `MIN_ACTIVATION_BALANCE` into a pending deposit
    /// for the same validator, so the excess re-enters via the activation
    /// churn path instead of immediately contributing to consolidated stake.
    pub fn queue_excess_active_balance(
        &mut self,
        index: ValidatorIndex,
    ) -> Result<(), TransitionError> {
        let balance = self
            .balances
            .get(index.as_usize())
            .copied()
            .unwrap_or(Gwei::ZERO);
        if balance <= MIN_ACTIVATION_BALANCE {
            return Ok(());
        }
        let excess_balance = balance.saturating_sub(MIN_ACTIVATION_BALANCE);
        self.balances[index.as_usize()] = MIN_ACTIVATION_BALANCE;
        let validator = self.validator(index)?;
        let pubkey = validator.pubkey;
        let withdrawal_credentials = validator.withdrawal_credentials;
        // Re-queued excess uses the G2 infinity signature and the genesis
        // slot as sentinels so the queue can distinguish it from a fresh
        // deposit request.
        self.pending_deposits
            .push(crate::containers::PendingDeposit {
                pubkey,
                withdrawal_credentials,
                amount: excess_balance,
                signature: crate::primitives::BLSSignature::G2_POINT_AT_INFINITY,
                slot: crate::constants::GENESIS_SLOT,
            });
        Ok(())
    }
}

/// Earliest epoch at which a validator activating or exiting now becomes
/// effective. Adds the seed-lookahead buffer so committee shuffling has
/// settled.
#[must_use]
pub fn compute_activation_exit_epoch(epoch: Epoch) -> Epoch {
    epoch.saturating_add(1 + MAX_SEED_LOOKAHEAD)
}

fn consume_churn_budget(
    current_epoch: Epoch,
    cursor_epoch: Epoch,
    cursor_balance: Gwei,
    requested_balance: Gwei,
    per_epoch_churn: Gwei,
) -> (Epoch, Gwei) {
    let earliest = cursor_epoch.max(compute_activation_exit_epoch(current_epoch));
    let mut available = if cursor_epoch < earliest {
        per_epoch_churn
    } else {
        cursor_balance
    };
    let mut epoch = earliest;

    if requested_balance > available {
        let per_epoch = per_epoch_churn.as_u64();
        if per_epoch == 0 {
            return (epoch, Gwei::ZERO);
        }
        let overflow = requested_balance.as_u64() - available.as_u64();
        let additional_epochs = (overflow - 1) / per_epoch + 1;
        epoch = epoch.saturating_add(additional_epochs);
        let added_budget = additional_epochs.saturating_mul(per_epoch);
        available = Gwei(available.as_u64().saturating_add(added_budget));
    }

    (epoch, available.saturating_sub(requested_balance))
}
