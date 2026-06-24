//! Small shared helpers: the store clock, walking ancestry, and
//! [slot](crate::glossary#slot) timing.
//!
//! These are the utilities the handlers and head selection lean on. They do
//! three kinds of job: read the current slot and [epoch](crate::glossary#epoch)
//! from the store's clock, walk a [block's](crate::glossary#beacon-block)
//! ancestors while carrying the payload branch along, and turn
//! fractions of a slot into millisecond deadlines. Each is tiny on its own. The
//! point is that the rest of the module can treat them as settled vocabulary.

use crate::constants::{SLOT_DURATION_MS, SLOTS_PER_EPOCH};
use crate::containers::BeaconState;
use crate::error::ForkChoiceError;
use crate::primitives::{Epoch, Gwei, Root, Slot};

use super::store::{ForkChoiceNode, LatestMessage, PayloadStatus, Store};

impl Store {
    /// How many whole slots have passed since genesis, by the store's own clock.
    ///
    /// A slot is the fixed time window in which one block may be proposed. Fork
    /// choice reads time from the store rather than from any [`BeaconState`],
    /// because the store is this node's live view of the clock and the messages
    /// it has seen.
    pub fn get_slots_since_genesis(&self) -> u64 {
        self.slot_at_time(self.time)
    }

    /// The slot the store thinks it is now.
    ///
    /// Just [`Self::get_slots_since_genesis`] wrapped as a [`Slot`].
    pub fn get_current_slot(&self) -> Slot {
        Slot::new(self.get_slots_since_genesis())
    }

    /// Which slot number a wall-clock `time` (Unix seconds) falls in, by this
    /// store's genesis.
    ///
    /// The store keeps time in seconds while slot durations are configured in
    /// milliseconds, so this is the single place that bridges the two when going
    /// from a time to a slot.
    pub fn slot_at_time(&self, time: u64) -> u64 {
        seconds_to_milliseconds(time.saturating_sub(self.genesis_time)) / SLOT_DURATION_MS
    }

    /// The wall-clock start time (Unix seconds) of `slot`, by this store's genesis.
    ///
    /// The inverse of [`Self::slot_at_time`]: it turns a slot number back into the
    /// second its window opens.
    pub fn slot_start_time(&self, slot: u64) -> u64 {
        self.genesis_time
            .saturating_add(slot.saturating_mul(SLOT_DURATION_MS) / 1_000)
    }

    /// The epoch the store thinks it is now.
    ///
    /// An epoch is a fixed run of slots, the period over which validator duties
    /// and justification are organised. This returns the current slot's epoch.
    pub fn get_current_store_epoch(&self) -> Epoch {
        self.get_current_slot().epoch()
    }

    /// Milliseconds elapsed since the start of the current slot.
    ///
    /// The store's clock counts whole seconds since genesis. This converts that to
    /// milliseconds and takes the remainder within one slot, giving how far into
    /// the current slot we are. In-slot deadlines such as [`get_attestation_due_ms`]
    /// are compared against this offset to decide whether something arrived on
    /// time.
    pub fn time_into_slot_ms(&self) -> u64 {
        let seconds_since_genesis = self.time.saturating_sub(self.genesis_time);
        seconds_to_milliseconds(seconds_since_genesis) % SLOT_DURATION_MS
    }

    /// The beacon state cached at the store's current justified checkpoint.
    ///
    /// Head scoring and the re-org guards measure against this baseline, so they
    /// all reach for it the same way. Returns
    /// [`ForkChoiceError::JustifiedStateMissing`] if the justified checkpoint's
    /// state was never cached.
    pub fn get_justified_state(&self) -> Result<&BeaconState, ForkChoiceError> {
        self.checkpoint_states
            .get(&self.justified_checkpoint)
            .ok_or(ForkChoiceError::JustifiedStateMissing)
    }

    /// Walk back up the chain from `node` to the block at or before `slot`.
    ///
    /// Following parent links is how fork choice asks "which earlier block is this
    /// one descended from". The twist here is that each step also recomputes the
    /// parent's payload branch from the child's bid (via
    /// [`Self::get_parent_payload_status`]), so the node we return carries the
    /// right empty-or-full [`PayloadStatus`], not just a block root.
    pub fn get_ancestor(
        &self,
        node: ForkChoiceNode,
        slot: Slot,
    ) -> Result<ForkChoiceNode, ForkChoiceError> {
        let mut current = node;
        loop {
            let block = self
                .blocks
                .get(&current.root)
                .ok_or(ForkChoiceError::UnknownBlock(current.root))?;
            if block.slot <= slot {
                return Ok(current);
            }
            let parent_status = self.get_parent_payload_status(block)?;
            current = ForkChoiceNode::new(block.parent_root, parent_status);
        }
    }

