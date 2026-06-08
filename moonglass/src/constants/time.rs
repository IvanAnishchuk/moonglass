//! Slot, epoch, and lookahead durations.

use crate::primitives::Epoch;

/// Wall-clock duration of a slot, in milliseconds.
///
/// Consumed by execution-payload timestamp validation in
/// `BeaconState::expected_execution_payload_timestamp`, so its value is
/// load-bearing for block acceptance, not purely cosmetic.
#[cfg(feature = "mainnet")]
pub const SLOT_DURATION_MS: u64 = 12_000;

/// Expected execution-layer block interval, in seconds.
pub const SECONDS_PER_ETH1_BLOCK: u64 = 14;

/// Execution blocks to wait before observing deposit data.
#[cfg(feature = "mainnet")]
pub const ETH1_FOLLOW_DISTANCE: u64 = 2_048;

/// Number of slots in an epoch.
#[cfg(feature = "mainnet")]
pub const SLOTS_PER_EPOCH: usize = 32;

/// Length of the block- and state-root ring buffers in `BeaconState`.
#[cfg(feature = "mainnet")]
pub const SLOTS_PER_HISTORICAL_ROOT: usize = 8_192;

/// Minimum slot delay between attestation creation and inclusion.
pub const MIN_ATTESTATION_INCLUSION_DELAY: u64 = 1;

/// Epochs of RANDAO lookahead used when seeding shuffling.
pub const MIN_SEED_LOOKAHEAD: usize = 1;

/// Maximum epochs into the future shuffling may be queried for.
pub const MAX_SEED_LOOKAHEAD: u64 = 4;

/// Window over which deposit-chain votes are aggregated, in epochs.
#[cfg(feature = "mainnet")]
pub const EPOCHS_PER_ETH1_VOTING_PERIOD: usize = 64;

/// Epochs of consecutive missed attestations before inactivity penalties begin to accrue.
pub const MIN_EPOCHS_TO_INACTIVITY_PENALTY: u64 = 4;

/// Epochs each sync committee remains active before rotation.
#[cfg(feature = "mainnet")]
pub const EPOCHS_PER_SYNC_COMMITTEE_PERIOD: u64 = 256;

/// Epoch delay between exit and balance withdrawability for validators.
pub const MIN_VALIDATOR_WITHDRAWABILITY_DELAY: u64 = 256;

/// Minimum epochs before a validator may voluntary-exit.
#[cfg(feature = "mainnet")]
pub const SHARD_COMMITTEE_PERIOD: u64 = 256;

/// Epoch delay between exit and balance withdrawability for builders.
#[cfg(feature = "mainnet")]
pub const MIN_BUILDER_WITHDRAWABILITY_DELAY: u64 = 8_192;

/// Sentinel in `Validator.exit_epoch` / `withdrawable_epoch` meaning "no exit scheduled".
pub const FAR_FUTURE_EPOCH: Epoch = Epoch(u64::MAX);

// Minimal preset values used only for testing.

#[cfg(feature = "minimal")]
pub const SLOTS_PER_EPOCH: usize = 8;

#[cfg(feature = "minimal")]
pub const SLOTS_PER_HISTORICAL_ROOT: usize = 64;

#[cfg(feature = "minimal")]
pub const EPOCHS_PER_ETH1_VOTING_PERIOD: usize = 4;

#[cfg(feature = "minimal")]
pub const EPOCHS_PER_SYNC_COMMITTEE_PERIOD: u64 = 8;

#[cfg(feature = "minimal")]
pub const MIN_BUILDER_WITHDRAWABILITY_DELAY: u64 = 2;

#[cfg(feature = "minimal")]
pub const SHARD_COMMITTEE_PERIOD: u64 = 64;

#[cfg(feature = "minimal")]
pub const SLOT_DURATION_MS: u64 = 6_000;

#[cfg(feature = "minimal")]
pub const ETH1_FOLLOW_DISTANCE: u64 = 16;
