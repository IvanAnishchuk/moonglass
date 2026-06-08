//! Builder payment window accounting and pending withdrawals.

use crate::constants::{
    BUILDER_PAYMENT_THRESHOLD_DENOMINATOR, BUILDER_PAYMENT_THRESHOLD_NUMERATOR,
    BUILDER_PENDING_WITHDRAWALS_LIMIT, SLOTS_PER_EPOCH,
};
use crate::containers::{BeaconState, BuilderPendingPayment, BuilderPendingWithdrawal};
use crate::error::{OperationError, TransitionError};
use crate::primitives::{BuilderIndex, Gwei, Slot};

impl BeaconState {
    /// Builder-payment window index for a slot in the current or previous epoch.
    #[must_use]
    pub fn builder_payment_index_for_slot(&self, slot: Slot) -> Option<usize> {
        let offset = slot % SLOTS_PER_EPOCH;
        if slot.epoch() == self.slot.epoch() {
            Some(SLOTS_PER_EPOCH + offset)
        } else if slot.epoch() == self.previous_epoch() {
            Some(offset)
        } else {
            None
        }
    }

    /// Quorum threshold for releasing a builder payment, computed from the active
    /// stake and the protocol fraction.
    #[must_use]
    pub fn builder_payment_quorum_threshold(&self) -> Gwei {
        let per_slot_balance = self.total_active_balance().as_u64() / SLOTS_PER_EPOCH as u64;
        Gwei(
            per_slot_balance.saturating_mul(BUILDER_PAYMENT_THRESHOLD_NUMERATOR)
                / BUILDER_PAYMENT_THRESHOLD_DENOMINATOR.max(1),
        )
    }

    /// Sum of balance owed to `builder_index` across both pending queues.
    ///
    /// The spec definition walks `builder_pending_withdrawals` (already-scheduled
    /// withdrawals from past slots) plus `builder_pending_payments` (the active
    /// payment window not yet finalized into withdrawals). Both must drain to
    /// zero before a builder may exit.
    #[must_use]
    pub fn pending_balance_to_withdraw_for_builder(&self, builder_index: BuilderIndex) -> Gwei {
        let withdrawals = self
            .builder_pending_withdrawals
            .iter()
            .filter(|w| w.builder_index == builder_index)
            .map(|w| w.amount)
            .fold(Gwei::ZERO, Gwei::saturating_add);
        let payments = self
            .builder_pending_payments
            .iter()
            .filter(|p| p.withdrawal.builder_index == builder_index)
            .map(|p| p.withdrawal.amount)
            .fold(Gwei::ZERO, Gwei::saturating_add);
        withdrawals.saturating_add(payments)
    }

    /// Release and clear the pending payment at `payment_index` when it is non-zero.
    pub fn settle_builder_payment(&mut self, payment_index: usize) -> Result<(), TransitionError> {
        let Some(payment) = self.builder_pending_payments.get(payment_index).copied() else {
            return Err(OperationError::BuilderPaymentIndexOutOfRange.into());
        };
        if payment.withdrawal.amount.as_u64() > 0 {
            self.queue_builder_pending_withdrawal(payment.withdrawal)?;
        }
        self.builder_pending_payments[payment_index] = BuilderPendingPayment::default();
        Ok(())
    }

    /// Release `payment` into the pending-withdrawal queue if it crossed the
    /// payload-timeliness quorum threshold.
    pub fn settle_builder_payment_if_quorum(
        &mut self,
        payment: BuilderPendingPayment,
    ) -> Result<(), TransitionError> {
        if payment.withdrawal.amount.as_u64() > 0
            && payment.weight >= self.builder_payment_quorum_threshold()
        {
            self.queue_builder_pending_withdrawal(payment.withdrawal)?;
        }
        Ok(())
    }

    /// Append a builder-pending withdrawal, rejecting when the queue is at its
    /// hard cap.
    pub(crate) fn queue_builder_pending_withdrawal(
        &mut self,
        withdrawal: BuilderPendingWithdrawal,
    ) -> Result<(), TransitionError> {
        if self.builder_pending_withdrawals.len() >= BUILDER_PENDING_WITHDRAWALS_LIMIT {
            return Err(OperationError::BuilderPendingWithdrawalsFull.into());
        }
        self.builder_pending_withdrawals.push(withdrawal);
        Ok(())
    }
}
