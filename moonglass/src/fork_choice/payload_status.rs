//! The empty, full, and pending payload model, and the small helpers that
//! resolve it.
//!
//! To read this file, hold one fact about how [blocks](crate::glossary#beacon-block)
//! are built here. A beacon block (the object [validators](crate::glossary#validator)
//! vote on) and its execution payload (the
//! transactions that actually run) are produced by two different parties and
//! can arrive at different times. A builder promises a payload ahead of time by
//! publishing a signed *bid*, and the payload itself is delivered later in an
//! *envelope*. Because that payload can be late or never arrive, the same block
//! can end up in one of three shapes, which we call its payload status:
//!
//!  - [`Empty`](PayloadStatus::Empty): the branch that carries the chain forward
//!    without this block's payload, as if this [slot](crate::glossary#slot)
//!    carried no transactions.
//!  - [`Full`](PayloadStatus::Full): the branch that carries the chain forward on
//!    top of this block's payload, once that payload has been delivered.
//!  - [`Pending`](PayloadStatus::Pending): not decided yet, because the votes
//!    that settle empty versus full have not arrived.
//!
//! Fork choice has to keep these outcomes apart, so a position in the
//! fork-choice tree is a block root paired with one of these statuses (a
//! [`ForkChoiceNode`]), not a block root on its own. The methods here answer
//! the small questions that resolve the status: did a block build on its
//! parent's payload, have we actually received a block's payload, and do the
//! [committee](crate::glossary#committee) votes say the payload was on time with
//! its data available.
//! [`Store::get_head`] and [`weight`](super::weight) build on every one of them.
//!
//! Keep one boundary in mind throughout: recording a payload here means only the
//! consensus-side checks passed. A data-availability check and an execution-engine
//! check are mocked for now (a TODO), so a recorded payload is not yet a complete
//! validity verdict.
use crate::containers::BeaconBlock;
use crate::error::ForkChoiceError;
use crate::primitives::Root;

use super::helpers::is_next_slot;
use super::store::{ForkChoiceNode, PayloadStatus, Store};

impl Store {
    /// Work out whether `child_block` was built on top of its parent's payload,
    /// or on a parent whose payload was skipped.
    ///
    /// This is the question that places a block on the *full* branch or the
    /// *empty* branch of its parent. We answer it by comparing two hashes the
    /// builders committed to in advance. Every block's bid names the
    /// `parent_block_hash` it intends to build on, and the parent block's own bid
    /// names the `block_hash` its payload was meant to produce. If the child
    /// points at the parent's payload hash, the child is continuing that payload,
    /// so the parent reads as [`Full`](PayloadStatus::Full) from the child's point
    /// of view. If it points elsewhere, the child treated the parent as having
    /// produced nothing, so the parent reads as [`Empty`](PayloadStatus::Empty).
    ///
    /// The parent must already be known to the store, otherwise there is nothing
    /// to compare against and the call returns [`ForkChoiceError::UnknownParent`].
    /// Note that this only reads what the bids promised. It does not check whether
    /// the parent's payload was actually delivered. That is the job of
    /// [`Self::is_payload_verified`].
    pub fn get_parent_payload_status(
        &self,
        child_block: &BeaconBlock,
    ) -> Result<PayloadStatus, ForkChoiceError> {
        let parent_root = child_block.parent_root;
        let parent_block = self
            .blocks
            .get(&parent_root)
            .ok_or(ForkChoiceError::UnknownParent(parent_root))?;

        let child_bid = &child_block.body.signed_execution_payload_bid.message;
        let parent_bid = &parent_block.body.signed_execution_payload_bid.message;
        let child_payload_parent_hash = child_bid.parent_block_hash;
        let parent_payload_block_hash = parent_bid.block_hash;

        // The child continues the parent's payload only when it builds on the very
        // block hash that payload was meant to produce.
        let parent_payload_status = if child_payload_parent_hash == parent_payload_block_hash {
            PayloadStatus::Full
        } else {
            PayloadStatus::Empty
        };

        Ok(parent_payload_status)
    }

