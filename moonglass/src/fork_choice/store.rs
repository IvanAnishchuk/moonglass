//! The fork-choice [`Store`], the data a node keeps in order to choose a
//! [head](crate::glossary#head).
//!
//! The store is not the blockchain's agreed-upon state. It is one node's private
//! notebook: the [blocks](crate::glossary#beacon-block) it has heard about and
//! the states they produce, who voted for what, which
//! [validators](crate::glossary#validator) cheated, which payloads have arrived,
//! and a clock. Every handler in this module folds new information into the
//! store, and head selection reads it back out. Writing to the store changes
//! only this node's opinion of the head, never the shared chain.
//!
//! # Where each field is written
//!
//! - [`get_forkchoice_store`] seeds clock fields,
//!   [checkpoints](crate::glossary#checkpoint), anchor block, anchor post-state,
//!   checkpoint states, unrealized
//!   [justification](crate::glossary#justification-and-finalization), and empty
//!   vote/envelope maps.
//! - [`on_tick()`](Store::on_tick) advances `time`, resets
//!   `proposer_boost_root` at [slot](crate::glossary#slot) boundaries, and
//!   realizes pulled-up checkpoints at [epoch](crate::glossary#epoch) boundaries.
//! - [`on_block()`](Store::on_block) inserts `blocks`, `block_states`, block
//!   timeliness, PTC vote vectors, proposer boost, realized checkpoints,
//!   `unrealized_justifications`, and pulled-up checkpoint updates.
//! - [`on_attestation()`](Store::on_attestation) updates `checkpoint_states`
//!   and `latest_messages`.
//! - [`on_attester_slashing()`](Store::on_attester_slashing) updates
//!   `equivocating_indices`.
//! - [`on_execution_payload_envelope()`](Store::on_execution_payload_envelope)
//!   inserts into `payloads` after the current consensus-side envelope checks
//!   pass.
//! - [`on_payload_attestation_message()`](Store::on_payload_attestation_message)
//!   updates PTC timeliness and data-availability vote vectors.

use std::collections::{BTreeSet, HashMap};

use crate::containers::{BeaconBlock, BeaconState, Checkpoint, ExecutionPayloadEnvelope};
use crate::error::{ForkChoiceError, StoreInvariant};
use crate::primitives::{Root, Slot, ValidatorIndex};

/// The most recent vote we have accepted from a single validator.
///
/// Validators attest by naming a block and the slot they are voting for. Fork
/// choice keeps only each validator's latest such vote, since older ones are
/// superseded, and [`on_attestation()`](Store::on_attestation) writes them. The
/// `payload_present` flag records which payload branch the vote picked: for a
/// vote about an older block it means full versus empty, while a vote about a
/// same-slot block stays undecided. Scoring then counts the vote toward a
/// [`ForkChoiceNode`], not just a block root.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LatestMessage {
    /// Slot the attestation was for.
    pub slot: Slot,
    /// Attested block root.
    pub root: Root,
    /// Branch bit from `AttestationData::index`.
    ///
    /// Interpreted as empty/full only when the voted block is older than
    /// [`slot`](Self::slot). Same-slot votes resolve to pending.
    pub payload_present: bool,
}

impl LatestMessage {
    /// The payload branch this vote supports for a block at `block_slot`.
    ///
    /// A vote about an older block also chose a branch: full when
    /// `payload_present`, otherwise empty. A vote about the block's own slot has
    /// not settled the branch, so it supports the pending node.
    pub fn supported_payload_status(&self, block_slot: Slot) -> PayloadStatus {
        let supported_payload_status;
        if block_slot < self.slot {
            if self.payload_present {
                supported_payload_status = PayloadStatus::Full;
            } else {
                supported_payload_status = PayloadStatus::Empty;
            }
        } else {
            supported_payload_status = PayloadStatus::Pending;
        }
        supported_payload_status
    }
}

/// Which payload branch a block sits on in the fork-choice tree.
///
/// Because a block's payload can be present, skipped, or not yet decided, the
/// same block can appear in the tree as up to three different nodes. This enum
/// names those three shapes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PayloadStatus {
    /// The branch that carries the chain forward without this block's payload, as
    /// if the slot produced no transactions.
    Empty,
    /// The branch that carries the chain forward on top of this block's delivered
    /// payload.
    ///
    /// Only reachable once the payload has arrived and been recorded by
    /// [`on_execution_payload_envelope()`](Store::on_execution_payload_envelope).
    Full,
    /// Not yet decided: the votes that settle empty versus full have not arrived,
    /// so the block is neither branch yet.
    Pending,
}

impl PayloadStatus {
    /// Is this an empty-or-full decision rather than still pending?
    pub const fn is_payload_decision(self) -> bool {
        matches!(self, Self::Empty | Self::Full)
    }

