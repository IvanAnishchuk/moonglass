//! Chain identity: genesis trigger parameters, signing-version schedule, and the
//! deposit-contract pointer.
//!
//! Version-schedule entries pair a domain-separation version tag with an
//! activation epoch. The tag is mixed into signing domains so signatures from
//! one signing-version window cannot be replayed under another.
//!
//! Entries are mainnet values for the modeled spec surface. A trailing
//! minimal block at the bottom of this file overrides the chain-identity
//! values used under `--features minimal` for reference-test runs.

use crate::constants::FAR_FUTURE_EPOCH;
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

/// Version stamped on the genesis state.
#[cfg(feature = "mainnet")]
pub const GENESIS_FORK_VERSION: Version = Version([0x00, 0x00, 0x00, 0x00]);

/// Domain-separation tag for this signing-version window.
#[cfg(feature = "mainnet")]
pub const ALTAIR_FORK_VERSION: Version = Version([0x01, 0x00, 0x00, 0x00]);
/// Mainnet activation epoch for this signing-version window.
#[cfg(feature = "mainnet")]
pub const ALTAIR_FORK_EPOCH: Epoch = Epoch(74_240);

/// Domain-separation tag for this signing-version window.
#[cfg(feature = "mainnet")]
pub const BELLATRIX_FORK_VERSION: Version = Version([0x02, 0x00, 0x00, 0x00]);
/// Mainnet activation epoch for this signing-version window.
#[cfg(feature = "mainnet")]
pub const BELLATRIX_FORK_EPOCH: Epoch = Epoch(144_896);

/// Domain-separation tag for this signing-version window.
#[cfg(feature = "mainnet")]
pub const CAPELLA_FORK_VERSION: Version = Version([0x03, 0x00, 0x00, 0x00]);
/// Mainnet activation epoch for this signing-version window.
#[cfg(feature = "mainnet")]
pub const CAPELLA_FORK_EPOCH: Epoch = Epoch(194_048);

/// Domain-separation tag for this signing-version window.
#[cfg(feature = "mainnet")]
pub const DENEB_FORK_VERSION: Version = Version([0x04, 0x00, 0x00, 0x00]);
/// Mainnet activation epoch for this signing-version window.
#[cfg(feature = "mainnet")]
pub const DENEB_FORK_EPOCH: Epoch = Epoch(269_568);

/// Domain-separation tag for this signing-version window.
#[cfg(feature = "mainnet")]
pub const ELECTRA_FORK_VERSION: Version = Version([0x05, 0x00, 0x00, 0x00]);
/// Mainnet activation epoch for this signing-version window.
#[cfg(feature = "mainnet")]
pub const ELECTRA_FORK_EPOCH: Epoch = Epoch(364_032);

/// Domain-separation tag for this signing-version window.
#[cfg(feature = "mainnet")]
pub const FULU_FORK_VERSION: Version = Version([0x06, 0x00, 0x00, 0x00]);
/// Mainnet activation epoch for this signing-version window.
#[cfg(feature = "mainnet")]
pub const FULU_FORK_EPOCH: Epoch = Epoch(411_392);

/// Domain-separation tag for this signing-version window.
#[cfg(feature = "mainnet")]
pub const GLOAS_FORK_VERSION: Version = Version([0x07, 0x00, 0x00, 0x00]);
/// Mainnet activation epoch for this signing-version window.
///
/// Set to the [`FAR_FUTURE_EPOCH`] sentinel while no mainnet activation epoch
/// is assigned.
#[cfg(feature = "mainnet")]
pub const GLOAS_FORK_EPOCH: Epoch = FAR_FUTURE_EPOCH;

/// Depth of the deposit-contract Merkle tree.
pub const DEPOSIT_CONTRACT_TREE_DEPTH: usize = 32;

/// Length of a deposit proof: Merkle path plus root chunk.
pub const DEPOSIT_PROOF_LEN: usize = DEPOSIT_CONTRACT_TREE_DEPTH + 1;

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

// Minimal preset values used only for testing.

#[cfg(feature = "minimal")]
pub const MIN_GENESIS_ACTIVE_VALIDATOR_COUNT: u64 = 64;

#[cfg(feature = "minimal")]
pub const MIN_GENESIS_TIME: u64 = 1_578_009_600;

#[cfg(feature = "minimal")]
pub const GENESIS_DELAY: u64 = 300;

#[cfg(feature = "minimal")]
pub const GENESIS_FORK_VERSION: Version = Version([0x00, 0x00, 0x00, 0x01]);

#[cfg(feature = "minimal")]
pub const ALTAIR_FORK_VERSION: Version = Version([0x01, 0x00, 0x00, 0x01]);
#[cfg(feature = "minimal")]
pub const ALTAIR_FORK_EPOCH: Epoch = FAR_FUTURE_EPOCH;

#[cfg(feature = "minimal")]
pub const BELLATRIX_FORK_VERSION: Version = Version([0x02, 0x00, 0x00, 0x01]);
#[cfg(feature = "minimal")]
pub const BELLATRIX_FORK_EPOCH: Epoch = FAR_FUTURE_EPOCH;

#[cfg(feature = "minimal")]
pub const CAPELLA_FORK_VERSION: Version = Version([0x03, 0x00, 0x00, 0x01]);
#[cfg(feature = "minimal")]
pub const CAPELLA_FORK_EPOCH: Epoch = FAR_FUTURE_EPOCH;

#[cfg(feature = "minimal")]
pub const DENEB_FORK_VERSION: Version = Version([0x04, 0x00, 0x00, 0x01]);
#[cfg(feature = "minimal")]
pub const DENEB_FORK_EPOCH: Epoch = FAR_FUTURE_EPOCH;

#[cfg(feature = "minimal")]
pub const ELECTRA_FORK_VERSION: Version = Version([0x05, 0x00, 0x00, 0x01]);
#[cfg(feature = "minimal")]
pub const ELECTRA_FORK_EPOCH: Epoch = FAR_FUTURE_EPOCH;

#[cfg(feature = "minimal")]
pub const FULU_FORK_VERSION: Version = Version([0x06, 0x00, 0x00, 0x01]);
#[cfg(feature = "minimal")]
pub const FULU_FORK_EPOCH: Epoch = FAR_FUTURE_EPOCH;

#[cfg(feature = "minimal")]
pub const GLOAS_FORK_VERSION: Version = Version([0x07, 0x00, 0x00, 0x01]);
#[cfg(feature = "minimal")]
pub const GLOAS_FORK_EPOCH: Epoch = FAR_FUTURE_EPOCH;

#[cfg(feature = "minimal")]
pub const DEPOSIT_CHAIN_ID: u64 = 5;

#[cfg(feature = "minimal")]
pub const DEPOSIT_NETWORK_ID: u64 = 5;

#[cfg(feature = "minimal")]
pub const DEPOSIT_CONTRACT_ADDRESS: ExecutionAddress = ExecutionAddress([
    0x12, 0x34, 0x56, 0x78, 0x90, 0x12, 0x34, 0x56, 0x78, 0x90, 0x12, 0x34, 0x56, 0x78, 0x90, 0x12,
    0x34, 0x56, 0x78, 0x90,
]);
