//! Error taxonomy for transition code.
//!
//! Most [`TransitionError`] variants mean the supplied state or block cannot be
//! accepted by the covered transition rules. [`IncompletePhase`] is
//! different: it means dispatch reached a spec phase that Moonglass has not
//! covered yet, so the error is a coverage boundary rather than a verdict that
//! the block is invalid.
//!
//! `PrimitivesError` covers operations on primitive protocol values and is
//! surfaced by transition helpers when primitive validation is part of a phase.

mod block;
mod fork_choice;
mod incomplete;
mod merkle;
mod operation;
mod primitive;
mod registry;
mod signature;
mod slot;

pub use block::*;
pub use fork_choice::*;
pub use incomplete::*;
pub use merkle::*;
pub use operation::*;
pub use primitive::*;
pub use registry::*;
pub use signature::*;
pub use slot::*;

use thiserror::Error;

use crate::primitives::Root;

/// Top-level error type returned by the consensus state transition.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[non_exhaustive]
pub enum TransitionError {
    /// Slot advancement failed.
    #[error(transparent)]
    Slot(#[from] SlotError),

    /// Block identity, header, deposit-vote, or proposer lookup failed.
    #[error(transparent)]
    Block(#[from] BlockError),

    /// Per-operation validation failed.
    #[error(transparent)]
    Operation(#[from] OperationError),

    /// Validator or builder registry lookup failed.
    #[error(transparent)]
    Registry(#[from] RegistryError),

    /// BLS signature verification failed.
    #[error(transparent)]
    Signature(#[from] SignatureError),

    /// SSZ merkleization failed while computing a root.
    #[error(transparent)]
    Merkle(#[from] MerkleError),

    /// Dispatch reached a spec phase outside Moonglass's current coverage.
    #[error(transparent)]
    Incomplete(#[from] IncompletePhase),

    /// Primitive protocol-value validation failed during transition processing.
    #[error(transparent)]
    Primitive(#[from] PrimitivesError),

    /// Block's `state_root` does not match the post-state's tree root.
    #[error("post-state root mismatch: got {got:?}, want {want:?}")]
    StateRootMismatch { got: Root, want: Root },
}
