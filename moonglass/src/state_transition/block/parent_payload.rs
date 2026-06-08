//! Parent-payload availability checks during block processing.

use crate::constants::SLOTS_PER_HISTORICAL_ROOT;
use crate::containers::{BeaconBlock, BeaconState, BuilderPendingWithdrawal, ExecutionRequests};
use crate::error::{BlockError, MerkleError, TransitionError};
use crate::primitives::{Hash32, Slot};
use crate::state_transition::TreeRootExt;

struct ParentPayloadCommitment {
    slot: Slot,
    block_hash: Hash32,
    payment: BuilderPendingWithdrawal,
}

impl BeaconState {
    /// Accept the parent payload commitment carried by `block`.
    pub fn accept_parent_payload_commitment(
        &mut self,
        block: &BeaconBlock,
    ) -> Result<(), TransitionError> {
        let Some(commitment) = self.parent_payload_commitment(block)? else {
            return Ok(());
        };
        self.apply_parent_execution_payload(&block.body.parent_execution_requests)?;
        self.settle_parent_payload_payment(&commitment)?;
        self.mark_parent_payload_available(&commitment);
        Ok(())
    }

    fn parent_payload_commitment(
        &self,
        block: &BeaconBlock,
    ) -> Result<Option<ParentPayloadCommitment>, TransitionError> {
        let bid = &block.body.signed_execution_payload_bid.message;
        let parent_bid = &self.latest_execution_payload_bid;
        let requests = &block.body.parent_execution_requests;

        if bid.parent_block_hash != parent_bid.block_hash {
            if !requests.is_empty() {
                return Err(BlockError::ParentPayloadRequestsMismatch.into());
            }
            return Ok(None);
        }

        let mut request_root_source = requests.clone();
        let requests_root = request_root_source.tree_root(MerkleError::ExecutionRequests)?;
        if requests_root != parent_bid.execution_requests_root {
            return Err(BlockError::ParentPayloadRequestsMismatch.into());
        }

        Ok(Some(ParentPayloadCommitment {
            slot: parent_bid.slot,
            block_hash: parent_bid.block_hash,
            payment: BuilderPendingWithdrawal {
                fee_recipient: parent_bid.fee_recipient,
                amount: parent_bid.value,
                builder_index: parent_bid.builder_index,
            },
        }))
    }

    fn settle_parent_payload_payment(
        &mut self,
        commitment: &ParentPayloadCommitment,
    ) -> Result<(), TransitionError> {
        if let Some(payment_index) = self.builder_payment_index_for_slot(commitment.slot) {
            self.settle_builder_payment(payment_index)?;
        } else if commitment.payment.amount.as_u64() > 0 {
            self.queue_builder_pending_withdrawal(commitment.payment)?;
        }
        Ok(())
    }

    fn mark_parent_payload_available(&mut self, commitment: &ParentPayloadCommitment) {
        self.execution_payload_availability
            .set(commitment.slot % SLOTS_PER_HISTORICAL_ROOT, true);
        self.latest_block_hash = commitment.block_hash;
    }

    /// Dispatch the per-request handlers for the parent payload's execution requests.
    pub fn apply_parent_execution_payload(
        &mut self,
        requests: &ExecutionRequests,
    ) -> Result<(), TransitionError> {
        for d in requests.deposits.iter() {
            self.process_deposit_request(d)?;
        }
        for w in requests.withdrawals.iter() {
            self.process_withdrawal_request(w)?;
        }
        for c in requests.consolidations.iter() {
            self.process_consolidation_request(c)?;
        }
        Ok(())
    }
}
