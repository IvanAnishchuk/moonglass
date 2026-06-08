//! Indexed-access failures into the validator and builder registries.

use thiserror::Error;

/// Failures from registry lookups.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
#[non_exhaustive]
pub enum RegistryError {
    /// Validator index out of range of the validator registry.
    #[error("validator index {0} out of range")]
    ValidatorIndexOutOfRange(u64),

    /// Builder index out of range of the builder registry.
    #[error("builder index {0} out of range")]
    BuilderIndexOutOfRange(u64),

    /// A sync-committee member's pubkey is not present in the validator registry.
    #[error("sync committee member pubkey not in validator registry")]
    SyncCommitteeMemberNotFound,
}
