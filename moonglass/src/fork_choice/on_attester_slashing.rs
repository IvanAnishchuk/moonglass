//! Spec: `on_attester_slashing`.

use crate::containers::AttesterSlashing;
use crate::error::{ForkChoiceError, SignatureError};

use super::store::Store;

/// Process an attester slashing: verify slashability, validate both attestation
/// signatures against the justified state, then record all overlapping validator
/// indices in the equivocating set.
pub fn on_attester_slashing(
    store: &mut Store,
    attester_slashing: &AttesterSlashing,
) -> Result<(), ForkChoiceError> {
    let a1 = &attester_slashing.attestation_1;
    let a2 = &attester_slashing.attestation_2;

    if !crate::state_transition::is_slashable_attestation_data(&a1.data, &a2.data) {
        return Err(ForkChoiceError::InvalidAttesterSlashing);
    }

    let state = store
        .block_states
        .get(&store.justified_checkpoint.root)
        .ok_or(ForkChoiceError::JustifiedStateMissing)?;
    state
        .validate_indexed_attestation(a1, SignatureError::AttesterSlashingAttestationOne)
        .map_err(|_| ForkChoiceError::InvalidAttesterSlashing)?;
    state
        .validate_indexed_attestation(a2, SignatureError::AttesterSlashingAttestationTwo)
        .map_err(|_| ForkChoiceError::InvalidAttesterSlashing)?;

    let a1_set: std::collections::BTreeSet<_> = a1.attesting_indices.iter().copied().collect();
    let a2_set: std::collections::BTreeSet<_> = a2.attesting_indices.iter().copied().collect();
    for &validator in a1_set.intersection(&a2_set) {
        store.equivocating_indices.insert(validator);
    }
    Ok(())
}
