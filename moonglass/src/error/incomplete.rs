//! Coverage boundaries outside Moonglass's consensus-state model.

use thiserror::Error;

/// External behavior reached by dispatch but outside current coverage.
///
/// This does not mean the supplied block or payload is invalid. It means the
/// verdict depends on behavior this crate does not model.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
#[non_exhaustive]
pub enum IncompletePhase {
    /// Full execution-engine payload validation is outside this crate.
    #[error("execution-engine payload validation is outside current coverage")]
    ExecutionEnginePayloadValidation,
    /// Fork-choice store and networking checks are outside this crate.
    #[error("fork-choice store behavior is outside current coverage")]
    ForkChoiceStore,
}
