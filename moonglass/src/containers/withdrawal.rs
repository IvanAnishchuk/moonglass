//! Validator lifecycle and execution-to-consensus request containers.
//!
//! These containers cover deposits, exits, withdrawals, credential changes, and
//! requests created by the execution layer for the consensus transition to
//! consume.
//!
//! The lifecycle path is: a deposit or deposit request enters a pending queue,
//! the validator becomes active, performs duties, may change withdrawal
//! credentials, may request voluntary exit, waits through churn scheduling, and
//! eventually becomes withdrawable. Consensus-layer withdrawals move balances
//! out. Execution-layer withdrawal requests ask consensus to schedule that
//! movement.
//!
//! Validator balances are consensus-layer accounting. Deposits and withdrawals
//! are controlled movements between that accounting and execution-layer ETH.

use crate::constants::{
    DEPOSIT_PROOF_LEN, MAX_CONSOLIDATION_REQUESTS_PER_PAYLOAD, MAX_DEPOSIT_REQUESTS_PER_PAYLOAD,
    MAX_WITHDRAWAL_REQUESTS_PER_PAYLOAD,
};
use crate::primitives::{
    BLSPubkey, BLSSignature, Bytes32, Epoch, ExecutionAddress, Gwei, ValidatorIndex,
    WithdrawalIndex,
};
use ssz_rs::prelude::*;

/// Atomic balance movement from the consensus layer to an execution-layer address.
#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, SimpleSerialize)]
pub struct Withdrawal {
    /// Monotonic index uniquely identifying this withdrawal across history.
    pub index: WithdrawalIndex,
    /// Validator whose balance is being withdrawn.
    pub validator_index: ValidatorIndex,
    /// Execution-layer destination address.
    pub address: ExecutionAddress,
    /// Withdrawn amount.
    pub amount: Gwei,
}

/// Request to swap a BLS withdrawal credential for an execution address.
#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, SimpleSerialize)]
pub struct BLSToExecutionChange {
    /// Validator initiating the credential change.
    pub validator_index: ValidatorIndex,
    /// BLS key authorising the change (must match the current withdrawal credential).
    pub from_bls_pubkey: BLSPubkey,
    /// New execution address for future withdrawals.
    pub to_execution_address: ExecutionAddress,
}

/// Credential-change request plus the BLS signature authorising it.
#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, SimpleSerialize)]
pub struct SignedBLSToExecutionChange {
    /// The credential-change being signed.
    pub message: BLSToExecutionChange,
    /// Signature over the domain-separated signing root of `message`.
    pub signature: BLSSignature,
}

/// Validator-initiated request to leave the active set.
#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, SimpleSerialize)]
pub struct VoluntaryExit {
    /// Epoch the exit is being signed at. The exit cannot take effect earlier.
    pub epoch: Epoch,
    /// Validator requesting to exit.
    pub validator_index: ValidatorIndex,
}

/// Voluntary exit plus the validator's signature authorising it.
#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, SimpleSerialize)]
pub struct SignedVoluntaryExit {
    /// The voluntary exit being signed.
    pub message: VoluntaryExit,
    /// Signature over the domain-separated signing root of `message`.
    pub signature: BLSSignature,
}

/// Deposit payload as written to the execution-layer deposit contract.
#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, SimpleSerialize)]
pub struct DepositData {
    /// Depositing validator's public key.
    pub pubkey: BLSPubkey,
    /// Withdrawal credential the deposit binds the validator to.
    pub withdrawal_credentials: Bytes32,
    /// Deposit amount.
    pub amount: Gwei,
    /// Signature over the deposit message (the unsigned tuple).
    pub signature: BLSSignature,
}

/// A deposit-contract event packaged with its Merkle inclusion proof.
#[derive(Default, Debug, Clone, PartialEq, Eq, SimpleSerialize)]
pub struct Deposit {
    /// Merkle proof of inclusion in the deposit-contract tree (depth + 1 nodes).
    pub proof: Vector<Bytes32, DEPOSIT_PROOF_LEN>,
    /// The deposit payload being proven.
    pub data: DepositData,
}

/// Execution-layer deposit request that supersedes deposit-vote-driven deposits.
#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, SimpleSerialize)]
pub struct DepositRequest {
    /// Depositing validator's public key.
    pub pubkey: BLSPubkey,
    /// Withdrawal credential the deposit binds the validator to.
    pub withdrawal_credentials: Bytes32,
    /// Deposit amount.
    pub amount: Gwei,
    /// Signature over the deposit message.
    pub signature: BLSSignature,
    /// Sequence index assigned by the deposit contract.
    pub index: u64,
}

/// Execution-layer request to withdraw or fully exit a validator.
#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, SimpleSerialize)]
pub struct WithdrawalRequest {
    /// Execution-layer address authorising the request (must match the credential).
    pub source_address: ExecutionAddress,
    /// Validator targeted by the request.
    pub validator_pubkey: BLSPubkey,
    /// Amount to withdraw, or `FULL_EXIT_REQUEST_AMOUNT` for a full exit.
    pub amount: Gwei,
}

/// Execution-layer request to consolidate one validator's balance into another.
#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, SimpleSerialize)]
pub struct ConsolidationRequest {
    /// Execution-layer address authorising the consolidation.
    pub source_address: ExecutionAddress,
    /// Source validator's public key (balance moved out).
    pub source_pubkey: BLSPubkey,
    /// Target validator's public key (balance folded in).
    pub target_pubkey: BLSPubkey,
}

/// All execution-to-consensus requests delivered by a payload, grouped by kind.
#[derive(Default, Debug, Clone, PartialEq, Eq, SimpleSerialize)]
pub struct ExecutionRequests {
    /// Execution-layer deposit requests.
    pub deposits: List<DepositRequest, MAX_DEPOSIT_REQUESTS_PER_PAYLOAD>,
    /// Execution-layer partial-withdrawal and full-exit requests.
    pub withdrawals: List<WithdrawalRequest, MAX_WITHDRAWAL_REQUESTS_PER_PAYLOAD>,
    /// Execution-layer consolidation requests.
    pub consolidations: List<ConsolidationRequest, MAX_CONSOLIDATION_REQUESTS_PER_PAYLOAD>,
}

impl ExecutionRequests {
    /// True when the payload carried no execution-to-consensus requests.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.deposits.is_empty() && self.withdrawals.is_empty() && self.consolidations.is_empty()
    }
}
