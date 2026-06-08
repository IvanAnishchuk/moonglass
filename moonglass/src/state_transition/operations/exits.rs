//! `process_voluntary_exit` and related lifecycle helpers.

use sha2::{Digest, Sha256};

use crate::constants::{
    BLS_WITHDRAWAL_PREFIX, CAPELLA_FORK_VERSION, DOMAIN_BLS_TO_EXECUTION_CHANGE,
    DOMAIN_VOLUNTARY_EXIT, ETH1_ADDRESS_WITHDRAWAL_PREFIX, FAR_FUTURE_EPOCH, GENESIS_FORK_VERSION,
    SHARD_COMMITTEE_PERIOD,
};
use crate::containers::{BeaconState, SignedBLSToExecutionChange, SignedVoluntaryExit};
use crate::error::{MerkleError, OperationError, SignatureError, TransitionError};
use crate::primitives::Gwei;
use crate::state_transition::{
    BeaconStateLookup, compute_domain, compute_signing_root, verify_signature,
};

impl BeaconState {
    /// Validate a voluntary exit and schedule the validator's (or builder's)
    /// departure from the active set.
    ///
    /// The operation is routed by index kind: an index with the
    /// `BUILDER_INDEX_FLAG` bit set targets a builder, everything else targets
    /// a validator. The two branches share the signed-epoch check and the
    /// signature-verification shape but use different active-set, eligibility,
    /// and pending-withdrawal checks.
    ///
    /// Spec: `process_voluntary_exit`
    pub fn process_voluntary_exit(
        &mut self,
        signed_exit: &SignedVoluntaryExit,
    ) -> Result<(), TransitionError> {
        let exit = &signed_exit.message;
        let current = self.slot.epoch();
        if current < exit.epoch {
            return Err(OperationError::ExitTooEarly {
                current,
                exit: exit.epoch,
            }
            .into());
        }

        // Spec pins the voluntary-exit signing domain to `CAPELLA_FORK_VERSION`
        // so signatures issued under a prior fork keep verifying after later
        // forks ship.
        let domain = compute_domain(
            DOMAIN_VOLUNTARY_EXIT,
            CAPELLA_FORK_VERSION,
            self.genesis_validators_root,
        )?;
        let mut exit_msg = *exit;
        let signing_root = compute_signing_root(&mut exit_msg, domain, MerkleError::VoluntaryExit)?;

        if exit.validator_index.is_builder_index() {
            return self.process_builder_voluntary_exit(signed_exit, signing_root);
        }

        let validator = self.validator(exit.validator_index)?;
        let pubkey = validator.pubkey;
        let activation_epoch = validator.activation_epoch;
        let already_exiting = validator.exit_epoch != FAR_FUTURE_EPOCH;
        let active = validator.is_active_at(current);

        if !active {
            return Err(OperationError::ValidatorNotActive(exit.validator_index).into());
        }
        if already_exiting {
            return Err(OperationError::ValidatorAlreadyExiting(exit.validator_index).into());
        }
        let eligible = activation_epoch.saturating_add(SHARD_COMMITTEE_PERIOD);
        if current < eligible {
            return Err(OperationError::ValidatorTooYoung {
                validator: exit.validator_index,
                eligible,
                current,
            }
            .into());
        }
        if self.pending_balance_to_withdraw(exit.validator_index) != Gwei::ZERO {
            return Err(OperationError::ValidatorHasPendingWithdrawal(exit.validator_index).into());
        }

        verify_signature(
            &pubkey,
            signing_root,
            &signed_exit.signature,
            SignatureError::VoluntaryExit(exit.validator_index),
        )?;

        self.initiate_validator_exit(exit.validator_index)?;
        Ok(())
    }

    fn process_builder_voluntary_exit(
        &mut self,
        signed_exit: &SignedVoluntaryExit,
        signing_root: crate::primitives::Root,
    ) -> Result<(), TransitionError> {
        let exit = &signed_exit.message;
        let builder_index = exit.validator_index.to_builder_index()?;
        if !self.is_active_builder(builder_index)? {
            return Err(OperationError::BuilderNotActive(builder_index).into());
        }
        if self.pending_balance_to_withdraw_for_builder(builder_index) != Gwei::ZERO {
            return Err(OperationError::BuilderHasPendingWithdrawal(builder_index).into());
        }
        let pubkey = self.builder(builder_index)?.pubkey;
        verify_signature(
            &pubkey,
            signing_root,
            &signed_exit.signature,
            SignatureError::VoluntaryExit(exit.validator_index),
        )?;
        self.initiate_builder_exit(builder_index)?;
        Ok(())
    }

    /// Swap a validator's BLS withdrawal credential for an execution-address
    /// credential.
    ///
    /// Spec: `process_bls_to_execution_change`
    pub fn process_bls_to_execution_change(
        &mut self,
        signed_change: &SignedBLSToExecutionChange,
    ) -> Result<(), TransitionError> {
        let change = &signed_change.message;
        let validator = self.validator(change.validator_index)?;
        let creds = validator.withdrawal_credentials;

        if creds[0] != BLS_WITHDRAWAL_PREFIX {
            return Err(OperationError::WithdrawalCredentialsNotBls(change.validator_index).into());
        }
        let pubkey_hash: [u8; 32] = Sha256::digest(change.from_bls_pubkey.0).into();
        if creds[1..] != pubkey_hash[1..] {
            return Err(OperationError::BlsChangeCredentialMismatch(change.validator_index).into());
        }

        let domain = compute_domain(
            DOMAIN_BLS_TO_EXECUTION_CHANGE,
            GENESIS_FORK_VERSION,
            self.genesis_validators_root,
        )?;
        let mut change_msg = *change;
        let signing_root =
            compute_signing_root(&mut change_msg, domain, MerkleError::BlsToExecutionChange)?;
        verify_signature(
            &change.from_bls_pubkey,
            signing_root,
            &signed_change.signature,
            SignatureError::BlsToExecutionChange(change.validator_index),
        )?;

        let mut new_creds = [0u8; 32];
        new_creds[0] = ETH1_ADDRESS_WITHDRAWAL_PREFIX;
        new_creds[12..].copy_from_slice(&change.to_execution_address.0);
        self.validators[change.validator_index.as_usize()].withdrawal_credentials = new_creds;
        Ok(())
    }
}
