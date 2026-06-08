//! Builder model constants.

use crate::constants::{MIN_SEED_LOOKAHEAD, SLOTS_PER_EPOCH};
use crate::primitives::BuilderIndex;

/// Length of the builder-payment accumulator window (two epochs of slot weights).
pub const BUILDER_PAYMENT_WINDOW_LEN: usize = 2 * SLOTS_PER_EPOCH;

/// Length of the rolling payload-timeliness committee assignment window.
pub const PTC_WINDOW_LEN: usize = (2 + MIN_SEED_LOOKAHEAD) * SLOTS_PER_EPOCH;

/// Bit-40 tag on a `ValidatorIndex` marking it as a [`BuilderIndex`]. Equals `2**40`.
///
/// Layout of the 64-bit slot:
///
/// ```text
///   bit  63                                  41 40                                   0
///        +-------------------------------------+--+------------------------------------+
///        |                unused               |F |              index                 |
///        +-------------------------------------+--+------------------------------------+
///         bits 41..64 are unused                ^   bits 0..40 carry the index value
///                                               |
///                                          bit 40: 1 = builder, 0 = validator
/// ```
///
/// Sentinels:
/// * [`BuilderIndex`] value `u64::MAX` is the self-build sentinel and is
///   rejected by [`BuilderIndex::to_validator_index`].
/// * Any [`BuilderIndex`] value `>= BUILDER_INDEX_FLAG` is out of range: the
///   raw index must fit in bits 0..40 so that setting bit 40 yields a valid
///   tagged [`crate::primitives::ValidatorIndex`].
pub const BUILDER_INDEX_FLAG: u64 = 1 << 40;

/// Sentinel [`BuilderIndex`] for proposer self-builds.
pub const BUILDER_INDEX_SELF_BUILD: BuilderIndex = BuilderIndex(u64::MAX);

/// Numerator of the builder payment quorum (`6/10` of per-slot balance).
pub const BUILDER_PAYMENT_THRESHOLD_NUMERATOR: u64 = 6;

/// Denominator of the builder payment quorum.
pub const BUILDER_PAYMENT_THRESHOLD_DENOMINATOR: u64 = 10;
