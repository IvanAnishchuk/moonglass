//! Spec: payload-status helpers.

use crate::containers::BeaconBlock;
use crate::error::ForkChoiceError;
use crate::primitives::Root;

use super::store::{ForkChoiceNode, PayloadStatus, Store};

pub fn get_parent_payload_status(
    store: &Store,
    block: &BeaconBlock,
) -> Result<PayloadStatus, ForkChoiceError> {
    let parent = store
        .blocks
        .get(&block.parent_root)
        .ok_or(ForkChoiceError::UnknownParent(block.parent_root))?;
    let parent_block_hash = block
        .body
        .signed_execution_payload_bid
        .message
        .parent_block_hash;
    let message_block_hash = parent.body.signed_execution_payload_bid.message.block_hash;
    Ok(if parent_block_hash == message_block_hash {
        PayloadStatus::Full
    } else {
        PayloadStatus::Empty
    })
}

pub(crate) fn is_parent_node_full(
    store: &Store,
    block: &BeaconBlock,
) -> Result<bool, ForkChoiceError> {
    Ok(get_parent_payload_status(store, block)? == PayloadStatus::Full)
}

/// True iff the block's payload has been recorded as verified. Returns
/// `false` for every root until the payload-verification gap is filled; see
/// the note at the top of `fork_choice.rs`.
pub(crate) fn is_payload_verified(store: &Store, root: Root) -> bool {
    store.payloads.contains_key(&root)
}

pub(crate) fn payload_timeliness(
    store: &Store,
    root: Root,
    timely: bool,
) -> Result<bool, ForkChoiceError> {
    let votes = store
        .payload_timeliness_vote
        .get(&root)
        .ok_or(ForkChoiceError::UnknownBlock(root))?;
    if !is_payload_verified(store, root) {
        return Ok(!timely);
    }
    let matching = votes.iter().filter(|v| **v == Some(timely)).count();
    let matching = u64::try_from(matching).unwrap_or(u64::MAX);
    Ok(matching > crate::constants::PAYLOAD_TIMELY_THRESHOLD)
}

pub(crate) fn payload_data_availability(
    store: &Store,
    root: Root,
    available: bool,
) -> Result<bool, ForkChoiceError> {
    let votes = store
        .payload_data_availability_vote
        .get(&root)
        .ok_or(ForkChoiceError::UnknownBlock(root))?;
    if !is_payload_verified(store, root) {
        return Ok(!available);
    }
    let matching = votes.iter().filter(|v| **v == Some(available)).count();
    let matching = u64::try_from(matching).unwrap_or(u64::MAX);
    Ok(matching > crate::constants::DATA_AVAILABILITY_TIMELY_THRESHOLD)
}

pub(crate) fn is_previous_slot_payload_decision(
    store: &Store,
    node: ForkChoiceNode,
) -> Result<bool, ForkChoiceError> {
    use super::helpers::get_current_slot;
    let block_slot = store
        .blocks
        .get(&node.root)
        .ok_or(ForkChoiceError::UnknownBlock(node.root))?
        .slot;
    let is_previous_slot = block_slot.as_u64() + 1 == get_current_slot(store).as_u64();
    let is_payload_decision = matches!(
        node.payload_status,
        PayloadStatus::Empty | PayloadStatus::Full
    );
    Ok(is_previous_slot && is_payload_decision)
}

pub(crate) fn should_extend_payload(store: &Store, root: Root) -> Result<bool, ForkChoiceError> {
    if !is_payload_verified(store, root) {
        return Ok(false);
    }
    let proposer_root = store.proposer_boost_root;
    let payload_is_timely = payload_timeliness(store, root, true)?;
    let payload_data_is_available = payload_data_availability(store, root, true)?;
    if payload_is_timely && payload_data_is_available {
        return Ok(true);
    }
    if proposer_root == crate::primitives::Root::default() {
        return Ok(true);
    }
    let proposer_block = store
        .blocks
        .get(&proposer_root)
        .ok_or(ForkChoiceError::UnknownBlock(proposer_root))?;
    if proposer_block.parent_root != root {
        return Ok(true);
    }
    is_parent_node_full(store, proposer_block)
}

pub(crate) fn get_payload_status_tiebreaker(
    store: &Store,
    node: ForkChoiceNode,
) -> Result<u8, ForkChoiceError> {
    if is_previous_slot_payload_decision(store, node)? {
        if node.payload_status == PayloadStatus::Empty {
            return Ok(1);
        }
        if should_extend_payload(store, node.root)? {
            return Ok(2);
        }
        return Ok(0);
    }
    Ok(match node.payload_status {
        PayloadStatus::Empty => 0,
        PayloadStatus::Full => 1,
        PayloadStatus::Pending => 2,
    })
}