    /// Convenience check: does `block`'s bid claim to build on its parent's full
    /// payload?
    ///
    /// This is just [`Self::get_parent_payload_status`] compared against
    /// [`Full`](PayloadStatus::Full). It reflects what the child block *claims*,
    /// not proof that the parent's payload actually arrived. That distinction
    /// matters during block admission: [`Store::on_block`] pairs this with
    /// [`Self::is_payload_verified`] so a block is only accepted onto the full
    /// branch once we have really seen the parent's payload.
    pub fn is_parent_node_full(&self, block: &BeaconBlock) -> Result<bool, ForkChoiceError> {
        Ok(self.get_parent_payload_status(block)? == PayloadStatus::Full)
    }

    /// Has this block's execution payload envelope been locally delivered and
    /// verified, via [`Store::on_execution_payload_envelope`]?
    ///
    /// That handler keeps every envelope that passes its checks in
    /// [`Store::payloads`](super::store::Store::payloads), so this just asks
    /// whether an entry exists for `root`. That single yes-or-no is the line
    /// between a block that *might* become full and one that is now eligible to be
    /// treated as full.
    ///
    /// "Verified" here means the consensus-side checks passed. The
    /// data-availability and execution-engine checks are mocked in this
    /// implementation and always pass, so it is not yet a full execution validity
    /// verdict.
    pub fn is_payload_verified(&self, root: Root) -> bool {
        self.payloads.contains_key(&root)
    }

    /// The committee's verdict for one payload-vote question.
    ///
    /// With no payload recorded, the only answer we can give is the inverse of
    /// `expected`: a payload we never stored cannot be timely, nor its data
    /// available. Otherwise the votes matching `expected` must clear `threshold`.
    /// Shared by [`Self::payload_timeliness`] and [`Self::payload_data_availability`]
    /// so the two cannot drift apart.
    pub fn payload_vote_verdict(
        &self,
        root: Root,
        votes: &[Option<bool>],
        expected: bool,
        threshold: u64,
    ) -> bool {
        if !self.is_payload_verified(root) {
            return !expected;
        }
        votes_clear_threshold(votes, expected, threshold)
    }

    /// Do enough committee votes agree that this block's payload was *timely*?
    ///
    /// Each slot, a small group of validators called the payload-timeliness
    /// committee (PTC) is sampled to watch for the block's payload and report
    /// whether it showed up on time. Their votes are stored by committee position
    /// in [`Store::payload_timeliness_vote`](super::store::Store::payload_timeliness_vote).
    /// Pass `timely = true` to ask "did the committee see it on time?", or
    /// `timely = false` to ask the opposite.
    ///
    /// One short-circuit comes first. If we have not even recorded the payload
    /// ourselves ([`Self::is_payload_verified`] is false), then locally
    /// it cannot be timely, so the only answer we support is the not-timely one.
    /// Once the payload is recorded, we count the committee votes matching the
    /// question and report whether they clear
    /// [`PAYLOAD_TIMELY_THRESHOLD`](crate::constants::PAYLOAD_TIMELY_THRESHOLD), a
    /// majority of the committee. The block must be known, otherwise
    /// [`ForkChoiceError::UnknownBlock`] is returned.
    pub fn payload_timeliness(&self, root: Root, timely: bool) -> Result<bool, ForkChoiceError> {
        let votes = self
            .payload_timeliness_vote
            .get(&root)
            .ok_or(ForkChoiceError::UnknownBlock(root))?;
        Ok(self.payload_vote_verdict(
            root,
            votes,
            timely,
            crate::constants::PAYLOAD_TIMELY_THRESHOLD,
        ))
    }

