//! Fork-choice tuning constants.

/// Denominator for basis-point conversions.
pub const BASIS_POINTS: u64 = 10_000;

/// Proposer-boost weight as a fraction of one committee, in percent.
pub const PROPOSER_SCORE_BOOST: u64 = 40;

/// Reorg threshold below which the head is considered weak, in percent of one committee.
pub const REORG_HEAD_WEIGHT_THRESHOLD: u64 = 20;

/// Strength threshold above which the parent is considered strong, in percent of one committee.
pub const REORG_PARENT_WEIGHT_THRESHOLD: u64 = 160;

/// Maximum epochs since finalization for a reorg to be considered safe.
pub const REORG_MAX_EPOCHS_SINCE_FINALIZATION: u64 = 2;

/// Reorg cutoff inside a slot, in basis points of `SLOT_DURATION_MS`.
pub const PROPOSER_REORG_CUTOFF_BPS: u64 = 1_667;

/// Attestation deadline in basis points of `SLOT_DURATION_MS`.
pub const ATTESTATION_DUE_BPS: u64 = 3_333;

/// Aggregate attestation deadline in basis points of `SLOT_DURATION_MS`.
pub const AGGREGATE_DUE_BPS: u64 = 6_667;

/// Sync message deadline in basis points of `SLOT_DURATION_MS`.
pub const SYNC_MESSAGE_DUE_BPS: u64 = 3_333;

/// Sync contribution deadline in basis points of `SLOT_DURATION_MS`.
pub const CONTRIBUTION_DUE_BPS: u64 = 6_667;

/// Attestation deadline in basis points of `SLOT_DURATION_MS` under the current fork-choice timing.
pub const ATTESTATION_DUE_BPS_GLOAS: u64 = 2_500;

/// Aggregate attestation deadline in basis points of `SLOT_DURATION_MS` under the current fork-choice timing.
pub const AGGREGATE_DUE_BPS_GLOAS: u64 = 5_000;

/// Sync message deadline in basis points of `SLOT_DURATION_MS` under the current fork-choice timing.
pub const SYNC_MESSAGE_DUE_BPS_GLOAS: u64 = 2_500;

/// Sync contribution deadline in basis points of `SLOT_DURATION_MS` under the current fork-choice timing.
pub const CONTRIBUTION_DUE_BPS_GLOAS: u64 = 5_000;

/// Payload deadline in basis points of `SLOT_DURATION_MS`.
pub const PAYLOAD_DUE_BPS: u64 = 7_500;

/// Payload attestation deadline in basis points of `SLOT_DURATION_MS`.
pub const PAYLOAD_ATTESTATION_DUE_BPS: u64 = 7_500;

/// PTC vote count above which the payload is considered timely.
pub const PAYLOAD_TIMELY_THRESHOLD: u64 = 256;

/// PTC vote count above which the payload's blob data is considered available.
pub const DATA_AVAILABILITY_TIMELY_THRESHOLD: u64 = 256;

/// Index into a block's timeliness array for the attestation deadline check.
pub const ATTESTATION_TIMELINESS_INDEX: usize = 0;

/// Index into a block's timeliness array for the PTC deadline check.
pub const PTC_TIMELINESS_INDEX: usize = 1;

/// Number of timeliness deadlines recorded per block.
pub const NUM_BLOCK_TIMELINESS_DEADLINES: usize = 2;
