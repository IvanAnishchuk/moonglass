//! Consensus-spec reference-test runner library for Moonglass.
//!
//! The binary crate is intentionally thin. This library owns discovery,
//! fixture loading, adapter dispatch, vector-cache validation, and reporting.

mod adapters;
mod error;
mod fixtures;
mod harness;
mod inventory;
mod vectors;

#[cfg(test)]
mod testing;

pub use error::{Error, Result};

/// `ethereum/consensus-specs` release targeted by the runner.
pub const CONSENSUS_SPECS_TAG: &str = "v1.7.0-alpha.11";

/// Fork currently targeted within the consensus-specs release.
const TARGET_FORK: &str = "gloas";

const MAINNET_PRESET: &str = "mainnet";
const MINIMAL_PRESET: &str = "minimal";

#[cfg(not(any(feature = "mainnet", feature = "minimal")))]
compile_error!("reftests must be built with exactly one of the `mainnet` or `minimal` features");

#[cfg(all(feature = "mainnet", feature = "minimal"))]
compile_error!(
    "reftests cannot be built with both `mainnet` and `minimal` features (cargo features are additive)"
);

/// Run the reftest harness using process arguments and the workspace vector cache.
pub fn run_from_env() -> Result<()> {
    harness::run_from_env()
}
