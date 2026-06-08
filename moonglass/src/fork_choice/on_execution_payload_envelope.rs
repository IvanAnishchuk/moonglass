//! Spec: `on_execution_payload_envelope`.
//!
//! Calls into `BeaconState::process_execution_payload` for the consensus-side
//! checks (signature, bid match, randao, gas, hash, requests-root, slot,
//! timestamp, withdrawals). Does not insert into `store.payloads` yet; see
//! the note at the top of `fork_choice.rs`.

use crate::containers::SignedExecutionPayloadEnvelope;
use crate::error::ForkChoiceError;

use super::store::Store;

/// Verify a builder-delivered execution payload envelope against the stored
/// post-state for its beacon block.
pub fn on_execution_payload_envelope(
    store: &mut Store,
    signed_envelope: &SignedExecutionPayloadEnvelope,
) -> Result<(), ForkChoiceError> {
    let beacon_block_root = signed_envelope.message.beacon_block_root;
    if !store.block_states.contains_key(&beacon_block_root) {
        return Err(ForkChoiceError::PayloadEnvelopeForUnknownBlock(
            beacon_block_root,
        ));
    }
    let mut state = store
        .block_states
        .get(&beacon_block_root)
        .ok_or(ForkChoiceError::PayloadEnvelopeForUnknownBlock(
            beacon_block_root,
        ))?
        .clone();
    state.process_execution_payload(signed_envelope)?;
    Ok(())
}
