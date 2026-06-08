//! Slot-advancement failures raised by `process_slots` / `process_slot`.

use thiserror::Error;

use crate::primitives::Slot;

/// Failures from slot advancement.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
#[non_exhaustive]
pub enum SlotError {
    /// `target` is not strictly after `current`, which is invalid for transition entry.
    #[error("target slot {target} is not after current slot {current}")]
    NotAfter { current: Slot, target: Slot },
}
