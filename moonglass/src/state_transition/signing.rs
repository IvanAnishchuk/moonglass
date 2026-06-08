//! Signing-root, signing-domain, and version-data root construction.
//!
//! Consensus signatures are not over a container root alone. The flow is:
//! container -> SSZ root -> `SigningData(object_root, domain)` -> signing root
//! -> BLS verification. The domain separates message kinds and signing versions
//! so a valid signature for one purpose cannot be replayed as another.

use crate::containers::{BeaconState, ForkData, SigningData};
use crate::error::{MerkleError, TransitionError};
use crate::primitives::{Domain, DomainType, Epoch, Root, Version};
use crate::state_transition::TreeRootExt;

pub use crate::crypto::bls::{aggregate_pubkeys, fast_aggregate_verify, verify_signature};

/// Tree root of the signing-version data for this chain.
pub fn compute_fork_data_root(
    version: Version,
    genesis_validators_root: Root,
) -> Result<Root, TransitionError> {
    ForkData {
        current_version: version,
        genesis_validators_root,
    }
    .tree_root(MerkleError::ForkData)
}

/// 32-byte signing domain: 4-byte `domain_type` concatenated with the first
/// 28 bytes of the signing-version root.
pub fn compute_domain(
    domain_type: DomainType,
    version: Version,
    genesis_validators_root: Root,
) -> Result<Domain, TransitionError> {
    let fork_data_root = compute_fork_data_root(version, genesis_validators_root)?;
    let mut bytes = [0u8; 32];
    bytes[..4].copy_from_slice(&domain_type.0);
    bytes[4..].copy_from_slice(&fork_data_root.0[..28]);
    Ok(Domain(bytes))
}

/// BLS signing root for `object` under `domain`.
///
/// This is the tree root of `SigningData(tree_root(object), domain)`, not the
/// object's tree root by itself.
pub fn compute_signing_root<T>(
    object: &mut T,
    domain: Domain,
    on_object_fail: MerkleError,
) -> Result<Root, TransitionError>
where
    T: ssz_rs::Merkleized,
{
    let object_root = object.tree_root(on_object_fail)?;
    SigningData {
        object_root,
        domain,
    }
    .tree_root(MerkleError::SigningData)
}

impl BeaconState {
    /// Signing version active at `epoch` per this state's version record.
    #[must_use]
    pub fn fork_version_at(&self, epoch: Epoch) -> Version {
        if epoch < self.fork.epoch {
            self.fork.previous_version
        } else {
            self.fork.current_version
        }
    }

    /// State-aware signing domain for `domain_type` at `epoch`.
    pub fn domain_for(
        &self,
        domain_type: DomainType,
        epoch: Epoch,
    ) -> Result<Domain, TransitionError> {
        compute_domain(
            domain_type,
            self.fork_version_at(epoch),
            self.genesis_validators_root,
        )
    }

    /// Signing root for `object` under this state's domain at `epoch`.
    pub fn signing_root_for<T>(
        &self,
        object: &mut T,
        domain_type: DomainType,
        epoch: Epoch,
        on_object_fail: MerkleError,
    ) -> Result<Root, TransitionError>
    where
        T: ssz_rs::Merkleized,
    {
        let domain = self.domain_for(domain_type, epoch)?;
        compute_signing_root(object, domain, on_object_fail)
    }

    /// Signing root when the object tree root is already available.
    pub fn signing_root_from_root(
        &self,
        object_root: Root,
        domain_type: DomainType,
        epoch: Epoch,
    ) -> Result<Root, TransitionError> {
        let domain = self.domain_for(domain_type, epoch)?;
        SigningData {
            object_root,
            domain,
        }
        .tree_root(MerkleError::SigningData)
    }
}
