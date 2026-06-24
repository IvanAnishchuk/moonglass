//! Adapter for `operations` reference-test fixtures.
//!
//! Operation fixtures share the same state-transition harness. Input
//! operations decode one operation-specific SSZ file before calling the
//! corresponding `BeaconState` method; state-only operations call directly into
//! the state. Missing post-state means the operation is expected to be rejected.

use std::marker::PhantomData;

use moonglass::containers::{
    Attestation, AttesterSlashing, BeaconBlock, BeaconState, BuilderDepositRequest,
    BuilderExitRequest, ConsolidationRequest, DepositRequest, PayloadAttestation, ProposerSlashing,
    SignedBLSToExecutionChange, SignedExecutionPayloadBid, SignedVoluntaryExit, SyncAggregate,
    WithdrawalRequest,
};
use moonglass::error::TransitionError;

use super::{
    Adapter, CaseRunner, Outcome, StateTransition, SupportedHandler, TraceData, run_state_case,
    trace_enabled, trace_fail, trace_pass,
};
use crate::fixtures::{CaseFiles, FixtureFile};
use crate::inventory::{Case, Runner};

#[derive(Clone, Copy)]
pub(super) enum OperationHandler {
    VoluntaryExit,
    VoluntaryExitChurn,
    BlsToExecutionChange,
    Attestation,
    AttesterSlashing,
    ProposerSlashing,
    SyncAggregate,
    BlockHeader,
    PayloadAttestation,
    DepositRequest,
    BuilderDepositRequest,
    BuilderExitRequest,
    WithdrawalRequest,
    ConsolidationRequest,
    ExecutionPayloadBid,
    ParentExecutionPayload,
    Withdrawals,
}

impl OperationHandler {
    const ATTESTATION: &'static str = "attestation";
    const ATTESTER_SLASHING: &'static str = "attester_slashing";
    const BLOCK_HEADER: &'static str = "block_header";
    const BLS_TO_EXECUTION_CHANGE: &'static str = "bls_to_execution_change";
    const BUILDER_DEPOSIT_REQUEST: &'static str = "builder_deposit_request";
    const BUILDER_EXIT_REQUEST: &'static str = "builder_exit_request";
    const CONSOLIDATION_REQUEST: &'static str = "consolidation_request";
    const DEPOSIT_REQUEST: &'static str = "deposit_request";
    const EXECUTION_PAYLOAD_BID: &'static str = "execution_payload_bid";
    const PARENT_EXECUTION_PAYLOAD: &'static str = "parent_execution_payload";
    const PAYLOAD_ATTESTATION: &'static str = "payload_attestation";
    const PROPOSER_SLASHING: &'static str = "proposer_slashing";
    const SYNC_AGGREGATE: &'static str = "sync_aggregate";
    const VOLUNTARY_EXIT: &'static str = "voluntary_exit";
    const VOLUNTARY_EXIT_CHURN: &'static str = "voluntary_exit_churn";
    const WITHDRAWAL_REQUEST: &'static str = "withdrawal_request";
    const WITHDRAWALS: &'static str = "withdrawals";
}

impl SupportedHandler for OperationHandler {
    const ALL: &'static [Self] = &[
        Self::VoluntaryExit,
        Self::VoluntaryExitChurn,
        Self::BlsToExecutionChange,
        Self::Attestation,
        Self::AttesterSlashing,
        Self::ProposerSlashing,
        Self::SyncAggregate,
        Self::BlockHeader,
        Self::PayloadAttestation,
        Self::DepositRequest,
        Self::BuilderDepositRequest,
        Self::BuilderExitRequest,
        Self::WithdrawalRequest,
        Self::ConsolidationRequest,
        Self::ExecutionPayloadBid,
        Self::ParentExecutionPayload,
        Self::Withdrawals,
    ];

    fn as_str(self) -> &'static str {
        match self {
            Self::VoluntaryExit => Self::VOLUNTARY_EXIT,
            Self::VoluntaryExitChurn => Self::VOLUNTARY_EXIT_CHURN,
            Self::BlsToExecutionChange => Self::BLS_TO_EXECUTION_CHANGE,
            Self::Attestation => Self::ATTESTATION,
            Self::AttesterSlashing => Self::ATTESTER_SLASHING,
            Self::ProposerSlashing => Self::PROPOSER_SLASHING,
            Self::SyncAggregate => Self::SYNC_AGGREGATE,
            Self::BlockHeader => Self::BLOCK_HEADER,
            Self::PayloadAttestation => Self::PAYLOAD_ATTESTATION,
            Self::DepositRequest => Self::DEPOSIT_REQUEST,
            Self::BuilderDepositRequest => Self::BUILDER_DEPOSIT_REQUEST,
            Self::BuilderExitRequest => Self::BUILDER_EXIT_REQUEST,
            Self::WithdrawalRequest => Self::WITHDRAWAL_REQUEST,
            Self::ConsolidationRequest => Self::CONSOLIDATION_REQUEST,
            Self::ExecutionPayloadBid => Self::EXECUTION_PAYLOAD_BID,
            Self::ParentExecutionPayload => Self::PARENT_EXECUTION_PAYLOAD,
            Self::Withdrawals => Self::WITHDRAWALS,
        }
    }
}

