//! Fork-choice failure modes.
//!
//! One variant per fork-choice rule. Each variant names the rule that
//! rejected the message. The adapter classifies any `Err` from a step
//! marked invalid as a pass, and any `Err` from a valid step as a failure.

use thiserror::Error;

use crate::error::{MerkleError, TransitionError};
use crate::primitives::{Root, Slot, ValidatorIndex};

/// Failures raised by fork-choice rule evaluation.
#[derive(Debug, Clone, Error)]
#[non_exhaustive]
pub enum ForkChoiceError {
    /// Referenced block is not present in the store.
    #[error("unknown block {0:?}")]
    UnknownBlock(Root),

    /// Block is present but no unrealized justification entry was recorded.
    #[error("missing unrealized justification for block {0:?}")]
    MissingUnrealizedJustification(Root),

    /// Block is present but no timeliness flags were recorded for it.
    #[error("missing block timeliness for block {0:?}")]
    MissingBlockTimeliness(Root),

    /// Block's parent root is not present in the store.
    #[error("unknown parent {0:?}")]
    UnknownParent(Root),

    /// Block is at or before the finalized slot and cannot be added.
    #[error("block slot {block_slot:?} is at or before finalized slot {finalized_slot:?}")]
    BlockBeforeFinalizedSlot {
        /// Slot carried by the candidate block.
        block_slot: Slot,
        /// Slot at the store's finalized checkpoint.
        finalized_slot: Slot,
    },

    /// Block does not descend from the store's finalized checkpoint.
    #[error("block does not descend from finalized checkpoint")]
    BlockNotDescendedFromFinalized,

    /// Block's slot is ahead of the store's current slot.
    #[error("block slot {block_slot:?} is in the future of current slot {current_slot:?}")]
    BlockFromFuture {
        /// Slot carried by the candidate block.
        block_slot: Slot,
        /// Slot derived from the local store clock.
        current_slot: Slot,
    },

    /// Attestation refers to a slot earlier than the block slot it covers.
    #[error("attestation slot precedes the slot it covers")]
    AttestationTooEarly,

    /// Attestation target epoch is outside the store clock's current or previous epoch.
    #[error("attestation epoch is outside current/previous store epoch")]
    AttestationFromFutureEpoch,

    /// Attestation `index` is not in the allowed set.
    #[error("attestation index {0} is not 0 or 1")]
    AttestationIndexInvalid(u64),

    /// Attestation votes for a full-payload branch before the local store has
    /// recorded the block's payload envelope.
    #[error("attestation for full payload but payload envelope not recorded")]
    AttestationPayloadEnvelopeNotRecorded,

    /// LMD vote is inconsistent with the FFG target the attestation claims.
    #[error("attestation LMD vote inconsistent with target")]
    AttestationLmdFfgMismatch,

    /// Attester slashing fails validation against the current state.
    #[error("attester slashing is not valid against current state")]
    InvalidAttesterSlashing,

    /// Block extends a full-payload parent branch before the local store has
    /// recorded the parent block's payload envelope.
    #[error("parent block {0:?} extends a full-payload chain but payload envelope not recorded")]
    PayloadParentEnvelopeNotRecorded(Root),

    /// Payload envelope references a block not in the store.
    #[error("payload envelope for unknown block {0:?}")]
    PayloadEnvelopeForUnknownBlock(Root),

    /// Payload data was not available for the block, so the envelope is rejected.
    #[error("payload data unavailable for block {0:?}")]
    PayloadDataUnavailable(Root),

    /// The execution engine reported the payload invalid for the block.
    #[error("execution engine rejected payload for block {0:?}")]
    PayloadExecutionInvalid(Root),

    /// Payload attestation references a block not in the store.
    #[error("payload attestation for unknown block {0:?}")]
    PayloadAttestationForUnknownBlock(Root),

    /// Payload attestation slot is outside the active PTC window.
    #[error("payload attestation slot outside PTC window")]
    PayloadAttestationSlotOutOfWindow,

    /// Payload attestation comes from a validator not in the PTC.
    #[error("payload attestation validator not in PTC")]
    PayloadAttestationValidatorNotInPtc,

    /// Payload attestation slot does not match the store's current slot.
    #[error("payload attestation slot does not match current slot")]
    PayloadAttestationWrongSlot,

    /// `on_tick` was called with a time earlier than the stored time.
    #[error("tick went backwards from {from} to {to}")]
    TickWentBackwards {
        /// Previous store time.
        from: u64,
        /// New time supplied to `on_tick`.
        to: u64,
    },

    /// Anchor block's `state_root` does not match the anchor state's tree root.
    #[error("anchor block state_root mismatch: got {got:?}, want {want:?}")]
    AnchorStateRootMismatch {
        /// State root carried by the anchor block.
        got: Root,
        /// State root computed from the anchor state.
        want: Root,
    },

    /// Anchor time arithmetic overflowed.
    #[error("anchor time overflow")]
    AnchorTimeOverflow,

    /// Store has no entry for the justified-checkpoint state.
    #[error("justified-checkpoint state missing from store")]
    JustifiedStateMissing,

    /// Validator index exceeds the registry length.
    #[error("validator index {0:?} out of bounds")]
    ValidatorOutOfBounds(ValidatorIndex),

    /// `get_proposer_head` was called while proposer boost still targets the head.
    #[error("proposer boost still targets head block {0:?}")]
    ProposerBoostStillActive(Root),

    /// `should_build_on_full` was given a still-pending node, so the empty-or-full
    /// choice is not yet resolved.
    #[error("cannot decide build-on-full for still-pending node {0:?}")]
    BuildOnPendingNode(Root),

    /// `should_extend_payload` was asked about a block that is not from the slot
    /// just before the current one.
    #[error("block {0:?} is not from the previous slot")]
    NotPreviousSlot(Root),

    /// A fork-choice weight sum overflowed `u64`, which would corrupt head
    /// scoring.
    #[error("fork-choice weight overflow")]
    WeightOverflow,

    /// A payload-attestation vote names a committee position past the end of the
    /// vote vector.
    #[error("payload vote position {0} out of bounds")]
    PayloadVoteIndexOutOfBounds(usize),

    /// A `state_transition` step rejected the block.
    #[error(transparent)]
    Transition(#[from] TransitionError),

    /// Merkle proof or branch check failed during fork-choice evaluation.
    #[error(transparent)]
    Merkle(#[from] MerkleError),
}

/// A broken structural invariant of the fork-choice store, found by
/// [`check_invariants`](crate::fork_choice::Store::check_invariants).
///
/// These signal a bug in this crate's own bookkeeping rather than invalid input,
/// so they are kept separate from [`ForkChoiceError`].
#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[non_exhaustive]
pub enum StoreInvariant {
    /// A stored block has no recorded post-state.
    #[error("block {0:?} has no post-state")]
    MissingBlockState(Root),

    /// A stored block has no recorded timeliness flags.
    #[error("block {0:?} has no timeliness flags")]
    MissingTimeliness(Root),

    /// A block's payload-timeliness vote vector is not `PTC_SIZE` long.
    #[error("payload-timeliness votes for block {0:?} are not PTC_SIZE")]
    TimelinessVotesNotPtcSize(Root),

    /// A block's payload-data-availability vote vector is not `PTC_SIZE` long.
    #[error("payload-data-availability votes for block {0:?} are not PTC_SIZE")]
    DataAvailabilityVotesNotPtcSize(Root),

    /// A block does not sit in a later slot than its in-store parent.
    #[error("block {0:?} does not sit after its parent in slot order")]
    NonDecreasingParentSlot(Root),
}
