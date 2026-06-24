//! Compact trace descriptions for decoded consensus containers.

use moonglass::containers::{
    Attestation, AttestationData, AttesterSlashing, BeaconBlock, BeaconBlockHeader,
    BuilderDepositRequest, BuilderExitRequest, ConsolidationRequest, DepositRequest,
    ExecutionRequests, PayloadAttestation, PayloadAttestationData, PayloadAttestationMessage,
    ProposerSlashing, SignedBLSToExecutionChange, SignedBeaconBlock, SignedExecutionPayloadBid,
    SignedExecutionPayloadEnvelope, SignedVoluntaryExit, SyncAggregate, WithdrawalRequest,
};
use moonglass::primitives::{BLSPubkey, ExecutionAddress, Hash32, Root};

use crate::fixtures::encode_hex;

/// Stable, compact diagnostic data for a decoded fixture input.
pub(crate) trait TraceData {
    fn trace_data(&self) -> String;
}

impl TraceData for SignedVoluntaryExit {
    fn trace_data(&self) -> String {
        format!(
            "validator_index={} epoch={}",
            self.message.validator_index.as_u64(),
            self.message.epoch.as_u64()
        )
    }
}

impl TraceData for SignedBLSToExecutionChange {
    fn trace_data(&self) -> String {
        format!(
            "validator_index={} from_bls_pubkey={} to_execution_address={}",
            self.message.validator_index.as_u64(),
            pubkey_hex(&self.message.from_bls_pubkey),
            address_hex(&self.message.to_execution_address)
        )
    }
}

impl TraceData for Attestation {
    fn trace_data(&self) -> String {
        format!(
            "aggregation_bits={}/{} committee_bits={}/{} {}",
            self.aggregation_bits.count_ones(),
            self.aggregation_bits.len(),
            self.committee_bits.count_ones(),
            self.committee_bits.len(),
            attestation_data(&self.data)
        )
    }
}

impl TraceData for AttesterSlashing {
    fn trace_data(&self) -> String {
        format!(
            "attestation_1={{attesters={} {}}} attestation_2={{attesters={} {}}}",
            self.attestation_1.attesting_indices.len(),
            attestation_data(&self.attestation_1.data),
            self.attestation_2.attesting_indices.len(),
            attestation_data(&self.attestation_2.data)
        )
    }
}

impl TraceData for ProposerSlashing {
    fn trace_data(&self) -> String {
        format!(
            "header_1={{{}}} header_2={{{}}}",
            header_data(&self.signed_header_1.message),
            header_data(&self.signed_header_2.message)
        )
    }
}

impl TraceData for SyncAggregate {
    fn trace_data(&self) -> String {
        format!(
            "sync_committee_bits={}/{}",
            self.sync_committee_bits.count_ones(),
            self.sync_committee_bits.len()
        )
    }
}

impl TraceData for BeaconBlock {
    fn trace_data(&self) -> String {
        block_data(self)
    }
}

impl TraceData for SignedBeaconBlock {
    fn trace_data(&self) -> String {
        block_data(&self.message)
    }
}

impl TraceData for PayloadAttestation {
    fn trace_data(&self) -> String {
        format!(
            "aggregation_bits={}/{} {}",
            self.aggregation_bits.count_ones(),
            self.aggregation_bits.len(),
            payload_attestation_data(&self.data)
        )
    }
}

impl TraceData for PayloadAttestationMessage {
    fn trace_data(&self) -> String {
        format!(
            "validator_index={} {}",
            self.validator_index.as_u64(),
            payload_attestation_data(&self.data)
        )
    }
}

impl TraceData for SignedExecutionPayloadBid {
    fn trace_data(&self) -> String {
        let bid = &self.message;
        format!(
            "slot={} builder={} value={} parent_block_root={} parent_block_hash={} block_hash={} blobs={} requests_root={}",
            bid.slot.as_u64(),
            bid.builder_index.as_u64(),
            bid.value.as_u64(),
            root_hex(&bid.parent_block_root),
            hash32_hex(&bid.parent_block_hash),
            hash32_hex(&bid.block_hash),
            bid.blob_kzg_commitments.len(),
            root_hex(&bid.execution_requests_root)
        )
    }
}

impl TraceData for SignedExecutionPayloadEnvelope {
    fn trace_data(&self) -> String {
        let message = &self.message;
        let payload = &message.payload;
        format!(
            "beacon_block_root={} parent_beacon_block_root={} builder={} payload={{slot_number={} block_number={} timestamp={} parent_hash={} block_hash={} gas_used={} gas_limit={} transactions={} withdrawals={} blob_gas_used={} excess_blob_gas={} access_list_bytes={}}} execution_requests={}",
            root_hex(&message.beacon_block_root),
            root_hex(&message.parent_beacon_block_root),
            message.builder_index.as_u64(),
            payload.slot_number,
            payload.block_number,
            payload.timestamp,
            hash32_hex(&payload.parent_hash),
            hash32_hex(&payload.block_hash),
            payload.gas_used,
            payload.gas_limit,
            payload.transactions.len(),
            payload.withdrawals.len(),
            payload.blob_gas_used,
            payload.excess_blob_gas,
            payload.block_access_list.len(),
            execution_requests_data(&message.execution_requests)
        )
    }
}

