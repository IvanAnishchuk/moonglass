//! Taking in a beacon [attestation](crate::glossary#attestation).
//!
//! An attestation is a [validator's](crate::glossary#validator) vote, and it
//! carries two things at once: a [head](crate::glossary#head) vote, naming the
//! [block](crate::glossary#beacon-block) it sees as the tip of the chain, and a
//! [finality](crate::glossary#justification-and-finalization) vote, naming the
//! [checkpoint](crate::glossary#checkpoint) it wants to finalize. Fork choice
//! records it as that validator's latest message. There is also a payload-branch
//! rule specific to this design: a vote whose `index` is 0 backs the empty branch
//! and 1 backs the full branch, except that a vote for a block in its own
//! [slot](crate::glossary#slot) must use 0 and stays
//! [`Pending`](super::store::PayloadStatus::Pending), and a full-branch vote is
//! only accepted once that block's payload has actually been recorded.

use crate::containers::Attestation;
use crate::error::{ForkChoiceError, SignatureError};
use crate::primitives::ValidatorIndex;

use super::helpers::is_at_or_after_next_slot;
use super::store::{LatestMessage, Store};

impl Store {
    /// Reject a gossip attestation whose target epoch is too old or in the future.
    ///
    /// A vote arriving over the network is only considered when it targets the
    /// current or the previous epoch. Votes bundled inside a block skip this
    /// check, because the block's own validation already constrains their timing.
    pub fn validate_target_epoch_against_current_time(
        &self,
        attestation: &Attestation,
    ) -> Result<(), ForkChoiceError> {
        let target_epoch = attestation.data.target.epoch;
        let current_epoch = self.get_current_store_epoch();
        let previous_epoch = current_epoch.saturating_sub(1);
        if target_epoch != current_epoch && target_epoch != previous_epoch {
            return Err(ForkChoiceError::AttestationFromFutureEpoch);
        }
        Ok(())
    }

    /// Check every rule for admitting an attestation, without changing anything.
    ///
    /// This gathers the gatekeeping checks in one place: the target epoch and slot
    /// agree, both the target and the voted block are known, the vote is not for a
    /// future slot, the payload-branch `index` is legal for that block, a
    /// full-branch vote only follows a recorded payload, and the voted block
    /// really leads to the checkpoint it claims. It reads the store but writes
    /// nothing. Passing here is what lets [`Self::on_attestation`] go on to record
    /// the vote.
    pub fn validate_on_attestation(
        &self,
        attestation: &Attestation,
        is_from_block: bool,
    ) -> Result<(), ForkChoiceError> {
        let data = attestation.data;
        let target = data.target;

        if !is_from_block {
            self.validate_target_epoch_against_current_time(attestation)?;
        }

        if target.epoch != data.slot.epoch() {
            return Err(ForkChoiceError::AttestationLmdFfgMismatch);
        }

        if !self.blocks.contains_key(&target.root) {
            return Err(ForkChoiceError::UnknownBlock(target.root));
        }
        let block_slot = self
            .blocks
            .get(&data.beacon_block_root)
            .ok_or(ForkChoiceError::UnknownBlock(data.beacon_block_root))?
            .slot;
        if block_slot > data.slot {
            return Err(ForkChoiceError::AttestationTooEarly);
        }

        let index = data.index.as_u64();
        if index != 0 && index != 1 {
            return Err(ForkChoiceError::AttestationIndexInvalid(index));
        }
        if block_slot == data.slot && index != 0 {
            return Err(ForkChoiceError::AttestationIndexInvalid(index));
        }
        if index == 1 && !self.is_payload_verified(data.beacon_block_root) {
            return Err(ForkChoiceError::AttestationPayloadEnvelopeNotRecorded);
        }

        let checkpoint_root = self.get_checkpoint_block(data.beacon_block_root, target.epoch)?;
        if target.root != checkpoint_root {
            return Err(ForkChoiceError::AttestationLmdFfgMismatch);
        }

        if !is_at_or_after_next_slot(data.slot, self.get_current_slot()) {
            return Err(ForkChoiceError::AttestationTooEarly);
        }

        Ok(())
    }

    /// Record each attester's vote as their newest, unless it is stale.
    ///
    /// For every non-equivocating validator in the attestation, this overwrites
    /// their stored [`LatestMessage`] when this vote is for a newer slot than the
    /// one already held. The stored message keeps both the block root and the
    /// payload branch the vote chose, so scoring can later place it on the right
    /// [`ForkChoiceNode`](super::store::ForkChoiceNode).
    pub fn update_latest_messages(
        &mut self,
        attesting_indices: &[ValidatorIndex],
        attestation: &Attestation,
    ) {
        let slot = attestation.data.slot;
        let root = attestation.data.beacon_block_root;
        let payload_present = attestation.data.index.as_u64() == 1;
        for index in attesting_indices {
            if self.equivocating_indices.contains(index) {
                continue;
            }
            let should_write = self
                .latest_messages
                .get(index)
                .is_none_or(|existing| slot > existing.slot);
            if should_write {
                self.latest_messages.insert(
                    *index,
                    LatestMessage {
                        slot,
                        root,
                        payload_present,
                    },
                );
            }
        }
    }

    /// Validate an attestation and store it as its voters' latest message.
    ///
    /// The full path for one attestation: run every check in
    /// [`Self::validate_on_attestation`], make sure the target checkpoint's state
    /// is cached, verify the aggregate signature against it, then record the vote
    /// with [`Self::update_latest_messages`]. From this point each voter's weight
    /// counts toward whatever node their vote supports.
    ///
    /// Runs on a scratch copy and commits only on success, so a rejected
    /// attestation leaves the store unchanged.
    pub fn on_attestation(
        &mut self,
        attestation: &Attestation,
        is_from_block: bool,
    ) -> Result<(), ForkChoiceError> {
        let mut scratch = self.clone();
        scratch.on_attestation_inner(attestation, is_from_block)?;
        *self = scratch;
        Ok(())
    }

    /// Non-transactional body of [`Self::on_attestation`], used by the block-import
    /// path so it does not clone the store again for every embedded message.
    pub fn on_attestation_inner(
        &mut self,
        attestation: &Attestation,
        is_from_block: bool,
    ) -> Result<(), ForkChoiceError> {
        self.validate_on_attestation(attestation, is_from_block)?;
        self.store_target_checkpoint_state(attestation.data.target)?;
        // Validate against the cached target state inside a scoped immutable
        // borrow, then drop it so the vote can be written without cloning the
        // state.
        let indexed = {
            let target_state = self
                .checkpoint_states
                .get(&attestation.data.target)
                .ok_or(ForkChoiceError::UnknownBlock(attestation.data.target.root))?;
            let indexed = target_state.indexed_attestation(attestation)?;
            target_state.validate_indexed_attestation(&indexed, SignatureError::Attestation)?;
            indexed
        };
        self.update_latest_messages(&indexed.attesting_indices, attestation);
        Ok(())
    }
}
