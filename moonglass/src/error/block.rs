//! Non-signature failures raised by `process_block` and its immediate sub-phases.

use thiserror::Error;

use crate::primitives::{CommitteeIndex, Hash32, Root, Slot, ValidatorIndex};

/// Failures from per-block processing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
#[non_exhaustive]
pub enum BlockError {
    /// Block's slot is not strictly greater than the parent header's slot.
    #[error("block slot {block} is not after parent slot {parent}")]
    SlotNotAfterParent { block: Slot, parent: Slot },

    /// Block's slot does not match the state's current slot.
    #[error("block slot {block} does not match state slot {state}")]
    BlockSlotMismatch { block: Slot, state: Slot },

    /// Block's parent root does not match the state's `latest_block_header` root.
    #[error("block parent root mismatch: got {got:?}, want {want:?}")]
    ParentRootMismatch { got: Root, want: Root },

    /// Block's `proposer_index` does not match the lookahead's expected proposer.
    #[error("block proposer index {got} does not match expected {want}")]
    ProposerIndexMismatch {
        got: ValidatorIndex,
        want: ValidatorIndex,
    },

    /// Block's proposer is already slashed.
    #[error("proposer {0} is slashed")]
    ProposerSlashed(ValidatorIndex),

    /// `proposer_lookahead` does not cover the requested slot.
    #[error("proposer lookahead does not cover slot {0}")]
    ProposerLookaheadOutOfRange(Slot),

    /// `eth1_data_votes` is already at capacity. Should only happen if an
    /// epoch reset was skipped, which is a state-machine bug.
    #[error("eth1_data_votes at capacity")]
    Eth1VotesFull,

    /// Committee index exceeds the per-slot committee count.
    #[error("committee index {0} out of range")]
    CommitteeIndexOutOfRange(CommitteeIndex),

    /// Active validator set is empty when a sampler needed it.
    #[error("active validator set is empty")]
    EmptyActiveValidatorSet,

    /// The parent payload's execution-requests root does not match what was delivered.
    #[error("parent payload requests root mismatch")]
    ParentPayloadRequestsMismatch,

    /// Payload envelope's builder index does not match the accepted bid.
    #[error("execution payload envelope builder mismatch")]
    EnvelopeBuilderMismatch,

    /// Payload envelope's `prev_randao` does not match the accepted bid.
    #[error("execution payload envelope randao mismatch")]
    EnvelopeRandaoMismatch,

    /// Payload envelope's gas limit does not match the accepted bid.
    #[error("execution payload envelope gas-limit mismatch")]
    EnvelopeGasLimitMismatch,

    /// Payload envelope's block hash does not match the accepted bid.
    #[error("execution payload envelope block-hash mismatch")]
    EnvelopePayloadHashMismatch,

    /// Payload envelope's execution requests do not match the accepted bid root.
    #[error("execution payload envelope requests root mismatch")]
    EnvelopeRequestsRootMismatch,

    /// Payload envelope's slot tag does not match the state slot.
    #[error("execution payload envelope slot mismatch")]
    EnvelopeSlotMismatch,

    /// Payload envelope's parent execution hash does not match the state.
    #[error("execution payload parent hash mismatch: got {got:?}, want {want:?}")]
    EnvelopeParentHashMismatch { got: Hash32, want: Hash32 },

    /// Payload envelope's timestamp does not match the state's slot time.
    #[error("execution payload timestamp mismatch: got {got}, want {want}")]
    EnvelopeTimestampMismatch { got: u64, want: u64 },

    /// Expected payload timestamp could not be represented as `u64`.
    #[error("execution payload timestamp overflow")]
    EnvelopeTimestampOverflow,

    /// Payload-bid envelope's beacon block root does not match the state header.
    #[error("execution payload envelope block root mismatch")]
    EnvelopeBlockRootMismatch,

    /// Payload-bid envelope's parent block root does not match the state header.
    #[error("execution payload envelope parent root mismatch")]
    EnvelopeParentMismatch,

    /// Withdrawals root carried by the bid does not match the expected list.
    #[error("withdrawals root mismatch")]
    WithdrawalsRootMismatch,
}
