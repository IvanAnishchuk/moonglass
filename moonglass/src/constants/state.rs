//! Bounded list and vector lengths used in `BeaconState`.

use crate::constants::{EPOCHS_PER_ETH1_VOTING_PERIOD, MIN_SEED_LOOKAHEAD, SLOTS_PER_EPOCH};

/// Length of the RANDAO-mix vector.
#[cfg(feature = "mainnet")]
pub const EPOCHS_PER_HISTORICAL_VECTOR: usize = 65_536;

/// Length of the `eth1_data_votes` list, one entry per slot in the deposit-voting period.
pub const ETH1_DATA_VOTES_LEN: usize = EPOCHS_PER_ETH1_VOTING_PERIOD * SLOTS_PER_EPOCH;

/// Length of the proposer-lookahead vector.
pub const PROPOSER_LOOKAHEAD_LEN: usize = (MIN_SEED_LOOKAHEAD + 1) * SLOTS_PER_EPOCH;

/// Length of the slashings ring buffer.
#[cfg(feature = "mainnet")]
pub const EPOCHS_PER_SLASHINGS_VECTOR: usize = 8_192;

/// Maximum length of the `historical_roots` summary list.
pub const HISTORICAL_ROOTS_LIMIT: usize = 16_777_216;

/// Maximum number of validators that may ever be registered.
pub const VALIDATOR_REGISTRY_LIMIT: usize = 1_099_511_627_776;

/// Maximum entries in the pending-deposit queue.
pub const PENDING_DEPOSITS_LIMIT: usize = 134_217_728;

/// Maximum entries in the pending-partial-withdrawals queue.
#[cfg(feature = "mainnet")]
pub const PENDING_PARTIAL_WITHDRAWALS_LIMIT: usize = 134_217_728;

/// Maximum entries in the pending-consolidations queue.
#[cfg(feature = "mainnet")]
pub const PENDING_CONSOLIDATIONS_LIMIT: usize = 262_144;

/// Maximum number of builders that may ever be registered.
pub const BUILDER_REGISTRY_LIMIT: usize = 1_099_511_627_776;

/// Maximum entries in the builder pending-withdrawals queue.
pub const BUILDER_PENDING_WITHDRAWALS_LIMIT: usize = 1_048_576;

/// Length of the rolling Casper finality-justification bitvector.
pub const JUSTIFICATION_BITS_LENGTH: usize = 4;

// Minimal preset values used only for testing.

#[cfg(feature = "minimal")]
pub const EPOCHS_PER_HISTORICAL_VECTOR: usize = 64;

#[cfg(feature = "minimal")]
pub const EPOCHS_PER_SLASHINGS_VECTOR: usize = 64;

#[cfg(feature = "minimal")]
pub const PENDING_PARTIAL_WITHDRAWALS_LIMIT: usize = 64;

#[cfg(feature = "minimal")]
pub const PENDING_CONSOLIDATIONS_LIMIT: usize = 64;
