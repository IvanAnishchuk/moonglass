//! Block-operation processing.
//!
//! Validates and applies the operations a proposer chose to include: slashings,
//! attestations, voluntary exits, deposits, credential changes, and the
//! execution-to-consensus requests delivered alongside the parent payload.

mod attestations;
mod deposits;
mod exits;
mod requests;
mod slashings;

pub use deposits::is_valid_merkle_branch;
pub(crate) use slashings::is_slashable_attestation_data;

use crate::containers::{BeaconBlockBody, BeaconState};
use crate::error::{OperationError, TransitionError};

impl BeaconState {
    /// Apply the body operations in consensus order.
    ///
    /// Spec: `process_operations`
    pub fn process_operations(&mut self, body: &BeaconBlockBody) -> Result<(), TransitionError> {
        // Spec gloas: `assert len(body.deposits) == 0` runs first.
        if !body.deposits.is_empty() {
            return Err(OperationError::DepositsNotAllowed.into());
        }
        for slashing in body.proposer_slashings.iter() {
            self.process_proposer_slashing(slashing)?;
        }
        for slashing in body.attester_slashings.iter() {
            self.process_attester_slashing(slashing)?;
        }
        for attestation in body.attestations.iter() {
            self.process_attestation(attestation)?;
        }
        for signed_exit in body.voluntary_exits.iter() {
            self.process_voluntary_exit(signed_exit)?;
        }
        for signed_change in body.bls_to_execution_changes.iter() {
            self.process_bls_to_execution_change(signed_change)?;
        }
        for attestation in body.payload_attestations.iter() {
            self.process_payload_attestation(attestation)?;
        }
        Ok(())
    }
}
