//! Sync-committee machinery.

use ssz_rs::prelude::*;

use crate::constants::SYNC_COMMITTEE_SIZE;
use crate::primitives::{BLSPubkey, BLSSignature};

/// Set of validators rotated in to sign sync updates each sync period.
#[derive(Default, Debug, Clone, PartialEq, Eq, SimpleSerialize)]
pub struct SyncCommittee {
    /// Member public keys, in committee order.
    pub pubkeys: Vector<BLSPubkey, SYNC_COMMITTEE_SIZE>,
    /// Sum of `pubkeys`, used for fast aggregate verification.
    pub aggregate_pubkey: BLSPubkey,
}

/// Aggregated sync-committee signature over the previous slot's block root.
#[derive(Default, Debug, Clone, PartialEq, Eq, SimpleSerialize)]
pub struct SyncAggregate {
    /// Bit per committee member, set to 1 if the member's signature is included.
    pub sync_committee_bits: Bitvector<SYNC_COMMITTEE_SIZE>,
    /// Aggregate signature of the participating committee members.
    pub sync_committee_signature: BLSSignature,
}
