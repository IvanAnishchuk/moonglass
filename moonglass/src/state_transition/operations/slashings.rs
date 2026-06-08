//! Proposer and attester slashing handlers.

use crate::constants::{DOMAIN_BEACON_PROPOSER, SLOTS_PER_EPOCH};
use crate::containers::{
    AttestationData, AttesterSlashing, BeaconState, BuilderPendingPayment, ProposerSlashing,
};
use crate::error::{MerkleError, OperationError, SignatureError, TransitionError};
use crate::primitives::ValidatorIndex;
use crate::state_transition::{
    BeaconStateLookup, TreeRootExt, compute_signing_root, verify_signature,
};

impl BeaconState {
    /// Validate a proposer slashing and apply the slashing mutation. The two
    /// signed headers must be by the same proposer for the same slot but differ.
    ///
    /// Spec: `process_proposer_slashing`
    pub fn process_proposer_slashing(
        &mut self,
        slashing: &ProposerSlashing,
    ) -> Result<(), TransitionError> {
        let h1 = &slashing.signed_header_1.message;
        let h2 = &slashing.signed_header_2.message;
        if h1.slot != h2.slot {
            return Err(OperationError::ProposerSlashingSlotMismatch.into());
        }
        if h1.proposer_index != h2.proposer_index {
            return Err(OperationError::ProposerSlashingProposerMismatch.into());
        }
        let mut h1c = *h1;
        let mut h2c = *h2;
        let r1 = h1c.tree_root(MerkleError::BeaconBlockHeader)?;
        let r2 = h2c.tree_root(MerkleError::BeaconBlockHeader)?;
        if r1 == r2 {
            return Err(OperationError::ProposerSlashingHeadersMatch.into());
        }

        let proposer = self.validator(h1.proposer_index)?;
        let current_epoch = self.slot.epoch();
        if !proposer.is_slashable_at(current_epoch) {
            return Err(OperationError::ProposerSlashingNotSlashable(h1.proposer_index).into());
        }
        let pubkey = proposer.pubkey;
        let mut h1m = *h1;
        let mut h2m = *h2;
        let domain_1 = self.domain_for(DOMAIN_BEACON_PROPOSER, h1.slot.epoch())?;
        let domain_2 = self.domain_for(DOMAIN_BEACON_PROPOSER, h2.slot.epoch())?;
        let sr1 = compute_signing_root(&mut h1m, domain_1, MerkleError::BeaconBlockHeader)?;
        let sr2 = compute_signing_root(&mut h2m, domain_2, MerkleError::BeaconBlockHeader)?;
        verify_signature(
            &pubkey,
            sr1,
            &slashing.signed_header_1.signature,
            SignatureError::ProposerSlashingHeaderOne,
        )?;
        verify_signature(
            &pubkey,
            sr2,
            &slashing.signed_header_2.signature,
            SignatureError::ProposerSlashingHeaderTwo,
        )?;

        // Remove the builder pending payment for this slot if the slashing
        // arrives within the 2-epoch window. Two windows are tracked: the
        // back half of the buffer holds the current epoch's slots, the front
        // half holds the previous epoch's. Slashings outside that window are
        // out of luck and the payment stays.
        let slot = h1.slot;
        let proposal_epoch = slot.epoch();
        let previous_epoch = current_epoch.as_u64().saturating_sub(1);
        let slot_offset = slot % SLOTS_PER_EPOCH;
        if proposal_epoch == current_epoch {
            self.builder_pending_payments[SLOTS_PER_EPOCH + slot_offset] =
                BuilderPendingPayment::default();
        } else if proposal_epoch.as_u64() == previous_epoch {
            self.builder_pending_payments[slot_offset] = BuilderPendingPayment::default();
        }

        self.slash_validator(h1.proposer_index, None)?;
        Ok(())
    }

    /// Validate an attester slashing: the two indexed attestations must form
    /// slashable evidence, overlap on at least one attester, and each verify.
    ///
    /// Spec: `process_attester_slashing`
    pub fn process_attester_slashing(
        &mut self,
        slashing: &AttesterSlashing,
    ) -> Result<(), TransitionError> {
        let a1 = &slashing.attestation_1;
        let a2 = &slashing.attestation_2;
        if !is_slashable_attestation_data(&a1.data, &a2.data) {
            return Err(OperationError::AttesterSlashingNotSlashable.into());
        }
        self.validate_indexed_attestation(a1, SignatureError::AttesterSlashingAttestationOne)?;
        self.validate_indexed_attestation(a2, SignatureError::AttesterSlashingAttestationTwo)?;

        let set1: std::collections::BTreeSet<u64> =
            a1.attesting_indices.iter().map(|i| i.as_u64()).collect();
        let set2: std::collections::BTreeSet<u64> =
            a2.attesting_indices.iter().map(|i| i.as_u64()).collect();
        let intersection: Vec<u64> = set1.intersection(&set2).copied().collect();
        if intersection.is_empty() {
            return Err(OperationError::AttesterSlashingNoIntersection.into());
        }
        let epoch = self.slot.epoch();
        let mut slashed_any = false;
        for raw in intersection {
            let vi = ValidatorIndex(raw);
            let v = self.validator(vi)?;
            if v.is_slashable_at(epoch) {
                self.slash_validator(vi, None)?;
                slashed_any = true;
            }
        }
        if !slashed_any {
            return Err(OperationError::AttesterSlashingNotSlashable.into());
        }
        Ok(())
    }
}

/// True if attestations 1 and 2 are slashable evidence (double vote or
/// surround vote per Casper FFG).
pub(crate) fn is_slashable_attestation_data(d1: &AttestationData, d2: &AttestationData) -> bool {
    let double = d1 != d2 && d1.target.epoch == d2.target.epoch;
    let surround = d1.source.epoch.as_u64() < d2.source.epoch.as_u64()
        && d2.target.epoch.as_u64() < d1.target.epoch.as_u64();
    double || surround
}
