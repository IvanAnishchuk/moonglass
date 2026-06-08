//! SSZ hash-tree-root failures, tagged by container.

use thiserror::Error;

/// Failures from `hash_tree_root` calls.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
#[non_exhaustive]
pub enum MerkleError {
    /// Failed while rooting `BeaconState`.
    #[error("failed to merkleize BeaconState")]
    BeaconState,
    /// Failed while rooting `BeaconBlock`.
    #[error("failed to merkleize BeaconBlock")]
    BeaconBlock,
    /// Failed while rooting `BeaconBlockHeader`.
    #[error("failed to merkleize BeaconBlockHeader")]
    BeaconBlockHeader,
    /// Failed while rooting `BeaconBlockBody`.
    #[error("failed to merkleize BeaconBlockBody")]
    BeaconBlockBody,
    /// Failed while rooting signing-version data.
    #[error("failed to merkleize ForkData")]
    ForkData,
    /// Failed while rooting `SigningData`.
    #[error("failed to merkleize SigningData")]
    SigningData,
    /// Failed while rooting `VoluntaryExit`.
    #[error("failed to merkleize VoluntaryExit")]
    VoluntaryExit,
    /// Failed while rooting `BLSToExecutionChange`.
    #[error("failed to merkleize BLSToExecutionChange")]
    BlsToExecutionChange,
    /// Failed while rooting `Epoch`.
    #[error("failed to merkleize Epoch")]
    Epoch,
    /// Failed while rooting `Attestation` data root.
    #[error("failed to merkleize Attestation")]
    Attestation,
    /// Failed while rooting `AttestationData`.
    #[error("failed to merkleize AttestationData")]
    AttestationData,
    /// Failed while rooting `IndexedAttestation`.
    #[error("failed to merkleize IndexedAttestation")]
    IndexedAttestation,
    /// Failed while rooting `DepositMessage`.
    #[error("failed to merkleize DepositMessage")]
    DepositMessage,
    /// Failed while rooting `ExecutionPayloadBid`.
    #[error("failed to merkleize ExecutionPayloadBid")]
    ExecutionPayloadBid,
    /// Failed while rooting `ExecutionPayloadEnvelope`.
    #[error("failed to merkleize ExecutionPayloadEnvelope")]
    ExecutionPayloadEnvelope,
    /// Failed while rooting `PayloadAttestationData`.
    #[error("failed to merkleize PayloadAttestationData")]
    PayloadAttestationData,
    /// Failed while rooting `ExecutionRequests`.
    #[error("failed to merkleize ExecutionRequests")]
    ExecutionRequests,
    /// Failed while rooting the `block_roots` historical ring buffer.
    #[error("failed to merkleize block_roots")]
    BlockRoots,
    /// Failed while rooting the `state_roots` historical ring buffer.
    #[error("failed to merkleize state_roots")]
    StateRoots,
}
