//! Parent-payload availability checks during block processing.
//!
//! The protocol separates the current block's builder bid from the parent
//! block's delivered payload. A child block first proves and applies the parent
//! payload's execution requests, then releases the parent builder payment and
//! marks the parent payload available.

use crate::constants::SLOTS_PER_HISTORICAL_ROOT;
use crate::containers::{BeaconBlock, BeaconState, BuilderPendingWithdrawal, ExecutionRequests};
use crate::error::{BlockError, MerkleError, TransitionError};
use crate::primitives::{Hash32, Slot};
use crate::state_transition::TreeRootExt;

/// Verified parent-payload data that the child block is allowed to settle.
struct ParentPayloadCommitment {
    /// Slot whose payload is being settled.
    slot: Slot,
    /// Execution block hash promised by the parent bid.
    block_hash: Hash32,
    /// Builder payment to release or queue when the parent payload is accepted.
    payment: BuilderPendingWithdrawal,
}

impl BeaconState {
    /// Settle the parent block's delivered payload as the first phase of block processing.
    ///
    /// This is the cross-slot handoff that runs before the current block's own
    /// identity and bid are touched. The block's bid must name the parent
    /// payload's `block_hash` through `parent_block_hash`, and when it does the
    /// carried `parent_execution_requests` must hash-match the request root the
    /// parent bid committed to. Once proven, the parent's deposit, withdrawal,
    /// and consolidation requests are applied, the parent builder payment is
    /// released, the parent bid slot's payload-availability bit is set, and
    /// `latest_block_hash` advances to the parent payload's block hash. A bid that
    /// extends no parent payload carries no requests and is a no-op, while
    /// requests that do not match raise [`BlockError::ParentPayloadRequestsMismatch`].
    /// This runs as a phase of [`BeaconState::process_block`], which itself
    /// operates on the clone [`BeaconState::apply_signed_block`] commits only
    /// after the whole transition succeeds, so a mid-phase failure is discarded
    /// with that clone rather than left in the committed state.
    pub fn accept_parent_payload_commitment(
        &mut self,
        block: &BeaconBlock,
    ) -> Result<(), TransitionError> {
        let Some(commitment) = self.verify_parent_payload_commitment(block)? else {
            return Ok(());
        };
        self.apply_parent_execution_requests(&block.body.parent_execution_requests)?;
        self.release_parent_builder_payment(&commitment)?;
        self.mark_parent_payload_available(&commitment);
        Ok(())
    }

    /// Verify whether `block` extends and proves the parent payload commitment.
    fn verify_parent_payload_commitment(
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

    /// Release or queue the builder payment tied to a settled parent payload.
    fn release_parent_builder_payment(
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

    /// Mark the parent slot's payload as available and advance `latest_block_hash`.
    fn mark_parent_payload_available(&mut self, commitment: &ParentPayloadCommitment) {
        self.execution_payload_availability
            .set(commitment.slot % SLOTS_PER_HISTORICAL_ROOT, true);
        self.latest_block_hash = commitment.block_hash;
    }

    /// Apply execution-layer requests delivered by the proven parent payload.
    fn apply_parent_execution_requests(
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
        for d in requests.builder_deposits.iter() {
            self.process_builder_deposit_request(d)?;
        }
        for e in requests.builder_exits.iter() {
            self.process_builder_exit_request(e)?;
        }
        Ok(())
    }
}
