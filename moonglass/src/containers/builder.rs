//! Builder-market containers.
//!
//! Builders supply execution payloads for proposed beacon blocks. They bid for
//! slots, beacon attestations add the state-transition quorum weight used to
//! release builder payments, and payload-timeliness committee votes feed fork
//! choice's local payload-status evidence. This module models the consensus
//! objects for that builder-supplied payload path.

use ssz_rs::prelude::*;

use crate::constants::PTC_SIZE;
use crate::primitives::{
    BLSPubkey, BLSSignature, BuilderIndex, Epoch, ExecutionAddress, Gwei, Root, Slot,
    ValidatorIndex,
};

/// A single builder entry in the builder registry, indexed by [`BuilderIndex`].
///
/// The `pubkey` verifies a builder's bid signatures, and `balance` is the stake that backs
/// accepted bids and funds the payments owed by them. A builder stays active until
/// `withdrawable_epoch`, which holds `FAR_FUTURE_EPOCH` while the builder is live.
#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, SimpleSerialize)]
pub struct Builder {
    /// Builder's BLS public key used to verify bid signatures.
    pub pubkey: BLSPubkey,
    /// Builder registry record version.
    pub version: u8,
    /// Execution-layer address that receives builder withdrawals.
    pub execution_address: ExecutionAddress,
    /// Builder's stake balance backing accepted bids.
    pub balance: Gwei,
    /// Epoch at which the builder was added to the registry.
    pub deposit_epoch: Epoch,
    /// Epoch the balance becomes withdrawable, or `FAR_FUTURE_EPOCH` while active.
    pub withdrawable_epoch: Epoch,
}

/// Outbound payment a builder owes for an accepted bid, queued for the withdrawal sweep.
///
/// Accepting a bid does not transfer funds immediately. It opens this obligation against the
/// builder's balance, payable to `fee_recipient`, which the withdrawal sweep settles once the
/// payment becomes due.
#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, SimpleSerialize)]
pub struct BuilderPendingWithdrawal {
    /// Execution-layer address the payment is destined for.
    pub fee_recipient: ExecutionAddress,
    /// Amount the builder owes.
    pub amount: Gwei,
    /// Builder making the payment.
    pub builder_index: BuilderIndex,
}

/// Builder payment accumulator entry for one accepted bid.
///
/// A bid opens the `withdrawal` obligation with zero `weight`. Beacon
/// attestations for that proposal slot add effective balance only when they set
/// a new participation flag for the attester. The entry can be queued by
/// parent-payload handoff, queued at epoch aging if `weight` clears the quorum,
/// dropped when it ages out below quorum, or cleared by proposer slashing while
/// still in the two-epoch payment window. PTC votes are separate fork-choice
/// evidence about payload timeliness and blob data availability. They do not add
/// this payment weight.
#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, SimpleSerialize)]
pub struct BuilderPendingPayment {
    /// Sum of committee members' effective balance weighting toward payment quorum.
    pub weight: Gwei,
    /// The payment obligation queued by parent-payload handoff or quorum release.
    pub withdrawal: BuilderPendingWithdrawal,
    /// Proposer that opened this payment, so proposer slashing only clears the
    /// payment when it slashes that same proposer.
    pub proposer_index: ValidatorIndex,
}

/// The vote a payload-timeliness committee member signs for a slot.
///
/// It records whether the payload for `beacon_block_root` at `slot` was seen
/// (`payload_present`) and whether its blob data arrived alongside it (`blob_data_available`).
/// The same data appears inside block aggregates ([`PayloadAttestation`]) and gossip messages
/// ([`PayloadAttestationMessage`]), so a committee member's verdict travels unchanged whether
/// it is broadcast individually or folded into an aggregate.
#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, SimpleSerialize)]
pub struct PayloadAttestationData {
    /// Beacon block root the payload is associated with.
    pub beacon_block_root: Root,
    /// Slot the attestation is for.
    pub slot: Slot,
    /// True if the payload was seen on time, before its due time in the slot.
    pub payload_present: bool,
    /// True if blob data was observed alongside the payload.
    pub blob_data_available: bool,
}

/// Aggregated payload-timeliness vote carried in the block body: per-position bitfield plus
/// aggregate signature.
/// `aggregation_bits` is indexed by committee position, not by validator index,
/// so a set bit means the validator occupying that position attested.
/// [`BeaconState::process_payload_attestation`](crate::containers::BeaconState::process_payload_attestation)
/// validates this aggregate form. The current fork-choice replay path expands
/// those bits to validator indices and then reuses
/// [`crate::fork_choice::Store::on_payload_attestation_message()`], whose local store
/// write expands each validator back to every PTC position it occupies. Contrast
/// [`PayloadAttestationMessage`], which names a single validator index directly.
#[derive(Default, Debug, Clone, PartialEq, Eq, SimpleSerialize)]
pub struct PayloadAttestation {
    /// Bit per committee position, set to 1 if that position's signature is included.
    pub aggregation_bits: Bitvector<PTC_SIZE>,
    /// The shared vote.
    pub data: PayloadAttestationData,
    /// Aggregate signature over `data`.
    pub signature: BLSSignature,
}

/// Single-member payload-timeliness vote used on gossip before aggregation.
///
/// Handled by [`crate::fork_choice::Store::on_payload_attestation_message()`]. This form
/// names a validator index, and fork choice expands it to every PTC position the
/// validator occupies before updating payload vote vectors.
#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, SimpleSerialize)]
pub struct PayloadAttestationMessage {
    /// Committee member that produced the vote.
    pub validator_index: ValidatorIndex,
    /// The vote.
    pub data: PayloadAttestationData,
    /// Member's signature over `data`.
    pub signature: BLSSignature,
}

/// Payload-timeliness vote expanded into sorted participant indices.
///
/// Built by [`BeaconState::indexed_payload_attestation`](crate::containers::BeaconState::indexed_payload_attestation) and validated by
/// [`BeaconState::validate_indexed_payload_attestation`](crate::containers::BeaconState::validate_indexed_payload_attestation). Unlike ordinary
/// indexed beacon attestations, duplicate validator indices are valid here
/// because PTC membership is position-based and a validator may occupy multiple
/// positions.
#[derive(Default, Debug, Clone, PartialEq, Eq, SimpleSerialize)]
pub struct IndexedPayloadAttestation {
    /// Sorted indices of committee members that attested. Duplicates are valid
    /// because a validator may occupy multiple PTC positions.
    pub attesting_indices: List<ValidatorIndex, PTC_SIZE>,
    /// The shared vote.
    pub data: PayloadAttestationData,
    /// Aggregate signature.
    pub signature: BLSSignature,
}
