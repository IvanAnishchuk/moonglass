//! Recording how on-time a [block](crate::glossary#beacon-block) was, and
//! choosing the proposer-boost target.
//!
//! When a block arrives, this node notes whether it showed up before the slot's
//! deadlines. That "timeliness" is local evidence, not part of the chain state.
//! Proposer boost then uses it: the first on-time block of a
//! [slot](crate::glossary#slot), as long as it belongs to the same duty window
//! as the current [head](crate::glossary#head), receives the temporary
//! boost that makes it harder to re-org away.

use crate::error::ForkChoiceError;
use crate::primitives::Root;

use super::helpers::{get_attestation_due_ms, get_payload_attestation_due_ms};
use super::store::{BlockTimeliness, Store};

impl Store {
    /// Note whether a freshly imported block beat its slot's deadlines.
    ///
    /// Records a [`BlockTimeliness`] for the block: whether it was seen before the
    /// attestation deadline, and before the later payload-attestation deadline. A
    /// block from an earlier slot never counts as timely. Proposer boost and the
    /// re-org guards read these flags later.
    pub fn record_block_timeliness(&mut self, root: Root) -> Result<(), ForkChoiceError> {
        let block_slot = self
            .blocks
            .get(&root)
            .ok_or(ForkChoiceError::UnknownBlock(root))?
            .slot;
        let time_into_slot = self.time_into_slot_ms();
        let is_current_slot = self.get_current_slot() == block_slot;
        let timeliness = BlockTimeliness {
            attestation_timely: is_current_slot && time_into_slot < get_attestation_due_ms(),
            payload_attestation_timely: is_current_slot
                && time_into_slot < get_payload_attestation_due_ms(),
        };
        self.block_timeliness.insert(root, timeliness);
        Ok(())
    }

    /// Give proposer boost to `root` if it is the slot's first on-time block on
    /// our chain.
    ///
    /// Proposer boost is the temporary extra weight a timely new block earns. It
    /// is granted at most once per slot, and only when all three hold: the block
    /// was on-time, nothing has been boosted yet this slot, and the block shares a
    /// duty window (the same dependent root) with the current head, so the boost
    /// cannot be carried onto an unrelated fork. Otherwise the boost target is
    /// left as is.
    pub fn update_proposer_boost_root(
        &mut self,
        head: Root,
        root: Root,
    ) -> Result<(), ForkChoiceError> {
        let is_first_block = self.proposer_boost_root == Root::ZERO;
        let is_timely = self
            .block_timeliness
            .get(&root)
            .ok_or(ForkChoiceError::UnknownBlock(root))?
            .attestation_timely;
        let dependent_root = self.get_dependent_root(root)?;
        let head_dependent_root = self.get_dependent_root(head)?;
        let is_same_dependent_root = dependent_root == head_dependent_root;
        if is_timely && is_first_block && is_same_dependent_root {
            self.proposer_boost_root = root;
        }
        Ok(())
    }
}
