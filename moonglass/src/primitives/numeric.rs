//! Numeric newtypes for protocol vocabulary: slots, epochs, indices, balances, flag bitmaps.

use crate::constants::{BUILDER_INDEX_FLAG, SLOTS_PER_EPOCH};
use crate::error::PrimitivesError;

/// Fixed time window in which one beacon block may be proposed.
///
/// Slots are grouped into [`Epoch`]s.
/// With 32 slots per epoch, slot 65 belongs to epoch 2.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct Slot(pub u64);

impl Slot {
    /// Construct from a raw slot number.
    #[inline]
    #[must_use]
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    /// Return the raw slot number.
    #[inline]
    #[must_use]
    pub const fn as_u64(self) -> u64 {
        self.0
    }

    /// Converts to `usize` for ring-buffer indexing.
    #[inline]
    #[must_use]
    // Safe: spec-bounded `u64` index fits in `usize` on supported 64-bit targets.
    #[allow(clippy::cast_possible_truncation)]
    pub const fn as_usize(self) -> usize {
        self.0 as usize
    }

    /// The epoch this slot belongs to.
    #[inline]
    #[must_use]
    pub const fn epoch(self) -> Epoch {
        Epoch(self.0 / SLOTS_PER_EPOCH as u64)
    }

    /// Saturating addition, clamped to `Slot(u64::MAX)`.
    #[inline]
    #[must_use]
    pub const fn saturating_add(self, rhs: u64) -> Self {
        Self(self.0.saturating_add(rhs))
    }

    /// Saturating subtraction, clamped to `Slot(0)`.
    #[inline]
    #[must_use]
    pub const fn saturating_sub(self, rhs: u64) -> Self {
        Self(self.0.saturating_sub(rhs))
    }
}

/// Group of slots used for validator rotation, rewards, penalties, and finality.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct Epoch(pub u64);

impl Epoch {
    /// Construct from a raw epoch number.
    #[inline]
    #[must_use]
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    /// Return the raw epoch number.
    #[inline]
    #[must_use]
    pub const fn as_u64(self) -> u64 {
        self.0
    }

    /// Converts to `usize` for collection indexing.
    #[inline]
    #[must_use]
    // Safe: spec-bounded `u64` index fits in `usize` on supported 64-bit targets.
    #[allow(clippy::cast_possible_truncation)]
    pub const fn as_usize(self) -> usize {
        self.0 as usize
    }

    /// First slot in this epoch, saturating for sentinel epochs.
    #[inline]
    #[must_use]
    pub const fn start_slot(self) -> Slot {
        Slot(self.0.saturating_mul(SLOTS_PER_EPOCH as u64))
    }

    /// Saturating addition, clamped to `Epoch(u64::MAX)`.
    #[inline]
    #[must_use]
    pub const fn saturating_add(self, rhs: u64) -> Self {
        Self(self.0.saturating_add(rhs))
    }

    /// Saturating subtraction, clamped to `Epoch(0)`.
    #[inline]
    #[must_use]
    pub const fn saturating_sub(self, rhs: u64) -> Self {
        Self(self.0.saturating_sub(rhs))
    }
}

/// Stable position of a validator in the consensus registry.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct ValidatorIndex(pub u64);

impl ValidatorIndex {
    /// Construct from a raw validator-registry position.
    #[inline]
    #[must_use]
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    /// Return the raw validator-registry position.
    #[inline]
    #[must_use]
    pub const fn as_u64(self) -> u64 {
        self.0
    }

    /// Converts to `usize` for collection indexing.
    #[inline]
    #[must_use]
    // Safe: spec-bounded `u64` index fits in `usize` on supported 64-bit targets.
    #[allow(clippy::cast_possible_truncation)]
    pub const fn as_usize(self) -> usize {
        self.0 as usize
    }

    /// True if this index encodes a [`BuilderIndex`].
    #[inline]
    #[must_use]
    pub const fn is_builder_index(self) -> bool {
        self.0 & BUILDER_INDEX_FLAG != 0
    }

    /// Decode this flagged validator index into a [`BuilderIndex`].
    #[inline]
    pub const fn to_builder_index(self) -> Result<BuilderIndex, PrimitivesError> {
        if !self.is_builder_index() {
            return Err(PrimitivesError::NotBuilderIndex);
        }
        let value = self.0 & !BUILDER_INDEX_FLAG;
        if value >= BUILDER_INDEX_FLAG {
            return Err(PrimitivesError::BuilderIndexOutOfRange);
        }
        Ok(BuilderIndex(value))
    }
}

