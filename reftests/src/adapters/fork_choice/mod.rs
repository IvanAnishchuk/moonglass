//! Fork-choice adapter: dispatches `fork_choice` reference-test fixtures.

mod runner;
mod steps;

use super::Outcome;
use crate::discover::Case;

pub(super) fn run(case: &Case) -> Outcome {
    runner::run_case(case)
}

pub(super) fn supports(handler: &str) -> bool {
    matches!(
        handler,
        "get_head"
            | "on_block"
            | "on_attestation"
            | "ex_ante"
            | "get_parent_payload_status"
            | "on_execution_payload_envelope"
            | "on_payload_attestation_message",
    )
}
