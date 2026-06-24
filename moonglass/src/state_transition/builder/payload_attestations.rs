//! Payload-attestation validation during block application.
//!
//! A block carries aggregate PTC votes about the previous slot's payload. State
//! transition validates the aggregate signature and that the vote targets the
//! parent slot, but it does not write fork-choice vote vectors. After
//! `on_block` stores the post-state, fork choice replays these validated
//! aggregates into local PTC vote maps by committee position.

use crate::constants::{DOMAIN_PTC_ATTESTER, PTC_SIZE, SLOTS_PER_EPOCH};
use crate::containers::{BeaconState, IndexedPayloadAttestation, PayloadAttestation};
use crate::error::{MerkleError, OperationError, SignatureError, TransitionError};
use crate::primitives::{BLSPubkey, Slot, ValidatorIndex};
use crate::state_transition::{BeaconStateLookup, compute_signing_root, fast_aggregate_verify};

impl BeaconState {
    /// Expand a payload attestation's bitfield into its sorted attesting set.
    ///
    /// The set bits in `aggregation_bits` are matched against the payload-timeliness
    /// committee assigned to `slot`, and the selected validator indices are
    /// sorted into an [`IndexedPayloadAttestation`] carrying the original data
    /// and signature. A slot with no live committee assignment raises
    /// [`OperationError::PayloadAttestationSlotMismatch`]. The sorted indices may
    /// repeat, since one validator can hold several committee positions.
    pub fn indexed_payload_attestation(
        &self,
        slot: Slot,
        attestation: &PayloadAttestation,
    ) -> Result<IndexedPayloadAttestation, TransitionError> {
        let ptc_index = self
            .ptc_window_index_for_slot(slot)
            .ok_or(OperationError::PayloadAttestationSlotMismatch)?;
        let committee = &self.ptc_window[ptc_index];
        let mut attesting: Vec<ValidatorIndex> = committee
            .iter()
            .zip(attestation.aggregation_bits.iter())
            .filter_map(|(vi, bit)| if *bit { Some(*vi) } else { None })
            .collect();
        attesting.sort_by_key(|v| v.as_u64());
        let mut indices = ssz_rs::List::<ValidatorIndex, PTC_SIZE>::default();
        for vi in attesting {
            indices.push(vi);
        }
        Ok(IndexedPayloadAttestation {
            attesting_indices: indices,
            data: attestation.data,
            signature: attestation.signature,
        })
    }

    /// Verify the aggregate signature over an indexed payload attestation.
    ///
    /// The attesting set must be non-empty and sorted, then the members'
    /// public keys are aggregated and the signature is checked over the
    /// attestation data under the payload-timeliness domain. Unlike an indexed
    /// beacon attestation the indices may carry sorted duplicates, since one
    /// validator can occupy several committee positions. An empty set, an
    /// out-of-order set, or a bad aggregate raises the matching
    /// [`OperationError`] or [`SignatureError`].
    pub fn validate_indexed_payload_attestation(
        &self,
        indexed: &IndexedPayloadAttestation,
    ) -> Result<(), TransitionError> {
        if indexed.attesting_indices.is_empty() {
            return Err(OperationError::AttestationParticipantsEmpty.into());
        }
        // Spec: payload-attestation indices must be sorted. Unlike indexed
        // beacon attestations, duplicate validator indices are valid because a
        // validator may appear in multiple PTC positions.
        if !indexed.attesting_indices.is_sorted() {
            return Err(OperationError::IndexedAttestationNotSorted.into());
        }
        let pubkeys: Vec<BLSPubkey> = indexed
            .attesting_indices
            .iter()
            .map(|i| self.validator(*i).map(|v| v.pubkey))
            .collect::<Result<_, _>>()?;
        let mut data = indexed.data;
        let domain = self.domain_for(DOMAIN_PTC_ATTESTER, data.slot.epoch())?;
        let signing_root =
            compute_signing_root(&mut data, domain, MerkleError::PayloadAttestationData)?;
        fast_aggregate_verify(
            &pubkeys,
            signing_root,
            &indexed.signature,
            SignatureError::PayloadAttestation,
        )
    }

    /// Validate a payload-timeliness aggregate over the previous slot's payload.
    ///
    /// Called from `process_operations` after `process_block_header`, so
    /// `latest_block_header.parent_root` is the containing block's parent root.
    /// The attestation data must reference that parent block through
    /// `beacon_block_root` and `self.slot - 1` through `slot`, and the expanded
    /// aggregate signature must verify under the assigned payload-timeliness
    /// committee. Shape and targeting failures raise [`OperationError`]. BLS
    /// failures raise [`SignatureError`]. Both surface as [`TransitionError`].
    /// This writes no `BeaconState`. The payload attestation votes on timeliness
    /// and data availability and does not add builder-payment weight. That
    /// weight comes only from beacon attestations for the proposal slot in
    /// [`BeaconState::process_attestation`]. The per-slot payload-availability
    /// bit is set separately when the child block accepts the parent payload, and
    /// [`crate::fork_choice::Store::on_block()`] later records local PTC vote vectors.
    /// Spec: `process_payload_attestation`
    pub fn process_payload_attestation(
        &mut self,
        attestation: &PayloadAttestation,
    ) -> Result<(), TransitionError> {
        let data = &attestation.data;
        let parent_block_root = self.latest_block_header.parent_root;
        if data.beacon_block_root != parent_block_root {
            return Err(OperationError::PayloadAttestationBlockRootMismatch.into());
        }
        let parent_slot = self.slot.saturating_sub(1);
        if data.slot != parent_slot {
            return Err(OperationError::PayloadAttestationSlotMismatch.into());
        }
        let indexed = self.indexed_payload_attestation(data.slot, attestation)?;
        self.validate_indexed_payload_attestation(&indexed)
    }

    /// Resolve a slot into its bucket inside `ptc_window`.
    ///
    /// `ptc_window` is laid out as three contiguous epoch buckets:
    ///   `[previous_epoch | current_epoch | next_epoch]`
    /// `slot` must fall within those three epochs relative to `state.slot`.
    /// A slot two or more epochs in the past returns `None`, as does a slot
    /// two or more epochs in the future.
    #[must_use]
    pub fn ptc_window_index_for_slot(&self, slot: Slot) -> Option<usize> {
        let epoch = slot.epoch().as_u64();
        let state_epoch = self.slot.epoch().as_u64();
        let bucket = if epoch.checked_add(1) == Some(state_epoch) {
            0
        } else if epoch == state_epoch {
            1
        } else if state_epoch.checked_add(1) == Some(epoch) {
            2
        } else {
            return None;
        };
        let offset = slot % SLOTS_PER_EPOCH;
        Some(bucket * SLOTS_PER_EPOCH + offset)
    }
}
