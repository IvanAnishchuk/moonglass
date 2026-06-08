//! Spec types and the store constructor.
//!
//! Field names match the consensus-specs `Store`.

use std::collections::{BTreeSet, HashMap};

use crate::containers::{BeaconBlock, BeaconState, Checkpoint, ExecutionPayloadEnvelope};
use crate::error::ForkChoiceError;
use crate::primitives::{Root, Slot, ValidatorIndex};

/// Most recent attestation message from a validator.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LatestMessage {
    /// Slot the attestation was for.
    pub slot: Slot,
    /// Attested block root.
    pub root: Root,
    /// Whether the validator also attested to a payload being present.
    pub payload_present: bool,
}

/// Execution-payload availability status for a fork-choice node.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PayloadStatus {
    /// No payload has been revealed for the block.
    Empty,
    /// A full payload has been revealed and verified.
    Full,
    /// A payload has been revealed but is awaiting verification.
    Pending,
}

/// A node in the fork-choice tree, identified by block root and payload status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ForkChoiceNode {
    /// Block root identifying the node.
    pub root: Root,
    /// Payload status associated with the block at this node.
    pub payload_status: PayloadStatus,
}

/// Fork-choice store holding all state needed to run the fork-choice rule.
#[derive(Debug, Clone)]
pub struct Store {
    /// Current consensus time.
    pub time: u64,
    /// Genesis consensus time.
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
    /// Timeliness flags for each block: `[consensus_timely, payload_timely]`.
    pub block_timeliness: HashMap<Root, [bool; 2]>,
    /// Cached beacon state for each checkpoint.
    pub checkpoint_states: HashMap<Checkpoint, BeaconState>,
    /// Latest attestation message from each validator.
    pub latest_messages: HashMap<ValidatorIndex, LatestMessage>,
    /// Unrealized justification checkpoint for each block root.
    pub unrealized_justifications: HashMap<Root, Checkpoint>,
    /// Execution payloads whose full availability/validity has been verified.
    /// Not yet populated; see the note at the top of `fork_choice.rs`.
    pub payloads: HashMap<Root, ExecutionPayloadEnvelope>,
    /// Payload timeliness votes per block root, indexed by committee position.
    pub payload_timeliness_vote: HashMap<Root, Vec<Option<bool>>>,
    /// Payload data-availability votes per block root, indexed by committee position.
    pub payload_data_availability_vote: HashMap<Root, Vec<Option<bool>>>,
}

/// Spec: `get_forkchoice_store`.
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
    block_timeliness.insert(anchor_root, [true, true]);

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
        proposer_boost_root: Root::default(),
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
