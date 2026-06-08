//! Fork-choice failure modes.
//!
//! One variant per fork-choice rule. Each variant names the rule that
//! rejected the message; the adapter classifies any `Err` from a step
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

    /// Block's parent root is not present in the store.
    #[error("unknown parent {0:?}")]
    UnknownParent(Root),

    /// Block is at or before the finalized slot and cannot be added.
    #[error("block slot {block_slot:?} is at or before finalized slot {finalized_slot:?}")]
    BlockBeforeFinalizedSlot {
        block_slot: Slot,
        finalized_slot: Slot,
    },

    /// Block does not descend from the store's finalized checkpoint.
    #[error("block does not descend from finalized checkpoint")]
    BlockNotDescendedFromFinalized,

    /// Block's slot is ahead of the store's current slot.
    #[error("block slot {block_slot:?} is in the future of current slot {current_slot:?}")]
    BlockFromFuture {
        block_slot: Slot,
        current_slot: Slot,
    },

    /// Attestation refers to a slot earlier than the block slot it covers.
    #[error("attestation slot precedes the slot it covers")]
    AttestationTooEarly,

    /// Attestation target checkpoint is not a descendant of the justified root.
    #[error("attestation target is not a descendant of justified")]
    AttestationTargetNotDescendant,

    /// Attestation target epoch is later than the store's current epoch.
    #[error("attestation epoch is in the future")]
    AttestationFromFutureEpoch,

    /// Attestation `index` is not in the allowed set.
    #[error("attestation index {0} is not 0 or 1")]
    AttestationIndexInvalid(u64),

    /// Attestation votes for a full payload but the payload is not verified yet.
    #[error("attestation for full payload but payload not verified")]
    AttestationPayloadNotVerified,

    /// LMD vote is inconsistent with the FFG target the attestation claims.
    #[error("attestation LMD vote inconsistent with target")]
    AttestationLmdFfgMismatch,

    /// Block-embedded attestation references its containing block.
    #[error("on-block attestation references the block itself")]
    AttestationFromBlockSelfReference,

    /// Attester slashing fails validation against the current state.
    #[error("attester slashing is not valid against current state")]
    InvalidAttesterSlashing,

    /// Payload references a parent block that is not in the store.
    #[error("unknown payload parent block {0:?}")]
    UnknownPayloadParent(Root),

    /// Payload extends a full chain whose parent has not been payload-verified.
    #[error("parent block {0:?} extends a full chain but its payload is not verified")]
    PayloadParentNotVerified(Root),

    /// Payload envelope references a block not in the store.
    #[error("payload envelope for unknown block {0:?}")]
    PayloadEnvelopeForUnknownBlock(Root),

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

    /// Validator has been recorded as equivocating in the store.
    #[error("equivocating validator {0:?}")]
    EquivocatingValidator(ValidatorIndex),

    /// `on_tick` was called with a time earlier than the stored time.
    #[error("tick went backwards from {from} to {to}")]
    TickWentBackwards { from: u64, to: u64 },

    /// Anchor block's `state_root` does not match the anchor state's tree root.
    #[error("anchor block state_root mismatch: got {got:?}, want {want:?}")]
    AnchorStateRootMismatch { got: Root, want: Root },

    /// Anchor time arithmetic overflowed.
    #[error("anchor time overflow")]
    AnchorTimeOverflow,

    /// Store has no entry for the justified-checkpoint state.
    #[error("justified-checkpoint state missing from store")]
    JustifiedStateMissing,

    /// Validator index exceeds the registry length.
    #[error("validator index {0:?} out of bounds")]
    ValidatorOutOfBounds(ValidatorIndex),

    /// A `state_transition` step rejected the block.
    #[error(transparent)]
    Transition(#[from] TransitionError),

    /// Merkle proof or branch check failed during fork-choice evaluation.
    #[error(transparent)]
    Merkle(#[from] MerkleError),
}
