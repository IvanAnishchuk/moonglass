//! Chain identity: genesis trigger parameters, fork versions, and the
//! deposit-contract pointer.
//!
//! Fork versions are mixed into signing domains before a validator signs. This
//! module keeps the Ethereum consensus-spec names for constants consumed by the
//! implemented paths, without exporting every unused historical config entry.

use crate::primitives::{Epoch, ExecutionAddress, Slot, Version};

/// Minimum active validator count required to trigger genesis.
#[cfg(feature = "mainnet")]
pub const MIN_GENESIS_ACTIVE_VALIDATOR_COUNT: u64 = 16_384;

/// Earliest Unix timestamp at which genesis may occur.
#[cfg(feature = "mainnet")]
pub const MIN_GENESIS_TIME: u64 = 1_606_824_000;

/// Delay, in seconds, between the genesis trigger and the genesis slot.
#[cfg(feature = "mainnet")]
pub const GENESIS_DELAY: u64 = 604_800;

/// First epoch number.
pub const GENESIS_EPOCH: Epoch = Epoch(0);

/// First slot number.
pub const GENESIS_SLOT: Slot = Slot(0);

/// Fork version stamped on the genesis state.
#[cfg(feature = "mainnet")]
pub const GENESIS_FORK_VERSION: Version = Version([0x00, 0x00, 0x00, 0x00]);

/// Fork version used by voluntary-exit signatures.
///
/// The consensus rules intentionally pin this domain to the same version used
/// when BLS-to-execution credential changes were introduced, so old voluntary
/// exits remain verifiable after later network upgrades.
#[cfg(feature = "mainnet")]
pub const CAPELLA_FORK_VERSION: Version = Version([0x03, 0x00, 0x00, 0x00]);

/// Depth of the deposit-contract Merkle tree.
pub const DEPOSIT_CONTRACT_TREE_DEPTH: usize = 32;

/// Length of a deposit proof: Merkle path plus root chunk.
pub const DEPOSIT_PROOF_LEN: usize = DEPOSIT_CONTRACT_TREE_DEPTH + 1;

/// Sentinel for `deposit_requests_start_index` meaning no start index has been
/// assigned yet, because no execution-layer deposit request has been processed.
pub const UNSET_DEPOSIT_REQUESTS_START_INDEX: u64 = u64::MAX;

/// Chain ID of the network the deposit contract lives on.
#[cfg(feature = "mainnet")]
pub const DEPOSIT_CHAIN_ID: u64 = 1;

/// Network ID of the network the deposit contract lives on.
#[cfg(feature = "mainnet")]
pub const DEPOSIT_NETWORK_ID: u64 = 1;

/// Execution-layer address of the canonical deposit contract.
#[cfg(feature = "mainnet")]
pub const DEPOSIT_CONTRACT_ADDRESS: ExecutionAddress = ExecutionAddress([
    0x00, 0x00, 0x00, 0x00, 0x21, 0x9a, 0xb5, 0x40, 0x35, 0x6c, 0xbb, 0x83, 0x9c, 0xbe, 0x05, 0x30,
    0x3d, 0x77, 0x05, 0xfa,
]);

// Minimal-preset values from the consensus-spec minimal configuration.

/// Minimal-preset active validator count required to trigger genesis.
#[cfg(feature = "minimal")]
pub const MIN_GENESIS_ACTIVE_VALIDATOR_COUNT: u64 = 64;

/// Minimal-preset earliest Unix timestamp at which genesis may occur.
#[cfg(feature = "minimal")]
pub const MIN_GENESIS_TIME: u64 = 1_578_009_600;

/// Minimal-preset delay, in seconds, between genesis trigger and genesis slot.
#[cfg(feature = "minimal")]
pub const GENESIS_DELAY: u64 = 300;

/// Minimal-preset fork version stamped on the genesis state.
#[cfg(feature = "minimal")]
pub const GENESIS_FORK_VERSION: Version = Version([0x00, 0x00, 0x00, 0x01]);

/// Minimal-preset fork version used by voluntary-exit signatures.
#[cfg(feature = "minimal")]
pub const CAPELLA_FORK_VERSION: Version = Version([0x03, 0x00, 0x00, 0x01]);

/// Minimal-preset deposit-contract chain ID.
#[cfg(feature = "minimal")]
pub const DEPOSIT_CHAIN_ID: u64 = 5;

/// Minimal-preset deposit-contract network ID.
#[cfg(feature = "minimal")]
pub const DEPOSIT_NETWORK_ID: u64 = 5;

/// Minimal-preset execution-layer address of the deposit contract.
#[cfg(feature = "minimal")]
pub const DEPOSIT_CONTRACT_ADDRESS: ExecutionAddress = ExecutionAddress([
    0x12, 0x34, 0x56, 0x78, 0x90, 0x12, 0x34, 0x56, 0x78, 0x90, 0x12, 0x34, 0x56, 0x78, 0x90, 0x12,
    0x34, 0x56, 0x78, 0x90,
]);
