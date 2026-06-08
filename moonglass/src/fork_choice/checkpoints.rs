//! Spec: `update_checkpoints`, `update_unrealized_checkpoints`,
//! `compute_pulled_up_tip`.

use crate::containers::Checkpoint;
use crate::error::ForkChoiceError;
use crate::primitives::{Root, Slot};

use super::store::Store;

/// Spec: `update_checkpoints`.
pub(crate) fn update_checkpoints(store: &mut Store, justified: Checkpoint, finalized: Checkpoint) {
    if justified.epoch > store.justified_checkpoint.epoch {
        store.justified_checkpoint = justified;
    }
    if finalized.epoch > store.finalized_checkpoint.epoch {
        store.finalized_checkpoint = finalized;
    }
}

/// Spec: `store_target_checkpoint_state`.
pub(super) fn store_target_checkpoint_state(
    store: &mut Store,
    target: Checkpoint,
) -> Result<(), ForkChoiceError> {
    if store.checkpoint_states.contains_key(&target) {
        return Ok(());
    }
    let mut state = store
        .block_states
        .get(&target.root)
        .ok_or(ForkChoiceError::UnknownBlock(target.root))?
        .clone();
    let slots_per_epoch = u64::try_from(crate::constants::SLOTS_PER_EPOCH).unwrap_or(u64::MAX);
    let target_slot = Slot::new(target.epoch.as_u64() * slots_per_epoch);
    if state.slot < target_slot {
        state.process_slots(target_slot)?;
    }
    store.checkpoint_states.insert(target, state);
    Ok(())
}

pub(crate) fn update_unrealized_checkpoints(
    store: &mut Store,
    unrealized_justified: Checkpoint,
    unrealized_finalized: Checkpoint,
) {
    if unrealized_justified.epoch > store.unrealized_justified_checkpoint.epoch {
        store.unrealized_justified_checkpoint = unrealized_justified;
    }
    if unrealized_finalized.epoch > store.unrealized_finalized_checkpoint.epoch {
        store.unrealized_finalized_checkpoint = unrealized_finalized;
    }
}

/// Computes pulled-up justification and finalization for `block_root`.
///
/// The stored post-state is cloned because `process_justification_and_finalization`
/// mutates the state to derive the pulled-up checkpoints. The original state in
/// the store must remain untouched so other lookups against `block_root` continue
/// to see the canonical post-state.
pub(crate) fn compute_pulled_up_tip(
    store: &mut Store,
    block_root: Root,
) -> Result<(), ForkChoiceError> {
    let mut pulled_up_state = store
        .block_states
        .get(&block_root)
        .ok_or(ForkChoiceError::UnknownBlock(block_root))?
        .clone();
    pulled_up_state.process_justification_and_finalization()?;
    store
        .unrealized_justifications
        .insert(block_root, pulled_up_state.current_justified_checkpoint);
    update_unrealized_checkpoints(
        store,
        pulled_up_state.current_justified_checkpoint,
        pulled_up_state.finalized_checkpoint,
    );

    let block_epoch = store
        .blocks
        .get(&block_root)
        .ok_or(ForkChoiceError::UnknownBlock(block_root))?
        .slot
        .epoch();
    let current_epoch = super::helpers::get_current_store_epoch(store);
    if block_epoch < current_epoch {
        update_checkpoints(
            store,
            pulled_up_state.current_justified_checkpoint,
            pulled_up_state.finalized_checkpoint,
        );
    }
    Ok(())
}
