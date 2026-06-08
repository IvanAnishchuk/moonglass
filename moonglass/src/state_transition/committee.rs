//! Committee and proposer-assignment behavior.
//!
//! Committees distribute validator duties so no small, predictable set controls
//! attestations for a slot. The flow is: collect the active validator set,
//! derive a RANDAO-backed seed, conceptually shuffle the active set, then slice
//! it into committees across the epoch. Proposer sampling uses the same
//! randomness with effective-balance weighting. Per-slot proposer lookup reads
//! the precomputed `state.proposer_lookahead`.

// Safe: spec-bounded `usize`<->`u64` casts on committee indices.
#![allow(clippy::cast_possible_truncation)]

use sha2::{Digest, Sha256};

use crate::constants::{
    DOMAIN_BEACON_ATTESTER, DOMAIN_PTC_ATTESTER, EPOCHS_PER_HISTORICAL_VECTOR,
    MAX_COMMITTEES_PER_SLOT, MAX_EFFECTIVE_BALANCE, MIN_SEED_LOOKAHEAD, PTC_SIZE,
    SHUFFLE_ROUND_COUNT, SLOTS_PER_EPOCH, TARGET_COMMITTEE_SIZE,
};
use crate::containers::BeaconState;
use crate::error::{BlockError, TransitionError};
use crate::primitives::{Bytes32, CommitteeIndex, DomainType, Epoch, Slot, ValidatorIndex};
use crate::state_transition::BeaconStateLookup;

/// Random-sample cap of `2**16 - 1` used by effective-balance-weighted
/// validator sampling in `compute_proposer_index` and `next_sync_committee_indices`.
pub(crate) const MAX_RANDOM_VALUE: u64 = (1 << 16) - 1;

impl BeaconState {
    /// Proposer for the state's current slot.
    pub fn beacon_proposer_index(&self) -> Result<ValidatorIndex, TransitionError> {
        let offset = self.slot % SLOTS_PER_EPOCH;
        self.proposer_lookahead
            .get(offset)
            .copied()
            .ok_or_else(|| BlockError::ProposerLookaheadOutOfRange(self.slot).into())
    }

    /// Indices of validators active during `epoch`.
    #[must_use]
    pub fn active_validator_indices(&self, epoch: Epoch) -> Vec<ValidatorIndex> {
        self.validators
            .iter()
            .enumerate()
            .filter_map(|(i, v)| v.is_active_at(epoch).then_some(ValidatorIndex(i as u64)))
            .collect()
    }

    /// Indices of validators active and not slashed during `epoch`.
    ///
    /// Used for proposer selection: the candidate set excludes slashed validators.
    #[must_use]
    pub fn active_unslashed_validator_indices(&self, epoch: Epoch) -> Vec<ValidatorIndex> {
        self.validators
            .iter()
            .enumerate()
            .filter_map(|(i, v)| {
                (v.is_active_at(epoch) && !v.slashed).then_some(ValidatorIndex(i as u64))
            })
            .collect()
    }

    /// RANDAO ring-buffer slot for `epoch`.
    #[must_use]
    pub fn randao_mix(&self, epoch: Epoch) -> Bytes32 {
        self.randao_mixes[epoch % EPOCHS_PER_HISTORICAL_VECTOR]
    }

    /// 32-byte seed mixing the domain tag, the epoch, and the randao value.
    #[must_use]
    pub fn seed(&self, epoch: Epoch, domain_type: DomainType) -> Bytes32 {
        let lookback = EPOCHS_PER_HISTORICAL_VECTOR as u64 - MIN_SEED_LOOKAHEAD as u64 - 1;
        let mix = self.randao_mix(epoch.saturating_add(lookback));
        let mut hasher = Sha256::new();
        hasher.update(domain_type.0);
        hasher.update(epoch.as_u64().to_le_bytes());
        hasher.update(mix);
        hasher.finalize().into()
    }

    /// Number of beacon committees produced per slot in `epoch`.
    #[must_use]
    pub fn committee_count_per_slot(&self, epoch: Epoch) -> u64 {
        let active = self.active_validator_indices(epoch).len() as u64;
        let raw = active / SLOTS_PER_EPOCH as u64 / TARGET_COMMITTEE_SIZE;
        raw.min(MAX_COMMITTEES_PER_SLOT as u64).max(1)
    }

