//! Next sync-committee selection.

use sha2::{Digest, Sha256};

use crate::constants::{
    DOMAIN_SYNC_COMMITTEE, EPOCHS_PER_SYNC_COMMITTEE_PERIOD, MAX_EFFECTIVE_BALANCE,
    SYNC_COMMITTEE_SIZE,
};
use crate::containers::{BeaconState, SyncCommittee};
use crate::error::{BlockError, TransitionError};
use crate::primitives::{BLSPubkey, Epoch, ValidatorIndex};
use crate::state_transition::committee::MAX_RANDOM_VALUE;
use crate::state_transition::{BeaconStateLookup, aggregate_pubkeys, compute_shuffled_index};

impl BeaconState {
    /// At a sync-committee period boundary, roll next into current and resample
    /// the next sync committee from the active set at the following epoch.
    ///
    /// Spec: `process_sync_committee_updates`
    pub fn process_sync_committee_updates(&mut self) -> Result<(), TransitionError> {
        let next_epoch = self.slot.epoch().as_u64() + 1;
        if next_epoch.is_multiple_of(EPOCHS_PER_SYNC_COMMITTEE_PERIOD) {
            let new_next_sync_committee = self.compute_next_sync_committee()?;
            self.current_sync_committee =
                std::mem::replace(&mut self.next_sync_committee, new_next_sync_committee);
        }
        Ok(())
    }

    /// Sample a fresh sync committee for the next sync-committee period.
    fn compute_next_sync_committee(&self) -> Result<SyncCommittee, TransitionError> {
        let target = Epoch::new(self.slot.epoch().as_u64() + 1);
        let indices = self.next_sync_committee_indices(target)?;
        let pubkeys: Vec<BLSPubkey> = indices
            .iter()
            .map(|i| self.validator(*i).map(|v| v.pubkey))
            .collect::<Result<_, _>>()?;
        let aggregate_pubkey = aggregate_pubkeys(&pubkeys)?;
        let mut pk_vec = ssz_rs::Vector::<BLSPubkey, SYNC_COMMITTEE_SIZE>::default();
        for (i, pk) in pubkeys.iter().enumerate().take(SYNC_COMMITTEE_SIZE) {
            pk_vec[i] = *pk;
        }
        Ok(SyncCommittee {
            pubkeys: pk_vec,
            aggregate_pubkey,
        })
    }

    /// Effective-balance-weighted sampling of `SYNC_COMMITTEE_SIZE` validators
    /// from the active set at `epoch`.
    fn next_sync_committee_indices(
        &self,
        epoch: Epoch,
    ) -> Result<Vec<ValidatorIndex>, TransitionError> {
        let active = self.active_validator_indices(epoch);
        let active_count = active.len() as u64;
        if active_count == 0 {
            return Err(BlockError::EmptyActiveValidatorSet.into());
        }
        let seed = self.seed(epoch, DOMAIN_SYNC_COMMITTEE);
        let mut out: Vec<ValidatorIndex> = Vec::with_capacity(SYNC_COMMITTEE_SIZE);
        let mut i: u64 = 0;
        while out.len() < SYNC_COMMITTEE_SIZE {
            let shuffled = compute_shuffled_index(i % active_count, active_count, seed);
            let candidate = active[shuffled as usize];
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
            let effective = self.validator(candidate)?.effective_balance.as_u64();
            if effective.saturating_mul(MAX_RANDOM_VALUE)
                >= MAX_EFFECTIVE_BALANCE.as_u64().saturating_mul(random_value)
            {
                out.push(candidate);
            }
            i = i.saturating_add(1);
        }
        Ok(out)
    }
}