/// Stable position of a builder in the builder registry.
///
/// Can be encoded into a [`ValidatorIndex`] by setting [`BUILDER_INDEX_FLAG`].
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct BuilderIndex(pub u64);

impl BuilderIndex {
    /// Construct from a raw builder-registry position.
    #[inline]
    #[must_use]
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    /// Return the raw builder-registry position.
    #[inline]
    #[must_use]
    pub const fn as_u64(self) -> u64 {
        self.0
    }

    /// Converts to `usize` for collection indexing.
    #[inline]
    #[must_use]
    // Safe: spec-bounded `u64` index fits in `usize` on supported 64-bit targets.
    #[allow(clippy::cast_possible_truncation)]
    pub const fn as_usize(self) -> usize {
        self.0 as usize
    }

    /// Encode this builder index as a flagged [`ValidatorIndex`].
    #[inline]
    pub const fn to_validator_index(self) -> Result<ValidatorIndex, PrimitivesError> {
        if self.0 == u64::MAX {
            return Err(PrimitivesError::SentinelBuilderIndex);
        }
        if self.0 >= BUILDER_INDEX_FLAG {
            return Err(PrimitivesError::BuilderIndexOutOfRange);
        }
        Ok(ValidatorIndex(self.0 | BUILDER_INDEX_FLAG))
    }
}

/// Committee index within a slot.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct CommitteeIndex(pub u64);

impl CommitteeIndex {
    /// Construct from a raw committee index.
    #[inline]
    #[must_use]
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    /// Return the raw committee index.
    #[inline]
    #[must_use]
    pub const fn as_u64(self) -> u64 {
        self.0
    }

    /// Converts to `usize` for collection indexing.
    #[inline]
    #[must_use]
    // Safe: spec-bounded `u64` index fits in `usize` on supported 64-bit targets.
    #[allow(clippy::cast_possible_truncation)]
    pub const fn as_usize(self) -> usize {
        self.0 as usize
    }
}

/// Sequence index of a withdrawal.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct WithdrawalIndex(pub u64);

impl WithdrawalIndex {
    /// Construct from a raw withdrawal sequence index.
    #[inline]
    #[must_use]
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    /// Return the raw withdrawal sequence index.
    #[inline]
    #[must_use]
    pub const fn as_u64(self) -> u64 {
        self.0
    }

    /// Converts to `usize` for collection indexing.
    #[inline]
    #[must_use]
    // Safe: spec-bounded `u64` index fits in `usize` on supported 64-bit targets.
    #[allow(clippy::cast_possible_truncation)]
    pub const fn as_usize(self) -> usize {
        self.0 as usize
    }
}

/// Consensus balance amount.
///
/// 1 ETH is `1_000_000_000` gwei.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct Gwei(pub u64);

impl Gwei {
    /// Zero gwei.
    pub const ZERO: Self = Self(0);

    /// Construct from a raw gwei amount.
    #[inline]
    #[must_use]
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    /// Return the raw gwei amount.
    #[inline]
    #[must_use]
    pub const fn as_u64(self) -> u64 {
        self.0
    }

    /// Saturating addition, clamped to `u64::MAX`.
    #[inline]
    #[must_use]
    pub const fn saturating_add(self, rhs: Self) -> Self {
        Self(self.0.saturating_add(rhs.0))
    }

    /// Saturating subtraction, clamped to `0`. Required by penalty math.
    #[inline]
    #[must_use]
    pub const fn saturating_sub(self, rhs: Self) -> Self {
        Self(self.0.saturating_sub(rhs.0))
    }

    /// Checked addition, returning `None` on overflow.
    #[inline]
    #[must_use]
    pub const fn checked_add(self, rhs: Self) -> Option<Self> {
        match self.0.checked_add(rhs.0) {
            Some(v) => Some(Self(v)),
            None => None,
        }
    }

    /// Checked subtraction, returning `None` on underflow.
    #[inline]
    #[must_use]
    pub const fn checked_sub(self, rhs: Self) -> Option<Self> {
        match self.0.checked_sub(rhs.0) {
            Some(v) => Some(Self(v)),
            None => None,
        }
    }
}

/// Per-validator participation-flag bitmap.
///
/// Bits are addressed by `TIMELY_{SOURCE,TARGET,HEAD}_FLAG_INDEX`.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct ParticipationFlags(pub u8);

impl ParticipationFlags {
    /// All flags unset.
    pub const NONE: Self = Self(0);

    /// Construct from raw flag bits.
    #[inline]
    #[must_use]
    pub const fn new(value: u8) -> Self {
        Self(value)
    }

    /// Return the raw flag bits.
    #[inline]
    #[must_use]
    pub const fn as_u8(self) -> u8 {
        self.0
    }