    /// Beacon committee at (`slot`, `committee_index`).
    pub fn beacon_committee(
        &self,
        slot: Slot,
        committee_index: CommitteeIndex,
    ) -> Result<Vec<ValidatorIndex>, TransitionError> {
        let epoch = slot.epoch();
        let committees_per_slot = self.committee_count_per_slot(epoch);
        if committee_index.as_u64() >= committees_per_slot {
            return Err(BlockError::CommitteeIndexOutOfRange(committee_index).into());
        }
        let indices = self.active_validator_indices(epoch);
        let seed = self.seed(epoch, DOMAIN_BEACON_ATTESTER);
        let count = committees_per_slot * SLOTS_PER_EPOCH as u64;
        let index =
            (slot % SLOTS_PER_EPOCH as u64) * committees_per_slot + committee_index.as_u64();
        Ok(compute_committee(&indices, seed, index, count))
    }

    /// Effective-balance-weighted random proposer sample from `indices`.
    ///
    /// Spec: `compute_proposer_index`.
    pub fn compute_proposer_index(
        &self,
        indices: &[ValidatorIndex],
        seed: Bytes32,
    ) -> Result<ValidatorIndex, TransitionError> {
        if indices.is_empty() {
            return Err(BlockError::EmptyActiveValidatorSet.into());
        }
        let total = indices.len() as u64;
        let mut i: u64 = 0;
        loop {
            let candidate = indices[compute_shuffled_index(i % total, total, seed) as usize];
            let random_bytes = {
                let mut hasher = Sha256::new();
                hasher.update(seed);
                hasher.update((i / 16).to_le_bytes());
                hasher.finalize()
            };
            let offset = ((i % 16) * 2) as usize;
            let random_value = u64::from(u16::from_le_bytes([
                random_bytes[offset],
                random_bytes[offset + 1],
            ]));
            let effective_balance = self.validator(candidate)?.effective_balance.as_u64();
            if effective_balance.saturating_mul(MAX_RANDOM_VALUE)
                >= MAX_EFFECTIVE_BALANCE.as_u64().saturating_mul(random_value)
            {
                return Ok(candidate);
            }
            i = i.saturating_add(1);
        }
    }

    /// Sample `size` indices from `candidates`, weighted by each candidate's
    /// effective balance. When `shuffle_indices` is true the candidate ordering
    /// is itself permuted through `compute_shuffled_index`. Otherwise the
    /// candidate list is traversed in order. Duplicates are possible.
    pub fn compute_balance_weighted_selection(
        &self,
        candidates: &[ValidatorIndex],
        seed: Bytes32,
        size: usize,
        shuffle_indices: bool,
    ) -> Result<Vec<ValidatorIndex>, TransitionError> {
        if candidates.is_empty() {
            return Err(BlockError::EmptyActiveValidatorSet.into());
        }
        let total = candidates.len() as u64;
        let effective_balances: Vec<u64> = candidates
            .iter()
            .map(|i| self.validator(*i).map(|v| v.effective_balance.as_u64()))
            .collect::<Result<Vec<_>, _>>()?;
        let mut selected: Vec<ValidatorIndex> = Vec::with_capacity(size);
        let mut i: u64 = 0;
        let mut random_bytes = [0_u8; 32];
        while selected.len() < size {
            let offset = ((i % 16) * 2) as usize;
            if offset == 0 {
                let mut hasher = Sha256::new();
                hasher.update(seed);
                hasher.update((i / 16).to_le_bytes());
                random_bytes = hasher.finalize().into();
            }
            let mut next_index = i % total;
            if shuffle_indices {
                next_index = compute_shuffled_index(next_index, total, seed);
            }
            let weight = effective_balances[next_index as usize].saturating_mul(MAX_RANDOM_VALUE);
            let random_value = u64::from(u16::from_le_bytes([
                random_bytes[offset],
                random_bytes[offset + 1],
            ]));
            let threshold = MAX_EFFECTIVE_BALANCE.as_u64().saturating_mul(random_value);
            if weight >= threshold {
                selected.push(candidates[next_index as usize]);
            }
            i = i.saturating_add(1);
        }
        Ok(selected)
    }