    /// Tie-break rank for an ordinary node: pending (2) beats full (1) beats
    /// empty (0).
    pub const fn ordinary_tiebreaker_rank(self) -> u8 {
        match self {
            Self::Empty => 0,
            Self::Full => 1,
            Self::Pending => 2,
        }
    }
}

/// One position in the fork-choice tree: a block together with its payload branch.
///
/// Fork choice does not pick between block roots alone, it picks between these
/// pairs, because a single block can be a [`Pending`](PayloadStatus::Pending)
/// node, an empty-branch node, or a full-branch node, each gathering its own
/// support. [`get_head`](Store::get_head) returns the winning pair, decided by
/// vote weight, proposer boost, and the payload-status tie-break.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ForkChoiceNode {
    /// Block root identifying the node.
    pub root: Root,
    /// Payload status associated with the block at this node.
    pub payload_status: PayloadStatus,
}

impl ForkChoiceNode {
    /// A node pairing `root` with `payload_status`.
    pub const fn new(root: Root, payload_status: PayloadStatus) -> Self {
        Self {
            root,
            payload_status,
        }
    }

    /// The block's [`Pending`](PayloadStatus::Pending) node, its empty-versus-full
    /// branch not yet decided.
    pub const fn pending(root: Root) -> Self {
        Self::new(root, PayloadStatus::Pending)
    }

    /// The block's [`Empty`](PayloadStatus::Empty) branch node, carrying the chain
    /// forward without this block's payload.
    pub const fn empty(root: Root) -> Self {
        Self::new(root, PayloadStatus::Empty)
    }

    /// The block's [`Full`](PayloadStatus::Full) branch node, carrying the chain
    /// forward on top of this block's payload.
    pub const fn full(root: Root) -> Self {
        Self::new(root, PayloadStatus::Full)
    }

    /// Is this node an empty-or-full payload decision rather than still pending?
    pub const fn is_payload_decision(self) -> bool {
        self.payload_status.is_payload_decision()
    }
}

/// Per-block payload votes, one entry per committee position, `None` until that
/// position has voted.
pub type PayloadVoteVector = Vec<Option<bool>>;

/// Whether a freshly imported block beat each of its slot's two deadlines.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BlockTimeliness {
    /// Seen before the slot's attestation deadline.
    pub attestation_timely: bool,
    /// Seen before the slot's later payload-attestation deadline.
    pub payload_attestation_timely: bool,
}

/// Everything one node needs to run fork choice.
///
/// The state transition produces the durable [`BeaconState`] after each block.
/// The store caches those states and surrounds them with this node's own
/// evidence: the current time, the latest vote from each validator, who has
/// equivocated, the payloads that have arrived, and the committee votes about
/// them. Reading the store yields a head. Writing to it changes only this node's
/// view, never the shared chain state.
#[derive(Debug, Clone)]
pub struct Store {
    /// The store's own clock, in seconds, driving message admission and deadlines.
    pub time: u64,
    /// Genesis time in seconds, the point the store's slots are counted from.
    pub genesis_time: u64,
    /// Current justified checkpoint.
    pub justified_checkpoint: Checkpoint,
    /// Current finalized checkpoint.
    pub finalized_checkpoint: Checkpoint,
    /// Unrealized justified checkpoint, ahead of the realized one.
    pub unrealized_justified_checkpoint: Checkpoint,
    /// Unrealized finalized checkpoint, ahead of the realized one.
    pub unrealized_finalized_checkpoint: Checkpoint,
    /// Root that receives the proposer boost weight.
    pub proposer_boost_root: Root,
    /// Validators whose conflicting attestations invalidate their weight.
    pub equivocating_indices: BTreeSet<ValidatorIndex>,
    /// All blocks known to the store, keyed by block root.
    pub blocks: HashMap<Root, BeaconBlock>,
    /// Post-state for each block in `blocks`.
    pub block_states: HashMap<Root, BeaconState>,
    /// Timeliness flags for each block, set when the block is imported.
    pub block_timeliness: HashMap<Root, BlockTimeliness>,
    /// Cached beacon state for each checkpoint.
    pub checkpoint_states: HashMap<Checkpoint, BeaconState>,
    /// Latest attestation message from each validator.
    pub latest_messages: HashMap<ValidatorIndex, LatestMessage>,
    /// Unrealized justification checkpoint for each block root.
    pub unrealized_justifications: HashMap<Root, Checkpoint>,
    /// Execution payloads recorded by
    /// [`on_execution_payload_envelope()`](Store::on_execution_payload_envelope)
    /// after the current envelope checks.
    pub payloads: HashMap<Root, ExecutionPayloadEnvelope>,
    /// Payload timeliness votes per block root, indexed by committee position.
    pub payload_timeliness_vote: HashMap<Root, PayloadVoteVector>,
    /// Payload data-availability votes per block root, indexed by committee position.
    pub payload_data_availability_vote: HashMap<Root, PayloadVoteVector>,
}

