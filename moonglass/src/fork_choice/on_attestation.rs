//! Spec: attestation handlers.

use crate::containers::Attestation;
use crate::error::{ForkChoiceError, SignatureError};
use crate::primitives::ValidatorIndex;

use super::checkpoints::store_target_checkpoint_state;
use super::helpers::{get_checkpoint_block, get_current_slot, get_current_store_epoch};
use super::payload_status::is_payload_verified;
use super::store::{LatestMessage, Store};

pub(crate) fn validate_target_epoch_against_current_time(
    store: &Store,
    attestation: &Attestation,
) -> Result<(), ForkChoiceError> {
    let target_epoch = attestation.data.target.epoch.as_u64();
    let current_epoch = get_current_store_epoch(store).as_u64();
    let previous_epoch = current_epoch.saturating_sub(1);
    if target_epoch != current_epoch && target_epoch != previous_epoch {
        return Err(ForkChoiceError::AttestationFromFutureEpoch);
    }
    Ok(())
}

pub(crate) fn validate_on_attestation(
    store: &Store,
    attestation: &Attestation,
    is_from_block: bool,
) -> Result<(), ForkChoiceError> {
    if !is_from_block {
        validate_target_epoch_against_current_time(store, attestation)?;
    }

    if attestation.data.target.epoch != attestation.data.slot.epoch() {
        return Err(ForkChoiceError::AttestationLmdFfgMismatch);
    }

    if !store.blocks.contains_key(&attestation.data.target.root) {
        return Err(ForkChoiceError::UnknownBlock(attestation.data.target.root));
    }
    if !store
        .blocks
        .contains_key(&attestation.data.beacon_block_root)
    {
        return Err(ForkChoiceError::UnknownBlock(
            attestation.data.beacon_block_root,
        ));
    }
    let block_slot = store
        .blocks
        .get(&attestation.data.beacon_block_root)
        .ok_or(ForkChoiceError::UnknownBlock(
            attestation.data.beacon_block_root,
        ))?
        .slot;
    if block_slot > attestation.data.slot {
        return Err(ForkChoiceError::AttestationTooEarly);
    }

    let index = attestation.data.index.as_u64();
    if index != 0 && index != 1 {
        return Err(ForkChoiceError::AttestationIndexInvalid(index));
    }
    if block_slot == attestation.data.slot && index != 0 {
        return Err(ForkChoiceError::AttestationIndexInvalid(index));
    }
    if index == 1 && !is_payload_verified(store, attestation.data.beacon_block_root) {
        return Err(ForkChoiceError::AttestationPayloadNotVerified);
    }

    let checkpoint_root = get_checkpoint_block(
        store,
        attestation.data.beacon_block_root,
        attestation.data.target.epoch,
    )?;
    if attestation.data.target.root != checkpoint_root {
        return Err(ForkChoiceError::AttestationLmdFfgMismatch);
    }

    if get_current_slot(store).as_u64() < attestation.data.slot.as_u64() + 1 {
        return Err(ForkChoiceError::AttestationTooEarly);
    }

    Ok(())
}

fn update_latest_messages(
    store: &mut Store,
    attesting_indices: &[ValidatorIndex],
    attestation: &Attestation,
) {
    let slot = attestation.data.slot;
    let root = attestation.data.beacon_block_root;
    let payload_present = attestation.data.index.as_u64() == 1;
    for index in attesting_indices {
        if store.equivocating_indices.contains(index) {
            continue;
        }
        let should_write = match store.latest_messages.get(index) {
            Some(existing) => slot.as_u64() > existing.slot.as_u64(),
            None => true,
        };
        if should_write {
            store.latest_messages.insert(
                *index,
                LatestMessage {
                    slot,
                    root,
                    payload_present,
                },
            );
        }
    }
}

/// Validate `attestation`, snapshot the target-checkpoint state, verify the
/// signature, then record each attester's latest message.
///
/// Spec: `on_attestation`.
pub fn on_attestation(
    store: &mut Store,
    attestation: &Attestation,
    is_from_block: bool,
) -> Result<(), ForkChoiceError> {
    validate_on_attestation(store, attestation, is_from_block)?;
    store_target_checkpoint_state(store, attestation.data.target)?;
    let target_state = store
        .checkpoint_states
        .get(&attestation.data.target)
        .ok_or(ForkChoiceError::UnknownBlock(attestation.data.target.root))?
        .clone();
    let indexed = target_state.indexed_attestation(attestation)?;
    target_state.validate_indexed_attestation(&indexed, SignatureError::Attestation)?;
    update_latest_messages(store, &indexed.attesting_indices, attestation);
    Ok(())
}
