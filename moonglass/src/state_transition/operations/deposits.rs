//! Deposit processing and validator/builder registry routing.
//!
//! This transition rejects non-empty legacy block-body deposits in
//! [`BeaconState::process_operations`](crate::containers::BeaconState::process_operations).
//! Deposit data that affects this transition arrives through parent-payload
//! execution-layer deposit requests. Validator deposits enter `pending_deposits`
//! and are activated later under epoch churn rules. Builder deposit requests
//! arrive separately and register a new builder or top up an existing one after a
//! signature check under the builder-deposit domain.

use sha2::{Digest, Sha256};
use ssz_rs::prelude::*;

use crate::constants::{
    COMPOUNDING_WITHDRAWAL_PREFIX, DEPOSIT_CONTRACT_TREE_DEPTH, DOMAIN_BUILDER_DEPOSIT,
    DOMAIN_DEPOSIT, EFFECTIVE_BALANCE_INCREMENT, FAR_FUTURE_EPOCH, GENESIS_FORK_VERSION,
    GENESIS_SLOT, MAX_EFFECTIVE_BALANCE, MIN_ACTIVATION_BALANCE, MIN_BUILDER_WITHDRAWABILITY_DELAY,
};
use crate::containers::{
    BeaconState, Builder, BuilderDepositRequest, Deposit, PendingDeposit, Validator,
};
use crate::error::{MerkleError, OperationError, SignatureError, TransitionError};
use crate::primitives::{BLSPubkey, BLSSignature, Bytes32, Gwei, ParticipationFlags, Root};
use crate::state_transition::{
    TreeRootExt, compute_domain, compute_signing_root, verify_signature,
};

/// SHA-256 Merkle inclusion check against `root`. `leaf` is folded with
/// `branch[i]` per the path bit of `index` for `depth` levels.
#[must_use]
pub fn is_valid_merkle_branch(
    leaf: Root,
    branch: &[Bytes32],
    depth: usize,
    index: u64,
    root: Root,
) -> bool {
    if branch.len() != depth {
        return false;
    }
    let mut value: [u8; 32] = leaf.0;
    for (i, sibling) in branch.iter().enumerate() {
        let mut hasher = Sha256::new();
        if (index >> i) & 1 == 1 {
            hasher.update(sibling);
            hasher.update(value);
        } else {
            hasher.update(value);
            hasher.update(sibling);
        }
        value = hasher.finalize().into();
    }
    value == root.0
}

/// SSZ container used to compute the deposit signing root.
#[derive(Default, Clone, PartialEq, Eq, SimpleSerialize)]
struct DepositMessage {
    /// Depositing validator public key.
    pub pubkey: BLSPubkey,
    /// Withdrawal credentials committed by the deposit.
    pub withdrawal_credentials: Bytes32,
    /// Deposit amount in gwei.
    pub amount: Gwei,
}

impl BeaconState {
    /// Append a fresh validator to the registry and its balance side-arrays.
    ///
    /// This writes every per-validator list that must stay index-aligned with
    /// `validators`: balances, participation flags, and inactivity scores.
    /// Activation fields start at `FAR_FUTURE_EPOCH`. Epoch processing later
    /// schedules eligibility and activation.
    pub fn add_validator_to_registry(
        &mut self,
        pubkey: BLSPubkey,
        withdrawal_credentials: Bytes32,
        amount: Gwei,
    ) -> Result<(), TransitionError> {
        let compounding = withdrawal_credentials[0] == COMPOUNDING_WITHDRAWAL_PREFIX;
        let max = if compounding {
            MAX_EFFECTIVE_BALANCE
        } else {
            MIN_ACTIVATION_BALANCE
        };
        let increment = EFFECTIVE_BALANCE_INCREMENT.as_u64();
        let effective = Gwei((amount.as_u64() - amount.as_u64() % increment).min(max.as_u64()));
        let validator = Validator {
            pubkey,
            withdrawal_credentials,
            effective_balance: effective,
            slashed: false,
            activation_eligibility_epoch: FAR_FUTURE_EPOCH,
            activation_epoch: FAR_FUTURE_EPOCH,
            exit_epoch: FAR_FUTURE_EPOCH,
            withdrawable_epoch: FAR_FUTURE_EPOCH,
        };
        self.validators.push(validator);
        self.balances.push(amount);
        self.previous_epoch_participation
            .push(ParticipationFlags::NONE);
        self.current_epoch_participation
            .push(ParticipationFlags::NONE);
        self.inactivity_scores.push(0);
        Ok(())
    }