impl TraceData for DepositRequest {
    fn trace_data(&self) -> String {
        format!(
            "index={} amount={} pubkey={}",
            self.index,
            self.amount.as_u64(),
            pubkey_hex(&self.pubkey)
        )
    }
}

impl TraceData for BuilderDepositRequest {
    fn trace_data(&self) -> String {
        format!(
            "amount={} pubkey={}",
            self.amount.as_u64(),
            pubkey_hex(&self.pubkey)
        )
    }
}

impl TraceData for BuilderExitRequest {
    fn trace_data(&self) -> String {
        format!(
            "pubkey={} source_address={}",
            pubkey_hex(&self.pubkey),
            address_hex(&self.source_address)
        )
    }
}

impl TraceData for WithdrawalRequest {
    fn trace_data(&self) -> String {
        format!(
            "amount={} validator_pubkey={} source_address={}",
            self.amount.as_u64(),
            pubkey_hex(&self.validator_pubkey),
            address_hex(&self.source_address)
        )
    }
}

impl TraceData for ConsolidationRequest {
    fn trace_data(&self) -> String {
        format!(
            "source_address={} source_pubkey={} target_pubkey={}",
            address_hex(&self.source_address),
            pubkey_hex(&self.source_pubkey),
            pubkey_hex(&self.target_pubkey)
        )
    }
}

pub(crate) fn block_data(block: &BeaconBlock) -> String {
    let body = &block.body;
    let bid = &body.signed_execution_payload_bid.message;
    format!(
        "slot={} proposer={} parent={} state_root={} body={{proposer_slashings={} attester_slashings={} attestations={} deposits={} exits={} bls_changes={} payload_attestations={} parent_requests={}}} bid={{slot={} builder={} value={} parent_block_root={} block_hash={} blobs={} requests_root={}}}",
        block.slot.as_u64(),
        block.proposer_index.as_u64(),
        root_hex(&block.parent_root),
        root_hex(&block.state_root),
        body.proposer_slashings.len(),
        body.attester_slashings.len(),
        body.attestations.len(),
        body.deposits.len(),
        body.voluntary_exits.len(),
        body.bls_to_execution_changes.len(),
        body.payload_attestations.len(),
        execution_requests_data(&body.parent_execution_requests),
        bid.slot.as_u64(),
        bid.builder_index.as_u64(),
        bid.value.as_u64(),
        root_hex(&bid.parent_block_root),
        hash32_hex(&bid.block_hash),
        bid.blob_kzg_commitments.len(),
        root_hex(&bid.execution_requests_root),
    )
}

fn header_data(header: &BeaconBlockHeader) -> String {
    format!(
        "slot={} proposer={} parent={} state_root={} body_root={}",
        header.slot.as_u64(),
        header.proposer_index.as_u64(),
        root_hex(&header.parent_root),
        root_hex(&header.state_root),
        root_hex(&header.body_root)
    )
}

fn attestation_data(data: &AttestationData) -> String {
    format!(
        "slot={} index={} beacon_block_root={} source={}/{} target={}/{}",
        data.slot.as_u64(),
        data.index.as_u64(),
        root_hex(&data.beacon_block_root),
        data.source.epoch.as_u64(),
        root_hex(&data.source.root),
        data.target.epoch.as_u64(),
        root_hex(&data.target.root)
    )
}

fn payload_attestation_data(data: &PayloadAttestationData) -> String {
    format!(
        "slot={} beacon_block_root={} payload_present={} blob_data_available={}",
        data.slot.as_u64(),
        root_hex(&data.beacon_block_root),
        data.payload_present,
        data.blob_data_available
    )
}

fn execution_requests_data(requests: &ExecutionRequests) -> String {
    format!(
        "deposits={} withdrawals={} consolidations={}",
        requests.deposits.len(),
        requests.withdrawals.len(),
        requests.consolidations.len()
    )
}

pub(crate) fn root_hex(root: &Root) -> String {
    format!("0x{}", encode_hex(&root.0))
}

fn hash32_hex(hash: &Hash32) -> String {
    format!("0x{}", encode_hex(&hash.0))
}

fn pubkey_hex(pubkey: &BLSPubkey) -> String {
    format!("0x{}", encode_hex(&pubkey.0))
}

fn address_hex(address: &ExecutionAddress) -> String {
    format!("0x{}", encode_hex(&address.0))
}
