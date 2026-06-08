//! Consensus-spec data objects.
//!
//! Ethereum consensus serializes these objects with `SimpleSerialize` (SSZ) and
//! identifies them by SSZ hash-tree-root. [`BeaconState`] is the long-lived
//! snapshot validators agree on. Blocks and operations carry proposed changes
//! to that snapshot. Supporting containers carry votes, signatures, deposits,
//! withdrawals, execution payload commitments, and builder-market data.
//!
//! **Scope.** Only containers the state transition consumes are modeled.
//! Out of scope:
//! - networking (`DataColumnSidecar`, `BlobSidecar`, p2p envelopes),
//! - sync-update helpers,
//! - validator duties and local keystore/signing flows.

mod attestation;
mod block;
mod builder;
mod chain;
mod execution;
mod state;
mod sync;
mod validator;
mod withdrawal;

pub use attestation::*;
pub use block::*;
pub use builder::*;
pub use chain::*;
pub use execution::*;
pub use state::*;
pub use sync::*;
pub use validator::*;
pub use withdrawal::*;