    /// Payload-timeliness committee for `slot`. Concatenates every beacon
    /// committee at this slot, derives a slot-specific seed, then samples
    /// `PTC_SIZE` indices by effective balance without further shuffling.
    pub fn compute_ptc(&self, slot: Slot) -> Result<Vec<ValidatorIndex>, TransitionError> {
        let epoch = slot.epoch();
        let base_seed = self.seed(epoch, DOMAIN_PTC_ATTESTER);
        let seed: Bytes32 = {
            let mut hasher = Sha256::new();
            hasher.update(base_seed);
            hasher.update(slot.as_u64().to_le_bytes());
            hasher.finalize().into()
        };
        let committees_per_slot = self.committee_count_per_slot(epoch);
        let mut indices: Vec<ValidatorIndex> = Vec::new();
        for ci in 0..committees_per_slot {
            let committee = self.beacon_committee(slot, CommitteeIndex(ci))?;
            indices.extend(committee);
        }
        self.compute_balance_weighted_selection(&indices, seed, PTC_SIZE, false)
    }
}

/// Swap-or-not shuffle locating `index` inside a population of `index_count`
/// elements, seeded by `seed`.
///
/// The result is deterministic across `SHUFFLE_ROUND_COUNT` rounds.
///
/// # Panics
///
/// Panics if `index >= index_count`. Callers must validate the bound.
#[must_use]
pub fn compute_shuffled_index(index: u64, index_count: u64, seed: Bytes32) -> u64 {
    assert!(index < index_count, "shuffle index out of range");
    let mut current = index;
    for round in 0..SHUFFLE_ROUND_COUNT {
        let round_byte = u8::try_from(round).expect("SHUFFLE_ROUND_COUNT fits in u8");
        let pivot = pivot_for_round(seed, round_byte, index_count);
        let flip = (pivot + index_count - current) % index_count;
        let position = current.max(flip);
        let bit = source_bit(seed, round_byte, position);
        if bit == 1 {
            current = flip;
        }
    }
    current
}

/// Round pivot: lower 8 bytes of `hash(seed || round)`, mod `index_count`.
fn pivot_for_round(seed: Bytes32, round_byte: u8, index_count: u64) -> u64 {
    let mut hasher = Sha256::new();
    hasher.update(seed);
    hasher.update([round_byte]);
    let digest = hasher.finalize();
    u64::from_le_bytes(digest[..8].try_into().expect("8-byte prefix")) % index_count
}

/// Bit selector for the swap-or-not source, derived from `hash(seed || round
/// || position/256)`.
fn source_bit(seed: Bytes32, round_byte: u8, position: u64) -> u8 {
    let position_chunk = u32::try_from(position / 256).expect("position chunk fits in u32");
    let mut hasher = Sha256::new();
    hasher.update(seed);
    hasher.update([round_byte]);
    hasher.update(position_chunk.to_le_bytes());
    let source = hasher.finalize();
    let byte_index = ((position % 256) / 8) as usize;
    let bit_index = (position % 8) as u8;
    (source[byte_index] >> bit_index) & 1
}

/// Slice of shuffled indices forming committee `index` of `count`.
///
/// Example in words: if an epoch has `SLOTS_PER_EPOCH * committees_per_slot`
/// committees, each committee index selects one contiguous slice of the
/// shuffled active set.
#[must_use]
pub fn compute_committee(
    indices: &[ValidatorIndex],
    seed: Bytes32,
    index: u64,
    count: u64,
) -> Vec<ValidatorIndex> {
    let total = indices.len() as u64;
    let start = (total * index / count) as usize;
    let end = (total * (index + 1) / count) as usize;
    (start..end)
        .map(|i| indices[compute_shuffled_index(i as u64, total, seed) as usize])
        .collect()
}

/// Set committee indices encoded by a `committee_bits` bitvector.
#[must_use]
pub fn committee_indices<const N: usize>(
    committee_bits: &ssz_rs::Bitvector<N>,
) -> Vec<CommitteeIndex> {
    committee_bits
        .iter()
        .enumerate()
        .filter_map(|(i, bit)| {
            if *bit {
                Some(CommitteeIndex(i as u64))
            } else {
                None
            }
        })
        .collect()
}