impl OperationHandler {
    fn apply(self, case: &Case, state: &mut BeaconState) -> StateTransition {
        self.operation().apply(case, state)
    }

    fn operation(self) -> &'static dyn Operation {
        match self {
            Self::VoluntaryExit | Self::VoluntaryExitChurn => &VOLUNTARY_EXIT_OPERATION,
            Self::BlsToExecutionChange => &BLS_TO_EXECUTION_CHANGE_OPERATION,
            Self::Attestation => &ATTESTATION_OPERATION,
            Self::AttesterSlashing => &ATTESTER_SLASHING_OPERATION,
            Self::ProposerSlashing => &PROPOSER_SLASHING_OPERATION,
            Self::SyncAggregate => &SYNC_AGGREGATE_OPERATION,
            Self::BlockHeader => &BLOCK_HEADER_OPERATION,
            Self::PayloadAttestation => &PAYLOAD_ATTESTATION_OPERATION,
            Self::DepositRequest => &DEPOSIT_REQUEST_OPERATION,
            Self::BuilderDepositRequest => &BUILDER_DEPOSIT_REQUEST_OPERATION,
            Self::BuilderExitRequest => &BUILDER_EXIT_REQUEST_OPERATION,
            Self::WithdrawalRequest => &WITHDRAWAL_REQUEST_OPERATION,
            Self::ConsolidationRequest => &CONSOLIDATION_REQUEST_OPERATION,
            Self::ExecutionPayloadBid => &EXECUTION_PAYLOAD_BID_OPERATION,
            Self::ParentExecutionPayload => &PARENT_EXECUTION_PAYLOAD_OPERATION,
            Self::Withdrawals => &WITHDRAWALS_OPERATION,
        }
    }
}

trait Operation: Sync {
    fn apply(&self, case: &Case, state: &mut BeaconState) -> StateTransition;
}

struct InputOperation<T> {
    file: FixtureFile,
    apply: fn(&mut BeaconState, &T) -> Result<(), TransitionError>,
    fixture: PhantomData<fn() -> T>,
}

impl<T> InputOperation<T> {
    const fn new(
        file: FixtureFile,
        apply: fn(&mut BeaconState, &T) -> Result<(), TransitionError>,
    ) -> Self {
        Self {
            file,
            apply,
            fixture: PhantomData,
        }
    }
}

impl<T> Operation for InputOperation<T>
where
    T: ssz_rs::Deserialize + TraceData,
{
    fn apply(&self, case: &Case, state: &mut BeaconState) -> StateTransition {
        match CaseFiles::new(case).decode_ssz_snappy::<T>(self.file) {
            Ok(op) => {
                trace_pass(
                    format_args!("decode {}", self.file.as_str()),
                    "decoded operation fixture",
                );
                if trace_enabled() {
                    trace_pass("input", op.trace_data());
                }
                StateTransition::Applied((self.apply)(state, &op))
            }
            Err(e) => {
                let detail = format!("decode {}: {e}", self.file.as_str());
                trace_fail(format_args!("decode {}", self.file.as_str()), &detail);
                StateTransition::HarnessError(detail)
            }
        }
    }
}

struct StateOperation {
    apply: fn(&mut BeaconState) -> Result<(), TransitionError>,
}

impl StateOperation {
    const fn new(apply: fn(&mut BeaconState) -> Result<(), TransitionError>) -> Self {
        Self { apply }
    }
}

