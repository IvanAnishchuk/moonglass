//! Scoring a node: how much support it has, plus the proposer boost.
//!
//! A node's weight is the [stake](crate::glossary#effective-balance) of every
//! [validator](crate::glossary#validator) whose latest vote backs it.
//! Each vote is first resolved to the exact node it supports, then counted if
//! that node lies on the line being scored. On top of that, a freshly proposed
//! [block](crate::glossary#beacon-block) can carry a temporary proposer boost.
//! This file also holds the "weak-head" guard, which asks whether a
//! [head](crate::glossary#head) has so little support that a re-org could fairly
//! replace it.
//!
//! [`head`](super::head) reads these scores to choose the head, and
//! [`proposer_head`](super::proposer_head) reuses the weak-head guard for its
//! re-org decision.

use crate::constants::{PROPOSER_SCORE_BOOST, REORG_HEAD_WEIGHT_THRESHOLD};
use crate::containers::BeaconState;
use crate::error::ForkChoiceError;
use crate::primitives::{CommitteeIndex, Gwei, Root, ValidatorIndex};

use super::helpers::{calculate_committee_fraction, is_next_slot};
use super::store::{ForkChoiceNode, Store};

impl Store {
    /// Add up the stake of every validator whose latest vote supports `node`.
    ///
    /// This is the heart of "heaviest chain wins": a node's attestation score is
    /// the total effective balance (the stake that counts for voting) behind it.
    /// We look at each active validator's most recent vote, skip anyone who
    /// equivocated and is therefore ignored, resolve the vote to the exact node it
    /// supports with [`Self::get_supported_node`], and count that validator's
    /// balance when the supported node sits on `node`'s line ([`Self::is_ancestor`]).
    pub fn get_attestation_score(
        &self,
        node: ForkChoiceNode,
        state: &BeaconState,
    ) -> Result<Gwei, ForkChoiceError> {
        let epoch = state.slot.epoch();
        let candidates = state.active_unslashed_validator_indices(epoch);
        let mut total = Gwei::ZERO;
        for validator in candidates {
            let Some(message) = self.latest_messages.get(&validator).copied() else {
                continue;
            };
            if self.equivocating_indices.contains(&validator) {
                continue;
            }
            let supported_node = self.get_supported_node(message)?;
            let vote_supports_candidate = self.is_ancestor(supported_node, node)?;
            if !vote_supports_candidate {
                continue;
            }
            let weight = state
                .validators
                .get(validator.as_usize())
                .ok_or(ForkChoiceError::ValidatorOutOfBounds(validator))?
                .effective_balance;
            total = add_weight(total, weight)?;
        }
        Ok(total)
    }

    /// The proposer boost amount, measured against the store's justified state.
    ///
    /// The same value as [`compute_proposer_score`], read from the justified
    /// checkpoint's state, which is the baseline all head scoring is measured
    /// against.
    pub fn get_proposer_score(&self) -> Result<Gwei, ForkChoiceError> {
        let state = self.get_justified_state()?;
        Ok(compute_proposer_score(state))
    }

    /// Should the current proposer boost actually be counted right now?
    ///
    /// Usually yes. The boost is withheld only in one delicate case meant to stop
    /// a late proposer from cementing a block that should have been re-orged: the
    /// boosted block's parent sits exactly one slot back and is weakly supported
    /// ([`Self::is_head_weak`]), and that parent's proposer also produced a second
    /// block in the parent's own slot that met the payload-attestation deadline,
    /// an equivocation. There the boost could lock in the wrong branch, so it is
    /// dropped. In every other case it applies.
    pub fn should_apply_proposer_boost(&self) -> Result<bool, ForkChoiceError> {
        if self.proposer_boost_root == Root::ZERO {
            return Ok(false);
        }
        let block = self
            .blocks
            .get(&self.proposer_boost_root)
            .ok_or(ForkChoiceError::UnknownBlock(self.proposer_boost_root))?;
        let parent_root = block.parent_root;
        let parent = self
            .blocks
            .get(&parent_root)
            .ok_or(ForkChoiceError::UnknownParent(parent_root))?;
        let slot = block.slot;

        // A stored block's parent always sits in an earlier slot, so this holds
        // exactly when the parent is not in the slot just before the block.
        if !is_next_slot(parent.slot, slot) {
            return Ok(true);
        }
        if !self.is_head_weak(parent_root)? {
            return Ok(true);
        }
        let proposer_index = parent.proposer_index;
        for (root, b) in &self.blocks {
            if *root == parent_root {
                continue;
            }
            if b.proposer_index != proposer_index {
                continue;
            }
            if !is_next_slot(b.slot, slot) {
                continue;
            }
            let timeliness = self
                .block_timeliness
                .get(root)
                .copied()
                .ok_or(ForkChoiceError::MissingBlockTimeliness(*root))?;
            if timeliness.payload_attestation_timely {
                return Ok(false);
            }
        }
        Ok(true)
    }