    /// Do enough committee votes agree that this block's payload *data was
    /// available*?
    ///
    /// This mirrors [`Self::payload_timeliness`], but answers the other question
    /// the same committee reports on: not "was the payload on time" but "could its
    /// data be downloaded by other nodes". A payload can carry data that the rest
    /// of the network must be able to fetch, and the committee vouches for whether
    /// that data was actually retrievable.
    ///
    /// The shape is identical to [`Self::payload_timeliness`]. If we have not
    /// recorded the payload, the only supported answer is "not available".
    /// Otherwise we count the committee votes matching `available` and compare
    /// against
    /// [`DATA_AVAILABILITY_TIMELY_THRESHOLD`](crate::constants::DATA_AVAILABILITY_TIMELY_THRESHOLD).
    /// This is a tally of votes, not a real proof that the data exists.
    pub fn payload_data_availability(
        &self,
        root: Root,
        available: bool,
    ) -> Result<bool, ForkChoiceError> {
        let votes = self
            .payload_data_availability_vote
            .get(&root)
            .ok_or(ForkChoiceError::UnknownBlock(root))?;
        Ok(self.payload_vote_verdict(
            root,
            votes,
            available,
            crate::constants::DATA_AVAILABILITY_TIMELY_THRESHOLD,
        ))
    }

    /// Is `node` an empty-or-full decision about the block from the slot just
    /// before now?
    ///
    /// The empty-versus-full choice for a block is settled by the votes that
    /// arrive during the *following* slot. That puts the block from one slot ago
    /// in a special position: it is the one whose payload outcome we are actively
    /// deciding right now. This predicate spots exactly that case, a node whose
    /// block sits one slot in the past and whose status is already
    /// [`Empty`](PayloadStatus::Empty) or [`Full`](PayloadStatus::Full), not still
    /// [`Pending`](PayloadStatus::Pending).
    ///
    /// Such nodes get special handling in scoring and tie-breaking (see
    /// [`Store::get_weight`] and [`Self::get_payload_status_tiebreaker`]), which is
    /// why they must be recognised on their own. Returns
    /// [`ForkChoiceError::UnknownBlock`] if the node's block is not in the store.
    pub fn is_previous_slot_payload_decision(
        &self,
        node: ForkChoiceNode,
    ) -> Result<bool, ForkChoiceError> {
        let block_slot = self
            .blocks
            .get(&node.root)
            .ok_or(ForkChoiceError::UnknownBlock(node.root))?
            .slot;
        let is_previous_slot = is_next_slot(block_slot, self.get_current_slot());
        Ok(is_previous_slot && node.is_payload_decision())
    }

    /// When deciding the previous slot's payload, should we extend the *full*
    /// branch of `root`?
    ///
    /// By the time we decide, the block from the previous slot may or may not have
    /// a delivered payload, and the committee may or may not have vouched for it.
    /// This method holds the rule for when the full branch wins.
    ///
    /// First, there must be a payload at all. With nothing recorded there is no
    /// full branch to extend, so the answer is false. With a payload recorded, the
    /// easy case is when the committee says it was both timely and its data
    /// available, in which case we extend. When that strong evidence is missing we
    /// still extend in every case but one: this slot's proposer-boosted block is a
    /// direct child of `root` that chose `root`'s empty branch. Equivalently, we
    /// keep the full branch whenever any of these holds:
    ///
    ///  - no block is boosted this slot,
    ///  - the boosted block builds on something other than `root`, or
    ///  - that boosted child built on `root`'s full branch.
    ///
    /// `root` must be the previous slot's block, otherwise
    /// [`ForkChoiceError::NotPreviousSlot`] is returned (or
    /// [`ForkChoiceError::UnknownBlock`] if it is not in the store).
    pub fn should_extend_payload(&self, root: Root) -> Result<bool, ForkChoiceError> {
        let block_slot = self
            .blocks
            .get(&root)
            .ok_or(ForkChoiceError::UnknownBlock(root))?
            .slot;
        if !is_next_slot(block_slot, self.get_current_slot()) {
            return Err(ForkChoiceError::NotPreviousSlot(root));
        }
        if !self.is_payload_verified(root) {
            return Ok(false);
        }

        let payload_is_timely = self.payload_timeliness(root, true)?;
        let payload_data_is_available = self.payload_data_availability(root, true)?;
        if payload_is_timely && payload_data_is_available {
            return Ok(true);
        }

        let proposer_root = self.proposer_boost_root;
        if proposer_root == Root::ZERO {
            return Ok(true);
        }
        let proposer_block = self
            .blocks
            .get(&proposer_root)
            .ok_or(ForkChoiceError::UnknownBlock(proposer_root))?;
        if proposer_block.parent_root != root {
            return Ok(true);
        }
        self.is_parent_node_full(proposer_block)
    }