    /// Does `ancestor` lie on `node`'s line, matching both block root and branch?
    ///
    /// First it walks `node` back to `ancestor`'s slot and checks the roots match.
    /// Then it checks the payload branch: a [`Pending`](PayloadStatus::Pending)
    /// ancestor matches either resolved branch, since it has not committed to
    /// empty or full yet, while a resolved ancestor must match exactly. Weighting
    /// uses this to decide whether a vote for one node also counts toward another.
    pub fn is_ancestor(
        &self,
        node: ForkChoiceNode,
        ancestor: ForkChoiceNode,
    ) -> Result<bool, ForkChoiceError> {
        let ancestor_block = self
            .blocks
            .get(&ancestor.root)
            .ok_or(ForkChoiceError::UnknownBlock(ancestor.root))?;
        let ancestor_slot = ancestor_block.slot;
        let node_ancestor = self.get_ancestor(node, ancestor_slot)?;
        if node_ancestor.root != ancestor.root {
            return Ok(false);
        }
        Ok(node_ancestor.payload_status == ancestor.payload_status
            || ancestor.payload_status == PayloadStatus::Pending)
    }

    /// Find the checkpoint block for `epoch` on `root`'s chain.
    ///
    /// A checkpoint is the block at an epoch's first slot, or the latest ancestor
    /// before it when that slot is empty, and it is what attestations name as
    /// their finality target. This walks `root`'s ancestry back to that slot (or
    /// the most recent block before it) and returns the block found there, so a
    /// vote's block can be matched against the target it claims to support.
    pub fn get_checkpoint_block(&self, root: Root, epoch: Epoch) -> Result<Root, ForkChoiceError> {
        let epoch_first_slot = epoch.start_slot();
        let node = ForkChoiceNode::pending(root);
        Ok(self.get_ancestor(node, epoch_first_slot)?.root)
    }

    /// Turn a validator's latest vote into the exact node it supports.
    ///
    /// A stored vote ([`LatestMessage`]) names a block and the slot it was cast
    /// for. The branch it supports follows from those via
    /// [`LatestMessage::supported_payload_status`]: full or empty for a vote about
    /// an older block, pending for a vote about the block's own slot. Scoring
    /// counts a validator's weight toward whatever node this returns.
    pub fn get_supported_node(
        &self,
        message: LatestMessage,
    ) -> Result<ForkChoiceNode, ForkChoiceError> {
        let block = self
            .blocks
            .get(&message.root)
            .ok_or(ForkChoiceError::UnknownBlock(message.root))?;
        Ok(ForkChoiceNode::new(
            message.root,
            message.supported_payload_status(block.slot),
        ))
    }

    /// Which justified checkpoint does the block at `block_root` vote from?
    ///
    /// A block's "voting source" is the justified checkpoint its view of finality
    /// rests on. If the block is from an earlier epoch, we use the justification
    /// we later pulled up for it (its
    /// [`unrealized justification`](super::store::Store::unrealized_justifications)).
    /// If it is from the current epoch, we use its own post-state's justified
    /// checkpoint. Fork-choice filtering uses this to drop branches that vote from
    /// a stale source.
    pub fn get_voting_source(
        &self,
        block_root: Root,
    ) -> Result<crate::containers::Checkpoint, ForkChoiceError> {
        let block = self
            .blocks
            .get(&block_root)
            .ok_or(ForkChoiceError::UnknownBlock(block_root))?;
        let current_epoch = self.get_current_store_epoch();
        let block_epoch = block.slot.epoch();
        if current_epoch > block_epoch {
            self.unrealized_justifications
                .get(&block_root)
                .copied()
                .ok_or(ForkChoiceError::MissingUnrealizedJustification(block_root))
        } else {
            let head_state = self
                .block_states
                .get(&block_root)
                .ok_or(ForkChoiceError::UnknownBlock(block_root))?;
            Ok(head_state.current_justified_checkpoint)
        }
    }

