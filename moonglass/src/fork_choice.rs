//! Fork choice store.
//!
//! Reads accepted blocks and attestations, decides which leaf the next
//! block should build on. Reuses `crate::state_transition` to advance
//! cached states; this module does not duplicate transition rules.
//!
//! Surface mirrors consensus-specs: field names and function names match
//! the spec verbatim so the two read side by side.
//!
//! # Not yet implemented: execution-payload verification
//!
//! Moonglass currently runs the consensus-side checks on an execution payload
//! envelope (signature, bid match, randao, gas, hash, requests-root, slot,
//! timestamp, withdrawals). Execution-engine validity and blob KZG
//! verification are listed under README's "Possible Next Steps" and are not
//! yet wired in. Until they are, no code path inserts into [`Store::payloads`],
//! so `is_payload_verified` returns `false` for every block root.
//!
//! Affected paths: any block extending a "full" chain is rejected, any
//! index-1 attestation is rejected, [`get_head`] never returns a node with
//! [`PayloadStatus::Full`], and the payload-extension tiebreaker degenerates
//! to the empty/pending branches.
//!
//! The future implementation will plug an execution-engine binding plus a
//! blob-KZG verifier into the envelope handler, feeding [`Store::payloads`].
//!
//! [`Store::payloads`]: store::Store::payloads
//! [`get_head`]: head::get_head
//! [`PayloadStatus::Full`]: store::PayloadStatus::Full

mod checkpoints;
mod filter;
mod head;
mod helpers;
mod on_attestation;
mod on_attester_slashing;
mod on_block;
mod on_execution_payload_envelope;
mod on_payload_attestation_message;
mod on_tick;
mod payload_status;
mod store;
mod timeliness;
mod weight;

pub use head::get_head;
pub use on_attestation::on_attestation;
pub use on_attester_slashing::on_attester_slashing;
pub use on_block::on_block;
pub use on_execution_payload_envelope::on_execution_payload_envelope;
pub use on_payload_attestation_message::on_payload_attestation_message;
pub use on_tick::on_tick;
pub use payload_status::get_parent_payload_status;
pub use store::{ForkChoiceNode, LatestMessage, PayloadStatus, Store, get_forkchoice_store};
