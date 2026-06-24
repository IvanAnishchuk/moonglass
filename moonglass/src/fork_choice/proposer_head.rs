//! The proposer's re-org decision: [`Store::get_proposer_head`] and its guards.
//!
//! Normally a proposer builds its new block on top of the current head. But if
//! the head showed up suspiciously late and looks weakly supported, the honest
//! move can be to skip it and build on its parent instead, a "re-org". This file
//! holds that decision and the safety checks around it: re-org only a single
//! late, weak head whose parent is strongly supported, never across an epoch
//! boundary, never while we are running behind, and never in a way that would
//! set back finality.
//!
//! Parent support is measured against the [`Pending`](super::store::PayloadStatus::Pending)
//! parent node, so support spread across the parent's empty and full payload
//! variants is added together rather than counted against a single variant.

use crate::constants::{
    PROPOSER_REORG_CUTOFF_BPS, REORG_MAX_EPOCHS_SINCE_FINALIZATION, REORG_PARENT_WEIGHT_THRESHOLD,
    SLOTS_PER_EPOCH,
};
use crate::error::ForkChoiceError;
use crate::primitives::{Root, Slot};

use super::helpers::{calculate_committee_fraction, get_slot_component_duration_ms, is_next_slot};
use super::store::{ForkChoiceNode, Store};

/// How far into a slot a proposer may still re-org, in milliseconds.
pub fn get_proposer_reorg_cutoff_ms() -> u64 {
    get_slot_component_duration_ms(PROPOSER_REORG_CUTOFF_BPS)
}

/// Is `slot` *not* the first slot of its epoch?
///
/// Returns true for every slot except an epoch's first. Re-orging across an epoch
/// boundary is forbidden, since it could disturb finality, so
/// [`Store::get_proposer_head`] requires this to hold.
pub fn is_epoch_boundary(slot: Slot) -> bool {
    let slots_per_epoch = u64::try_from(SLOTS_PER_EPOCH).unwrap_or(u64::MAX);
    !slot.as_u64().is_multiple_of(slots_per_epoch)
}

impl Store {
    /// Did the head block miss its slot's attestation deadline?
    ///
    /// A late head is the first sign it may be worth re-orging. This just reads
    /// the timeliness flag recorded by [`Self::record_block_timeliness`] when the
    /// block was imported.
    pub fn is_head_late(&self, head_root: Root) -> Result<bool, ForkChoiceError> {
        let timeliness = self
            .block_timeliness
            .get(&head_root)
            .ok_or(ForkChoiceError::MissingBlockTimeliness(head_root))?;
        Ok(!timeliness.attestation_timely)
    }

    /// Is the parent of `root` strongly enough supported to re-org onto?
    ///
    /// A re-org only makes sense if the block we fall back to, the parent, is
    /// itself well supported. We score it against the parent's
    /// [`Pending`](super::store::PayloadStatus::Pending) node, so votes spread
    /// across its empty and full variants are added together, and compare that to
    /// a configured fraction of a committee's stake.
    pub fn is_parent_strong(&self, root: Root) -> Result<bool, ForkChoiceError> {
        let justified_state = self.get_justified_state()?;
        let parent_threshold =
            calculate_committee_fraction(justified_state, REORG_PARENT_WEIGHT_THRESHOLD);
        let parent_root = self
            .blocks
            .get(&root)
            .ok_or(ForkChoiceError::UnknownBlock(root))?
            .parent_root;
        let parent_node = ForkChoiceNode::pending(parent_root);
        let parent_weight = self.get_attestation_score(parent_node, justified_state)?;
        Ok(parent_weight > parent_threshold)
    }

    /// Would re-orging keep the chain's finality view intact?
    ///
    /// FFG is the part of consensus that justifies and finalizes checkpoints. A
    /// re-org must not throw away finality progress, so this checks that the head
    /// and the parent we would switch to imply the same justified checkpoint.
    pub fn is_ffg_competitive(
        &self,
        head_root: Root,
        parent_root: Root,
    ) -> Result<bool, ForkChoiceError> {
        let head = self
            .unrealized_justifications
            .get(&head_root)
            .ok_or(ForkChoiceError::MissingUnrealizedJustification(head_root))?;
        let parent = self
            .unrealized_justifications
            .get(&parent_root)
            .ok_or(ForkChoiceError::MissingUnrealizedJustification(parent_root))?;
        Ok(head == parent)
    }

    /// Is the chain finalizing recently enough to allow a re-org?
    ///
    /// If finality has stalled, proposers should stop re-orging and just extend
    /// the chain. This returns true only when the gap between now and the last
    /// finalized epoch is within the allowed bound.
    pub fn is_finalization_ok(&self, slot: Slot) -> bool {
        let epochs_since_finalization = slot
            .epoch()
            .as_u64()
            .saturating_sub(self.finalized_checkpoint.epoch.as_u64());
        epochs_since_finalization <= REORG_MAX_EPOCHS_SINCE_FINALIZATION
    }