/// Create a fresh store anchored at a trusted starting block and state.
///
/// Fork choice has to start somewhere it trusts, called the anchor (usually a
/// recent finalized block). This checks that the anchor block and state match,
/// points the justified and finalized checkpoints at the anchor, and records the
/// anchor block and its state as the store's first entries. There are no votes
/// or payloads yet. Those arrive later through the handlers. Returns
/// [`ForkChoiceError::AnchorStateRootMismatch`] if the block and state do not
/// agree.
pub fn get_forkchoice_store(
    mut anchor_state: BeaconState,
    anchor_block: &BeaconBlock,
) -> Result<Store, ForkChoiceError> {
    use crate::constants::SLOT_DURATION_MS;
    use crate::error::MerkleError;
    use crate::state_transition::TreeRootExt as _;

    let computed_state_root = anchor_state.tree_root(MerkleError::BeaconState)?;
    if anchor_block.state_root != computed_state_root {
        return Err(ForkChoiceError::AnchorStateRootMismatch {
            got: anchor_block.state_root,
            want: computed_state_root,
        });
    }

    let mut anchor_block_clone = anchor_block.clone();
    let anchor_root = anchor_block_clone.tree_root(MerkleError::BeaconBlock)?;
    let anchor_epoch = anchor_state.slot.epoch();
    let justified = Checkpoint {
        epoch: anchor_epoch,
        root: anchor_root,
    };
    let finalized = justified;

    let time = SLOT_DURATION_MS
        .checked_mul(anchor_state.slot.as_u64())
        .and_then(|product| product.checked_div(1_000))
        .and_then(|slot_offset| anchor_state.genesis_time.checked_add(slot_offset))
        .ok_or(ForkChoiceError::AnchorTimeOverflow)?;

    let mut blocks = HashMap::new();
    blocks.insert(anchor_root, anchor_block.clone());

    let mut block_states = HashMap::new();
    block_states.insert(anchor_root, anchor_state.clone());

    let mut block_timeliness = HashMap::new();
    block_timeliness.insert(
        anchor_root,
        BlockTimeliness {
            attestation_timely: true,
            payload_attestation_timely: true,
        },
    );

    let mut checkpoint_states = HashMap::new();
    checkpoint_states.insert(justified, anchor_state.clone());

    let mut unrealized_justifications = HashMap::new();
    unrealized_justifications.insert(anchor_root, justified);

    Ok(Store {
        time,
        genesis_time: anchor_state.genesis_time,
        justified_checkpoint: justified,
        finalized_checkpoint: finalized,
        unrealized_justified_checkpoint: justified,
        unrealized_finalized_checkpoint: finalized,
        proposer_boost_root: Root::ZERO,
        equivocating_indices: BTreeSet::new(),
        blocks,
        block_states,
        block_timeliness,
        checkpoint_states,
        latest_messages: HashMap::new(),
        unrealized_justifications,
        payloads: HashMap::new(),
        payload_timeliness_vote: HashMap::new(),
        payload_data_availability_vote: HashMap::new(),
    })
}

impl Store {
    /// Check the store's structural invariants, intended for use in tests.
    ///
    /// Returns the first [`StoreInvariant`] found, or `Ok(())` when the store is
    /// well formed. Covers: every block has a post-state and timeliness flags,
    /// every payload vote vector has `PTC_SIZE` entries, and every block whose
    /// parent is in the store sits in a later slot than that parent.
    pub fn check_invariants(&self) -> Result<(), StoreInvariant> {
        for root in self.blocks.keys() {
            if !self.block_states.contains_key(root) {
                return Err(StoreInvariant::MissingBlockState(*root));
            }
            if !self.block_timeliness.contains_key(root) {
                return Err(StoreInvariant::MissingTimeliness(*root));
            }
        }
        for (root, votes) in &self.payload_timeliness_vote {
            if votes.len() != crate::constants::PTC_SIZE {
                return Err(StoreInvariant::TimelinessVotesNotPtcSize(*root));
            }
        }
        for (root, votes) in &self.payload_data_availability_vote {
            if votes.len() != crate::constants::PTC_SIZE {
                return Err(StoreInvariant::DataAvailabilityVotesNotPtcSize(*root));
            }
        }
        for (root, block) in &self.blocks {
            let parent_out_of_order = self
                .blocks
                .get(&block.parent_root)
                .is_some_and(|parent| parent.slot >= block.slot);
            if parent_out_of_order {
                return Err(StoreInvariant::NonDecreasingParentSlot(*root));
            }
        }
        Ok(())
    }
}
