//! Reward and penalty quotients, inactivity scoring, and participation
//! flags + weights.
//!
//! These parameters turn validator balance, participation flags, and slashing
//! history into reward and penalty amounts during epoch and sync processing.

/// Base-reward divisor that controls per-validator issuance.
pub const BASE_REWARD_FACTOR: u64 = 64;

/// Proposer share of attestation rewards (1/N of the attester reward).
pub const PROPOSER_REWARD_QUOTIENT: u64 = 8;

/// Whistleblower share of a slashed validator's effective balance.
pub const WHISTLEBLOWER_REWARD_QUOTIENT: u64 = 4_096;

/// Inactivity-leak quotient.
pub const INACTIVITY_PENALTY_QUOTIENT: u64 = 16_777_216;

/// Minimum divisor used when computing per-validator slashing penalty.
pub const MIN_SLASHING_PENALTY_QUOTIENT: u64 = 4_096;

/// Multiplier on aggregated slashings when computing per-slashing penalty.
pub const PROPORTIONAL_SLASHING_MULTIPLIER: u64 = 3;

/// Per-epoch increment added to an inactive validator's inactivity score.
pub const INACTIVITY_SCORE_BIAS: u64 = 4;

/// Per-epoch decrement applied to an active validator's inactivity score.
pub const INACTIVITY_SCORE_RECOVERY_RATE: u64 = 16;

/// Bit index of the "timely source vote" participation flag.
pub const TIMELY_SOURCE_FLAG_INDEX: usize = 0;

/// Bit index of the "timely target vote" participation flag.
pub const TIMELY_TARGET_FLAG_INDEX: usize = 1;

/// Bit index of the "timely head vote" participation flag.
pub const TIMELY_HEAD_FLAG_INDEX: usize = 2;

/// Reward weight for a timely source vote.
pub const TIMELY_SOURCE_WEIGHT: u64 = 14;

/// Reward weight for a timely target vote.
pub const TIMELY_TARGET_WEIGHT: u64 = 26;

/// Reward weight for a timely head vote.
pub const TIMELY_HEAD_WEIGHT: u64 = 14;

/// Reward weight for sync-committee participation.
pub const SYNC_REWARD_WEIGHT: u64 = 2;

/// Reward weight for the block proposer.
pub const PROPOSER_WEIGHT: u64 = 8;

/// Denominator the participation weights are expressed against.
pub const WEIGHT_DENOMINATOR: u64 = 64;

/// Participation weights indexed by `TIMELY_*_FLAG_INDEX`.
pub const PARTICIPATION_FLAG_WEIGHTS: [u64; 3] = [
    TIMELY_SOURCE_WEIGHT,
    TIMELY_TARGET_WEIGHT,
    TIMELY_HEAD_WEIGHT,
];
