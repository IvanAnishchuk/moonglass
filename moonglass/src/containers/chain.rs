//! Chain metadata containers.
//!
//! Covers signing-version records, Casper finality checkpoints, historical
//! summaries, deposit-vote data, and signing-domain data. `Eth1Data` keeps the
//! historical spec name for the execution-layer deposit-chain vote.

use ssz_rs::prelude::*;

use crate::primitives::{Domain, Epoch, Hash32, Root, Version};

/// Tracks the active signing version and the version it succeeded.
#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, SimpleSerialize)]
pub struct Fork {
    /// Signing version active before `epoch`.
    pub previous_version: Version,
    /// Signing version active from `epoch` onward.
    pub current_version: Version,
    /// Epoch at which the current version became active.
    pub epoch: Epoch,
}

/// Domain-separation tuple fed into signing-domain construction.
#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, SimpleSerialize)]
pub struct ForkData {
    /// Signing version used for domain separation.
    pub current_version: Version,
    /// Root of the validator set at genesis, tying the signature to this chain.
    pub genesis_validators_root: Root,
}

/// Casper finality checkpoint: the `(epoch, block_root)` pair finality refers to.
#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, Hash, SimpleSerialize)]
pub struct Checkpoint {
    /// Epoch the checkpoint is at.
    pub epoch: Epoch,
    /// Block root at the start of `epoch`.
    pub root: Root,
}

/// Per-historical-period roots stored in `BeaconState.historical_summaries`.
#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, SimpleSerialize)]
pub struct HistoricalSummary {
    /// Merkle root of the block-roots ring buffer for the period.
    pub block_summary_root: Root,
    /// Merkle root of the state-roots ring buffer for the period.
    pub state_summary_root: Root,
}

/// Deposit-voting data observed in a block and aggregated across the voting period.
#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, SimpleSerialize)]
pub struct Eth1Data {
    /// Root of the deposit-contract Merkle tree at the voted deposit-chain block.
    pub deposit_root: Root,
    /// Total deposits observed up to and including the voted deposit-chain block.
    pub deposit_count: u64,
    /// Hash of the voted deposit-chain block.
    pub block_hash: Hash32,
}

/// Helper tuple whose tree root is the BLS signing message.
///
/// This is not a network message. It exists so signatures bind an object root
/// to the domain that identifies its message kind and signing context.
#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, SimpleSerialize)]
pub struct SigningData {
    /// Tree root of the object being signed.
    pub object_root: Root,
    /// Signing domain combining the domain type and signing-version root.
    pub domain: Domain,
}
