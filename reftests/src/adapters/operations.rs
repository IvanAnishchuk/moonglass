//! Adapter for `operations` reference-test fixtures.

use moonglass::containers::{
    Attestation, AttesterSlashing, BeaconBlock, BeaconState, ConsolidationRequest, DepositRequest,
    PayloadAttestation, ProposerSlashing, SignedBLSToExecutionChange, SignedVoluntaryExit,
    SyncAggregate, WithdrawalRequest,
};
use moonglass::error::TransitionError;

use super::{Outcome, finish_state, load_pre_state};
use crate::discover::Case;
use crate::fixture;

/// What an operations adapter produces before the post-state comparison.
enum Applied {
    /// Operation executed, carrying its result for post-state comparison.
    Op(Result<(), TransitionError>),
    /// Harness ran into a problem (decode failure, missing file).
    HarnessError(String),
}

#[derive(Clone, Copy)]
enum Operation {
    VoluntaryExit,
    BlsToExecutionChange,
    Attestation,
    AttesterSlashing,
    ProposerSlashing,
    SyncAggregate,
    BlockHeader,
    PayloadAttestation,
    DepositRequest,
    WithdrawalRequest,
    ConsolidationRequest,
    ExecutionPayloadBid,
    ParentExecutionPayload,
    Withdrawals,
}

#[must_use]
pub(super) fn run(case: &Case) -> Outcome {
    let mut state = match load_pre_state(case) {
        Ok(state) => state,
        Err(msg) => return Outcome::Fail(msg),
    };

    let applied = dispatch(case, &mut state);
    match applied {
        Applied::Op(result) => finish_state(case, &mut state, result, "operation"),
        Applied::HarnessError(msg) => Outcome::Fail(msg),
    }
}

#[must_use]
pub(super) fn supports(handler: &str) -> bool {
    operation(handler).is_some()
}

fn operation(handler: &str) -> Option<Operation> {
    Some(match handler {
        "voluntary_exit" | "voluntary_exit_churn" => Operation::VoluntaryExit,
        "bls_to_execution_change" => Operation::BlsToExecutionChange,
        "attestation" => Operation::Attestation,
        "attester_slashing" => Operation::AttesterSlashing,
        "proposer_slashing" => Operation::ProposerSlashing,
        "sync_aggregate" => Operation::SyncAggregate,
        "block_header" => Operation::BlockHeader,
        "payload_attestation" => Operation::PayloadAttestation,
        "deposit_request" => Operation::DepositRequest,
        "withdrawal_request" => Operation::WithdrawalRequest,
        "consolidation_request" => Operation::ConsolidationRequest,
        "execution_payload_bid" => Operation::ExecutionPayloadBid,
        "parent_execution_payload" => Operation::ParentExecutionPayload,
        "withdrawals" => Operation::Withdrawals,
        _ => return None,
    })
}

fn dispatch(case: &Case, state: &mut BeaconState) -> Applied {
    match operation(case.handler.as_str()) {
        Some(Operation::VoluntaryExit) => {
            apply::<SignedVoluntaryExit>(case, "voluntary_exit.ssz_snappy", |op| {
                state.process_voluntary_exit(op)
            })
        }
        Some(Operation::BlsToExecutionChange) => {
            apply::<SignedBLSToExecutionChange>(case, "address_change.ssz_snappy", |op| {
                state.process_bls_to_execution_change(op)
            })
        }
        Some(Operation::Attestation) => {
            apply::<Attestation>(case, "attestation.ssz_snappy", |op| {
                state.process_attestation(op)
            })
        }
        Some(Operation::AttesterSlashing) => {
            apply::<AttesterSlashing>(case, "attester_slashing.ssz_snappy", |op| {
                state.process_attester_slashing(op)
            })
        }
        Some(Operation::ProposerSlashing) => {
            apply::<ProposerSlashing>(case, "proposer_slashing.ssz_snappy", |op| {
                state.process_proposer_slashing(op)
            })
        }
        Some(Operation::SyncAggregate) => {
            apply::<SyncAggregate>(case, "sync_aggregate.ssz_snappy", |op| {
                state.process_sync_aggregate(op)
            })
        }
        Some(Operation::BlockHeader) => apply::<BeaconBlock>(case, "block.ssz_snappy", |op| {
            state.process_block_header(op)
        }),
        Some(Operation::PayloadAttestation) => {
            apply::<PayloadAttestation>(case, "payload_attestation.ssz_snappy", |op| {
                state.process_payload_attestation(op)
            })
        }
        Some(Operation::DepositRequest) => {
            apply::<DepositRequest>(case, "deposit_request.ssz_snappy", |op| {
                state.process_deposit_request(op)
            })
        }
        Some(Operation::WithdrawalRequest) => {
            apply::<WithdrawalRequest>(case, "withdrawal_request.ssz_snappy", |op| {
                state.process_withdrawal_request(op)
            })
        }
        Some(Operation::ConsolidationRequest) => {
            apply::<ConsolidationRequest>(case, "consolidation_request.ssz_snappy", |op| {
                state.process_consolidation_request(op)
            })
        }
        Some(Operation::ExecutionPayloadBid) => {
            apply::<BeaconBlock>(case, "block.ssz_snappy", |op| {
                state.process_execution_payload_bid(op)
            })
        }
        Some(Operation::ParentExecutionPayload) => {
            apply::<BeaconBlock>(case, "block.ssz_snappy", |op| {
                state.accept_parent_payload_commitment(op)
            })
        }
        Some(Operation::Withdrawals) => Applied::Op(state.process_withdrawals()),
        None => Applied::HarnessError(format!(
            "operations handler '{}' not wired in moonglass",
            case.handler
        )),
    }
}

fn apply<T>(
    case: &Case,
    input_filename: &str,
    run_op: impl FnOnce(&T) -> Result<(), TransitionError>,
) -> Applied
where
    T: ssz_rs::Deserialize,
{
    let path = case.root.join(input_filename);
    if !path.exists() {
        return Applied::HarnessError(format!("missing {input_filename}"));
    }
    match fixture::decode_ssz_snappy::<T>(&path) {
        Ok(op) => Applied::Op(run_op(&op)),
        Err(e) => Applied::HarnessError(format!("decode {input_filename}: {e:#}")),
    }
}
