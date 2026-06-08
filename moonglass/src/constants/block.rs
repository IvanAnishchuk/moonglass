//! Per-block bounds: committee sizes, operation count caps, execution-payload
//! structural limits, and blob constants.
//!
//! These constants do two jobs: they cap what a valid block may contain, and
//! they set SSZ list/vector bounds for the containers that carry those fields.
//! They are protocol limits and serialization commitments, not predictions of
//! typical block or state sizes.

use crate::primitives::Epoch;

/// Maximum committees that may attest in a single slot.
#[cfg(feature = "mainnet")]
pub const MAX_COMMITTEES_PER_SLOT: usize = 64;

/// Target validator count per beacon committee.
#[cfg(feature = "mainnet")]
pub const TARGET_COMMITTEE_SIZE: u64 = 128;

/// Hard cap on committee size. Bounds attestation bitfield length.
pub const MAX_VALIDATORS_PER_COMMITTEE: usize = 2_048;

/// Maximum total attesters across all committees in a slot.
///
/// Bounds attestation bitfields and indexed-attestation lists.
pub const MAX_ATTESTING_INDICES: usize = MAX_VALIDATORS_PER_COMMITTEE * MAX_COMMITTEES_PER_SLOT;

/// Number of rounds of the swap-or-not shuffle.
#[cfg(feature = "mainnet")]
pub const SHUFFLE_ROUND_COUNT: u64 = 90;

/// Validators in each sync committee.
#[cfg(feature = "mainnet")]
pub const SYNC_COMMITTEE_SIZE: usize = 512;

/// Minimum participants required for a valid sync aggregate.
pub const MIN_SYNC_COMMITTEE_PARTICIPANTS: u64 = 1;

/// Validators in the payload-timeliness committee.
#[cfg(feature = "mainnet")]
pub const PTC_SIZE: usize = 512;

/// Maximum proposer slashings includable in a block.
pub const MAX_PROPOSER_SLASHINGS: usize = 16;

/// Maximum attester slashings includable in a block.
pub const MAX_ATTESTER_SLASHINGS: usize = 1;

/// Maximum attestations includable in a block.
pub const MAX_ATTESTATIONS: usize = 8;

/// Maximum deposits includable in a block.
pub const MAX_DEPOSITS: usize = 16;

/// Maximum voluntary exits includable in a block.
pub const MAX_VOLUNTARY_EXITS: usize = 16;

/// Maximum BLS-to-execution credential changes includable in a block.
pub const MAX_BLS_TO_EXECUTION_CHANGES: usize = 16;

/// Maximum payload attestations includable in a block.
pub const MAX_PAYLOAD_ATTESTATIONS: usize = 4;

/// Maximum deposit requests per execution payload.
pub const MAX_DEPOSIT_REQUESTS_PER_PAYLOAD: usize = 8_192;

/// Maximum withdrawal requests per execution payload.
pub const MAX_WITHDRAWAL_REQUESTS_PER_PAYLOAD: usize = 16;

/// Maximum consolidation requests per execution payload.
pub const MAX_CONSOLIDATION_REQUESTS_PER_PAYLOAD: usize = 2;

/// Maximum size, in bytes, of an opaque transaction blob.
pub const MAX_BYTES_PER_TRANSACTION: usize = 1_073_741_824;

/// Maximum transactions per execution payload.
pub const MAX_TRANSACTIONS_PER_PAYLOAD: usize = 1_048_576;

/// Fixed length, in bytes, of the logs-bloom field.
pub const BYTES_PER_LOGS_BLOOM: usize = 256;

/// Maximum length, in bytes, of the proposer's `extra_data` field.
pub const MAX_EXTRA_DATA_BYTES: usize = 32;

/// Maximum KZG blob commitments per block.
pub const MAX_BLOB_COMMITMENTS_PER_BLOCK: usize = 4_096;

/// Field elements in a single blob.
pub const FIELD_ELEMENTS_PER_BLOB: usize = 4_096;

/// Field elements per data-column cell.
pub const FIELD_ELEMENTS_PER_CELL: usize = 64;

/// Field elements per extended blob after Reed-Solomon expansion.
pub const FIELD_ELEMENTS_PER_EXT_BLOB: usize = 8_192;

/// Cells per extended blob.
pub const CELLS_PER_EXT_BLOB: usize = 128;

/// Number of data columns sampled across each blob.
pub const NUMBER_OF_COLUMNS: usize = 128;

/// Blob limit applied before the first [`BLOB_SCHEDULE`] entry activates.
pub const MAX_BLOBS_PER_BLOCK: u64 = 9;

/// Stepwise blob limits keyed by activation epoch.
///
/// Before the first entry activates, use the configured pre-schedule limit.
/// At and after an entry's epoch, the latest active entry gives the block limit.
#[cfg(feature = "mainnet")]
pub const BLOB_SCHEDULE: &[(Epoch, u64)] = &[(Epoch(412_672), 15), (Epoch(419_072), 21)];

// Minimal preset values used only for testing.

#[cfg(feature = "minimal")]
pub const MAX_COMMITTEES_PER_SLOT: usize = 4;

#[cfg(feature = "minimal")]
pub const TARGET_COMMITTEE_SIZE: u64 = 4;

#[cfg(feature = "minimal")]
pub const SHUFFLE_ROUND_COUNT: u64 = 10;

#[cfg(feature = "minimal")]
pub const SYNC_COMMITTEE_SIZE: usize = 32;

#[cfg(feature = "minimal")]
pub const PTC_SIZE: usize = 16;

#[cfg(feature = "minimal")]
pub const BLOB_SCHEDULE: &[(Epoch, u64)] = &[];
