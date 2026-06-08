//! Consensus-spec constants, organized by concern.
//!
//! These parameters define timing, rewards, limits, domains, chain identity,
//! and registry behavior used by the state transition.
//! Networking and validator-duty constants are omitted unless
//! the state transition itself consumes them.

mod block;
mod builder;
mod chain;
mod domains;
mod fork_choice;
mod rewards;
mod state;
mod time;
mod validator;

pub use block::*;
pub use builder::*;
pub use chain::*;
pub use domains::*;
pub use fork_choice::*;
pub use rewards::*;
pub use state::*;
pub use time::*;
pub use validator::*;
