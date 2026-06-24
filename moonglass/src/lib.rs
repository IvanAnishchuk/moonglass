//! A behavior-first guide to the Ethereum consensus specs.
//!
//! Ethereum consensus is the rulebook validators use to agree on chain state.
//! This crate models the data validators agree on, the signed blocks that
//! propose changes to it, and the transition rules that decide whether those
//! changes are valid.
//!
//! This crate is a readable execution map, not a production client architecture.
//! Its data structures and main function shapes stay close to the consensus
//! specs where that helps orientation, and helpers may use clearer Rust shapes when
//! that makes the protocol behavior easier to follow.
//! The covered surface is the currently wired consensus-specs reference-test
//! surface. `mainnet` and `minimal` are spec presets, not claims that every
//! documented branch is live-network behavior or fixture-covered.
//!
//! Start with a protocol object, not with a file tree. Ask which handler owns
//! the object, which fields it reads, which fields it writes, whether the write
//! is durable consensus state or local fork-choice view, where the current
//! implementation deliberately stops, and which fixture family exercises that
//! path. The core route is object -> owning handler -> reads -> writes ->
//! decision -> boundary -> fixture.
//!
//! # Start by object
//!
//! Follow one route at a time. Each route names the object to start from, the
//! handler that owns it, the state/store distinction to watch, and the fixture
//! family that closes the loop.
//!
//! - Block acceptance:
//!   start at [`containers::SignedBeaconBlock`], then inspect
//!   [`containers::BeaconState::apply_signed_block`] and
//!   [`fork_choice::Store::on_block()`].
//!   Watch the state transition mutate a cloned
//!   [`BeaconState`](containers::BeaconState) while fork choice caches the
//!   post-state in [`fork_choice::Store`]. Fixtures:
//!   `sanity/blocks`, `fork_choice/on_block`.
//! - Bid commitment:
//!   start at [`containers::ExecutionPayloadBid`], then inspect
//!   [`containers::BeaconState::process_execution_payload_bid`]. A bid commits
//!   consensus state to payload fields and opens any builder-payment obligation.
//!   It is not delivered payload evidence. Fixture:
//!   `operations/execution_payload_bid`.
//! - Delivered payload evidence:
//!   start at [`containers::SignedExecutionPayloadEnvelope`], then inspect
//!   [`fork_choice::Store::on_execution_payload_envelope()`] and
//!   [`fork_choice::Store::payloads`]. The envelope passed the implemented
//!   consensus-side checks, not full execution-engine or blob-availability
//!   verification. Fixture: `fork_choice/on_execution_payload_envelope`.
//! - Parent-payload settlement:
//!   start at [`containers::ExecutionPayloadBid`] and
//!   [`containers::ExecutionRequests`], then inspect
//!   [`containers::BeaconState::accept_parent_payload_commitment`]. The child
//!   proves its parent payload handoff, applies the parent payload's requests,
//!   and releases the parent builder payment. Fixture:
//!   `operations/parent_execution_payload`.
//! - PTC votes:
//!   start at [`containers::PayloadAttestation`] and
//!   [`containers::PayloadAttestationMessage`], then inspect
//!   [`containers::BeaconState::process_payload_attestation`] and
//!   [`fork_choice::Store::on_payload_attestation_message()`]. Block aggregates
//!   and gossip messages are admitted differently, but the store records local
//!   PTC vote evidence by position. Fixtures: `operations/payload_attestation`,
//!   `fork_choice/on_payload_attestation_message`.
//! - Beacon attestation branch choice:
//!   start at [`containers::Attestation`], then inspect
//!   [`containers::BeaconState::process_attestation`] and
//!   [`fork_choice::Store::on_attestation()`]. Votes for a block at `data.slot`
//!   stay pending. Votes for an older head use `AttestationData::index` as the
//!   payload empty/full selector. Fixtures: `operations/attestation`,
//!   `fork_choice/on_attestation`.
//! - Head selection:
//!   start at [`fork_choice::ForkChoiceNode`], then inspect
//!   [`fork_choice::Store::get_head`]. Fork choice returns both a block root and
//!   a [`fork_choice::PayloadStatus`]. Fixture: `fork_choice/get_head`.
//!
//! Use [`state_transition`] to follow a block through the rulebook, then use
//! [`containers`] for the data being moved, [`primitives`] and [`constants`] for
//! vocabulary and parameters, [`error`] for the difference between an invalid
//! transition and behavior not yet covered, and [`fork_choice`] for
//! the head-selection rule that reads accepted blocks and attestations to decide
//! which leaf the next block should build on.
//!
//! Build docs with private items when reading this crate as an executable spec:
//! most of the useful phase maps live in private modules because they mirror
//! consensus sub-phases rather than form a public API surface.
//!
//! Coverage boundaries are part of the reading surface. When a consensus branch
//! can be exercised without yet implementing every external verifier, the
//! relevant module docs should name that boundary explicitly.
//!
//! # Hold these distinctions before reading
//!
//! A few distinctions decide whether the rest of the code reads correctly.
//! Hold them before following any route.
//!
//! - [`BeaconState`](containers::BeaconState) is durable consensus state, the
//!   snapshot validators agree on and carry forward. The fork-choice
//!   [`Store`](fork_choice::Store) is one node's local view, the accepted
//!   blocks, attestations, and clock that node has seen. The store is
//!   bookkeeping for head selection, not consensus state, and two honest nodes
//!   can hold different stores.
//! - A builder's bid is a commitment, not an accepted payload. Recording the bid
//!   promises a payload at a hash, but the payload effects settle only when a
//!   child block proves and applies the parent payload commitment.
//! - A recorded payload envelope has passed only the consensus-side checks:
//!   beacon block roots, required envelope signature, bid-matched payload
//!   fields, payload slot, parent execution hash, timestamp, requests root, and
//!   withdrawals.
//!   Recording it is not an execution-engine validity verdict and not a
//!   data-availability verdict.
//! - Payload-timeliness votes are indexed by committee position. A gossip
//!   message names one validator and expands to the committee positions that
//!   validator holds, so the same vote reads as a validator on the wire and as
//!   a set of positions in the aggregate.
//! - A beacon attestation's `index` is a payload-branch selector only when
//!   the voted block is older than `attestation.data.slot`. If the voted block is
//!   at `data.slot`, the vote must use the empty/pending form. The two rulebooks
//!   then read that selector differently. In state transition, a historical
//!   vote's `index` is matched against the `BeaconState` payload-availability bit
//!   to decide whether the vote earns its head flag, not whether the vote is
//!   accepted. In fork choice, an older full-branch vote is admitted only once the
//!   local [`fork_choice::Store::payloads`] map holds the recorded envelope. One
//!   shapes a head-flag reward from durable consensus state, the other is a
//!   node-local admission gate.
//! - A child block applies the parent payload's effects before its own bid. It
//!   settles the parent block's promised payload first, then records its own
//!   commitment for a later child to prove.
//!
//! # Where this implementation stops
//!
//! The implementation runs the consensus-side rules and stops at the external
//! verifiers a production client would also wire in. Three boundaries are
//! deliberate and not yet implemented. Execution-engine payload validity is not
//! checked, so a recorded payload is consensus-checked, not engine-confirmed.
//! Blob and data-availability verification is not performed. Networking and
//! gossip validation are not wired in, so handlers accept already-deserialized
//! objects rather than messages off the wire. Within those boundaries the code
//! can still exercise the covered consensus branches, including the
//! payload-status branches in fork choice, which is why each affected module
//! names its own boundary in its docs.
#[cfg(not(any(feature = "mainnet", feature = "minimal")))]
compile_error!("crate must be built with exactly one of the `mainnet` or `minimal` features");

#[cfg(all(feature = "mainnet", feature = "minimal"))]
compile_error!(
    "crate cannot be built with both `mainnet` and `minimal` features (cargo features are additive)"
);

pub mod constants;
pub mod containers;
pub mod crypto;
pub mod error;
pub mod fork_choice;
pub mod glossary;
pub mod primitives;
pub mod state_transition;
