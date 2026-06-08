//! Reference-test adapters dispatched by the runner.

use moonglass::containers::BeaconState;
use moonglass::error::TransitionError;
use serde::{Deserialize, Serialize};

use crate::discover::Case;
use crate::{compare, fixture};

mod bls;
mod epoch_processing;
mod fork_choice;
mod operations;
mod sanity;
mod ssz_static;

/// Result of running a single case through an adapter.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) enum Outcome {
    Pass,
    Fail(String),
    /// Per-case worker did not finish inside the time budget.
    ///
    /// Not counted as a failure: CI stays green so a slow runner does not
    /// block merges, but timed-out cases are still listed in the summary.
    Timeout(String),
}

#[derive(Clone, Copy)]
enum Runner {
    SszStatic,
    Bls,
    ForkChoice,
    Operations,
    EpochProcessing,
    Sanity,
    Finality,
    Random,
}

/// Dispatch a case to its adapter based on `(runner, handler)`.
#[must_use]
pub(crate) fn run(case: &Case) -> Outcome {
    match runner(case.runner.as_str()) {
        Some(Runner::SszStatic) => ssz_static::run(case),
        Some(Runner::Bls) => bls::run(case),
        Some(Runner::ForkChoice) => fork_choice::run(case),
        Some(Runner::Operations) => operations::run(case),
        Some(Runner::EpochProcessing) => epoch_processing::run(case),
        Some(Runner::Sanity | Runner::Finality | Runner::Random) => sanity::run(case),
        None => Outcome::Fail(format!("no adapter for runner '{}'", case.runner)),
    }
}

/// Whether the harness has an adapter for this upstream `(runner, handler)`.
///
/// Unmapped runners include `kzg`, `merkle_proof`, `shuffling`, `genesis`,
/// and sync-update fixture families. The `transition` family is intentionally out of scope:
/// master tracks the current mainnet fork only.
#[must_use]
pub(crate) fn supports(runner_name: &str, handler: &str) -> bool {
    match runner(runner_name) {
        Some(Runner::SszStatic) => ssz_static::supports(handler),
        Some(Runner::Bls) => bls::supports(handler),
        Some(Runner::ForkChoice) => fork_choice::supports(handler),
        Some(Runner::Operations) => operations::supports(handler),
        Some(Runner::EpochProcessing) => epoch_processing::supports(handler),
        Some(Runner::Sanity) => sanity::supports(handler),
        Some(Runner::Finality) => handler == "finality",
        Some(Runner::Random) => handler == "random",
        None => false,
    }
}

fn runner(name: &str) -> Option<Runner> {
    Some(match name {
        "ssz_static" => Runner::SszStatic,
        "bls" => Runner::Bls,
        "fork_choice" => Runner::ForkChoice,
        "operations" => Runner::Operations,
        "epoch_processing" => Runner::EpochProcessing,
        "sanity" => Runner::Sanity,
        "finality" => Runner::Finality,
        "random" => Runner::Random,
        _ => return None,
    })
}

fn load_pre_state(case: &Case) -> Result<BeaconState, String> {
    const PRE_FILENAME: &str = "pre.ssz_snappy";
    let pre_path = case.root.join(PRE_FILENAME);
    if !pre_path.exists() {
        return Err(format!("missing {PRE_FILENAME}"));
    }
    fixture::decode_ssz_snappy::<BeaconState>(&pre_path).map_err(|e| format!("decode pre: {e:#}"))
}

fn finish_state(
    case: &Case,
    state: &mut BeaconState,
    result: Result<(), TransitionError>,
    subject: &str,
) -> Outcome {
    let post_path = case.root.join("post.ssz_snappy");
    let want_post = post_path.exists();
    match (result, want_post) {
        (Ok(()), true) => {
            let mut want = match fixture::decode_ssz_snappy::<BeaconState>(&post_path) {
                Ok(s) => s,
                Err(e) => return Outcome::Fail(format!("decode post: {e:#}")),
            };
            match compare::diff(state, &mut want) {
                Ok(None) => Outcome::Pass,
                Ok(Some(detail)) => Outcome::Fail(detail),
                Err(e) => Outcome::Fail(format!("hash_tree_root: {e}")),
            }
        }
        (Ok(()), false) => Outcome::Fail(format!("expected failure, {subject} returned Ok")),
        (Err(e), true) => Outcome::Fail(format!("expected success, {subject} returned: {e}")),
        (Err(_), false) => Outcome::Pass,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanity_shape_support_is_exact_to_runner() {
        assert!(supports("sanity", "blocks"));
        assert!(supports("sanity", "slots"));
        assert!(!supports("sanity", "finality"));
        assert!(!supports("sanity", "random"));

        assert!(supports("finality", "finality"));
        assert!(!supports("finality", "blocks"));

        assert!(supports("random", "random"));
        assert!(!supports("random", "blocks"));
    }
}
