//! BLS signature-verification failures, tagged by call site.
//!
//! A variant can mean malformed key bytes, malformed signature bytes, or a
//! well-formed signature that did not verify for the expected signing root.

use crate::primitives::{BuilderIndex, ValidatorIndex};
use thiserror::Error;

/// Failures from BLS signature verification.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
#[non_exhaustive]
pub enum SignatureError {
    /// Outer block-proposer signature on `SignedBeaconBlock` failed.
    #[error("invalid block proposer signature for validator {0}")]
    BlockProposer(ValidatorIndex),

    /// Proposer's `randao_reveal` failed to verify.
    #[error("invalid randao reveal")]
    RandaoReveal,

    /// `SignedVoluntaryExit` signature did not verify.
    #[error("invalid voluntary exit signature for validator {0}")]
    VoluntaryExit(ValidatorIndex),

    /// `SignedBLSToExecutionChange` signature did not verify.
    #[error("invalid BLS-to-execution change signature for validator {0}")]
    BlsToExecutionChange(ValidatorIndex),

    /// Aggregate sync-committee signature did not verify.
    #[error("invalid sync aggregate signature")]
    SyncAggregate,

    /// Sync aggregate with no participants must carry the G2 point-at-infinity signature.
    #[error("empty sync aggregate must carry the BLS infinity signature")]
    SyncInfinitySignatureRequired,

    /// Aggregate attestation signature did not verify.
    #[error("invalid attestation aggregate signature")]
    Attestation,

    /// Proposer-slashing first header signature did not verify.
    #[error("invalid proposer-slashing header-one signature")]
    ProposerSlashingHeaderOne,

    /// Proposer-slashing second header signature did not verify.
    #[error("invalid proposer-slashing header-two signature")]
    ProposerSlashingHeaderTwo,

    /// Attester-slashing first attestation aggregate signature did not verify.
    #[error("invalid attester-slashing attestation-one signature")]
    AttesterSlashingAttestationOne,

    /// Attester-slashing second attestation aggregate signature did not verify.
    #[error("invalid attester-slashing attestation-two signature")]
    AttesterSlashingAttestationTwo,

    /// Payload-timeliness committee aggregate signature did not verify.
    #[error("invalid payload-attestation aggregate signature")]
    PayloadAttestation,

    /// Builder signature on a payload bid did not verify.
    #[error("invalid execution payload bid signature for builder {0}")]
    ExecutionPayloadBid(BuilderIndex),

    /// Builder signature on a delivered payload envelope did not verify.
    #[error("invalid execution payload envelope signature for builder {0}")]
    ExecutionPayloadEnvelope(BuilderIndex),

    /// Deposit signature did not verify (deposit is dropped, not invalidated).
    #[error("invalid deposit signature")]
    Deposit,

    /// Public-key aggregation for a sync-committee derivation failed.
    #[error("failed to aggregate sync committee public keys")]
    AggregatePubkey,

    /// Public-key aggregation was called with an empty input set.
    #[error("cannot aggregate an empty set of public keys")]
    EmptyAggregatePubkeySet,
}
