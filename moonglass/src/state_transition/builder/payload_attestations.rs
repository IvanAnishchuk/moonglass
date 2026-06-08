//! Payload attestation processing during block application.

use crate::constants::{DOMAIN_PTC_ATTESTER, PTC_SIZE, SLOTS_PER_EPOCH};
use crate::containers::{BeaconState, IndexedPayloadAttestation, PayloadAttestation};
use crate::error::{MerkleError, OperationError, SignatureError, TransitionError};
use crate::primitives::{BLSPubkey, Slot, ValidatorIndex};
use crate::state_transition::{BeaconStateLookup, compute_signing_root, fast_aggregate_verify};

impl BeaconState {
    /// Resolve `attestation`'s bitfield into the sorted attesting indices using
    /// the PTC assignment for `slot`.
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

    /// Verify that `indexed` carries a valid aggregate signature under
    /// `DOMAIN_PTC_ATTESTER` for the indexed attester set.
    pub fn validate_indexed_payload_attestation(
        &self,
        indexed: &IndexedPayloadAttestation,
    ) -> Result<(), TransitionError> {
        if indexed.attesting_indices.is_empty() {
            return Err(OperationError::AttestationParticipantsEmpty.into());
        }
        // Spec: indices must be strictly sorted.
        if !indexed
            .attesting_indices
            .windows(2)
            .all(|w| w[0].as_u64() < w[1].as_u64())
        {
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

    /// Validate a payload-timeliness vote: the data must reference the parent
    /// block and previous slot, and the aggregate signature must verify under
    /// the assigned PTC. No state mutation here. Effective-balance weight is
    /// accumulated by `process_attestation` for the same-slot quorum, and the
    /// per-slot payload-availability bit is set by `apply_parent_execution_payload`
    /// when the parent's payload arrives.
    ///
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

    /// Resolve a slot into its slot's bucket inside `ptc_window`.
    ///
    /// `ptc_window` is laid out as three contiguous epoch buckets:
    ///   `[previous_epoch | current_epoch | next_epoch]`
    ///
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
