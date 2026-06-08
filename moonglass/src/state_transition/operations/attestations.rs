//! `process_attestation` and per-attestation reward accounting.

use crate::constants::{
    DOMAIN_BEACON_ATTESTER, MAX_ATTESTING_INDICES, MIN_ATTESTATION_INCLUSION_DELAY,
    PARTICIPATION_FLAG_WEIGHTS, PROPOSER_WEIGHT, SLOTS_PER_EPOCH, SLOTS_PER_HISTORICAL_ROOT,
    TIMELY_HEAD_FLAG_INDEX, TIMELY_SOURCE_FLAG_INDEX, TIMELY_TARGET_FLAG_INDEX, WEIGHT_DENOMINATOR,
};
use crate::containers::{Attestation, AttestationData, BeaconState, IndexedAttestation};
use crate::error::{BlockError, MerkleError, OperationError, SignatureError, TransitionError};
use crate::primitives::{BLSPubkey, Gwei, Root, Slot, ValidatorIndex};
use crate::state_transition::balance::isqrt_u64;
use crate::state_transition::{
    BeaconStateLookup, committee_indices, compute_signing_root, fast_aggregate_verify,
};

struct AcceptedAttestation {
    attesting_indices: Vec<ValidatorIndex>,
    participation_flags: Vec<usize>,
    target_is_current_epoch: bool,
    is_same_slot: bool,
    builder_payment_index: usize,
}

impl BeaconState {
    /// Decode committee-bit aggregation into sorted attesting validator indices.
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

    /// Validate `indexed` and verify its aggregate signature under
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

    /// Validate and apply a committee-bit attestation with per-attester
    /// participation flag updates and proposer reward.
    ///
    /// Spec: `process_attestation`
    pub fn process_attestation(
        &mut self,
        attestation: &Attestation,
    ) -> Result<(), TransitionError> {
        let accepted = self.accept_attestation(attestation)?;
        let proposer_reward_numerator = self.record_attestation_participation(&accepted)?;
        self.reward_attestation_proposer(proposer_reward_numerator)
    }

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
                let eb = self.validator(*vi)?.effective_balance;
                payment.weight = payment.weight.saturating_add(eb);
            }
        }
        self.builder_pending_payments[accepted.builder_payment_index] = payment;
        Ok(proposer_reward_numerator)
    }

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
    pub(crate) fn block_root_at_slot(&self, slot: Slot) -> Root {
        self.block_roots[slot % SLOTS_PER_HISTORICAL_ROOT]
    }
}

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
