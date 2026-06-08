//! Builder-market containers.
//!
//! Builders supply execution payloads for proposed beacon blocks. They bid for
//! slots, and the payload-timeliness committee votes on whether the payload and
//! blob data were available in time for builder payment to be released.
//! This module models the consensus objects for that builder-supplied payload
//! path.

use ssz_rs::prelude::*;

use crate::constants::PTC_SIZE;
use crate::primitives::{
    BLSPubkey, BLSSignature, BuilderIndex, Epoch, ExecutionAddress, Gwei, Root, Slot,
    ValidatorIndex,
};

/// A single builder entry in the builder registry.
#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, SimpleSerialize)]
pub struct Builder {
    /// Builder's BLS public key used to verify bid signatures.
    pub pubkey: BLSPubkey,
    /// Builder registry record version.
    pub version: u8,
    /// Execution-layer address that receives builder payments.
    pub execution_address: ExecutionAddress,
    /// Builder's stake balance backing accepted bids.
    pub balance: Gwei,
    /// Epoch at which the builder was added to the registry.
    pub deposit_epoch: Epoch,
    /// Epoch the balance becomes withdrawable, or `FAR_FUTURE_EPOCH` while active.
    pub withdrawable_epoch: Epoch,
}

/// Outbound payment a builder owes for an accepted bid, queued for the withdrawal sweep.
#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, SimpleSerialize)]
pub struct BuilderPendingWithdrawal {
    /// Execution-layer address the payment is destined for.
    pub fee_recipient: ExecutionAddress,
    /// Amount the builder owes.
    pub amount: Gwei,
    /// Builder making the payment.
    pub builder_index: BuilderIndex,
}

/// Builder payment accumulator entry weighted by payload-timeliness participation.
#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, SimpleSerialize)]
pub struct BuilderPendingPayment {
    /// Sum of committee members' effective balance weighting toward payment quorum.
    pub weight: Gwei,
    /// The payment that gets released once the quorum threshold is met.
    pub withdrawal: BuilderPendingWithdrawal,
}

/// The vote a payload-timeliness committee member signs for a slot.
#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, SimpleSerialize)]
pub struct PayloadAttestationData {
    /// Beacon block root the payload is associated with.
    pub beacon_block_root: Root,
    /// Slot the attestation is for.
    pub slot: Slot,
    /// True if the payload was observed to be available.
    pub payload_present: bool,
    /// True if blob data was observed alongside the payload.
    pub blob_data_available: bool,
}

/// Aggregated payload-timeliness vote: per-member bitfield plus aggregate signature.
#[derive(Default, Debug, Clone, PartialEq, Eq, SimpleSerialize)]
pub struct PayloadAttestation {
    /// Bit per committee member, set to 1 if their signature is included.
    pub aggregation_bits: Bitvector<PTC_SIZE>,
    /// The shared vote.
    pub data: PayloadAttestationData,
    /// Aggregate signature over `data`.
    pub signature: BLSSignature,
}

/// Single-member payload-timeliness vote used on gossip before aggregation.
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
#[derive(Default, Debug, Clone, PartialEq, Eq, SimpleSerialize)]
pub struct IndexedPayloadAttestation {
    /// Sorted, deduplicated indices of committee members that attested.
    pub attesting_indices: List<ValidatorIndex, PTC_SIZE>,
    /// The shared vote.
    pub data: PayloadAttestationData,
    /// Aggregate signature.
    pub signature: BLSSignature,
}
