//! Adapter for `epoch_processing` reference-test fixtures.

use moonglass::containers::BeaconState;
use moonglass::error::TransitionError;

use super::{Outcome, finish_state, load_pre_state};
use crate::discover::Case;

type EpochMethod = fn(&mut BeaconState) -> Result<(), TransitionError>;

#[must_use]
pub(super) fn run(case: &Case) -> Outcome {
    let mut state = match load_pre_state(case) {
        Ok(state) => state,
        Err(msg) => return Outcome::Fail(msg),
    };

    let Some(method) = method(case.handler.as_str()) else {
        return Outcome::Fail(format!(
            "epoch_processing handler '{}' not wired in moonglass",
            case.handler
        ));
    };

    let result = method(&mut state);
    finish_state(case, &mut state, result, "sub-phase")
}

#[must_use]
pub(super) fn supports(handler: &str) -> bool {
    method(handler).is_some()
}

fn method(handler: &str) -> Option<EpochMethod> {
    Some(match handler {
        "builder_pending_payments" => BeaconState::process_builder_pending_payments,
        "effective_balance_updates" => BeaconState::process_effective_balance_updates,
        "eth1_data_reset" => BeaconState::process_eth1_data_reset,
        "historical_summaries_update" => BeaconState::process_historical_summaries_update,
        "inactivity_updates" => BeaconState::process_inactivity_updates,
        "justification_and_finalization" => BeaconState::process_justification_and_finalization,
        "participation_flag_updates" => BeaconState::process_participation_flag_updates,
        "pending_consolidations" => BeaconState::process_pending_consolidations,
        "pending_deposits" | "pending_deposits_churn" => BeaconState::process_pending_deposits,
        "proposer_lookahead" => BeaconState::process_proposer_lookahead,
        "ptc_window" => BeaconState::process_ptc_window,
        "randao_mixes_reset" => BeaconState::process_randao_mixes_reset,
        "registry_updates" => BeaconState::process_registry_updates,
        "rewards_and_penalties" => BeaconState::process_rewards_and_penalties,
        "slashings" => BeaconState::process_slashings,
        "slashings_reset" => BeaconState::process_slashings_reset,
        "sync_committee_updates" => BeaconState::process_sync_committee_updates,
        _ => return None,
    })
}
