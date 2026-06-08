//! The full consensus state.
//!
//! `BeaconState` combines the validator registry, historical ring buffers,
//! processing queues, builder state, and per-slot working data.
//! Conceptually, it is the chain clock and identity, recent block/state roots,
//! randomness, validator registry and balances, participation/finality records,
//! execution and withdrawal handoff, lifecycle queues, proposer lookahead, and
//! builder/payload-timeliness state.

use crate::constants::{
    BUILDER_PAYMENT_WINDOW_LEN, BUILDER_PENDING_WITHDRAWALS_LIMIT, BUILDER_REGISTRY_LIMIT,
    EPOCHS_PER_HISTORICAL_VECTOR, EPOCHS_PER_SLASHINGS_VECTOR, ETH1_DATA_VOTES_LEN,
    HISTORICAL_ROOTS_LIMIT, JUSTIFICATION_BITS_LENGTH, MAX_WITHDRAWALS_PER_PAYLOAD,
    PENDING_CONSOLIDATIONS_LIMIT, PENDING_DEPOSITS_LIMIT, PENDING_PARTIAL_WITHDRAWALS_LIMIT,
    PROPOSER_LOOKAHEAD_LEN, PTC_SIZE, PTC_WINDOW_LEN, SLOTS_PER_HISTORICAL_ROOT,
    VALIDATOR_REGISTRY_LIMIT,
};
use crate::containers::{
    BeaconBlockHeader, Builder, BuilderPendingPayment, BuilderPendingWithdrawal, Checkpoint,
    Eth1Data, ExecutionPayloadBid, Fork, HistoricalSummary, PendingConsolidation, PendingDeposit,
    PendingPartialWithdrawal, SyncCommittee, Validator, Withdrawal,
};
use crate::primitives::{
    BuilderIndex, Bytes32, Epoch, Gwei, Hash32, ParticipationFlags, Root, Slot, ValidatorIndex,
    WithdrawalIndex,
};
use ssz_rs::prelude::*;

