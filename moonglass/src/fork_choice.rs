//! Fork choice: deciding which block is the head of the chain.
//!
//! Many valid blocks can compete to extend the chain. Fork choice is the rule
//! that picks one canonical [head](crate::glossary#head) from them, by following
//! the branch with the most [validator](crate::glossary#validator) support. This
//! module holds a node's local view of that contest and walks it to a head,
//! reusing [`crate::state_transition`] to advance states rather than re-checking
//! blocks itself.
//!
//! # Model
//!
//! Fork choice answers one question: which node is the head? The unit of choice
//! is a [`ForkChoiceNode`], a block root paired with a [`PayloadStatus`] of
//! [`Empty`](PayloadStatus::Empty), [`Full`](PayloadStatus::Full), or
//! [`Pending`](PayloadStatus::Pending). One block root is therefore several
//! nodes: pending before its payload resolves, then the empty or full branch
//! once votes and a recorded envelope decide. Weight, votes, and ancestry all
//! range over these nodes, not over bare block roots. Everything else follows:
//!
//! - [`Store`] holds this node's local evidence: known blocks and post-states,
//!   latest votes, recorded payload envelopes, and PTC vote vectors.
//! - Handlers fold each incoming message into the store.
//! - [`get_head`](Store::get_head) walks the store from the justified root,
//!   expands every block into its branch nodes, and keeps the heaviest node at
//!   each step.
//!
//! # Reading order
//!
//! The submodules read top to bottom as the life of a head decision.
//!
//! 1. [`store`] is the evidence set and the core data model it is built on,
//!    [`ForkChoiceNode`] and [`PayloadStatus`]. [`get_forkchoice_store`] seeds it
//!    from an anchor block and state.
//! 2. [`payload_status`] holds the rules that resolve a node's empty/full/pending
//!    status, which the rest of the module reasons about.
//! 3. Admission folds messages into the store: [`on_block()`](Store::on_block)
//!    runs the state transition and records the block,
//!    [`on_execution_payload_envelope()`](Store::on_execution_payload_envelope)
//!    records a delivered payload, [`on_attestation()`](Store::on_attestation)
//!    records latest votes,
//!    [`on_payload_attestation_message()`](Store::on_payload_attestation_message)
//!    records PTC votes, and [`on_tick()`](Store::on_tick) advances the clock.
//! 4. Selection reads the store: [`filter`] keeps only viable branches,
//!    [`weight`] scores a node from votes plus proposer boost, and [`head`]
//!    walks branch by branch to the heaviest leaf.
//! 5. [`proposer_head`] decides whether the current proposer should skip a late,
//!    weakly-supported head and build on its parent instead (a re-org).
//! 6. Plumbing supports the above: [`helpers`] (clock, ancestry, checkpoint
//!    roots), [`checkpoints`] (realized and pulled-up justification), and
//!    [`timeliness`] (block arrival and proposer-boost selection).
//!
//! # What "recording a payload" means
//!
//! One boundary is worth stating up front. Beyond the consensus-side checks, a
//! complete verdict needs a data-availability check and an execution-engine
//! check, but in
//! [`on_execution_payload_envelope()`](Store::on_execution_payload_envelope) both
//! are mocked and always pass (a TODO). So today a recorded payload means only
//! that the consensus-side checks passed (the signature, and that the payload
//! matches the committed bid, slot, parent hash, timestamp, and withdrawals).
//! That is enough for fork choice to open a block's full branch, but it is not
//! yet a complete validity verdict.
//!
//! # A block's payload, end to end
//!
//! It helps to follow one block `B`, proposed on top of a parent `P`.
//!
//! 1. The proposer commits to a payload by including a builder bid in `B`, and
//!    the state transition checks and records that bid through
//!    [`process_execution_payload_bid`](crate::containers::BeaconState::process_execution_payload_bid).
//! 2. [`on_block()`](Store::on_block) validates `B`, stores it and its resulting
//!    state, and sets up empty vote vectors for it.
//! 3. When `B`'s payload arrives,
//!    [`on_execution_payload_envelope()`](Store::on_execution_payload_envelope)
//!    checks and records it, which is what lets `B`'s full branch exist.
//! 4. The payload-timeliness committee votes on whether that payload was on time
//!    and its data available, and
//!    [`on_payload_attestation_message()`](Store::on_payload_attestation_message)
//!    folds those votes in.
//! 5. A slot later, beacon attestations for `B` arrive, and
//!    [`on_attestation()`](Store::on_attestation) records each as its voter's
//!    latest message, choosing the empty or full branch.
//! 6. [`get_head`](Store::get_head) reads all of this and returns the winning
//!    block-and-branch node. Later, when a child block builds on `B`'s payload,
//!    it proves and applies that payload's commitment to its own state through
//!    [`accept_parent_payload_commitment`](crate::containers::BeaconState::accept_parent_payload_commitment).

#![allow(clippy::must_use_candidate)]

pub mod checkpoints;
pub mod filter;
pub mod head;
pub mod helpers;
pub mod on_attestation;
pub mod on_attester_slashing;
pub mod on_block;
pub mod on_execution_payload_envelope;
pub mod on_payload_attestation_message;
pub mod on_tick;
pub mod payload_status;
pub mod proposer_head;
pub mod store;
pub mod timeliness;
pub mod weight;

pub use store::{ForkChoiceNode, LatestMessage, PayloadStatus, Store, get_forkchoice_store};

/// Diagnostic helpers for tests and developer tooling.
///
/// These APIs expose intermediate fork-choice observations without changing
/// head-selection semantics.
pub mod diagnostics {
    pub use super::head::{WeightedForkChoiceNode, get_viable_for_head_nodes};
}