    /// Is the proposer early enough in its slot to re-org?
    ///
    /// A re-org is only safe when we are proposing promptly, before the cutoff.
    /// This compares how far into the slot we are against
    /// [`get_proposer_reorg_cutoff_ms`].
    pub fn is_proposing_on_time(&self) -> bool {
        self.time_into_slot_ms() <= get_proposer_reorg_cutoff_ms()
    }

    /// Did the proposer of `root`'s slot publish more than one block in it?
    ///
    /// A proposer is meant to publish exactly one block per slot. Two or more is
    /// an equivocation, and grounds for a more aggressive re-org. This counts the
    /// stored blocks at that slot from that proposer and reports whether there is
    /// more than one.
    pub fn is_proposer_equivocation(&self, root: Root) -> Result<bool, ForkChoiceError> {
        let block = self
            .blocks
            .get(&root)
            .ok_or(ForkChoiceError::UnknownBlock(root))?;
        let proposer_index = block.proposer_index;
        let slot = block.slot;
        let matching = self
            .blocks
            .values()
            .filter(|b| b.proposer_index == proposer_index && b.slot == slot)
            .count();
        Ok(matching > 1)
    }

    /// Decide which node the proposer should build on, given the `head_node` it
    /// would otherwise extend and the `slot` it is proposing for.
    ///
    /// Returns `head_node` unchanged in the normal case, and swaps to the head's
    /// parent only when a re-org is justified. There are two routes to a re-org.
    /// The main one needs every guard to agree: the head is late and weak, its
    /// parent is strong, the move spans a single slot, we are proposing on time,
    /// and the epoch-boundary, FFG, and finalization checks all pass. The second
    /// is narrower: a weak head from the previous slot whose proposer equivocated.
    /// If neither route fires, the head is returned unchanged.
    ///
    /// # Errors
    ///
    /// Returns [`ForkChoiceError::ProposerBoostStillActive`] if proposer boost
    /// still points at the head, since the boost must have worn off before this
    /// decision, and propagates any missing block, state, or justification
    /// lookups.
    pub fn get_proposer_head(
        &self,
        head_node: ForkChoiceNode,
        slot: Slot,
    ) -> Result<ForkChoiceNode, ForkChoiceError> {
        let head_block = self
            .blocks
            .get(&head_node.root)
            .ok_or(ForkChoiceError::UnknownBlock(head_node.root))?;
        let parent_root = head_block.parent_root;
        let head_slot = head_block.slot;
        let parent_block = self
            .blocks
            .get(&parent_root)
            .ok_or(ForkChoiceError::UnknownParent(parent_root))?;
        let parent_slot = parent_block.slot;
        let parent_payload_status = self.get_parent_payload_status(head_block)?;
        let parent_node = ForkChoiceNode::new(parent_root, parent_payload_status);

        // Boost must have worn off before the proposer re-org decision is made.
        if self.proposer_boost_root == head_node.root {
            return Err(ForkChoiceError::ProposerBoostStillActive(head_node.root));
        }

        let head_late = self.is_head_late(head_node.root)?;
        let ffg_competitive = self.is_ffg_competitive(head_node.root, parent_root)?;
        let finalization_ok = self.is_finalization_ok(slot);
        let proposing_on_time = self.is_proposing_on_time();
        let single_slot_reorg =
            is_next_slot(parent_slot, head_slot) && is_next_slot(head_slot, slot);
        let current_time_ok = is_next_slot(head_slot, slot);
        let head_weak = self.is_head_weak(head_node.root)?;
        let parent_strong = self.is_parent_strong(head_node.root)?;
        let proposer_equivocation = self.is_proposer_equivocation(head_node.root)?;
        // `is_epoch_boundary` is true when `slot` is NOT an epoch's first slot, so
        // it doubles as "the re-org stays inside the epoch".
        let reorg_stays_inside_epoch = is_epoch_boundary(slot);

        // Group the primary-re-org guards by the reason each speaks to.
        let timing_allows_primary_reorg =
            reorg_stays_inside_epoch && proposing_on_time && single_slot_reorg;
        let finality_allows_primary_reorg = ffg_competitive && finalization_ok;
        let support_allows_primary_reorg = head_late && head_weak && parent_strong;

        let primary_reorg = timing_allows_primary_reorg
            && finality_allows_primary_reorg
            && support_allows_primary_reorg;
        let equivocation_reorg = head_weak && current_time_ok && proposer_equivocation;
        let reorg_to_parent = primary_reorg || equivocation_reorg;

        let proposer_head = if reorg_to_parent {
            parent_node
        } else {
            head_node
        };
        Ok(proposer_head)
    }
}