    /// True if the bit at `flag_index` is set.
    #[inline]
    pub const fn has_flag(self, flag_index: usize) -> Result<bool, PrimitivesError> {
        if flag_index >= 8 {
            return Err(PrimitivesError::FlagIndexOutOfRange(flag_index));
        }
        Ok(self.0 & (1u8 << flag_index) != 0)
    }

    /// Return a copy with the bit at `flag_index` set.
    #[inline]
    pub const fn with_flag(self, flag_index: usize) -> Result<Self, PrimitivesError> {
        if flag_index >= 8 {
            return Err(PrimitivesError::FlagIndexOutOfRange(flag_index));
        }
        Ok(Self(self.0 | (1u8 << flag_index)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::constants::BUILDER_INDEX_FLAG;
    use crate::error::PrimitivesError;

    #[test]
    fn slot_epoch_boundaries() {
        assert_eq!(Slot(0).epoch(), Epoch(0));
        assert_eq!(Slot((SLOTS_PER_EPOCH as u64) - 1).epoch(), Epoch(0));
        assert_eq!(Slot(SLOTS_PER_EPOCH as u64).epoch(), Epoch(1));
        assert_eq!(Slot(65).epoch(), Epoch(65 / SLOTS_PER_EPOCH as u64));
    }

    #[test]
    fn epoch_start_slot_round_trip() {
        let e = Epoch(7);
        assert_eq!(e.start_slot(), Slot(7 * SLOTS_PER_EPOCH as u64));
        assert_eq!(e.start_slot().epoch(), e);
    }

    #[test]
    fn epoch_start_slot_saturates_for_sentinel() {
        assert_eq!(Epoch(u64::MAX).start_slot(), Slot(u64::MAX));
    }

    #[test]
    fn slot_saturating_arithmetic() {
        assert_eq!(Slot(u64::MAX).saturating_add(1), Slot(u64::MAX));
        assert_eq!(Slot(0).saturating_sub(1), Slot(0));
    }

    #[test]
    fn gwei_checked_arithmetic_at_bounds() {
        assert_eq!(Gwei(1).checked_add(Gwei(u64::MAX)), None);
        assert_eq!(Gwei(0).checked_sub(Gwei(1)), None);
        assert_eq!(Gwei(2).checked_add(Gwei(3)), Some(Gwei(5)));
        assert_eq!(Gwei(5).checked_sub(Gwei(3)), Some(Gwei(2)));
    }

    #[test]
    fn gwei_saturating_arithmetic_at_bounds() {
        assert_eq!(Gwei(1).saturating_add(Gwei(u64::MAX)), Gwei(u64::MAX));
        assert_eq!(Gwei::ZERO.saturating_sub(Gwei(1)), Gwei::ZERO);
    }

    #[test]
    fn builder_index_round_trip_normal_value() {
        let b = BuilderIndex(42);
        let v = b.to_validator_index().expect("encode");
        assert!(v.is_builder_index());
        assert_eq!(v.0 & !BUILDER_INDEX_FLAG, 42);
        assert_eq!(v.to_builder_index().expect("decode"), b);
    }

    #[test]
    fn builder_index_to_validator_rejects_self_build_sentinel() {
        assert!(matches!(
            BuilderIndex(u64::MAX).to_validator_index(),
            Err(PrimitivesError::SentinelBuilderIndex),
        ));
    }

    #[test]
    fn builder_index_to_validator_rejects_overflowing_value() {
        assert!(matches!(
            BuilderIndex(BUILDER_INDEX_FLAG).to_validator_index(),
            Err(PrimitivesError::BuilderIndexOutOfRange),
        ));
    }

    #[test]
    fn validator_index_to_builder_rejects_unflagged() {
        assert!(matches!(
            ValidatorIndex(42).to_builder_index(),
            Err(PrimitivesError::NotBuilderIndex),
        ));
    }

    #[test]
    fn participation_flags_has_and_with_flag() {
        let flags = ParticipationFlags::NONE;
        assert_eq!(flags.has_flag(0), Ok(false));
        let flags = flags.with_flag(2).expect("set bit 2");
        assert_eq!(flags.has_flag(2), Ok(true));
        assert_eq!(flags.has_flag(0), Ok(false));
    }

    #[test]
    fn participation_flags_reject_out_of_range_index() {
        assert!(matches!(
            ParticipationFlags::NONE.has_flag(8),
            Err(PrimitivesError::FlagIndexOutOfRange(8)),
        ));
        assert!(matches!(
            ParticipationFlags::NONE.with_flag(8),
            Err(PrimitivesError::FlagIndexOutOfRange(8)),
        ));
    }
}