    /// Turn a node's payload status into a small ranking number used to break
    /// ties when two candidates weigh the same.
    ///
    /// [`Store::get_head`] walks the tree by always taking the heaviest child, and
    /// when two children weigh exactly the same it falls back to this ranking,
    /// where a higher number wins. For an ordinary node the order is pending (2),
    /// then full (1), then empty (0): all else equal, prefer the not-yet-committed
    /// branch, then full, then empty.
    ///
    /// The previous-slot decision (the case
    /// [`Self::is_previous_slot_payload_decision`] catches) switches to a
    /// purpose-built order. There an empty branch ranks 1, and a full branch ranks
    /// 2 only when [`Self::should_extend_payload`] judges the evidence strong
    /// enough, otherwise it ranks 0. The effect is that a well-supported full
    /// branch beats empty, but an unsupported full branch loses to it. Returns
    /// [`ForkChoiceError::UnknownBlock`] if the node's block is missing.
    pub fn get_payload_status_tiebreaker(
        &self,
        node: ForkChoiceNode,
    ) -> Result<u8, ForkChoiceError> {
        let payload_status_tiebreaker = if self.is_previous_slot_payload_decision(node)? {
            match node.payload_status {
                // Empty outranks a full branch the evidence does not support.
                PayloadStatus::Empty => 1,
                // Full wins only when the evidence argues for extending it.
                PayloadStatus::Full => {
                    if self.should_extend_payload(node.root)? {
                        2
                    } else {
                        0
                    }
                }
                // A pending node is never a previous-slot decision, so rank it as
                // in the ordinary case for exhaustiveness.
                PayloadStatus::Pending => 2,
            }
        } else {
            node.payload_status.ordinary_tiebreaker_rank()
        };

        Ok(payload_status_tiebreaker)
    }

    /// Decide whether a proposer building this slot should extend the head's
    /// *full* payload branch rather than its *empty* one.
    ///
    /// `head` must be a resolved node, [`Empty`](PayloadStatus::Empty) or
    /// [`Full`](PayloadStatus::Full); a still-[`Pending`](PayloadStatus::Pending)
    /// head has no branch to choose and is rejected. For a head from earlier than
    /// the previous slot the choice just follows its branch: build full exactly
    /// when the head is already full. For the previous slot's block the committee's
    /// view decides: keep the full branch only while the payload-timeliness
    /// committee has not reported the payload late ([`Self::payload_timeliness`])
    /// or its data unavailable ([`Self::payload_data_availability`]).
    pub fn should_build_on_full(&self, head: ForkChoiceNode) -> Result<bool, ForkChoiceError> {
        if head.payload_status == PayloadStatus::Pending {
            return Err(ForkChoiceError::BuildOnPendingNode(head.root));
        }
        let head_block_slot = self
            .blocks
            .get(&head.root)
            .ok_or(ForkChoiceError::UnknownBlock(head.root))?
            .slot;
        if !is_next_slot(head_block_slot, self.get_current_slot()) {
            return Ok(head.payload_status == PayloadStatus::Full);
        }
        if head.payload_status == PayloadStatus::Empty {
            return Ok(false);
        }
        if self.payload_timeliness(head.root, false)? {
            return Ok(false);
        }
        if self.payload_data_availability(head.root, false)? {
            return Ok(false);
        }
        Ok(true)
    }
}

/// Do more than `threshold` of the recorded votes equal `expected`?
///
/// The shared tally behind [`Store::payload_timeliness`] and
/// [`Store::payload_data_availability`]: count the committee positions whose vote
/// matches `expected`, then report whether that count clears `threshold`.
pub fn votes_clear_threshold(votes: &[Option<bool>], expected: bool, threshold: u64) -> bool {
    let matching_vote_count = votes.iter().filter(|vote| **vote == Some(expected)).count();
    let matching_vote_count = u64::try_from(matching_vote_count).unwrap_or(u64::MAX);
    matching_vote_count > threshold
}
