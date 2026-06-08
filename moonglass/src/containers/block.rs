//! Block-shaped containers: header, body, block, and their signed envelopes.

use ssz_rs::prelude::*;

use crate::constants::{
    MAX_ATTESTATIONS, MAX_ATTESTER_SLASHINGS, MAX_BLS_TO_EXECUTION_CHANGES, MAX_DEPOSITS,
    MAX_PAYLOAD_ATTESTATIONS, MAX_PROPOSER_SLASHINGS, MAX_VOLUNTARY_EXITS,
};
use crate::containers::{
    Attestation, AttesterSlashing, Deposit, Eth1Data, ExecutionRequests, PayloadAttestation,
    ProposerSlashing, SignedBLSToExecutionChange, SignedExecutionPayloadBid, SignedVoluntaryExit,
    SyncAggregate,
};
use crate::primitives::{BLSSignature, Bytes32, Root, Slot, ValidatorIndex};

/// Compact block summary stored in state and signed by proposers.
///
/// It carries the roots needed to identify a block without storing the full
/// body, and it is reused as proposer-slashing evidence.
#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, SimpleSerialize)]
pub struct BeaconBlockHeader {
    /// Slot the block is proposed for.
    pub slot: Slot,
    /// Validator that proposed the block.
    pub proposer_index: ValidatorIndex,
    /// Root of the parent block.
    pub parent_root: Root,
    /// Root of the post-state after applying the block.
    pub state_root: Root,
    /// Root of `BeaconBlockBody`.
    pub body_root: Root,
}

impl BeaconBlockHeader {
    /// Return this header with `state_root` set.
    #[must_use]
    pub fn with_state_root(mut self, state_root: Root) -> Self {
        self.state_root = state_root;
        self
    }
}

/// Header plus the proposer's signature.
#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, SimpleSerialize)]
pub struct SignedBeaconBlockHeader {
    /// The header being signed.
    pub message: BeaconBlockHeader,
    /// Proposer's signature over the domain-separated signing root of `message`.
    pub signature: BLSSignature,
}

/// All operations the proposer chose to include in this block.
///
/// Parent-payload requests and withdrawals are processed around this body. The
/// body itself carries randomness, votes, slashings, lifecycle operations,
/// payload-timeliness votes, and sync-committee participation.
#[derive(Default, Debug, Clone, PartialEq, Eq, SimpleSerialize)]
pub struct BeaconBlockBody {
    /// Proposer's RANDAO reveal, mixed into committee-shuffling randomness.
    pub randao_reveal: BLSSignature,
    /// Proposer's deposit-chain vote used to track new deposits.
    pub eth1_data: Eth1Data,
    /// Proposer-supplied freeform 32-byte tag, ignored by consensus.
    pub graffiti: Bytes32,
    /// Evidence of duplicate block proposals.
    pub proposer_slashings: List<ProposerSlashing, MAX_PROPOSER_SLASHINGS>,
    /// Evidence of double-vote or surround-vote misbehavior.
    pub attester_slashings: List<AttesterSlashing, MAX_ATTESTER_SLASHINGS>,
    /// Validator votes for the head block and finality checkpoints.
    pub attestations: List<Attestation, MAX_ATTESTATIONS>,
    /// Deposits observed on the execution-layer deposit contract.
    pub deposits: List<Deposit, MAX_DEPOSITS>,
    /// Validator-signed requests to leave the active set.
    pub voluntary_exits: List<SignedVoluntaryExit, MAX_VOLUNTARY_EXITS>,
    /// Aggregate sync-committee signature over the previous-slot block root.
    pub sync_aggregate: SyncAggregate,
    /// Requests to swap BLS withdrawal credentials for execution addresses.
    pub bls_to_execution_changes: List<SignedBLSToExecutionChange, MAX_BLS_TO_EXECUTION_CHANGES>,
    /// Builder bid the proposer committed to for this slot.
    pub signed_execution_payload_bid: SignedExecutionPayloadBid,
    /// Payload-timeliness committee votes for the parent slot's payload.
    pub payload_attestations: List<PayloadAttestation, MAX_PAYLOAD_ATTESTATIONS>,
    /// Execution-to-consensus requests from the parent slot's payload.
    ///
    /// The block proves these requests by matching the accepted parent bid's
    /// `execution_requests_root` before applying them.
    pub parent_execution_requests: ExecutionRequests,
}

/// Proposed beacon block with its slot identity, claimed post-state root, and
/// operations.
#[derive(Default, Debug, Clone, PartialEq, Eq, SimpleSerialize)]
pub struct BeaconBlock {
    /// Slot the block is for.
    pub slot: Slot,
    /// Validator that proposed the block.
    pub proposer_index: ValidatorIndex,
    /// Root of the parent block.
    pub parent_root: Root,
    /// Root of the post-state produced by applying this block.
    pub state_root: Root,
    /// Block operations.
    pub body: BeaconBlockBody,
}

impl BeaconBlock {
    /// Header corresponding to this block and the supplied body/state roots.
    #[must_use]
    pub fn header(&self, body_root: Root, state_root: Root) -> BeaconBlockHeader {
        BeaconBlockHeader {
            slot: self.slot,
            proposer_index: self.proposer_index,
            parent_root: self.parent_root,
            state_root,
            body_root,
        }
    }
}

/// Beacon block plus the proposer's signature.
#[derive(Default, Debug, Clone, PartialEq, Eq, SimpleSerialize)]
pub struct SignedBeaconBlock {
    /// The block being signed.
    pub message: BeaconBlock,
    /// Proposer's signature over the domain-separated signing root of `message`.
    pub signature: BLSSignature,
}
