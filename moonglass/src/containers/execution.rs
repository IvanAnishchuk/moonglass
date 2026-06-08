//! Execution-layer data carried through consensus.
//!
//! Execution payloads contain opaque execution transactions and block-access
//! list bytes. Consensus does not inspect their internal execution semantics.
//! It tracks roots, hashes, requests, and builder commitments needed by the
//! beacon-state transition.
//!
//! Inclusion-list bid extensions are not represented in the current
//! `ExecutionPayloadBid` shape.

use crate::constants::{
    BYTES_PER_LOGS_BLOOM, MAX_BLOB_COMMITMENTS_PER_BLOCK, MAX_BYTES_PER_TRANSACTION,
    MAX_EXTRA_DATA_BYTES, MAX_TRANSACTIONS_PER_PAYLOAD, MAX_WITHDRAWALS_PER_PAYLOAD,
};
use crate::containers::{ExecutionRequests, Withdrawal};
use crate::primitives::{
    BLSSignature, BuilderIndex, Bytes32, ExecutionAddress, Gwei, Hash32, KZGCommitment, Root, Slot,
    Uint256,
};
use ssz_rs::prelude::*;

/// Opaque RLP-encoded block access list. Layout is not unpacked by consensus.
pub type BlockAccessList = List<u8, MAX_BYTES_PER_TRANSACTION>;

/// A single execution-layer transaction as an opaque byte list.
pub type Transaction = List<u8, MAX_BYTES_PER_TRANSACTION>;

/// The list of transactions an `ExecutionPayload` carries.
pub type Transactions = List<Transaction, MAX_TRANSACTIONS_PER_PAYLOAD>;

/// Execution-layer block payload delivered for a beacon block.
#[derive(Default, Debug, Clone, PartialEq, Eq, SimpleSerialize)]
pub struct ExecutionPayload {
    /// Parent execution block hash.
    pub parent_hash: Hash32,
    /// Address that receives priority fees and builder payments.
    pub fee_recipient: ExecutionAddress,
    /// State root of the execution-layer trie after applying this payload.
    pub state_root: Bytes32,
    /// Root of the transaction-receipts trie.
    pub receipts_root: Bytes32,
    /// Bloom filter over event logs emitted by transactions in the payload.
    pub logs_bloom: Vector<u8, BYTES_PER_LOGS_BLOOM>,
    /// RANDAO mix carried from the consensus layer for execution-layer use.
    pub prev_randao: Bytes32,
    /// Execution-layer block height.
    pub block_number: u64,
    /// Maximum gas budget for the block.
    pub gas_limit: u64,
    /// Gas actually consumed by the block's transactions.
    pub gas_used: u64,
    /// Unix timestamp the payload was produced for.
    pub timestamp: u64,
    /// Proposer-supplied freeform bytes (capped to 32).
    pub extra_data: List<u8, MAX_EXTRA_DATA_BYTES>,
    /// Base fee per gas (little-endian 256-bit integer).
    pub base_fee_per_gas: Uint256,
    /// Execution-layer block hash.
    pub block_hash: Hash32,
    /// Opaque transactions, each itself a length-prefixed byte list.
    pub transactions: Transactions,
    /// Withdrawals applied at the execution layer (capped per payload).
    pub withdrawals: List<Withdrawal, MAX_WITHDRAWALS_PER_PAYLOAD>,
    /// Blob gas consumed by this payload.
    pub blob_gas_used: u64,
    /// Excess blob gas used to price the next payload's blobs.
    pub excess_blob_gas: u64,
    /// Block-access list as opaque RLP-encoded bytes.
    pub block_access_list: BlockAccessList,
    /// Slot tag echoed back to the consensus layer.
    pub slot_number: u64,
}

/// Builder's bid for the proposer's slot.
///
/// The proposer commits to the bid by signing the appropriate
/// domain-separated root.
#[derive(Default, Debug, Clone, PartialEq, Eq, SimpleSerialize)]
pub struct ExecutionPayloadBid {
    /// Execution-layer parent block hash the bid is conditioned on.
    pub parent_block_hash: Hash32,
    /// Consensus-layer parent block root the bid is conditioned on.
    pub parent_block_root: Root,
    /// Hash the builder commits to producing as the next execution block.
    pub block_hash: Hash32,
    /// RANDAO value the builder expects the proposer to reveal for the slot.
    pub prev_randao: Bytes32,
    /// Address receiving priority fees and the accepted bid value.
    pub fee_recipient: ExecutionAddress,
    /// Gas budget the builder commits to honoring.
    pub gas_limit: u64,
    /// Builder offering the bid, or `BUILDER_INDEX_SELF_BUILD` for self-builds.
    pub builder_index: BuilderIndex,
    /// Slot the bid is for, which must equal the proposer's slot at inclusion time.
    pub slot: Slot,
    /// Total value (priority fees + tip) the bid promises the proposer.
    pub value: Gwei,
    /// Portion of `value` paid up front to fund execution.
    pub execution_payment: Gwei,
    /// KZG commitments the builder pre-commits to including blobs for.
    pub blob_kzg_commitments: List<KZGCommitment, MAX_BLOB_COMMITMENTS_PER_BLOCK>,
    /// Tree root of the execution-to-consensus requests the builder commits to.
    pub execution_requests_root: Root,
}

/// Builder bid plus the builder's signature over its tree root.
#[derive(Default, Debug, Clone, PartialEq, Eq, SimpleSerialize)]
pub struct SignedExecutionPayloadBid {
    /// The bid being signed.
    pub message: ExecutionPayloadBid,
    /// Builder signature under `DOMAIN_BEACON_BUILDER`.
    pub signature: BLSSignature,
}

/// Delivered payload plus execution-to-consensus requests and provenance roots.
#[derive(Default, Debug, Clone, PartialEq, Eq, SimpleSerialize)]
pub struct ExecutionPayloadEnvelope {
    /// The execution payload delivered for the bid.
    pub payload: ExecutionPayload,
    /// Execution-to-consensus requests carried by the payload.
    pub execution_requests: ExecutionRequests,
    /// Builder that produced the envelope.
    pub builder_index: BuilderIndex,
    /// Root of the beacon block this envelope is bound to.
    pub beacon_block_root: Root,
    /// Root of the parent beacon block.
    pub parent_beacon_block_root: Root,
}

/// Envelope plus the builder's signature.
#[derive(Default, Debug, Clone, PartialEq, Eq, SimpleSerialize)]
pub struct SignedExecutionPayloadEnvelope {
    /// The envelope being signed.
    pub message: ExecutionPayloadEnvelope,
    /// Builder signature under `DOMAIN_BEACON_BUILDER`.
    pub signature: BLSSignature,
}
