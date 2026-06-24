//! Beacon-attestation validation, participation flags, and builder-payment weight.
//!
//! A beacon attestation does three things during state transition: it updates
//! current/previous participation flags, rewards the proposer for newly counted
//! participation, and, when the attested block was fresh in its own slot, adds
//! effective-balance weight to that slot's pending builder payment. Inclusion
//! still happens after the normal attestation delay. "Same-slot" describes the
//! attestation data relative to the attested block, not the inclusion block.
//! Payload attestations are separate PTC objects and do not add this payment
//! weight.

use crate::constants::{
    DOMAIN_BEACON_ATTESTER, MAX_ATTESTING_INDICES, MIN_ATTESTATION_INCLUSION_DELAY,
    PARTICIPATION_FLAG_WEIGHTS, PROPOSER_WEIGHT, SLOTS_PER_EPOCH, SLOTS_PER_HISTORICAL_ROOT,
    TIMELY_HEAD_FLAG_INDEX, TIMELY_SOURCE_FLAG_INDEX, TIMELY_TARGET_FLAG_INDEX, WEIGHT_DENOMINATOR,
};
use crate::containers::{
    Attestation, AttestationData, BeaconState, BuilderPendingPayment, IndexedAttestation,
};
use crate::error::{BlockError, MerkleError, OperationError, SignatureError, TransitionError};
use crate::primitives::{BLSPubkey, Gwei, Root, Slot, ValidatorIndex};
use crate::state_transition::balance::isqrt_u64;
use crate::state_transition::{
    BeaconStateLookup, committee_indices, compute_signing_root, fast_aggregate_verify,
};

/// Validated attestation data ready to mutate participation and builder payment state.
struct AcceptedAttestation {
    /// Sorted validator indices whose aggregation bits participated.
    attesting_indices: Vec<ValidatorIndex>,
    /// Participation flag indices this attestation earns.
    participation_flags: Vec<usize>,
    /// Whether the target epoch maps to current or previous participation flags.
    target_is_current_epoch: bool,
    /// Whether the attested block was fresh in the attestation's own slot.
    is_same_slot: bool,
    /// Builder-payment window slot that may receive proposal-slot attestation weight.
    builder_payment_index: usize,
}

impl BeaconState {
    /// Decode committee-bit aggregation into sorted attesting validator indices.
    ///
    /// `committee_bits` chooses which slot committees contributed, and
    /// `aggregation_bits` is then consumed across those committees in order.
    /// The result is deduplicated because each validator can contribute at most
    /// once to an ordinary beacon attestation.
    pub fn attesting_indices(
        &self,
        attestation: &Attestation,
    ) -> Result<Vec<ValidatorIndex>, TransitionError> {
        let committee_indices = committee_indices(&attestation.committee_bits);
        let mut offset: usize = 0;
        let mut out: Vec<ValidatorIndex> = Vec::new();
        let agg_bits: Vec<bool> = attestation.aggregation_bits.iter().map(|b| *b).collect();
        for ci in committee_indices {
            let committee = self.beacon_committee(attestation.data.slot, ci)?;
            let before = out.len();
            for (i, vi) in committee.iter().enumerate() {
                if agg_bits.get(offset + i).copied().unwrap_or(false) {
                    out.push(*vi);
                }
            }
            if out.len() == before {
                return Err(OperationError::AttestationParticipantsEmpty.into());
            }
            offset = offset.saturating_add(committee.len());
        }
        if agg_bits.len() != offset {
            return Err(OperationError::AttestationAggregationBitsLength.into());
        }
        out.sort_by_key(|v| v.as_u64());
        out.dedup();
        Ok(out)
    }

    /// Build a sorted [`IndexedAttestation`] from `attestation`.
    ///
    /// This is the signature-verification shape: the bitfields are expanded
    /// into validator indices before public keys can be aggregated.
    pub fn indexed_attestation(
        &self,
        attestation: &Attestation,
    ) -> Result<IndexedAttestation, TransitionError> {
        let attesting = self.attesting_indices(attestation)?;
        let mut indices = ssz_rs::List::<ValidatorIndex, MAX_ATTESTING_INDICES>::default();
        for vi in attesting {
            indices.push(vi);
        }
        Ok(IndexedAttestation {
            attesting_indices: indices,
            data: attestation.data,
            signature: attestation.signature,
        })
    }

    /// Validate `indexed` and verify its aggregate beacon-attestation signature.
    ///
    /// The indices must be non-empty and strictly increasing, then the members'
    /// pubkeys are aggregated and checked over `data` under
    /// `DOMAIN_BEACON_ATTESTER` for `data.target.epoch`.
    pub fn validate_indexed_attestation(
        &self,
        indexed: &IndexedAttestation,
        on_fail: SignatureError,
    ) -> Result<(), TransitionError> {
        if indexed.attesting_indices.is_empty() {
            return Err(OperationError::IndexedAttestationEmpty.into());
        }
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
        let domain = self.domain_for(DOMAIN_BEACON_ATTESTER, data.target.epoch)?;
        let signing_root = compute_signing_root(&mut data, domain, MerkleError::AttestationData)?;
        fast_aggregate_verify(&pubkeys, signing_root, &indexed.signature, on_fail)
    }