/// Complete consensus snapshot needed to validate the next slot or block.
///
/// Consensus is convergence on this object: validators that apply the same
/// valid slots and blocks should arrive at the same `BeaconState`.
///
/// The state transition replays this structure forward one block at a time.
/// Current coverage mutates the clock, recent roots, latest header, RANDAO,
/// deposit-chain vote, covered operation effects, and sync rewards. Other
/// groups are modeled so later transition phases have their consensus shape.
///
/// The default value is the SSZ zero state. It is useful for construction, but
/// it is not a valid initialized chain state on its own. Upgrade routines may
/// set fields such as `execution_payload_availability` to non-zero values.
#[derive(Debug, Clone, PartialEq, Eq, SimpleSerialize)]
pub struct BeaconState {
    /// Unix timestamp the chain started at.
    pub genesis_time: u64,
    /// Root of the validator set at genesis. Tags this chain in domain separators.
    pub genesis_validators_root: Root,
    /// Current slot of the state.
    pub slot: Slot,
    /// Active signing-version pair.
    pub fork: Fork,
    /// Header of the most recent block applied to this state.
    pub latest_block_header: BeaconBlockHeader,
    /// Ring buffer of past block roots indexed by `slot % SLOTS_PER_HISTORICAL_ROOT`.
    pub block_roots: Vector<Root, SLOTS_PER_HISTORICAL_ROOT>,
    /// Ring buffer of past state roots indexed by `slot % SLOTS_PER_HISTORICAL_ROOT`.
    pub state_roots: Vector<Root, SLOTS_PER_HISTORICAL_ROOT>,
    /// Roll-up roots of historical block-root buffers, trimmed for cheap proofs.
    pub historical_roots: List<Root, HISTORICAL_ROOTS_LIMIT>,
    /// Currently winning deposit-chain vote (`Eth1Data` in the spec).
    pub eth1_data: Eth1Data,
    /// Deposit-chain votes accumulated over the current voting period.
    pub eth1_data_votes: List<Eth1Data, ETH1_DATA_VOTES_LEN>,
    /// Index of the next deposit to be processed from the deposit contract.
    pub eth1_deposit_index: u64,
    /// The validator registry.
    pub validators: List<Validator, VALIDATOR_REGISTRY_LIMIT>,
    /// Per-validator balances before effective-balance rounding.
    pub balances: List<Gwei, VALIDATOR_REGISTRY_LIMIT>,
    /// Ring buffer of past RANDAO mixes.
    pub randao_mixes: Vector<Bytes32, EPOCHS_PER_HISTORICAL_VECTOR>,
    /// Ring buffer of accumulated slashing totals per epoch.
    pub slashings: Vector<Gwei, EPOCHS_PER_SLASHINGS_VECTOR>,
    /// Per-validator participation flags from the previous epoch.
    pub previous_epoch_participation: List<ParticipationFlags, VALIDATOR_REGISTRY_LIMIT>,
    /// Per-validator participation flags from the current epoch.
    pub current_epoch_participation: List<ParticipationFlags, VALIDATOR_REGISTRY_LIMIT>,
    /// Rolling Casper finality-justification bitvector.
    pub justification_bits: Bitvector<JUSTIFICATION_BITS_LENGTH>,
    /// Justified checkpoint from the previous epoch.
    pub previous_justified_checkpoint: Checkpoint,
    /// Justified checkpoint from the current epoch.
    pub current_justified_checkpoint: Checkpoint,
    /// Most recent finalized checkpoint.
    pub finalized_checkpoint: Checkpoint,
    /// Per-validator inactivity-leak scores.
    pub inactivity_scores: List<u64, VALIDATOR_REGISTRY_LIMIT>,
    /// Sync committee active this period.
    pub current_sync_committee: SyncCommittee,
    /// Sync committee active next period, cached one period in advance.
    pub next_sync_committee: SyncCommittee,
    /// Hash of the most recently included execution payload.
    pub latest_block_hash: Hash32,
    /// Sequence index of the next withdrawal to emit.
    pub next_withdrawal_index: WithdrawalIndex,
    /// Sweep cursor over the validator registry for the next withdrawal scan.
    pub next_withdrawal_validator_index: ValidatorIndex,
    /// Summarized history beyond the live ring buffer.
    pub historical_summaries: List<HistoricalSummary, HISTORICAL_ROOTS_LIMIT>,
    /// Starting index for processing deposit-contract requests.
    pub deposit_requests_start_index: u64,
    /// Remaining deposit-balance budget for the current epoch.
    pub deposit_balance_to_consume: Gwei,
    /// Remaining exit-balance budget for the current epoch.
    pub exit_balance_to_consume: Gwei,
    /// Earliest epoch a new exit may be scheduled at.
    pub earliest_exit_epoch: Epoch,
    /// Remaining consolidation-balance budget for the current epoch.
    pub consolidation_balance_to_consume: Gwei,
    /// Earliest epoch a new consolidation may be scheduled at.
    pub earliest_consolidation_epoch: Epoch,
    /// Queue of deposits awaiting signature verification and activation.
    pub pending_deposits: List<PendingDeposit, PENDING_DEPOSITS_LIMIT>,
    /// Queue of scheduled partial withdrawals.
    pub pending_partial_withdrawals:
        List<PendingPartialWithdrawal, PENDING_PARTIAL_WITHDRAWALS_LIMIT>,
    /// Queue of scheduled consolidations.
    pub pending_consolidations: List<PendingConsolidation, PENDING_CONSOLIDATIONS_LIMIT>,
    /// Lookahead proposer assignments for the next few slots.
    pub proposer_lookahead: Vector<ValidatorIndex, PROPOSER_LOOKAHEAD_LEN>,
    /// The builder registry.
    pub builders: List<Builder, BUILDER_REGISTRY_LIMIT>,
    /// Sweep cursor over the builder registry for the next withdrawal scan.
    pub next_withdrawal_builder_index: BuilderIndex,
    /// Per-slot bit indicating whether the slot's payload was observed available.
    pub execution_payload_availability: Bitvector<SLOTS_PER_HISTORICAL_ROOT>,
    /// Rolling 2-epoch accumulator of payload-timeliness builder-payment weights.
    pub builder_pending_payments: Vector<BuilderPendingPayment, BUILDER_PAYMENT_WINDOW_LEN>,
    /// Queue of builder payments awaiting the next withdrawal sweep.
    pub builder_pending_withdrawals:
        List<BuilderPendingWithdrawal, BUILDER_PENDING_WITHDRAWALS_LIMIT>,
    /// Most-recently-accepted builder bid for the current slot.
    pub latest_execution_payload_bid: ExecutionPayloadBid,
    /// Withdrawals the next payload is expected to include.
    pub payload_expected_withdrawals: List<Withdrawal, MAX_WITHDRAWALS_PER_PAYLOAD>,
    /// Per-slot payload-timeliness committee assignments over the lookahead window.
    pub ptc_window: Vector<Vector<ValidatorIndex, PTC_SIZE>, PTC_WINDOW_LEN>,
}

