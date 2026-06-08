//! Spec: `on_payload_attestation_message`.

use crate::constants::PTC_SIZE;
use crate::containers::{IndexedPayloadAttestation, PayloadAttestationMessage};
use crate::error::ForkChoiceError;
use crate::primitives::ValidatorIndex;

use super::helpers::get_current_slot;
use super::store::Store;

/// Record a payload-timeliness committee vote for the targeted block, after
/// confirming the validator sits in the PTC for that slot and (off-block only)
/// the message is for the current slot.
///
/// Spec: `on_payload_attestation_message`.
pub fn on_payload_attestation_message(
    store: &mut Store,
    ptc_message: &PayloadAttestationMessage,
    is_from_block: bool,
) -> Result<(), ForkChoiceError> {
    let data = ptc_message.data;

    if !store.block_states.contains_key(&data.beacon_block_root) {
        return Err(ForkChoiceError::PayloadAttestationForUnknownBlock(
            data.beacon_block_root,
        ));
    }
    let state = store
        .block_states
        .get(&data.beacon_block_root)
        .ok_or(ForkChoiceError::PayloadAttestationForUnknownBlock(
            data.beacon_block_root,
        ))?
        .clone();

    if data.slot != state.slot {
        return Ok(());
    }

    let bucket_index = state
        .ptc_window_index_for_slot(data.slot)
        .ok_or(ForkChoiceError::PayloadAttestationSlotOutOfWindow)?;
    let ptc = &state.ptc_window[bucket_index];

    let mut ptc_indices: Vec<usize> = Vec::new();
    for (ptc_index, validator_index) in ptc.iter().enumerate() {
        if *validator_index == ptc_message.validator_index {
            ptc_indices.push(ptc_index);
        }
    }
    if ptc_indices.is_empty() {
        return Err(ForkChoiceError::PayloadAttestationValidatorNotInPtc);
    }

    if !is_from_block {
        if data.slot != get_current_slot(store) {
            return Err(ForkChoiceError::PayloadAttestationWrongSlot);
        }
        let indexed = build_single_indexed(ptc_message);
        state.validate_indexed_payload_attestation(&indexed)?;
    }

    let timeliness_votes = store
        .payload_timeliness_vote
        .get_mut(&data.beacon_block_root)
        .ok_or(ForkChoiceError::PayloadAttestationForUnknownBlock(
            data.beacon_block_root,
        ))?;
    for ptc_index in &ptc_indices {
        if let Some(slot) = timeliness_votes.get_mut(*ptc_index) {
            *slot = Some(data.payload_present);
        }
    }

    let availability_votes = store
        .payload_data_availability_vote
        .get_mut(&data.beacon_block_root)
        .ok_or(ForkChoiceError::PayloadAttestationForUnknownBlock(
            data.beacon_block_root,
        ))?;
    for ptc_index in &ptc_indices {
        if let Some(slot) = availability_votes.get_mut(*ptc_index) {
            *slot = Some(data.blob_data_available);
        }
    }

    Ok(())
}

fn build_single_indexed(msg: &PayloadAttestationMessage) -> IndexedPayloadAttestation {
    let mut indices = ssz_rs::List::<ValidatorIndex, PTC_SIZE>::default();
    indices.push(msg.validator_index);
    IndexedPayloadAttestation {
        attesting_indices: indices,
        data: msg.data,
        signature: msg.signature,
    }
}