    /// Choose the registry index a new builder should occupy.
    ///
    /// Reuses the lowest index of an exited builder whose balance is fully
    /// drained, otherwise appends at the end. This keeps builder indices stable
    /// while making emptied slots reusable.
    #[must_use]
    pub fn index_for_new_builder(&self) -> usize {
        let current_epoch = self.slot.epoch();
        for (i, builder) in self.builders.iter().enumerate() {
            if builder.withdrawable_epoch <= current_epoch && builder.balance == Gwei::ZERO {
                return i;
            }
        }
        self.builders.len()
    }

    /// Insert (or reassign at an exited slot) a builder record. Mirrors the
    /// spec `add_builder_to_registry` plus the `set_or_append_list` semantics.
    pub fn add_builder_to_registry(
        &mut self,
        pubkey: BLSPubkey,
        withdrawal_credentials: Bytes32,
        amount: Gwei,
    ) -> Result<(), TransitionError> {
        let mut execution_address = [0u8; 20];
        execution_address.copy_from_slice(&withdrawal_credentials[12..]);
        let deposit_epoch = self.slot.epoch();
        let builder = Builder {
            pubkey,
            version: withdrawal_credentials[0],
            execution_address: crate::primitives::ExecutionAddress(execution_address),
            balance: amount,
            deposit_epoch,
            withdrawable_epoch: FAR_FUTURE_EPOCH,
        };
        let idx = self.index_for_new_builder();
        if idx < self.builders.len() {
            self.builders[idx] = builder;
        } else {
            self.builders.push(builder);
        }
        Ok(())
    }

    /// Apply a builder deposit request delivered by the parent payload.
    ///
    /// A deposit for a pubkey not yet in the registry registers a new builder when
    /// its signature verifies under the builder-deposit domain. A deposit for an
    /// existing builder tops up its balance, and if that builder had already
    /// started exiting, pushes its withdrawable epoch back out so the new stake is
    /// not paid out immediately. Spec: `process_builder_deposit_request`.
    pub fn process_builder_deposit_request(
        &mut self,
        request: &BuilderDepositRequest,
    ) -> Result<(), TransitionError> {
        match self
            .builders
            .iter()
            .position(|b| b.pubkey == request.pubkey)
        {
            None => {
                if Self::is_valid_builder_deposit_signature(request)? {
                    self.add_builder_to_registry(
                        request.pubkey,
                        request.withdrawal_credentials,
                        request.amount,
                    )?;
                }
            }
            Some(idx) => {
                let current_epoch = self.slot.epoch();
                let builder = &mut self.builders[idx];
                builder.balance = builder
                    .balance
                    .checked_add(request.amount)
                    .ok_or(TransitionError::BalanceOverflow)?;
                if builder.withdrawable_epoch != FAR_FUTURE_EPOCH {
                    builder.withdrawable_epoch =
                        current_epoch.saturating_add(MIN_BUILDER_WITHDRAWABILITY_DELAY);
                }
            }
        }
        Ok(())
    }

    /// Verify a builder deposit's signature under [`DOMAIN_BUILDER_DEPOSIT`].
    ///
    /// Mirrors validator deposit verification but with the builder domain, so a
    /// validator deposit signature cannot be replayed as a builder deposit.
    /// Spec: `is_valid_builder_deposit_signature`.
    fn is_valid_builder_deposit_signature(
        request: &BuilderDepositRequest,
    ) -> Result<bool, TransitionError> {
        let domain = compute_domain(
            DOMAIN_BUILDER_DEPOSIT,
            GENESIS_FORK_VERSION,
            Root::default(),
        )?;
        let mut msg = DepositMessage {
            pubkey: request.pubkey,
            withdrawal_credentials: request.withdrawal_credentials,
            amount: request.amount,
        };
        let signing_root = compute_signing_root(&mut msg, domain, MerkleError::DepositMessage)?;
        match verify_signature(
            &request.pubkey,
            signing_root,
            &request.signature,
            SignatureError::Deposit,
        ) {
            Ok(()) => Ok(true),
            Err(TransitionError::Signature(_)) => Ok(false),
            Err(e) => Err(e),
        }
    }

