//! Adapter for `ssz_static` reference-test fixtures (SSZ round-trip and merkleization).

use std::path::Path;

use serde::Deserialize;

use moonglass::containers::{
    Attestation, AttestationData, AttesterSlashing, BLSToExecutionChange, BeaconBlock,
    BeaconBlockBody, BeaconBlockHeader, BeaconState, Builder, BuilderPendingPayment,
    BuilderPendingWithdrawal, Checkpoint, ConsolidationRequest, Deposit, DepositData,
    DepositRequest, Eth1Data, ExecutionPayload, ExecutionPayloadBid, ExecutionPayloadEnvelope,
    ExecutionRequests, Fork, ForkData, HistoricalSummary, IndexedAttestation,
    IndexedPayloadAttestation, PayloadAttestation, PayloadAttestationData,
    PayloadAttestationMessage, PendingConsolidation, PendingDeposit, PendingPartialWithdrawal,
    ProposerSlashing, SignedBLSToExecutionChange, SignedBeaconBlock, SignedBeaconBlockHeader,
    SignedExecutionPayloadBid, SignedExecutionPayloadEnvelope, SignedVoluntaryExit, SigningData,
    SingleAttestation, SyncAggregate, SyncCommittee, Validator, VoluntaryExit, Withdrawal,
    WithdrawalRequest,
};
use moonglass::primitives::Root;

use crate::adapters::Outcome;
use crate::discover::Case;
use crate::fixture;
use crate::hex;

const SERIALIZED_FILENAME: &str = "serialized.ssz_snappy";
const ROOTS_FILENAME: &str = "roots.yaml";

type StaticRunner = fn(&[u8], &[u8; 32]) -> Outcome;

struct Container {
    name: &'static str,
    run: StaticRunner,
}

const CONTAINERS: &[Container] = &[
    Container {
        name: "Attestation",
        run: run_one::<Attestation>,
    },
    Container {
        name: "AttestationData",
        run: run_one::<AttestationData>,
    },
    Container {
        name: "AttesterSlashing",
        run: run_one::<AttesterSlashing>,
    },
    Container {
        name: "BeaconBlock",
        run: run_one::<BeaconBlock>,
    },
    Container {
        name: "BeaconBlockBody",
        run: run_one::<BeaconBlockBody>,
    },
    Container {
        name: "BeaconBlockHeader",
        run: run_one::<BeaconBlockHeader>,
    },
    Container {
        name: "BeaconState",
        run: run_one::<BeaconState>,
    },
    Container {
        name: "BLSToExecutionChange",
        run: run_one::<BLSToExecutionChange>,
    },
    Container {
        name: "Builder",
        run: run_one::<Builder>,
    },
    Container {
        name: "BuilderPendingPayment",
        run: run_one::<BuilderPendingPayment>,
    },
    Container {
        name: "BuilderPendingWithdrawal",
        run: run_one::<BuilderPendingWithdrawal>,
    },
    Container {
        name: "Checkpoint",
        run: run_one::<Checkpoint>,
    },
    Container {
        name: "ConsolidationRequest",
        run: run_one::<ConsolidationRequest>,
    },
    Container {
        name: "Deposit",
        run: run_one::<Deposit>,
    },
    Container {
        name: "DepositData",
        run: run_one::<DepositData>,
    },
    Container {
        name: "DepositRequest",
        run: run_one::<DepositRequest>,
    },
    Container {
        name: "Eth1Data",
        run: run_one::<Eth1Data>,
    },
    Container {
        name: "ExecutionPayload",
        run: run_one::<ExecutionPayload>,
    },
    Container {
        name: "ExecutionPayloadBid",
        run: run_one::<ExecutionPayloadBid>,
    },
    Container {
        name: "ExecutionPayloadEnvelope",
        run: run_one::<ExecutionPayloadEnvelope>,
    },
    Container {
        name: "ExecutionRequests",
        run: run_one::<ExecutionRequests>,
    },
    Container {
        name: "Fork",
        run: run_one::<Fork>,
    },
    Container {
        name: "ForkData",
        run: run_one::<ForkData>,
    },
    Container {
        name: "HistoricalSummary",
        run: run_one::<HistoricalSummary>,
    },
    Container {
        name: "IndexedAttestation",
        run: run_one::<IndexedAttestation>,
    },
    Container {
        name: "IndexedPayloadAttestation",
        run: run_one::<IndexedPayloadAttestation>,
    },
    Container {
        name: "PayloadAttestation",
        run: run_one::<PayloadAttestation>,
    },
    Container {
        name: "PayloadAttestationData",
        run: run_one::<PayloadAttestationData>,
    },
    Container {
        name: "PayloadAttestationMessage",
        run: run_one::<PayloadAttestationMessage>,
    },
    Container {
        name: "PendingConsolidation",
        run: run_one::<PendingConsolidation>,
    },
    Container {
        name: "PendingDeposit",
        run: run_one::<PendingDeposit>,
    },
    Container {
        name: "PendingPartialWithdrawal",
        run: run_one::<PendingPartialWithdrawal>,
    },
    Container {
        name: "ProposerSlashing",
        run: run_one::<ProposerSlashing>,
    },
    Container {
        name: "SignedBeaconBlock",
        run: run_one::<SignedBeaconBlock>,
    },
    Container {
        name: "SignedBeaconBlockHeader",
        run: run_one::<SignedBeaconBlockHeader>,
    },
    Container {
        name: "SignedBLSToExecutionChange",
        run: run_one::<SignedBLSToExecutionChange>,
    },
    Container {
        name: "SignedExecutionPayloadBid",
        run: run_one::<SignedExecutionPayloadBid>,
    },
    Container {
        name: "SignedExecutionPayloadEnvelope",
        run: run_one::<SignedExecutionPayloadEnvelope>,
    },
    Container {
        name: "SignedVoluntaryExit",
        run: run_one::<SignedVoluntaryExit>,
    },
    Container {
        name: "SigningData",
        run: run_one::<SigningData>,
    },
    Container {
        name: "SingleAttestation",
        run: run_one::<SingleAttestation>,
    },
    Container {
        name: "SyncAggregate",
        run: run_one::<SyncAggregate>,
    },
    Container {
        name: "SyncCommittee",
        run: run_one::<SyncCommittee>,
    },
    Container {
        name: "Validator",
        run: run_one::<Validator>,
    },
    Container {
        name: "VoluntaryExit",
        run: run_one::<VoluntaryExit>,
    },
    Container {
        name: "Withdrawal",
        run: run_one::<Withdrawal>,
    },
    Container {
        name: "WithdrawalRequest",
        run: run_one::<WithdrawalRequest>,
    },
];

