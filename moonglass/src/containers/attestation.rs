//! Validator vote containers and slashing evidence.
//!
//! An attestation is a validator vote for the current head block and for the
//! source/target checkpoints used by Casper finality. Conflicting votes can
//! become slashing evidence.

use crate::constants::{MAX_ATTESTING_INDICES, MAX_COMMITTEES_PER_SLOT};
use crate::containers::{Checkpoint, SignedBeaconBlockHeader};
use crate::primitives::{BLSSignature, CommitteeIndex, Root, Slot, ValidatorIndex};
use ssz_rs::prelude::*;

/// The vote payload an attester signs.
///
/// It names the slot, spec index field, head block, and finality checkpoints.
/// Aggregate attestations use `committee_bits` to identify participating
/// committees. The meaning of `index` depends on the attestation form being
/// evaluated. In payload-branch checks, a vote for a block whose slot equals
/// `data.slot` must use `index == 0` and remains pending for fork-choice
/// scoring. A vote naming an older head for `data.slot` uses `index == 0` for
/// the empty/no-payload branch and `index == 1` for the full/payload branch.
#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, SimpleSerialize)]
pub struct AttestationData {
    /// Slot the attestation is for.
    pub slot: Slot,
    /// Spec index field.
    ///
    /// For aggregate attestations this is not the sole committee selector, so use
    /// `committee_bits` to identify participating committees. For payload
    /// branch voting, a vote for a block at `data.slot` must use `0`. A vote for
    /// an older head uses `0` for the empty branch and `1` for the full branch.
    pub index: CommitteeIndex,
    /// Head block root being voted for by chain-head selection.
    pub beacon_block_root: Root,
    /// Source checkpoint used by finality.
    pub source: Checkpoint,
    /// Target checkpoint used by finality.
    pub target: Checkpoint,
}

/// Attestation expanded into sorted attester indices for signature verification.
///
/// `attesting_indices` is sorted and deduplicated, the form [`BeaconState::validate_indexed_attestation`](crate::containers::BeaconState::validate_indexed_attestation)
/// requires before checking the aggregate `signature` over `data`. Unlike
/// [`crate::containers::IndexedPayloadAttestation`], duplicate indices are invalid here because
/// each validator votes at most once.
#[derive(Default, Debug, Clone, PartialEq, Eq, SimpleSerialize)]
pub struct IndexedAttestation {
    /// Sorted, deduplicated indices of validators that attested.
    pub attesting_indices: List<ValidatorIndex, MAX_ATTESTING_INDICES>,
    /// The vote shared by all listed attesters.
    pub data: AttestationData,
    /// Aggregate BLS signature of the listed attesters.
    pub signature: BLSSignature,
}

/// Wire-form attestation: per-committee participation bitfield + aggregate signature.
///
/// The state transition consumes block-body attestations through
/// [`BeaconState::process_attestation`](crate::containers::BeaconState::process_attestation). Fork choice consumes both block and
/// gossip attestations through [`crate::fork_choice::Store::on_attestation()`] to update
/// latest messages for LMD-GHOST.
#[derive(Default, Debug, Clone, PartialEq, Eq, SimpleSerialize)]
pub struct Attestation {
    /// Per-attester participation, packed across all committees of the slot.
    pub aggregation_bits: Bitlist<MAX_ATTESTING_INDICES>,
    /// The vote shared by all participants.
    pub data: AttestationData,
    /// Aggregate signature over `data` from the participating attesters.
    pub signature: BLSSignature,
    /// Bitfield selecting which committees within the slot participated.
    pub committee_bits: Bitvector<MAX_COMMITTEES_PER_SLOT>,
}

/// Single-attester attestation form used on gossip before aggregation.
///
/// It names one `attester_index` and its `committee_index` directly rather than packing
/// participation into bitfields, so an individual vote can travel the network before it is
/// folded into an aggregate [`Attestation`].
#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, SimpleSerialize)]
pub struct SingleAttestation {
    /// Committee the attester belongs to.
    pub committee_index: CommitteeIndex,
    /// Validator that produced the attestation.
    pub attester_index: ValidatorIndex,
    /// The vote.
    pub data: AttestationData,
    /// Attester's signature over `data`.
    pub signature: BLSSignature,
}

/// Evidence of two contradictory attestations by overlapping validator sets.
///
/// Block processing validates and applies this through
/// [`BeaconState::process_attester_slashing`](crate::containers::BeaconState::process_attester_slashing). Fork choice records the
/// equivocating validators through [`crate::fork_choice::Store::on_attester_slashing()`].
#[derive(Default, Debug, Clone, PartialEq, Eq, SimpleSerialize)]
pub struct AttesterSlashing {
    /// First conflicting attestation.
    pub attestation_1: IndexedAttestation,
    /// Second conflicting attestation.
    ///
    /// Must share at least one attester with `attestation_1`.
    pub attestation_2: IndexedAttestation,
}

/// Evidence that a proposer signed two distinct block headers for the same slot.
///
/// Block processing validates and applies this through
/// [`BeaconState::process_proposer_slashing`](crate::containers::BeaconState::process_proposer_slashing).
#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, SimpleSerialize)]
pub struct ProposerSlashing {
    /// First signed header by the proposer.
    pub signed_header_1: SignedBeaconBlockHeader,
    /// Conflicting signed header by the same proposer for the same slot.
    pub signed_header_2: SignedBeaconBlockHeader,
}
