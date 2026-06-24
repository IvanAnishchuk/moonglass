//! Fork-choice tuning constants.

/// Denominator for basis-point conversions.
pub const BASIS_POINTS: u64 = 10_000;

/// Proposer-boost weight as a fraction of one committee, in percent.
pub const PROPOSER_SCORE_BOOST: u64 = 40;

/// Reorg threshold below which the head is considered weak, in percent of one committee.
pub const REORG_HEAD_WEIGHT_THRESHOLD: u64 = 20;

/// Reorg threshold above which the parent is considered strong, in percent of one committee.
pub const REORG_PARENT_WEIGHT_THRESHOLD: u64 = 160;

/// Maximum epochs since finalization that still permit a proposer reorg.
pub const REORG_MAX_EPOCHS_SINCE_FINALIZATION: u64 = 2;

/// Proposer-reorg cutoff deadline in basis points of `SLOT_DURATION_MS`.
pub const PROPOSER_REORG_CUTOFF_BPS: u64 = 1_667;

/// Attestation deadline in basis points of `SLOT_DURATION_MS`.
pub const ATTESTATION_DUE_BPS_GLOAS: u64 = 2_500;

/// Payload attestation deadline in basis points of `SLOT_DURATION_MS`.
pub const PAYLOAD_ATTESTATION_DUE_BPS: u64 = 7_500;

/// PTC vote count above which the payload is considered timely.
///
/// Half the payload-timeliness committee, so it tracks [`PTC_SIZE`](super::PTC_SIZE)
/// across presets.
pub const PAYLOAD_TIMELY_THRESHOLD: u64 = super::PTC_SIZE as u64 / 2;

/// PTC vote count above which the payload's blob data is considered available.
///
/// Half the payload-timeliness committee, so it tracks [`PTC_SIZE`](super::PTC_SIZE)
/// across presets.
pub const DATA_AVAILABILITY_TIMELY_THRESHOLD: u64 = super::PTC_SIZE as u64 / 2;

/// Index into a block's timeliness array for the attestation deadline check.
pub const ATTESTATION_TIMELINESS_INDEX: usize = 0;

/// Index into a block's timeliness array for the PTC deadline check.
pub const PTC_TIMELINESS_INDEX: usize = 1;