#[derive(Debug, Deserialize)]
struct Roots {
    root: String,
}

#[must_use]
pub(super) fn run(case: &Case) -> Outcome {
    let serialized = case.root.join(SERIALIZED_FILENAME);
    let roots_path = case.root.join(ROOTS_FILENAME);
    if !serialized.exists() || !roots_path.exists() {
        return Outcome::Fail(format!("missing {SERIALIZED_FILENAME} or {ROOTS_FILENAME}"));
    }
    let expected = match read_expected_root(&roots_path) {
        Ok(e) => e,
        Err(e) => return Outcome::Fail(format!("read roots.yaml: {e:#}")),
    };
    let bytes = match fixture::read_snappy_file(&serialized) {
        Ok(b) => b,
        Err(e) => return Outcome::Fail(format!("snappy decode: {e:#}")),
    };
    dispatch(case, &bytes, &expected)
}

#[must_use]
pub(super) fn supports(handler: &str) -> bool {
    container(handler).is_some()
}

fn dispatch(case: &Case, bytes: &[u8], expected: &[u8; 32]) -> Outcome {
    if let Some(container) = container(case.handler.as_str()) {
        return (container.run)(bytes, expected);
    }

    Outcome::Fail(format!(
        "ssz_static container '{}' not wired in moonglass",
        case.handler
    ))
}

fn container(name: &str) -> Option<&'static Container> {
    CONTAINERS.iter().find(|container| container.name == name)
}

fn run_one<T>(bytes: &[u8], expected: &[u8; 32]) -> Outcome
where
    T: ssz_rs::Deserialize + ssz_rs::Serialize + ssz_rs::Merkleized,
{
    let mut value = match T::deserialize(bytes) {
        Ok(v) => v,
        Err(e) => return Outcome::Fail(format!("ssz decode: {e}")),
    };
    let mut reencoded = Vec::with_capacity(bytes.len());
    if let Err(e) = ssz_rs::Serialize::serialize(&value, &mut reencoded) {
        return Outcome::Fail(format!("ssz re-encode: {e}"));
    }
    if reencoded != bytes {
        return Outcome::Fail(format!(
            "ssz re-encode mismatch: got {} bytes, want {} bytes",
            reencoded.len(),
            bytes.len(),
        ));
    }
    let node = match ssz_rs::Merkleized::hash_tree_root(&mut value) {
        Ok(r) => r,
        Err(e) => return Outcome::Fail(format!("hash_tree_root: {e}")),
    };
    let got = Root::from(node).0;
    if got == *expected {
        Outcome::Pass
    } else {
        Outcome::Fail(format!(
            "root mismatch: got 0x{}, want 0x{}",
            hex::encode(&got),
            hex::encode(expected)
        ))
    }
}

fn read_expected_root(path: &Path) -> anyhow::Result<[u8; 32]> {
    let text = std::fs::read_to_string(path)?;
    let roots: Roots = serde_yaml::from_str(&text)?;
    hex::decode_prefixed_fixed(&roots.root)
}