    /// Apply an Eth1-bridge deposit. For a never-seen pubkey, validate the
    /// `proof-of-possession` signature: a valid `PoP` eagerly adds the
    /// validator to the registry with zero effective balance, and an invalid
    /// `PoP` drops the deposit. The deposit payload is then queued onto
    /// `pending_deposits` with `slot = GENESIS_SLOT` to distinguish bridge
    /// deposits from EL deposit requests when the queue is drained during
    /// epoch processing.
    /// Spec: `apply_deposit`.
    pub fn apply_deposit(
        &mut self,
        pubkey: BLSPubkey,
        withdrawal_credentials: Bytes32,
        amount: Gwei,
        signature: BLSSignature,
    ) -> Result<(), TransitionError> {
        let is_new = !self.validators.iter().any(|v| v.pubkey == pubkey);
        if is_new {
            if !self.is_valid_deposit_signature(
                &pubkey,
                withdrawal_credentials,
                amount,
                &signature,
            )? {
                return Ok(());
            }
            self.add_validator_to_registry(pubkey, withdrawal_credentials, Gwei::ZERO)?;
        }
        self.pending_deposits.push(PendingDeposit {
            pubkey,
            withdrawal_credentials,
            amount,
            signature,
            slot: GENESIS_SLOT,
        });
        Ok(())
    }

    /// Validate the deposit's Merkle inclusion proof, bump the deposit cursor,
    /// and queue the payload via [`BeaconState::apply_deposit`].
    /// Spec: `process_deposit`
    pub fn process_deposit(&mut self, deposit: &Deposit) -> Result<(), TransitionError> {
        let mut deposit_data = deposit.data;
        let leaf = deposit_data.tree_root(MerkleError::DepositMessage)?;
        let branch: Vec<Bytes32> = deposit.proof.iter().copied().collect();
        if !is_valid_merkle_branch(
            leaf,
            &branch,
            DEPOSIT_CONTRACT_TREE_DEPTH + 1,
            self.eth1_deposit_index,
            self.eth1_data.deposit_root,
        ) {
            return Err(OperationError::DepositMerkleInvalid.into());
        }
        self.eth1_deposit_index = self.eth1_deposit_index.saturating_add(1);
        self.apply_deposit(
            deposit.data.pubkey,
            deposit.data.withdrawal_credentials,
            deposit.data.amount,
            deposit.data.signature,
        )
    }

    /// True when the deposit's BLS signature verifies as a proof-of-possession
    /// under the genesis fork-version deposit domain. Distinguishes signature
    /// failures (returns `Ok(false)`) from internal merkleization or domain
    /// computation failures (propagated as `Err`).
    pub fn is_valid_deposit_signature(
        &self,
        pubkey: &BLSPubkey,
        withdrawal_credentials: Bytes32,
        amount: Gwei,
        signature: &BLSSignature,
    ) -> Result<bool, TransitionError> {
        match Self::verify_deposit_signature(pubkey, withdrawal_credentials, amount, signature) {
            Ok(()) => Ok(true),
            Err(TransitionError::Signature(_)) => Ok(false),
            Err(e) => Err(e),
        }
    }

    /// Verify a deposit's BLS signature under the genesis fork-version domain.
    ///
    /// The genesis-validators-root is intentionally fixed at the all-zero root
    /// so the same signed deposit is valid across forks. State-bound roots
    /// would partition the deposit message space per network.
    fn verify_deposit_signature(
        pubkey: &BLSPubkey,
        withdrawal_credentials: Bytes32,
        amount: Gwei,
        signature: &BLSSignature,
    ) -> Result<(), TransitionError> {
        let domain = compute_domain(DOMAIN_DEPOSIT, GENESIS_FORK_VERSION, Root::default())?;
        let mut msg = DepositMessage {
            pubkey: *pubkey,
            withdrawal_credentials,
            amount,
        };
        let signing_root = compute_signing_root(&mut msg, domain, MerkleError::DepositMessage)?;
        verify_signature(pubkey, signing_root, signature, SignatureError::Deposit)
    }
}
