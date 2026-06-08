//! Spec: `on_tick`, `on_tick_per_slot`.

use crate::constants::SLOT_DURATION_MS;
use crate::error::ForkChoiceError;
use crate::primitives::Root;

use super::checkpoints::update_checkpoints;
use super::helpers::{compute_slots_since_epoch_start, get_current_slot, get_slots_since_genesis};
use super::store::Store;

/// Advance the store's clock to `time`, replaying one synthetic tick per
/// crossed slot boundary so checkpoint updates and proposer-boost resets fire
/// at the correct slot.
pub fn on_tick(store: &mut Store, time: u64) -> Result<(), ForkChoiceError> {
    if time < store.time {
        return Err(ForkChoiceError::TickWentBackwards {
            from: store.time,
            to: time,
        });
    }
    let tick_slot = (time - store.genesis_time) * 1_000 / SLOT_DURATION_MS;
    while get_slots_since_genesis(store) < tick_slot {
        let next_slot = get_current_slot(store).as_u64() + 1;
        let previous_time = store.genesis_time + next_slot * SLOT_DURATION_MS / 1_000;
        on_tick_per_slot(store, previous_time);
    }
    on_tick_per_slot(store, time);
    Ok(())
}

fn on_tick_per_slot(store: &mut Store, time: u64) {
    let previous_slot = get_current_slot(store);
    store.time = time;
    let current = get_current_slot(store);

    if current > previous_slot {
        store.proposer_boost_root = Root::default();
    }

    if current > previous_slot && compute_slots_since_epoch_start(current) == 0 {
        update_checkpoints(
            store,
            store.unrealized_justified_checkpoint,
            store.unrealized_finalized_checkpoint,
        );
    }
}
