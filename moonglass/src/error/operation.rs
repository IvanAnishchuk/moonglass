//! Validation failures from currently covered block-operation behavior.

use crate::primitives::{BuilderIndex, Epoch, Slot, ValidatorIndex};
use thiserror::Error;

/// Failures from operation processing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
#[non_exhaustive]
pub enum OperationError {
    /// A voluntary-exit operation targets a validator that is not in the active set.
    #[error("validator {0} is not active")]
    ValidatorNotActive(ValidatorIndex),

    /// A voluntary-exit operation targets a validator whose exit was already scheduled.
    #[error("validator {0} already exiting")]
    ValidatorAlreadyExiting(ValidatorIndex),

    /// A voluntary exit's signed epoch is in the future relative to the state.
    #[error("voluntary exit signed for epoch {exit} but state is at {current}")]
    ExitTooEarly { current: Epoch, exit: Epoch },

    /// A validator tried to voluntary-exit before serving `SHARD_COMMITTEE_PERIOD`.
    #[error("validator {validator} not eligible to exit until epoch {eligible}, current {current}")]
    ValidatorTooYoung {
        validator: ValidatorIndex,
        eligible: Epoch,
        current: Epoch,
    },

    /// Voluntary exit refused because the validator has a non-zero pending-withdrawal queue entry.
    #[error("validator {0} has a pending withdrawal queue entry")]
    ValidatorHasPendingWithdrawal(ValidatorIndex),

    /// Voluntary exit refused because the builder has a non-zero pending-withdrawal queue entry.
    #[error("builder {0} has a pending withdrawal queue entry")]
    BuilderHasPendingWithdrawal(BuilderIndex),

    /// A BLS-to-execution change targets a validator that is not BLS-credentialed.
    #[error("validator {0} does not have BLS withdrawal credentials")]
    WithdrawalCredentialsNotBls(ValidatorIndex),

    /// A BLS-to-execution change does not match the validator's existing BLS credential hash.
    #[error("BLS change credential mismatch for validator {0}")]
    BlsChangeCredentialMismatch(ValidatorIndex),

    /// Attestation slot, source, target, or inclusion delay failed validation.
    #[error("attestation slot {0} invalid")]
    AttestationSlotInvalid(Slot),
    /// Attestation payload-status/index field is invalid for this slot.
    #[error("attestation payload status invalid")]
    AttestationPayloadStatusInvalid,
    /// Attestation source checkpoint disagrees with the state's justified checkpoint.
    #[error("attestation source checkpoint mismatch")]
    AttestationSourceMismatch,
    /// Attestation target epoch does not match the slot's epoch.
    #[error("attestation target epoch mismatch")]
    AttestationTargetEpochInvalid,
    /// Attestation has no attesting validators after committee decoding.
    #[error("attestation has no attesting participants")]
    AttestationParticipantsEmpty,
    /// Attestation aggregation bits do not match the decoded committee size.
    #[error("attestation aggregation bits do not match committee size")]
    AttestationAggregationBitsLength,
    /// Attester slashing references an attestation that is not slashable.
    #[error("attester slashing attestations are not a slashable pair")]
    AttesterSlashingNotSlashable,
    /// Attester slashing has no overlap between its two attestations' attester sets.
    #[error("attester slashing has no overlapping attesters")]
    AttesterSlashingNoIntersection,
    /// Indexed attestation has no attesting indices.
    #[error("indexed attestation has no attesting indices")]
    IndexedAttestationEmpty,
    /// Indexed attestation's attesting indices are not sorted unique.
    #[error("indexed attestation attesting indices not sorted unique")]
    IndexedAttestationNotSorted,
    /// Proposer slashing's two signed headers have the same tree root.
    #[error("proposer slashing headers are identical")]
    ProposerSlashingHeadersMatch,
    /// Proposer slashing's two headers are not for the same slot.
    #[error("proposer slashing slots do not match")]
    ProposerSlashingSlotMismatch,
    /// Proposer slashing's two headers are not by the same proposer.
    #[error("proposer slashing proposers do not match")]
    ProposerSlashingProposerMismatch,
    /// Proposer slashing targets a non-slashable validator.
    #[error("proposer slashing target validator {0} not slashable")]
    ProposerSlashingNotSlashable(ValidatorIndex),
    /// Deposit Merkle proof did not verify against `eth1_data.deposit_root`.
    #[error("deposit merkle proof invalid")]
    DepositMerkleInvalid,
    /// Deposit request did not pass per-request validation.
    #[error("deposit request invalid")]
    DepositRequestInvalid,
    /// Withdrawal request did not pass per-request validation.
    #[error("withdrawal request invalid")]
    WithdrawalRequestInvalid,
    /// Consolidation request did not pass per-request validation.
    #[error("consolidation request invalid")]
    ConsolidationRequestInvalid,
    /// Payload attestation slot does not match the parent slot.
    #[error("payload attestation slot mismatch")]
    PayloadAttestationSlotMismatch,
    /// Payload attestation references a block root the state does not know.
    #[error("payload attestation block root mismatch")]
    PayloadAttestationBlockRootMismatch,
    /// Builder is not active at the time of bid acceptance.
    #[error("builder {0} not active")]
    BuilderNotActive(BuilderIndex),
    /// Builder cannot cover the bid amount with its current balance.
    #[error("builder {0} balance insufficient to cover bid")]
    BuilderInsufficientBalance(BuilderIndex),
    /// A proposer self-build bid promised a non-zero payment.
    #[error("self-build bid value must be zero")]
    BuilderBidSelfBuildNonZero,
    /// A proposer self-build bid did not use the point-at-infinity signature.
    #[error("self-build bid signature must be the BLS point at infinity")]
    BuilderBidSelfBuildSignature,
    /// Builder bid slot does not match the proposer's slot.
    #[error("builder bid slot mismatch")]
    BuilderBidSlotMismatch,
    /// Builder bid parent block hash or root does not match the state's view.
    #[error("builder bid parent mismatch")]
    BuilderBidParentMismatch,
    /// Builder bid `prev_randao` does not match the proposer's expected randao.
    #[error("builder bid randao mismatch")]
    BuilderBidRandaoMismatch,
    /// Builder bid contains more blob commitments than the active limit allows.
    #[error("builder bid blob commitment count {got} exceeds limit {max}")]
    BuilderBidBlobLimitExceeded { got: usize, max: usize },
    /// Builder-payment window index is outside the accumulator.
    #[error("builder payment index out of range")]
    BuilderPaymentIndexOutOfRange,
    /// Builder pending-withdrawal queue is full.
    #[error("builder pending-withdrawal queue is full")]
    BuilderPendingWithdrawalsFull,
    /// Block-body deposits are not part of the parent-payload request path.
    #[error("block-body deposits are not accepted in this transition path")]
    DepositsNotAllowed,
    /// Computing a validator's withdrawable epoch overflowed the epoch type.
    #[error("withdrawable epoch overflow for validator {0}")]
    WithdrawableEpochOverflow(ValidatorIndex),
}
