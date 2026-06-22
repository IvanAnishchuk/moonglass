//! Taking in a delivered execution payload.
//!
//! When a [block's](crate::glossary#beacon-block) payload finally arrives,
//! carried in a signed envelope, this handler checks it and records it.
//! Recording it is what lets the block's *full* branch appear in the fork-choice
//! tree. Three kinds of check run: the consensus-side ones (the signature, and
//! that the payload matches the bid, [slot](crate::glossary#slot), parent hash,
//! timestamp, and withdrawals), a data-availability check, and an
//! execution-engine check. The last two are mocked here and always pass, see
//! [`is_data_available`] and [`verify_and_notify_new_payload`]. Wiring real ones
//! is a TODO.
use crate::containers::SignedExecutionPayloadEnvelope;
use crate::error::ForkChoiceError;
use crate::primitives::Root;

use super::store::Store;

impl Store {
    /// Check a delivered payload against its block and record it in the store.
    ///
    /// In order: confirm the block is known, that its data is available
    /// ([`is_data_available`]), that the envelope verifies against the block's
    /// state
    /// ([`verify_execution_payload_envelope`](crate::containers::BeaconState::verify_execution_payload_envelope),
    /// run on a throwaway copy), and that the execution engine accepts it
    /// ([`verify_and_notify_new_payload`]). Only then is the envelope filed in
    /// [`Store::payloads`](super::store::Store::payloads). A payload's lasting
    /// effects are applied later, when a child block builds on it.
    ///
    /// # Errors
    ///
    /// [`ForkChoiceError::PayloadEnvelopeForUnknownBlock`] if the block is unknown,
    /// [`ForkChoiceError::PayloadDataUnavailable`] if the data is missing, a
    /// verification error if the consensus checks fail, and
    /// [`ForkChoiceError::PayloadExecutionInvalid`] if the engine rejects it.
    pub fn on_execution_payload_envelope(
        &mut self,
        signed_envelope: &SignedExecutionPayloadEnvelope,
    ) -> Result<(), ForkChoiceError> {
        let envelope = &signed_envelope.message;
        let beacon_block_root = envelope.beacon_block_root;
        // Prove the block is known, then verify the envelope on a throwaway copy of
        // its state so the verification records nothing.
        let block_state = self.block_states.get(&beacon_block_root).ok_or(
            ForkChoiceError::PayloadEnvelopeForUnknownBlock(beacon_block_root),
        )?;
        if !is_data_available(beacon_block_root) {
            return Err(ForkChoiceError::PayloadDataUnavailable(beacon_block_root));
        }
        let mut state = block_state.clone();
        state.verify_execution_payload_envelope(signed_envelope)?;
        if !verify_and_notify_new_payload(signed_envelope) {
            return Err(ForkChoiceError::PayloadExecutionInvalid(beacon_block_root));
        }
        self.payloads.insert(beacon_block_root, envelope.clone());
        Ok(())
    }
}

/// Whether the block's payload data was available to download.
///
/// TODO: a real implementation samples the payload's data column sidecars and
/// verifies their KZG proofs. No such verifier is wired in, so this mock always
/// reports the data available.
pub fn is_data_available(_beacon_block_root: Root) -> bool {
    true
}

/// Whether the execution engine accepts the payload as valid.
///
/// TODO: a real implementation hands the payload to an execution engine, which
/// runs the transactions and reports back. No engine is wired in, so this mock
/// always reports the payload valid.
pub fn verify_and_notify_new_payload(_signed_envelope: &SignedExecutionPayloadEnvelope) -> bool {
    true
}