    /// Flag indices the attestation earns given its source/target/head match and
    /// inclusion delay. Returns indices into [`PARTICIPATION_FLAG_WEIGHTS`].
    pub fn participation_flags_for(
        &self,
        data: &AttestationData,
        inclusion_delay: u64,
    ) -> Result<Vec<usize>, TransitionError> {
        let current_epoch = self.slot.epoch();
        let justified = if data.target.epoch == current_epoch {
            &self.current_justified_checkpoint
        } else {
            &self.previous_justified_checkpoint
        };
        let is_matching_source = &data.source == justified;
        if !is_matching_source {
            return Err(OperationError::AttestationSourceMismatch.into());
        }
        let is_matching_target = is_matching_source
            && data.target.root == self.block_root_at_slot(data.target.epoch.start_slot());
        let payload_matches = if self.is_attestation_same_slot(data) {
            if data.index.as_u64() != 0 {
                return Err(OperationError::AttestationPayloadStatusInvalid.into());
            }
            true
        } else {
            let slot_index = data.slot % SLOTS_PER_HISTORICAL_ROOT;
            let payload_index = u64::from(self.execution_payload_availability[slot_index]);
            data.index.as_u64() == payload_index
        };
        let is_matching_head = is_matching_target
            && data.beacon_block_root == self.block_root_at_slot(data.slot)
            && payload_matches;
        let sqrt_slots = isqrt_u64(SLOTS_PER_EPOCH as u64);
        let mut out = Vec::new();
        if is_matching_source && inclusion_delay <= sqrt_slots {
            out.push(TIMELY_SOURCE_FLAG_INDEX);
        }
        if is_matching_target {
            out.push(TIMELY_TARGET_FLAG_INDEX);
        }
        if is_matching_head && inclusion_delay == MIN_ATTESTATION_INCLUSION_DELAY {
            out.push(TIMELY_HEAD_FLAG_INDEX);
        }
        Ok(out)
    }

    /// True if `data` references the canonical block at its own slot and that
    /// block is distinct from the prior slot's root, i.e. a fresh block was
    /// proposed at `data.slot`. Gates the head-flag payload check and the
    /// per-slot builder-payment weight contribution.
    #[must_use]
    pub fn is_attestation_same_slot(&self, data: &AttestationData) -> bool {
        if data.slot.as_u64() == 0 {
            return true;
        }
        let block_root = data.beacon_block_root;
        let slot_root = self.block_root_at_slot(data.slot);
        let prev_root = self.block_root_at_slot(data.slot.saturating_sub(1));
        block_root == slot_root && block_root != prev_root
    }

    /// Validate and apply a committee-bit beacon attestation.
    ///
    /// This writes participation flags, proposer reward balance changes, and
    /// possibly builder-payment quorum weight when the attested block was fresh
    /// in its own slot. It does not write fork-choice latest messages.
    /// [`Store::on_attestation()`](crate::fork_choice::Store::on_attestation)
    /// handles that in the local store.
    /// Spec: `process_attestation`
    pub fn process_attestation(
        &mut self,
        attestation: &Attestation,
    ) -> Result<(), TransitionError> {
        let accepted = self.accept_attestation(attestation)?;
        let proposer_reward_numerator = self.record_attestation_participation(&accepted)?;
        self.reward_attestation_proposer(proposer_reward_numerator)
    }

    /// Validate an attestation and summarize the state writes it is allowed to make.
    ///
    /// This rejects invalid timing, payload-branch indices, committee bits, and
    /// signatures before returning the participation flags and builder-payment
    /// slot used by the mutation phase.
    fn accept_attestation(
        &self,
        attestation: &Attestation,
    ) -> Result<AcceptedAttestation, TransitionError> {
        let data = &attestation.data;
        let current = self.slot.epoch();
        let previous = self.previous_epoch();
        if data.target.epoch != previous && data.target.epoch != current {
            return Err(OperationError::AttestationTargetEpochInvalid.into());
        }
        if data.target.epoch != data.slot.epoch() {
            return Err(OperationError::AttestationTargetEpochInvalid.into());
        }
        if data.slot.as_u64() + MIN_ATTESTATION_INCLUSION_DELAY > self.slot.as_u64() {
            return Err(OperationError::AttestationSlotInvalid(data.slot).into());
        }
        if data.index.as_u64() >= 2 {
            return Err(OperationError::AttestationPayloadStatusInvalid.into());
        }

        for ci in committee_indices(&attestation.committee_bits) {
            if ci.as_u64() >= self.committee_count_per_slot(data.target.epoch) {
                return Err(BlockError::CommitteeIndexOutOfRange(ci).into());
            }
        }

        let attesting_indices = self.attesting_indices(attestation)?;
        if attesting_indices.is_empty() {
            return Err(OperationError::AttestationParticipantsEmpty.into());
        }
        let indexed = indexed_attestation_from_known_indices(attestation, &attesting_indices);
        self.validate_indexed_attestation(&indexed, SignatureError::Attestation)?;

        let inclusion_delay = self.slot.as_u64().saturating_sub(data.slot.as_u64());
        let participation_flags = self.participation_flags_for(data, inclusion_delay)?;
        let is_same_slot = self.is_attestation_same_slot(data);
        let builder_payment_index = self
            .builder_payment_index_for_slot(data.slot)
            .ok_or(OperationError::AttestationSlotInvalid(data.slot))?;
        Ok(AcceptedAttestation {
            attesting_indices,
            participation_flags,
            target_is_current_epoch: data.target.epoch == current,
            is_same_slot,
            builder_payment_index,
        })
    }

