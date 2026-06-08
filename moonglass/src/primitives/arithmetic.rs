//! Arithmetic operator impls on the primitive newtypes.

use core::ops::{Add, AddAssign, Div, Mul, Rem, Sub, SubAssign};

use super::{BuilderIndex, CommitteeIndex, Epoch, Gwei, Slot, ValidatorIndex};

// `Slot` and `Epoch` add/subtract scalar `u64` counts (`Slot + Slot` is
// nonsensical because both are timestamps). `Rem<u64>` returns a bare
// `u64` because the result is a position-within-window, not a slot or
// epoch value. `Gwei` adds and subtracts other `Gwei` values.
//
// Overflow policy: the trait operators follow Rust's default integer
// arithmetic semantics. Transition code handling untrusted arithmetic should
// use the inherent `checked_*` / `saturating_*` methods explicitly.

impl Add<u64> for Slot {
    type Output = Self;

    #[inline]
    fn add(self, rhs: u64) -> Self {
        Self(self.0 + rhs)
    }
}

impl Sub<u64> for Slot {
    type Output = Self;

    #[inline]
    fn sub(self, rhs: u64) -> Self {
        Self(self.0 - rhs)
    }
}

impl Rem<u64> for Slot {
    type Output = u64;

    #[inline]
    fn rem(self, rhs: u64) -> u64 {
        self.0 % rhs
    }
}

impl Rem<usize> for Slot {
    type Output = usize;

    #[inline]
    // Safe: `self.0 % rhs` is strictly less than `rhs`, which already fits in `usize`.
    #[allow(clippy::cast_possible_truncation)]
    fn rem(self, rhs: usize) -> usize {
        (self.0 % rhs as u64) as usize
    }
}

impl AddAssign<u64> for Slot {
    #[inline]
    fn add_assign(&mut self, rhs: u64) {
        self.0 += rhs;
    }
}

impl SubAssign<u64> for Slot {
    #[inline]
    fn sub_assign(&mut self, rhs: u64) {
        self.0 -= rhs;
    }
}

impl Add<u64> for Epoch {
    type Output = Self;

    #[inline]
    fn add(self, rhs: u64) -> Self {
        Self(self.0 + rhs)
    }
}

impl Sub<u64> for Epoch {
    type Output = Self;

    #[inline]
    fn sub(self, rhs: u64) -> Self {
        Self(self.0 - rhs)
    }
}

impl Rem<u64> for Epoch {
    type Output = u64;

    #[inline]
    fn rem(self, rhs: u64) -> u64 {
        self.0 % rhs
    }
}

impl Rem<usize> for Epoch {
    type Output = usize;

    #[inline]
    // Safe: `self.0 % rhs` is strictly less than `rhs`, which already fits in `usize`.
    #[allow(clippy::cast_possible_truncation)]
    fn rem(self, rhs: usize) -> usize {
        (self.0 % rhs as u64) as usize
    }
}

impl AddAssign<u64> for Epoch {
    #[inline]
    fn add_assign(&mut self, rhs: u64) {
        self.0 += rhs;
    }
}

impl SubAssign<u64> for Epoch {
    #[inline]
    fn sub_assign(&mut self, rhs: u64) {
        self.0 -= rhs;
    }
}

impl Add for Gwei {
    type Output = Self;

    #[inline]
    fn add(self, rhs: Self) -> Self {
        Self(self.0 + rhs.0)
    }
}

impl Sub for Gwei {
    type Output = Self;

    #[inline]
    fn sub(self, rhs: Self) -> Self {
        Self(self.0 - rhs.0)
    }
}

impl AddAssign for Gwei {
    #[inline]
    fn add_assign(&mut self, rhs: Self) {
        self.0 += rhs.0;
    }
}

impl SubAssign for Gwei {
    #[inline]
    fn sub_assign(&mut self, rhs: Self) {
        self.0 -= rhs.0;
    }
}

impl Mul<u64> for Gwei {
    type Output = Self;

    #[inline]
    fn mul(self, rhs: u64) -> Self {
        Self(self.0 * rhs)
    }
}

impl Div<u64> for Gwei {
    type Output = Self;

    #[inline]
    fn div(self, rhs: u64) -> Self {
        Self(self.0 / rhs)
    }
}

impl core::fmt::Display for Slot {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        self.0.fmt(f)
    }
}

impl core::fmt::Display for Epoch {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        self.0.fmt(f)
    }
}

impl core::fmt::Display for ValidatorIndex {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        self.0.fmt(f)
    }
}

impl core::fmt::Display for BuilderIndex {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        self.0.fmt(f)
    }
}

impl core::fmt::Display for CommitteeIndex {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        self.0.fmt(f)
    }
}
