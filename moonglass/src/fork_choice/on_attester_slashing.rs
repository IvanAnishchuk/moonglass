//! Taking in proof that a [validator](crate::glossary#validator)
//! [equivocated](crate::glossary#equivocation).
//!
//! An attester slashing is evidence that one validator signed two conflicting
//! [attestations](crate::glossary#attestation). Fork choice does not punish
//! anyone here, that is the state transition's job. It simply notes the offender
//! in its `equivocating_indices` set so that, from then on, that validator's
//! votes are ignored when weighing the chain.

use crate::containers::AttesterSlashing;
use crate::error::{ForkChoiceError, SignatureError};

use super::store::Store;

impl Store {
    /// Validate slashing evidence and mark the offending validators as
    /// equivocating.
    ///
    /// Confirms the two attestations really do conflict and that both are properly
    /// signed, then adds every validator that signed both to the store's
    /// `equivocating_indices`, after which their stake stops counting toward any
    /// node's weight. Returns [`ForkChoiceError::InvalidAttesterSlashing`] if the
    /// evidence does not hold up.
    pub fn on_attester_slashing(
        &mut self,
        attester_slashing: &AttesterSlashing,
    ) -> Result<(), ForkChoiceError> {
        let a1 = &attester_slashing.attestation_1;
        let a2 = &attester_slashing.attestation_2;

        if !crate::state_transition::is_slashable_attestation_data(&a1.data, &a2.data) {
            return Err(ForkChoiceError::InvalidAttesterSlashing);
        }

        let state = self
            .block_states
            .get(&self.justified_checkpoint.root)
            .ok_or(ForkChoiceError::JustifiedStateMissing)?;
        state
            .validate_indexed_attestation(a1, SignatureError::AttesterSlashingAttestationOne)
            .map_err(|_| ForkChoiceError::InvalidAttesterSlashing)?;
        state
            .validate_indexed_attestation(a2, SignatureError::AttesterSlashingAttestationTwo)
            .map_err(|_| ForkChoiceError::InvalidAttesterSlashing)?;

        let a1_set: std::collections::BTreeSet<_> = a1.attesting_indices.iter().copied().collect();
        let a2_set: std::collections::BTreeSet<_> = a2.attesting_indices.iter().copied().collect();
        self.equivocating_indices
            .extend(a1_set.intersection(&a2_set).copied());
        Ok(())
    }
}