    /// Is `head_root` weak enough that proposer boost could overturn it?
    ///
    /// "Weak" means the head's attestation support sits below a configured
    /// fraction of one committee's stake. A proposer weighing a re-org checks
    /// this: if the current head is weak, a fresh on-time block plus proposer
    /// boost can outweigh it. To keep the test stable, the stake of any
    /// equivocating validators in the head's own committees is added in, so more
    /// votes can only ever make a head look stronger, never weaker.
    pub fn is_head_weak(&self, head_root: Root) -> Result<bool, ForkChoiceError> {
        let justified_state = self.get_justified_state()?;
        let reorg_threshold =
            calculate_committee_fraction(justified_state, REORG_HEAD_WEIGHT_THRESHOLD);

        let head_state = self
            .block_states
            .get(&head_root)
            .ok_or(ForkChoiceError::UnknownBlock(head_root))?;
        let head_block = self
            .blocks
            .get(&head_root)
            .ok_or(ForkChoiceError::UnknownBlock(head_root))?;
        let head_node = ForkChoiceNode::pending(head_root);
        let mut head_weight = self.get_attestation_score(head_node, justified_state)?;

        let epoch = head_block.slot.epoch();
        let committees = head_state.committee_count_per_slot(epoch);
        for index in 0..committees {
            let committee = head_state.beacon_committee(head_block.slot, CommitteeIndex(index))?;
            let weight = self.equivocating_committee_weight(justified_state, &committee)?;
            head_weight = add_weight(head_weight, weight)?;
        }
        Ok(head_weight < reorg_threshold)
    }

    /// Total effective balance of the equivocating validators in `committee`.
    ///
    /// Folded back into a head's weight so equivocations can only ever make it
    /// look stronger, never weaker. A committee member missing from the registry
    /// is a broken store invariant and returns
    /// [`ForkChoiceError::ValidatorOutOfBounds`] rather than being silently
    /// skipped.
    pub fn equivocating_committee_weight(
        &self,
        state: &BeaconState,
        committee: &[ValidatorIndex],
    ) -> Result<Gwei, ForkChoiceError> {
        committee
            .iter()
            .filter(|index| self.equivocating_indices.contains(*index))
            .try_fold(Gwei::ZERO, |total, index| {
                let validator = state
                    .validators
                    .get(index.as_usize())
                    .ok_or(ForkChoiceError::ValidatorOutOfBounds(*index))?;
                add_weight(total, validator.effective_balance)
            })
    }

    /// The total fork-choice weight of `node`: attestation support plus any boost.
    ///
    /// This is the number [`Store::get_head`] maximises at each step. A node that
    /// is a previous-slot empty-or-full decision carries no direct weight of its
    /// own (it is settled by tie-break instead), so it scores zero. Otherwise the
    /// weight is its attestation score, plus the proposer boost when the boost
    /// applies ([`Self::should_apply_proposer_boost`]) and the boosted block
    /// descends from this node.
    pub fn get_weight(&self, node: ForkChoiceNode) -> Result<Gwei, ForkChoiceError> {
        if self.is_previous_slot_payload_decision(node)? {
            return Ok(Gwei::ZERO);
        }
        let state = self.get_justified_state()?;
        let attestation_score = self.get_attestation_score(node, state)?;
        if !self.should_apply_proposer_boost()? {
            return Ok(attestation_score);
        }
        let proposer_boost_node = ForkChoiceNode::pending(self.proposer_boost_root);
        let proposer_score = if self.is_ancestor(proposer_boost_node, node)? {
            self.get_proposer_score()?
        } else {
            Gwei::ZERO
        };
        add_weight(attestation_score, proposer_score)
    }
}

/// The size of the proposer boost, in stake.
///
/// When a block is proposed on time, fork choice temporarily adds extra weight,
/// the proposer boost, to discourage others from re-orging it away. The boost is
/// one slot committee's stake scaled by a fixed percentage. This computes that
/// amount from a given state, and [`Store::get_head`] adds it only to nodes the
/// boosted block descends from.
pub fn compute_proposer_score(state: &BeaconState) -> Gwei {
    calculate_committee_fraction(state, PROPOSER_SCORE_BOOST)
}

/// Add two fork-choice weights, returning [`ForkChoiceError::WeightOverflow`] on
/// `u64` overflow.
///
/// Head scoring sums validator stake, which stays well below `u64::MAX`, so this
/// never overflows in practice. Erroring instead of saturating keeps a would-be
/// overflow from silently changing which node wins the head comparison.
pub fn add_weight(total: Gwei, weight: Gwei) -> Result<Gwei, ForkChoiceError> {
    total
        .checked_add(weight)
        .ok_or(ForkChoiceError::WeightOverflow)
}
