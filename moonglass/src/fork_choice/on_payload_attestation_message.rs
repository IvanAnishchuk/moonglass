//! Taking in a payload-timeliness [committee](crate::glossary#committee) (PTC)
//! vote.
//!
//! Each [slot](crate::glossary#slot) a committee watches for the
//! [block's](crate::glossary#beacon-block) payload and votes on whether it
//! arrived on time with its data available. A gossip
//! [`PayloadAttestationMessage`] names a single
//! [validator](crate::glossary#validator), but the store records
//! votes by committee *position*, because one validator can hold several
//! positions. So this module's job is to find every position that validator
//! occupies and write its vote into each.

use crate::constants::PTC_SIZE;
use crate::containers::{BeaconState, IndexedPayloadAttestation, PayloadAttestationMessage};
use crate::error::ForkChoiceError;
use crate::primitives::{Root, Slot, ValidatorIndex};

use super::store::Store;

impl Store {
    /// Record one validator's PTC vote about a block's payload.
    ///
    /// The vote only counts for the block from its own slot, so if the slots do
    /// not match the handler quietly does nothing. For a gossip vote it also
    /// insists the vote is for the current slot and checks the validator's
    /// signature. A vote that arrived inside a block has already been checked by
    /// the state transition. It then writes the validator's "payload present" and
    /// "data available" answers into the timeliness and data-availability vote
    /// vectors, at each committee position the validator holds.
    ///
    /// Runs on a scratch copy and commits only on success, so a rejected vote
    /// leaves the store unchanged.
    pub fn on_payload_attestation_message(
        &mut self,
        ptc_message: &PayloadAttestationMessage,
        is_from_block: bool,
    ) -> Result<(), ForkChoiceError> {
        let mut scratch = self.clone();
        scratch.on_payload_attestation_message_inner(ptc_message, is_from_block)?;
        *self = scratch;
        Ok(())
    }

    /// Non-transactional body of [`Self::on_payload_attestation_message`], used by
    /// the block-import path so it does not clone the store for every vote.
    pub fn on_payload_attestation_message_inner(
        &mut self,
        ptc_message: &PayloadAttestationMessage,
        is_from_block: bool,
    ) -> Result<(), ForkChoiceError> {
        let data = ptc_message.data;

        // Resolve the validator's PTC positions against the targeted block's state
        // inside this scope, so the borrow of the store ends before the
        // vote-vector mutation below. This avoids cloning the whole `BeaconState`
        // just to outlive the recording step.
        let ptc_positions = {
            let state = self.block_states.get(&data.beacon_block_root).ok_or(
                ForkChoiceError::PayloadAttestationForUnknownBlock(data.beacon_block_root),
            )?;

            // A vote can only change its assigned beacon block, so the vote's slot
            // must equal the targeted block's slot.
            if data.slot != state.slot {
                return Ok(());
            }

            let ptc_positions =
                ptc_positions_for_validator(state, data.slot, ptc_message.validator_index)?;

            // A gossip message must be for the current slot and carry a valid
            // signature. A block-embedded message has already passed both in the
            // state transition.
            if !is_from_block {
                if data.slot != self.get_current_slot() {
                    return Err(ForkChoiceError::PayloadAttestationWrongSlot);
                }
                let indexed = build_single_indexed(ptc_message);
                state.validate_indexed_payload_attestation(&indexed)?;
            }

            ptc_positions
        };

        self.record_payload_vote_positions(
            data.beacon_block_root,
            &ptc_positions,
            data.payload_present,
            data.blob_data_available,
        )
    }

    /// Write the "payload present" and "data available" votes into given
    /// positions.
    ///
    /// The low-level write step: for each committee position, set both vote
    /// vectors for the block. Gossip votes reach here after a validator is
    /// expanded into its positions ([`ptc_positions_for_validator`]), and
    /// block-bundled ones after the state transition validated them as a group.
    /// Either way this only touches local fork-choice evidence, never the chain
    /// state.
    pub fn record_payload_vote_positions(
        &mut self,
        block_root: Root,
        positions: &[usize],
        payload_present: bool,
        blob_data_available: bool,
    ) -> Result<(), ForkChoiceError> {
        // Resolve both vote vectors before writing either. A known timeliness
        // vector paired with a missing data-availability vector is a broken store
        // invariant, and fetching both up front keeps that error from leaving one
        // vector half-updated.
        let timeliness_votes = self.payload_timeliness_vote.get_mut(&block_root).ok_or(
            ForkChoiceError::PayloadAttestationForUnknownBlock(block_root),
        )?;
        let data_availability_votes = self
            .payload_data_availability_vote
            .get_mut(&block_root)
            .ok_or(ForkChoiceError::PayloadAttestationForUnknownBlock(
                block_root,
            ))?;
        write_vote_positions(timeliness_votes, positions, payload_present)?;
        write_vote_positions(data_availability_votes, positions, blob_data_available)?;
        Ok(())
    }
}

/// Find every committee position `validator_index` holds for `slot`.
///
/// Committee membership is by position, and the same validator can fill more
/// than one. Since a gossip vote names only the validator, we look up the slot's
/// committee and collect every position that is them, so the vote can be
/// recorded in each.
pub fn ptc_positions_for_validator(
    state: &BeaconState,
    slot: Slot,
    validator_index: ValidatorIndex,
) -> Result<Vec<usize>, ForkChoiceError> {
    let bucket_index = state
        .ptc_window_index_for_slot(slot)
        .ok_or(ForkChoiceError::PayloadAttestationSlotOutOfWindow)?;
    let ptc = state
        .ptc_window
        .get(bucket_index)
        .ok_or(ForkChoiceError::PayloadAttestationSlotOutOfWindow)?;

    let positions: Vec<usize> = ptc
        .iter()
        .enumerate()
        .filter_map(|(ptc_index, ptc_validator_index)| {
            (*ptc_validator_index == validator_index).then_some(ptc_index)
        })
        .collect();
    if positions.is_empty() {
        return Err(ForkChoiceError::PayloadAttestationValidatorNotInPtc);
    }
    Ok(positions)
}

/// Set `value` at each committee position in `positions` within one vote vector.
///
/// Every committee position must exist in the vector, so a position past its end
/// is a broken store invariant and returns
/// [`ForkChoiceError::PayloadVoteIndexOutOfBounds`] rather than being skipped. The
/// same position list drives both the timeliness and data-availability vectors.
pub fn write_vote_positions(
    votes: &mut [Option<bool>],
    positions: &[usize],
    value: bool,
) -> Result<(), ForkChoiceError> {
    for &index in positions {
        let cell = votes
            .get_mut(index)
            .ok_or(ForkChoiceError::PayloadVoteIndexOutOfBounds(index))?;
        *cell = Some(value);
    }
    Ok(())
}

/// Wrap a gossip vote as a one-validator indexed attestation for the signature
/// check.
///
/// Only the gossip path needs this. A vote that arrived inside a block was
/// already verified as part of the block.
pub fn build_single_indexed(msg: &PayloadAttestationMessage) -> IndexedPayloadAttestation {
    let mut indices = ssz_rs::List::<ValidatorIndex, PTC_SIZE>::default();
    indices.push(msg.validator_index);
    IndexedPayloadAttestation {
        attesting_indices: indices,
        data: msg.data,
        signature: msg.signature,
    }
}
