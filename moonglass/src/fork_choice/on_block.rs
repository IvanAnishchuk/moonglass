//! Spec: `on_block`, `notify_ptc_messages`.

use crate::constants::PTC_SIZE;
use crate::containers::{
    BeaconState, PayloadAttestation, PayloadAttestationMessage, SignedBeaconBlock,
};
use crate::error::{ForkChoiceError, MerkleError};
use crate::primitives::{BLSSignature, Slot};
use crate::state_transition::TreeRootExt as _;

use super::checkpoints::{compute_pulled_up_tip, update_checkpoints};
use super::head::get_head;
use super::helpers::{get_checkpoint_block, get_current_slot};
use super::on_payload_attestation_message::on_payload_attestation_message;
use super::payload_status::{is_parent_node_full, is_payload_verified};
use super::store::Store;
use super::timeliness::{record_block_timeliness, update_proposer_boost_root};

/// Validate `signed_block` against fork-choice rules, apply the state
/// transition, snapshot the post-state in the store, notify the PTC for
/// block-embedded payload attestations, then advance proposer-boost and
/// realized/unrealized checkpoints.
///
/// Spec: `on_block`. Block-embedded attestations and attester slashings are
/// handled by separate network-message handlers, not by `on_block`.
pub fn on_block(
    store: &mut Store,
    signed_block: &SignedBeaconBlock,
) -> Result<(), ForkChoiceError> {
    let block = &signed_block.message;

    if !store.block_states.contains_key(&block.parent_root) {
        return Err(ForkChoiceError::UnknownParent(block.parent_root));
    }

    if is_parent_node_full(store, block)? && !is_payload_verified(store, block.parent_root) {
        return Err(ForkChoiceError::PayloadParentNotVerified(block.parent_root));
    }

    let current = get_current_slot(store);
    if current < block.slot {
        return Err(ForkChoiceError::BlockFromFuture {
            block_slot: block.slot,
            current_slot: current,
        });
    }

    let slots_per_epoch = u64::try_from(crate::constants::SLOTS_PER_EPOCH).unwrap_or(u64::MAX);
    let finalized_slot = Slot::new(store.finalized_checkpoint.epoch.as_u64() * slots_per_epoch);
    if block.slot <= finalized_slot {
        return Err(ForkChoiceError::BlockBeforeFinalizedSlot {
            block_slot: block.slot,
            finalized_slot,
        });
    }

    let finalized_checkpoint_block =
        get_checkpoint_block(store, block.parent_root, store.finalized_checkpoint.epoch)?;
    if store.finalized_checkpoint.root != finalized_checkpoint_block {
        return Err(ForkChoiceError::BlockNotDescendedFromFinalized);
    }

    let mut state = store
        .block_states
        .get(&block.parent_root)
        .ok_or(ForkChoiceError::UnknownParent(block.parent_root))?
        .clone();

    let mut block_clone = block.clone();
    let block_root = block_clone.tree_root(MerkleError::BeaconBlock)?;
    state.apply_signed_block(signed_block)?;

    let head = get_head(store)?;
    store.blocks.insert(block_root, block.clone());
    store.block_states.insert(block_root, state.clone());
    store
        .payload_timeliness_vote
        .insert(block_root, vec![None; PTC_SIZE]);
    store
        .payload_data_availability_vote
        .insert(block_root, vec![None; PTC_SIZE]);

    let payload_attestations: Vec<PayloadAttestation> =
        block.body.payload_attestations.iter().cloned().collect();
    notify_ptc_messages(store, &state, &payload_attestations)?;

    record_block_timeliness(store, block_root)?;
    update_proposer_boost_root(store, head.root, block_root)?;

    update_checkpoints(
        store,
        state.current_justified_checkpoint,
        state.finalized_checkpoint,
    );
    compute_pulled_up_tip(store, block_root)?;

    Ok(())
}

fn notify_ptc_messages(
    store: &mut Store,
    state: &BeaconState,
    payload_attestations: &[PayloadAttestation],
) -> Result<(), ForkChoiceError> {
    if state.slot.as_u64() == 0 {
        return Ok(());
    }
    for attestation in payload_attestations {
        let indexed = state.indexed_payload_attestation(attestation.data.slot, attestation)?;
        for &validator_index in indexed.attesting_indices.iter() {
            let message = PayloadAttestationMessage {
                validator_index,
                data: attestation.data,
                signature: BLSSignature::default(),
            };
            on_payload_attestation_message(store, &message, true)?;
        }
    }
    Ok(())
}
