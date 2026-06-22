//! Vocabulary for reading this crate, from the bedrock consensus terms to the
//! fork-choice and payload terms that newcomers find most confusing.
//!
//! Start here whenever a term is new. Entries are listed alphabetically. Each
//! gives a plain definition and links to the Rust item that represents it, so a
//! name in the code can always be traced back to its meaning.
//!
//! ## Anchor
//!
//! The trusted starting point fork choice begins from, usually a recent finalized
//! block and its state, loaded by
//! [`get_forkchoice_store`](crate::fork_choice::get_forkchoice_store). Everything
//! in the store descends from the anchor.
//!
//! ## Attestation
//!
//! A validator's signed vote, an [`Attestation`](crate::containers::Attestation).
//! It names the block the validator sees as the head and the checkpoint it wants
//! to finalize. For checking, an aggregate is expanded into an
//! [`IndexedAttestation`](crate::containers::IndexedAttestation).
//!
//! ## Beacon block
//!
//! The consensus object a proposer publishes for its slot, a
//! [`BeaconBlock`](crate::containers::BeaconBlock). It carries the votes, builder
//! bid, and other operations that move the chain forward.
//!
//! ## Beacon state
//!
//! The consensus state that results from applying blocks in order, a
//! [`BeaconState`](crate::containers::BeaconState). It is fully determined by the
//! blocks applied to it, so any node that applies the same blocks computes the
//! same state. Not the same as the fork-choice
//! [`Store`](crate::fork_choice::Store), which is one node's private view and can
//! differ between nodes that currently see different heads.
//!
//! ## Builder
//!
//! The party that constructs an execution payload and bids for it to be included,
//! identified by a [`BuilderIndex`](crate::primitives::BuilderIndex). A builder
//! need not be the proposer.
//!
//! ## Checkpoint
//!
//! The chain landmark used for finality, a
//! [`Checkpoint`](crate::containers::Checkpoint): the block at an epoch's first
//! slot, or, when that slot is empty, the latest ancestor block before it. Votes
//! name checkpoints as their finality targets.
//!
//! ## Committee
//!
//! The subset of validators assigned to vote in a particular slot, identified by
//! a [`CommitteeIndex`](crate::primitives::CommitteeIndex). Splitting validators
//! into committees keeps the number of votes per slot manageable.
//!
//! ## Committee position
//!
//! A validator's slot within a committee, as opposed to its global
//! [`ValidatorIndex`](crate::primitives::ValidatorIndex). Payload-timeliness votes
//! are recorded by position, and one validator can hold several, listed by
//! [`ptc_positions_for_validator`](crate::fork_choice::on_payload_attestation_message::ptc_positions_for_validator).
//!
//! ## Data availability
//!
//! Whether the data needed to use and check a payload can actually be downloaded
//! from the network. Fork choice gates a payload on it through
//! [`is_data_available`](crate::fork_choice::on_execution_payload_envelope::is_data_available),
//! which is currently mocked.
//!
//! ## Dependent root
//!
//! The earlier block whose root fixes the randomness for a given epoch's duties,
//! computed by
//! [`get_dependent_root`](crate::fork_choice::Store::get_dependent_root).
//! Proposer boost only competes blocks that share a dependent root, so a boost
//! cannot carry across a change in duties.
//!
//! ## Effective balance
//!
//! The rounded amount of stake that counts toward a validator's voting weight,
//! measured in [`Gwei`](crate::primitives::Gwei). More effective balance behind a
//! block means more weight in fork choice. Not the same as a validator's exact
//! balance, which it tracks only in coarse steps.
//!
//! ## Epoch
//!
//! A fixed run of slots, an [`Epoch`](crate::primitives::Epoch). It is the unit
//! over which validator duties are assigned and over which the chain decides what
//! to finalize.
//!
//! ## Equivocation
//!
//! Signing conflicting consensus messages: two different blocks for one slot, two
//! different attestations for one target epoch (a double vote), or one
//! attestation surrounding another (a surround vote). Consensus treats it as
//! misbehaviour, and fork choice records the offender in
//! [`equivocating_indices`](crate::fork_choice::Store::equivocating_indices),
//! then ignores their votes.
//!
//! ## Execution engine
//!
//! The execution-layer component that runs a payload's transactions and reports
//! whether it is valid. Fork choice consults it through
//! [`verify_and_notify_new_payload`](crate::fork_choice::on_execution_payload_envelope::verify_and_notify_new_payload),
//! which is currently mocked.
//!
//! ## Execution payload
//!
//! The execution-layer contents of a slot, an
//! [`ExecutionPayload`](crate::containers::ExecutionPayload): the transactions,
//! the withdrawals, and the execution header fields. Not the same as the beacon
//! block, which is the consensus object. The payload is produced separately and
//! delivered in an envelope, which also carries the execution-to-consensus
//! requests.
//!
//! ## Execution payload bid
//!
//! A builder's up-front commitment to produce a particular payload, carried in
//! the beacon block as an
//! [`ExecutionPayloadBid`](crate::containers::ExecutionPayloadBid). It is only a
//! promise. The actual payload arrives later in an envelope.
//!
//! ## Execution payload envelope
//!
//! The later object that carries the actual payload and proves it matches the
//! bid, an [`ExecutionPayloadEnvelope`](crate::containers::ExecutionPayloadEnvelope).
//! Recording one is what lets a block's full branch exist in fork choice.
//!
//! ## FFG
//!
//! The finality layer of consensus: the part that justifies and finalizes
//! checkpoints (see
//! [justification and finalization](crate::glossary#justification-and-finalization)).
//! A re-org must not set finality back, a rule
//! [`is_ffg_competitive`](crate::fork_choice::Store::is_ffg_competitive)
//! enforces. Distinct from [LMD-GHOST](crate::glossary#lmd-ghost), which only
//! picks the head.
//!
//! ## Fork choice
//!
//! The rule that picks the canonical chain when branches compete, implemented in
//! [`fork_choice`](crate::fork_choice). It follows the branch with the most
//! supporting stake at each step.
//!
//! ## `ForkChoiceNode`
//!
//! A position in the fork-choice tree: a block [root](crate::glossary#root) paired
//! with a [payload status](crate::glossary#payload-status), a
//! [`ForkChoiceNode`](crate::fork_choice::ForkChoiceNode). Because a payload
//! arrives apart from its block, one block can appear as more than one node, so
//! fork choice weighs nodes, not bare block roots.
//!
//! ## Genesis
//!
//! The chain's starting point, time zero, recorded as
//! [`genesis_time`](crate::containers::BeaconState::genesis_time). Every slot and
//! epoch is counted forward from genesis.
//!
//! ## Head
//!
//! The node fork choice currently treats as the tip of the canonical chain,
//! returned by [`get_head`](crate::fork_choice::Store::get_head). It is a
//! [`ForkChoiceNode`](crate::fork_choice::ForkChoiceNode), a
//! [block](crate::glossary#beacon-block) root paired with a payload branch, not a
//! bare block. A proposer usually builds its next block on the head, but may
//! instead build on the head's parent when
//! [`get_proposer_head`](crate::fork_choice::Store::get_proposer_head) calls for a
//! re-org. Not the same as the latest finalized checkpoint, which sits further
//! back in the chain.
//!
//! ## Justification and finalization
//!
//! The two stages by which consensus commits to a checkpoint. Justified means the
//! chain has voted for it this round. Finalized means enough rounds have agreed
//! that reverting it would require a slashable safety violation, where a large
//! fraction of validators lose their stake. The chain records the two as
//! [`current_justified_checkpoint`](crate::containers::BeaconState::current_justified_checkpoint)
//! and [`finalized_checkpoint`](crate::containers::BeaconState::finalized_checkpoint).
//!
//! ## Latest message
//!
//! The most recent attestation fork choice counts for a validator, a
//! [`LatestMessage`](crate::fork_choice::LatestMessage) held in the store. Only
//! the newest one per validator counts, so a later vote replaces an earlier one.
//!
//! ## LMD-GHOST
//!
//! The specific rule fork choice uses, run by
//! [`get_head`](crate::fork_choice::Store::get_head). It takes each validator's latest
//! vote, then from the justified root walks to the child branch with the most
//! supporting stake, over and over, until it reaches a head.
//!
//! ## Payload attestation
//!
//! A committee member's vote about a payload's timeliness and data availability,
//! a [`PayloadAttestationMessage`](crate::containers::PayloadAttestationMessage).
//! Its aggregate form is a
//! [`PayloadAttestation`](crate::containers::PayloadAttestation). Distinct from a
//! normal [`Attestation`](crate::containers::Attestation), which votes on the head
//! and finality, not on the payload.
//!
//! ## Payload separation
//!
//! The design this crate implements, in which a block's execution payload is
//! produced and delivered apart from the beacon block itself: a
//! [builder](crate::glossary#builder) commits to it with a
//! [bid](crate::glossary#execution-payload-bid), then delivers it later in an
//! [envelope](crate::glossary#execution-payload-envelope). It is why fork choice
//! needs a [payload status](crate::glossary#payload-status) per block and a
//! [payload-timeliness committee](crate::glossary#payload-timeliness-committee).
//!
//! ## Payload status
//!
//! Which fork-choice branch a block sits on, given that its execution payload
//! arrives separately: [`Empty`](crate::fork_choice::PayloadStatus::Empty)
//! carries the chain forward without this payload,
//! [`Full`](crate::fork_choice::PayloadStatus::Full) carries it forward on top of
//! the recorded payload, and
//! [`Pending`](crate::fork_choice::PayloadStatus::Pending) is unresolved. Because
//! the payload arrives apart from the beacon block, one block can appear in the
//! tree as more than one node, one per branch. See
//! [`PayloadStatus`](crate::fork_choice::PayloadStatus).
//!
//! ## Payload-timeliness committee
//!
//! A committee, often shortened to PTC, assigned each slot to watch for the
//! payload and report whether it arrived on time with its data available. Its
//! size is [`PTC_SIZE`](crate::constants::PTC_SIZE). Not the same as the beacon
//! committee that casts head and finality votes.
//!
//! ## Proposer
//!
//! The validator chosen for a slot to publish the beacon block, returned by
//! [`beacon_proposer_index`](crate::containers::BeaconState::beacon_proposer_index).
//! Not the same as the builder: the proposer publishes the block, the builder
//! supplies the payload.
//!
//! ## Proposer boost
//!
//! Temporary extra weight given to a block proposed on time in the current slot,
//! applied by
//! [`should_apply_proposer_boost`](crate::fork_choice::Store::should_apply_proposer_boost).
//! It makes a fresh, timely head harder to re-org, and it wears off at the next
//! slot.
//!
//! ## Pulled-up checkpoint
//!
//! The justification and finalization a block's state already implies, found by
//! replaying them on a copy of that state in
//! [`compute_pulled_up_tip`](crate::fork_choice::Store::compute_pulled_up_tip).
//! It can run ahead of what the store has formally adopted.
//!
//! ## Re-org
//!
//! When fork choice switches the canonical head from one branch to a competing
//! one. A proposer can deliberately trigger a single-slot re-org of a late, weak
//! head through [`get_proposer_head`](crate::fork_choice::Store::get_proposer_head).
//!
//! ## Root
//!
//! A hash that identifies a block, state, or other container, a
//! [`Root`](crate::primitives::Root). The code passes roots around constantly as
//! compact stand-ins for the things they name, and two objects with the same root
//! are identical.
//!
//! ## Slot
//!
//! The fixed, short time window in which at most one block may be proposed,
//! counted from genesis as a [`Slot`](crate::primitives::Slot). A slot can also be
//! empty, when its proposer produces nothing.
//!
//! ## Store
//!
//! The node-local fork-choice view, a [`Store`](crate::fork_choice::Store): the
//! blocks and states it knows, the latest vote from each validator, recorded
//! payloads, checkpoints, timing, and proposer boost. Not the same as the beacon
//! state, which is shared consensus state, while the store is private to one node
//! and two nodes can hold different ones.
//!
//! ## Unrealized checkpoint
//!
//! A justified or finalized checkpoint a block's state implies but the store has
//! not yet adopted as official, held in
//! [`unrealized_justified_checkpoint`](crate::fork_choice::Store::unrealized_justified_checkpoint).
//! The next epoch boundary promotes it to the confirmed checkpoint. One implied
//! by a block already in a past epoch is adopted at once instead.
//!
//! ## Validator
//!
//! A participant that locks up funds to take part in consensus, in return for the
//! right to propose blocks and to vote. Each one is identified by a
//! [`ValidatorIndex`](crate::primitives::ValidatorIndex), and its full record
//! (stake, keys, status) is a [`Validator`](crate::containers::Validator).
//!
//! ## Viable block
//!
//! A block allowed to compete for head because its justified and finalized view
//! is compatible with the store's, decided by
//! [`get_filtered_block_tree`](crate::fork_choice::Store::get_filtered_block_tree).
//! Blocks that are not viable are pruned before any weighing.
//!
//! ## Voting source
//!
//! The justified [checkpoint](crate::glossary#checkpoint) a block's view of
//! finality rests on, computed by
//! [`get_voting_source`](crate::fork_choice::Store::get_voting_source). A block
//! competes for head only while its voting source is recent enough, part of what
//! makes it a [viable block](crate::glossary#viable-block).
