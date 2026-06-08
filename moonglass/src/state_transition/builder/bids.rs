//! Builder bid validation and selection.

use crate::constants::{
    BLOB_SCHEDULE, BUILDER_INDEX_SELF_BUILD, DOMAIN_BEACON_BUILDER, MAX_BLOBS_PER_BLOCK,
    MIN_DEPOSIT_AMOUNT, SLOTS_PER_EPOCH,
};
use crate::containers::{
    BeaconBlock, BeaconState, BuilderPendingPayment, BuilderPendingWithdrawal, ExecutionPayloadBid,
    SignedExecutionPayloadBid,
};
use crate::error::{MerkleError, OperationError, SignatureError, TransitionError};
use crate::primitives::{BuilderIndex, Epoch, Gwei};
use crate::state_transition::{BeaconStateLookup, verify_signature};

impl BeaconState {
    /// Active blob commitment limit for `epoch`.
    #[must_use]
    pub fn max_blobs_per_block_at(epoch: Epoch) -> usize {
        let active = BLOB_SCHEDULE
            .iter()
            .rev()
            .find_map(|(entry_epoch, limit)| (epoch >= *entry_epoch).then_some(*limit))
            .unwrap_or(MAX_BLOBS_PER_BLOCK);
        usize::try_from(active).unwrap_or(usize::MAX)
    }

    /// True when the builder's stake balance is large enough to cover the bid
    /// while keeping `MIN_DEPOSIT_AMOUNT` plus all already-queued outflows
    /// reserved. Queued outflows are both `builder_pending_withdrawals` and the
    /// payment-side of `builder_pending_payments`.
    #[must_use]
    pub fn builder_balance_covers_bid(&self, builder_index: BuilderIndex, bid_value: Gwei) -> bool {
        if builder_index == BUILDER_INDEX_SELF_BUILD {
            return true;
        }
        let Some(builder) = self.builders.get(builder_index.as_usize()) else {
            return false;
        };
        let pending = self
            .pending_balance_to_withdraw_for_builder(builder_index)
            .as_u64();
        let min_balance = MIN_DEPOSIT_AMOUNT.as_u64().saturating_add(pending);
        let balance = builder.balance.as_u64();
        if balance < min_balance {
            return false;
        }
        balance - min_balance >= bid_value.as_u64()
    }

    /// Verify the builder's BLS signature on a payload bid. Self-build bids skip
    /// signature verification.
    pub fn verify_execution_payload_bid_signature(
        &self,
        signed_bid: &SignedExecutionPayloadBid,
    ) -> Result<(), TransitionError> {
        let builder_index = signed_bid.message.builder_index;
        if builder_index == BUILDER_INDEX_SELF_BUILD {
            return Ok(());
        }
        let builder = self.builder(builder_index)?;
        let mut bid_msg = signed_bid.message.clone();
        let signing_root = self.signing_root_for(
            &mut bid_msg,
            DOMAIN_BEACON_BUILDER,
            self.slot.epoch(),
            MerkleError::ExecutionPayloadBid,
        )?;
        verify_signature(
            &builder.pubkey,
            signing_root,
            &signed_bid.signature,
            SignatureError::ExecutionPayloadBid(builder_index),
        )
    }

    /// Accept a proposer-committed builder bid for the current slot.
    ///
    /// Spec: `process_execution_payload_bid`
    pub fn process_execution_payload_bid(
        &mut self,
        block: &BeaconBlock,
    ) -> Result<(), TransitionError> {
        let signed_bid = &block.body.signed_execution_payload_bid;
        let bid = &signed_bid.message;

        self.validate_bid_identity(block, bid)?;
        self.validate_bid_signer_and_funding(signed_bid)?;
        self.record_accepted_bid(bid);
        Ok(())
    }

    fn validate_bid_identity(
        &self,
        block: &BeaconBlock,
        bid: &ExecutionPayloadBid,
    ) -> Result<(), TransitionError> {
        if bid.slot != self.slot || bid.slot != block.slot {
            return Err(OperationError::BuilderBidSlotMismatch.into());
        }

        if bid.parent_block_root != block.parent_root {
            return Err(OperationError::BuilderBidParentMismatch.into());
        }
        if bid.parent_block_hash != self.latest_block_hash {
            return Err(OperationError::BuilderBidParentMismatch.into());
        }
        if bid.prev_randao != self.randao_mix(self.slot.epoch()) {
            return Err(OperationError::BuilderBidRandaoMismatch.into());
        }
        let blob_limit = Self::max_blobs_per_block_at(self.slot.epoch());
        if bid.blob_kzg_commitments.len() > blob_limit {
            return Err(OperationError::BuilderBidBlobLimitExceeded {
                got: bid.blob_kzg_commitments.len(),
                max: blob_limit,
            }
            .into());
        }
        Ok(())
    }

    fn validate_bid_signer_and_funding(
        &self,
        signed_bid: &SignedExecutionPayloadBid,
    ) -> Result<(), TransitionError> {
        let bid = &signed_bid.message;
        let builder_index = bid.builder_index;
        if builder_index == BUILDER_INDEX_SELF_BUILD {
            if bid.value.as_u64() != 0 {
                return Err(OperationError::BuilderBidSelfBuildNonZero.into());
            }
            if !signed_bid.signature.is_g2_point_at_infinity() {
                return Err(OperationError::BuilderBidSelfBuildSignature.into());
            }
            return Ok(());
        }

        if !self.is_active_builder(builder_index)? {
            return Err(OperationError::BuilderNotActive(builder_index).into());
        }
        if !self.builder_balance_covers_bid(builder_index, bid.value) {
            return Err(OperationError::BuilderInsufficientBalance(builder_index).into());
        }
        self.verify_execution_payload_bid_signature(signed_bid)
    }

    fn record_accepted_bid(&mut self, bid: &ExecutionPayloadBid) {
        self.latest_execution_payload_bid = bid.clone();
        if bid.value.as_u64() == 0 {
            return;
        }
        let payment = BuilderPendingPayment {
            weight: Gwei::ZERO,
            withdrawal: BuilderPendingWithdrawal {
                fee_recipient: bid.fee_recipient,
                amount: bid.value,
                builder_index: bid.builder_index,
            },
        };
        let window_index = SLOTS_PER_EPOCH + bid.slot % SLOTS_PER_EPOCH;
        self.builder_pending_payments[window_index] = payment;
    }
}