/// SSZ zero state, field by field.
///
/// # Warning
///
/// This is **not** a valid initialized chain state. It is the all-zero SSZ
/// value, useful only as a starting point for construction (genesis seeding,
/// test fixtures, upgrade routines). Treating it as a live state will produce
/// spec-invalid behavior. Genesis state is built by the spec's initialization
/// routine; later spec upgrade routines may set fields such as
/// `execution_payload_availability` to non-zero starting values.
impl Default for BeaconState {
    fn default() -> Self {
        Self {
            genesis_time: u64::default(),
            genesis_validators_root: Root::default(),
            slot: Slot::default(),
            fork: Fork::default(),
            latest_block_header: BeaconBlockHeader::default(),
            block_roots: Vector::default(),
            state_roots: Vector::default(),
            historical_roots: List::default(),
            eth1_data: Eth1Data::default(),
            eth1_data_votes: List::default(),
            eth1_deposit_index: u64::default(),
            validators: List::default(),
            balances: List::default(),
            randao_mixes: Vector::default(),
            slashings: Vector::default(),
            previous_epoch_participation: List::default(),
            current_epoch_participation: List::default(),
            justification_bits: Bitvector::default(),
            previous_justified_checkpoint: Checkpoint::default(),
            current_justified_checkpoint: Checkpoint::default(),
            finalized_checkpoint: Checkpoint::default(),
            inactivity_scores: List::default(),
            current_sync_committee: SyncCommittee::default(),
            next_sync_committee: SyncCommittee::default(),
            latest_block_hash: Hash32::default(),
            next_withdrawal_index: WithdrawalIndex::default(),
            next_withdrawal_validator_index: ValidatorIndex::default(),
            historical_summaries: List::default(),
            deposit_requests_start_index: u64::default(),
            deposit_balance_to_consume: Gwei::default(),
            exit_balance_to_consume: Gwei::default(),
            earliest_exit_epoch: Epoch::default(),
            consolidation_balance_to_consume: Gwei::default(),
            earliest_consolidation_epoch: Epoch::default(),
            pending_deposits: List::default(),
            pending_partial_withdrawals: List::default(),
            pending_consolidations: List::default(),
            proposer_lookahead: Vector::default(),
            builders: List::default(),
            next_withdrawal_builder_index: BuilderIndex::default(),
            execution_payload_availability: Bitvector::default(),
            builder_pending_payments: Vector::default(),
            builder_pending_withdrawals: List::default(),
            latest_execution_payload_bid: ExecutionPayloadBid::default(),
            payload_expected_withdrawals: List::default(),
            ptc_window: Vector::default(),
        }
    }
}
