//! Fork-choice adapter for imperative `steps.yaml` fixtures.
//!
//! Upstream exposes many `fork_choice/<handler>` directories, but every
//! supported target-fork family currently uses the same fixture contract:
//! `anchor_state.ssz_snappy`, `anchor_block.ssz_snappy`, and a sequence of
//! steps in `steps.yaml`. The handler names below are therefore discovery gates
//! for supported upstream families, not separate dispatch functions.

mod checks;
mod runner;
mod steps;

use super::{Adapter, CaseRunner, Outcome, SupportedHandler};
use crate::inventory::{Case, Runner};

pub(super) static ADAPTER: Adapter<ForkChoice> = Adapter::new();

pub(super) struct ForkChoice;

impl CaseRunner for ForkChoice {
    type Handler = ForkChoiceHandler;

    const RUNNER: Runner = Runner::ForkChoice;

    fn run(case: &Case, _handler: Self::Handler) -> Outcome {
        runner::run_case(case)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ForkChoiceHandler {
    DepositWithReorg,
    ExAnte,
    GetHead,
    GetParentPayloadStatus,
    GetProposerHead,
    OnAttestation,
    OnBlock,
    OnExecutionPayloadEnvelope,
    OnPayloadAttestationMessage,
    PayloadDataAvailability,
    PayloadTimeliness,
    Reorg,
    Withholding,
}

impl ForkChoiceHandler {
    const DEPOSIT_WITH_REORG: &'static str = "deposit_with_reorg";
    const EX_ANTE: &'static str = "ex_ante";
    const GET_HEAD: &'static str = "get_head";
    const GET_PARENT_PAYLOAD_STATUS: &'static str = "get_parent_payload_status";
    const GET_PROPOSER_HEAD: &'static str = "get_proposer_head";
    const ON_ATTESTATION: &'static str = "on_attestation";
    const ON_BLOCK: &'static str = "on_block";
    const ON_EXECUTION_PAYLOAD_ENVELOPE: &'static str = "on_execution_payload_envelope";
    const ON_PAYLOAD_ATTESTATION_MESSAGE: &'static str = "on_payload_attestation_message";
    const PAYLOAD_DATA_AVAILABILITY: &'static str = "payload_data_availability";
    const PAYLOAD_TIMELINESS: &'static str = "payload_timeliness";
    const REORG: &'static str = "reorg";
    const WITHHOLDING: &'static str = "withholding";
}

impl SupportedHandler for ForkChoiceHandler {
    const ALL: &'static [Self] = &[
        Self::DepositWithReorg,
        Self::ExAnte,
        Self::GetHead,
        Self::GetParentPayloadStatus,
        Self::GetProposerHead,
        Self::OnAttestation,
        Self::OnBlock,
        Self::OnExecutionPayloadEnvelope,
        Self::OnPayloadAttestationMessage,
        Self::PayloadDataAvailability,
        Self::PayloadTimeliness,
        Self::Reorg,
        Self::Withholding,
    ];

    fn as_str(self) -> &'static str {
        match self {
            Self::DepositWithReorg => Self::DEPOSIT_WITH_REORG,
            Self::ExAnte => Self::EX_ANTE,
            Self::GetHead => Self::GET_HEAD,
            Self::GetParentPayloadStatus => Self::GET_PARENT_PAYLOAD_STATUS,
            Self::GetProposerHead => Self::GET_PROPOSER_HEAD,
            Self::OnAttestation => Self::ON_ATTESTATION,
            Self::OnBlock => Self::ON_BLOCK,
            Self::OnExecutionPayloadEnvelope => Self::ON_EXECUTION_PAYLOAD_ENVELOPE,
            Self::OnPayloadAttestationMessage => Self::ON_PAYLOAD_ATTESTATION_MESSAGE,
            Self::PayloadDataAvailability => Self::PAYLOAD_DATA_AVAILABILITY,
            Self::PayloadTimeliness => Self::PAYLOAD_TIMELINESS,
            Self::Reorg => Self::REORG,
            Self::Withholding => Self::WITHHOLDING,
        }
    }
}
