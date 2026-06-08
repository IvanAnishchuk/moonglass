//! A behavior-first guide to the Ethereum consensus specs.
//!
//! Ethereum consensus is the rulebook validators use to agree on chain state.
//! Moonglass models the data validators agree on, the signed blocks that
//! propose changes to it, and the transition rules that decide whether those
//! changes are valid.
//!
//! Start with [`state_transition`] to follow a block through the rulebook, then
//! use [`containers`] for the data being moved, [`primitives`] and [`constants`]
//! for vocabulary and parameters, [`error`] for the difference between an
//! invalid transition and behavior Moonglass does not yet cover, and
//! [`fork_choice`] for the head-selection rule that reads accepted blocks and
//! attestations to decide which leaf the next block should build on.

#[cfg(not(any(feature = "mainnet", feature = "minimal")))]
compile_error!("moonglass must be built with exactly one of the `mainnet` or `minimal` features");

#[cfg(all(feature = "mainnet", feature = "minimal"))]
compile_error!(
    "moonglass cannot be built with both `mainnet` and `minimal` features (cargo features are additive)"
);

pub mod constants;
pub mod containers;
pub mod crypto;
pub mod error;
pub mod fork_choice;
pub mod primitives;
pub mod state_transition;