    /// The earlier block whose root seeds the randomness for `root`'s recent
    /// duties.
    ///
    /// Validator duties for an epoch are derived from a random "seed", and that
    /// seed is fixed by one particular earlier block, called the dependent root.
    /// Proposer boost only competes blocks that share a dependent root, so a boost
    /// cannot be carried across a change in duties. Near genesis there is no such
    /// block, so the zero root is returned.
    pub fn get_dependent_root(&self, root: Root) -> Result<Root, ForkChoiceError> {
        let epoch = self.get_current_store_epoch();
        let min_seed_lookahead =
            u64::try_from(crate::constants::MIN_SEED_LOOKAHEAD).unwrap_or(u64::MAX);
        if epoch.as_u64() <= min_seed_lookahead {
            return Ok(Root::ZERO);
        }
        let node = ForkChoiceNode::pending(root);
        let dependent_epoch_start = epoch
            .saturating_sub(min_seed_lookahead)
            .start_slot()
            .as_u64();
        let dependent_slot = Slot::new(dependent_epoch_start.saturating_sub(1));
        Ok(self.get_ancestor(node, dependent_slot)?.root)
    }
}

/// Is `next` the slot immediately after `previous`?
///
/// Several re-org and payload rules turn on whether one slot directly follows
/// another, such as a block built in the slot right after its parent. This names
/// that adjacency check so the rules read in terms of it rather than raw `+ 1`
/// arithmetic.
pub fn is_next_slot(previous: Slot, next: Slot) -> bool {
    previous.as_u64().checked_add(1) == Some(next.as_u64())
}

/// Is `current` at or after the slot immediately following `previous`?
///
/// The "a full slot has elapsed" test, used to reject messages that arrive too
/// early, such as an attestation that only counts once its own slot has passed.
/// A `previous` at the maximum slot has no following slot, so nothing is at or
/// after it.
pub fn is_at_or_after_next_slot(previous: Slot, current: Slot) -> bool {
    previous
        .as_u64()
        .checked_add(1)
        .is_some_and(|next| current.as_u64() >= next)
}

/// How far `slot` sits into its own epoch, counted in slots.
///
/// Returns 0 at an epoch's first slot. Timing guards such as proposer boost and
/// the attestation deadline use this offset.
pub fn compute_slots_since_epoch_start(slot: Slot) -> u64 {
    slot.as_u64() - slot.epoch().start_slot().as_u64()
}

/// A committee-sized slice of the total stake, taken as a percentage.
///
/// Several thresholds (proposer boost, the weak-head guard) are sized against
/// the stake of a single slot's committee, not the whole validator set. This
/// divides total active balance by the number of slots in an epoch to get one
/// committee's worth, then takes `committee_percent` of that.
pub fn calculate_committee_fraction(state: &BeaconState, committee_percent: u64) -> Gwei {
    let slots_per_epoch = u64::try_from(SLOTS_PER_EPOCH).unwrap_or(u64::MAX);
    let committee_weight = state.total_active_balance() / slots_per_epoch;
    committee_weight * committee_percent / 100
}

/// Multiply seconds by 1000, saturating at `u64::MAX` instead of overflowing.
pub fn seconds_to_milliseconds(seconds: u64) -> u64 {
    seconds.saturating_mul(1_000)
}

/// Turn a fraction of a slot, given in basis points, into milliseconds.
///
/// Deadlines inside a slot (when an attestation is due, when a payload is due)
/// are configured as basis points, where 10000 means the whole slot. This turns
/// that fraction into a concrete millisecond offset from the slot's start.
pub fn get_slot_component_duration_ms(basis_points: u64) -> u64 {
    basis_points * crate::constants::SLOT_DURATION_MS / crate::constants::BASIS_POINTS
}

/// How long into a slot an attestation still counts as on time, in milliseconds.
pub fn get_attestation_due_ms() -> u64 {
    get_slot_component_duration_ms(crate::constants::ATTESTATION_DUE_BPS_GLOAS)
}

/// How long into a slot a payload attestation still counts as on time, in
/// milliseconds.
pub fn get_payload_attestation_due_ms() -> u64 {
    get_slot_component_duration_ms(crate::constants::PAYLOAD_ATTESTATION_DUE_BPS)
}
