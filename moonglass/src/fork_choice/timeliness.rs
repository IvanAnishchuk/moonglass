//! Spec: `record_block_timeliness`, `update_proposer_boost_root`.

use crate::constants::{ATTESTATION_TIMELINESS_INDEX, SLOT_DURATION_MS};
use crate::error::ForkChoiceError;
use crate::primitives::Root;

use super::helpers::{
    get_attestation_due_ms, get_current_slot, get_dependent_root, get_payload_attestation_due_ms,
    seconds_to_milliseconds,
};
use super::store::Store;

pub(crate) fn record_block_timeliness(
    store: &mut Store,
    root: Root,
) -> Result<(), ForkChoiceError> {
    let block = store
        .blocks
        .get(&root)
        .ok_or(ForkChoiceError::UnknownBlock(root))?;
    let seconds_since_genesis = store.time.saturating_sub(store.genesis_time);
    let time_into_slot_ms = seconds_to_milliseconds(seconds_since_genesis) % SLOT_DURATION_MS;
    let is_current_slot = get_current_slot(store) == block.slot;
    let attestation_threshold = get_attestation_due_ms();
    let ptc_threshold = get_payload_attestation_due_ms();
    let timeliness = [
        is_current_slot && time_into_slot_ms < attestation_threshold,
        is_current_slot && time_into_slot_ms < ptc_threshold,
    ];
    store.block_timeliness.insert(root, timeliness);
    Ok(())
}

pub(crate) fn update_proposer_boost_root(
    store: &mut Store,
    head: Root,
    root: Root,
) -> Result<(), ForkChoiceError> {
    let is_first_block = store.proposer_boost_root == Root::default();
    let timeliness = store
        .block_timeliness
        .get(&root)
        .ok_or(ForkChoiceError::UnknownBlock(root))?;
    let is_timely = timeliness[ATTESTATION_TIMELINESS_INDEX];
    let dependent_root = get_dependent_root(store, root)?;
    let head_dependent_root = get_dependent_root(store, head)?;
    let is_same_dependent_root = dependent_root == head_dependent_root;
    if is_timely && is_first_block && is_same_dependent_root {
        store.proposer_boost_root = root;
    }
    Ok(())
}
