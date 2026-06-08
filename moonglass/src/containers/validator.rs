//! Validator registry record and the pending queues that gate registry transitions.

use ssz_rs::prelude::*;

use crate::primitives::{BLSPubkey, BLSSignature, Bytes32, Epoch, Gwei, Slot, ValidatorIndex};

/// A single validator entry in the registry.
#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, SimpleSerialize)]
pub struct Validator {
    /// BLS public key used to verify the validator's signatures.
    pub pubkey: BLSPubkey,
    /// 32-byte credential whose leading byte selects the withdrawal-credential variant.
    pub withdrawal_credentials: Bytes32,
    /// Quantized balance contributing to weight, voting power, and base-reward math.
    pub effective_balance: Gwei,
    /// True once the validator has been successfully slashed.
    pub slashed: bool,
    /// Epoch at which the validator became eligible to enter the activation queue.
    pub activation_eligibility_epoch: Epoch,
    /// Epoch at which the validator became active.
    pub activation_epoch: Epoch,
    /// Epoch at which the validator exited (or `FAR_FUTURE_EPOCH` if not exited).
    pub exit_epoch: Epoch,
    /// Epoch at which the balance becomes withdrawable.
    pub withdrawable_epoch: Epoch,
}

/// Entry in the deferred-deposit queue awaiting signature verification and activation.
#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, SimpleSerialize)]
pub struct PendingDeposit {
    /// Depositing validator's public key.
    pub pubkey: BLSPubkey,
    /// Withdrawal credential the deposit binds the validator to.
    pub withdrawal_credentials: Bytes32,
    /// Deposit amount.
    pub amount: Gwei,
    /// Signature over the deposit message, checked when the deposit is processed.
    pub signature: BLSSignature,
    /// Slot the deposit was observed in.
    pub slot: Slot,
}

/// Scheduled partial withdrawal of a validator's surplus balance.
#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, SimpleSerialize)]
pub struct PendingPartialWithdrawal {
    /// Validator the withdrawal applies to.
    pub validator_index: ValidatorIndex,
    /// Amount to withdraw at `withdrawable_epoch`.
    pub amount: Gwei,
    /// Earliest epoch the withdrawal becomes due.
    pub withdrawable_epoch: Epoch,
}

/// Pending merge of one validator's balance into another's.
#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, SimpleSerialize)]
pub struct PendingConsolidation {
    /// Source validator (balance is moved out of this one).
    pub source_index: ValidatorIndex,
    /// Target validator (balance is folded into this one).
    pub target_index: ValidatorIndex,
}