impl Operation for StateOperation {
    fn apply(&self, _case: &Case, state: &mut BeaconState) -> StateTransition {
        trace_pass("input", "operation uses pre-state only");
        StateTransition::Applied((self.apply)(state))
    }
}

static VOLUNTARY_EXIT_OPERATION: InputOperation<SignedVoluntaryExit> = InputOperation::new(
    FixtureFile::new("voluntary_exit.ssz_snappy"),
    BeaconState::process_voluntary_exit,
);
static BLS_TO_EXECUTION_CHANGE_OPERATION: InputOperation<SignedBLSToExecutionChange> =
    InputOperation::new(
        FixtureFile::new("address_change.ssz_snappy"),
        BeaconState::process_bls_to_execution_change,
    );
static ATTESTATION_OPERATION: InputOperation<Attestation> = InputOperation::new(
    FixtureFile::new("attestation.ssz_snappy"),
    BeaconState::process_attestation,
);
static ATTESTER_SLASHING_OPERATION: InputOperation<AttesterSlashing> = InputOperation::new(
    FixtureFile::new("attester_slashing.ssz_snappy"),
    BeaconState::process_attester_slashing,
);
static PROPOSER_SLASHING_OPERATION: InputOperation<ProposerSlashing> = InputOperation::new(
    FixtureFile::new("proposer_slashing.ssz_snappy"),
    BeaconState::process_proposer_slashing,
);
static SYNC_AGGREGATE_OPERATION: InputOperation<SyncAggregate> = InputOperation::new(
    FixtureFile::new("sync_aggregate.ssz_snappy"),
    BeaconState::process_sync_aggregate,
);
static BLOCK_HEADER_OPERATION: InputOperation<BeaconBlock> = InputOperation::new(
    FixtureFile::new("block.ssz_snappy"),
    BeaconState::process_block_header,
);
static PAYLOAD_ATTESTATION_OPERATION: InputOperation<PayloadAttestation> = InputOperation::new(
    FixtureFile::new("payload_attestation.ssz_snappy"),
    BeaconState::process_payload_attestation,
);
static DEPOSIT_REQUEST_OPERATION: InputOperation<DepositRequest> = InputOperation::new(
    FixtureFile::new("deposit_request.ssz_snappy"),
    BeaconState::process_deposit_request,
);
static BUILDER_DEPOSIT_REQUEST_OPERATION: InputOperation<BuilderDepositRequest> =
    InputOperation::new(
        FixtureFile::new("builder_deposit_request.ssz_snappy"),
        BeaconState::process_builder_deposit_request,
    );
static BUILDER_EXIT_REQUEST_OPERATION: InputOperation<BuilderExitRequest> = InputOperation::new(
    FixtureFile::new("builder_exit_request.ssz_snappy"),
    BeaconState::process_builder_exit_request,
);
static WITHDRAWAL_REQUEST_OPERATION: InputOperation<WithdrawalRequest> = InputOperation::new(
    FixtureFile::new("withdrawal_request.ssz_snappy"),
    BeaconState::process_withdrawal_request,
);
static CONSOLIDATION_REQUEST_OPERATION: InputOperation<ConsolidationRequest> = InputOperation::new(
    FixtureFile::new("consolidation_request.ssz_snappy"),
    BeaconState::process_consolidation_request,
);
static EXECUTION_PAYLOAD_BID_OPERATION: InputOperation<SignedExecutionPayloadBid> =
    InputOperation::new(
        FixtureFile::new("execution_payload_bid.ssz_snappy"),
        BeaconState::process_execution_payload_bid,
    );
static PARENT_EXECUTION_PAYLOAD_OPERATION: InputOperation<BeaconBlock> = InputOperation::new(
    FixtureFile::new("block.ssz_snappy"),
    BeaconState::accept_parent_payload_commitment,
);
static WITHDRAWALS_OPERATION: StateOperation =
    StateOperation::new(BeaconState::process_withdrawals);

pub(super) static ADAPTER: Adapter<Operations> = Adapter::new();

pub(super) struct Operations;

impl CaseRunner for Operations {
    type Handler = OperationHandler;

    const RUNNER: Runner = Runner::Operations;

    fn run(case: &Case, handler: Self::Handler) -> Outcome {
        let subject = format!("operation/{}", handler.as_str());
        run_state_case(case, &subject, |case, state| handler.apply(case, state))
    }
}