    /// Set participation flags and accumulate proposer-reward numerator.
    ///
    /// Same-slot beacon attestations also add effective balance to the pending
    /// builder payment. Payload attestations do not mutate that payment weight.
    fn record_attestation_participation(
        &mut self,
        accepted: &AcceptedAttestation,
    ) -> Result<u64, TransitionError> {
        let mut payment = self.builder_pending_payments[accepted.builder_payment_index];
        let mut proposer_reward_numerator: u64 = 0;

        for vi in &accepted.attesting_indices {
            let base = self.base_reward(*vi)?.as_u64();
            let mut will_set_new_flag = false;
            for (flag_index, weight) in PARTICIPATION_FLAG_WEIGHTS.iter().enumerate() {
                if !accepted.participation_flags.contains(&flag_index) {
                    continue;
                }
                let participation = if accepted.target_is_current_epoch {
                    &mut self.current_epoch_participation
                } else {
                    &mut self.previous_epoch_participation
                };
                let slot_flags = participation
                    .get(vi.as_usize())
                    .copied()
                    .unwrap_or_default();
                if !slot_flags.has_flag(flag_index)? {
                    participation[vi.as_usize()] = slot_flags.with_flag(flag_index)?;
                    proposer_reward_numerator =
                        proposer_reward_numerator.saturating_add(base.saturating_mul(*weight));
                    will_set_new_flag = true;
                }
            }
            if accepted.is_same_slot && will_set_new_flag && payment.withdrawal.amount.as_u64() > 0
            {
                self.record_builder_payment_weight_from_same_slot_attestation(&mut payment, *vi)?;
            }
        }
        self.builder_pending_payments[accepted.builder_payment_index] = payment;
        Ok(proposer_reward_numerator)
    }

    /// Add builder-payment quorum weight earned by a beacon attestation for a
    /// block fresh in its own slot.
    /// The pending builder payment is weighted by ordinary beacon
    /// attestation participation for the slot. Payload attestation objects vote
    /// on timeliness and data availability. They do not directly add this
    /// payment weight.
    fn record_builder_payment_weight_from_same_slot_attestation(
        &self,
        payment: &mut BuilderPendingPayment,
        validator_index: ValidatorIndex,
    ) -> Result<(), TransitionError> {
        let effective_balance = self.validator(validator_index)?.effective_balance;
        payment.weight = payment.weight.saturating_add(effective_balance);
        Ok(())
    }

    /// Pay the block proposer for newly recorded attestation participation.
    ///
    /// The numerator is accumulated only when a validator gains a new
    /// participation flag, so duplicate attestations do not keep paying the
    /// proposer.
    fn reward_attestation_proposer(
        &mut self,
        proposer_reward_numerator: u64,
    ) -> Result<(), TransitionError> {
        let proposer_reward_denominator = WEIGHT_DENOMINATOR.saturating_sub(PROPOSER_WEIGHT)
            * WEIGHT_DENOMINATOR
            / PROPOSER_WEIGHT;
        let proposer_reward = Gwei(proposer_reward_numerator / proposer_reward_denominator.max(1));
        let proposer = self.beacon_proposer_index()?;
        self.increase_balance(proposer, proposer_reward)?;
        Ok(())
    }

    /// Block root at `slot` from the historical ring buffer.
    ///
    /// Attestation processing uses this to compare a vote's claimed head and
    /// target against the state history.
    pub(crate) fn block_root_at_slot(&self, slot: Slot) -> Root {
        self.block_roots[slot % SLOTS_PER_HISTORICAL_ROOT]
    }
}

/// Build an indexed attestation from already decoded participant indices.
fn indexed_attestation_from_known_indices(
    attestation: &Attestation,
    attesting_indices: &[ValidatorIndex],
) -> IndexedAttestation {
    let mut indexed_indices = ssz_rs::List::<ValidatorIndex, MAX_ATTESTING_INDICES>::default();
    for vi in attesting_indices.iter().copied() {
        indexed_indices.push(vi);
    }
    IndexedAttestation {
        attesting_indices: indexed_indices,
        data: attestation.data,
        signature: attestation.signature,
    }
}
