//! Failures from primitive protocol-value operations.

use thiserror::Error;

/// Failures from primitive-value operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
#[non_exhaustive]
pub enum PrimitivesError {
    /// `ValidatorIndex::to_builder_index` called on an index without `BUILDER_INDEX_FLAG` set.
    #[error("validator index does not encode a builder index")]
    NotBuilderIndex,

    /// Participation-flag bit-index out of range (must be `< 8`).
    #[error("participation flag index {0} out of range (must be < 8)")]
    FlagIndexOutOfRange(usize),

    /// `BuilderIndex::to_validator_index` called on the `BUILDER_INDEX_SELF_BUILD` sentinel.
    #[error("builder-index sentinel cannot be encoded as a validator index")]
    SentinelBuilderIndex,

    /// Builder index cannot be encoded without losing information.
    #[error("builder index out of encodable range")]
    BuilderIndexOutOfRange,
}
