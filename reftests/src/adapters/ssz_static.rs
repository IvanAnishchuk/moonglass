//! Adapter for `ssz_static` reference-test fixtures.
//!
//! Each supported container is decoded from `serialized.ssz_snappy`, serialized
//! back to bytes, and merkleized. A case passes only when the bytes round-trip
//! exactly and the computed root matches `roots.yaml`.

use std::path::{Path, PathBuf};

use serde::Deserialize;
use thiserror::Error;

use moonglass::containers::{
    Attestation, AttestationData, AttesterSlashing, BLSToExecutionChange, BeaconBlock,
    BeaconBlockBody, BeaconBlockHeader, BeaconState, Builder, BuilderDepositRequest,
    BuilderExitRequest, BuilderPendingPayment, BuilderPendingWithdrawal, Checkpoint,
    ConsolidationRequest, Deposit, DepositData, DepositRequest, Eth1Data, ExecutionPayload,
    ExecutionPayloadBid, ExecutionPayloadEnvelope, ExecutionRequests, Fork, ForkData,
    HistoricalSummary, IndexedAttestation, IndexedPayloadAttestation, PayloadAttestation,
    PayloadAttestationData, PayloadAttestationMessage, PendingConsolidation, PendingDeposit,
    PendingPartialWithdrawal, ProposerSlashing, SignedBLSToExecutionChange, SignedBeaconBlock,
    SignedBeaconBlockHeader, SignedExecutionPayloadBid, SignedExecutionPayloadEnvelope,
    SignedVoluntaryExit, SigningData, SingleAttestation, SyncAggregate, SyncCommittee, Validator,
    VoluntaryExit, Withdrawal, WithdrawalRequest,
};
use moonglass::primitives::Root;

use crate::adapters::{Adapter, CaseRunner, Outcome, SupportedHandler, trace_fail, trace_pass};
use crate::error::{FixtureError, HexError};
use crate::fixtures::{CaseFiles, FixtureFile, decode_fixed_hex, encode_hex, read_yaml_path};
use crate::inventory::{Case, Runner};

const SERIALIZED: FixtureFile = FixtureFile::new("serialized.ssz_snappy");
const ROOTS: FixtureFile = FixtureFile::new("roots.yaml");

pub(super) static ADAPTER: Adapter<SszStatic> = Adapter::new();

pub(super) struct SszStatic;

impl CaseRunner for SszStatic {
    type Handler = StaticContainer;

    const RUNNER: Runner = Runner::SszStatic;

    fn run(case: &Case, handler: Self::Handler) -> Outcome {
        run(case, handler)
    }
}

/// `ssz_static` sidecar parsing result.
type Result<T> = std::result::Result<T, StaticError>;

/// Error returned while reading `ssz_static` sidecar fixtures.
#[derive(Debug, Error)]
enum StaticError {
    /// Reading or parsing a fixture file failed.
    #[error(transparent)]
    Fixture(#[from] FixtureError),
    /// Hex decoding of the expected root failed.
    #[error("decode root in {path:?}: {source}")]
    Hex {
        /// File being parsed.
        path: PathBuf,
        /// Underlying hex error.
        #[source]
        source: HexError,
    },
}

#[derive(Clone, Copy)]
pub(super) struct StaticContainer {
    name: &'static str,
    run: fn(&[u8], &[u8; 32], &'static str) -> Outcome,
}

impl StaticContainer {
    const fn new(name: &'static str, run: fn(&[u8], &[u8; 32], &'static str) -> Outcome) -> Self {
        Self { name, run }
    }

    fn run(self, bytes: &[u8], expected: &[u8; 32]) -> Outcome {
        (self.run)(bytes, expected, self.name)
    }
}

impl SupportedHandler for StaticContainer {
    const ALL: &'static [Self] = &[
        Self::new("Attestation", run_one::<Attestation>),
        Self::new("AttestationData", run_one::<AttestationData>),
        Self::new("AttesterSlashing", run_one::<AttesterSlashing>),
        Self::new("BeaconBlock", run_one::<BeaconBlock>),
        Self::new("BeaconBlockBody", run_one::<BeaconBlockBody>),
        Self::new("BeaconBlockHeader", run_one::<BeaconBlockHeader>),
        Self::new("BeaconState", run_one::<BeaconState>),
        Self::new("BLSToExecutionChange", run_one::<BLSToExecutionChange>),
        Self::new("Builder", run_one::<Builder>),
        Self::new("BuilderDepositRequest", run_one::<BuilderDepositRequest>),
        Self::new("BuilderExitRequest", run_one::<BuilderExitRequest>),
        Self::new("BuilderPendingPayment", run_one::<BuilderPendingPayment>),
        Self::new(
            "BuilderPendingWithdrawal",
            run_one::<BuilderPendingWithdrawal>,
        ),
        Self::new("Checkpoint", run_one::<Checkpoint>),
        Self::new("ConsolidationRequest", run_one::<ConsolidationRequest>),
        Self::new("Deposit", run_one::<Deposit>),
        Self::new("DepositData", run_one::<DepositData>),
        Self::new("DepositRequest", run_one::<DepositRequest>),
        Self::new("Eth1Data", run_one::<Eth1Data>),
        Self::new("ExecutionPayload", run_one::<ExecutionPayload>),
        Self::new("ExecutionPayloadBid", run_one::<ExecutionPayloadBid>),
        Self::new(
            "ExecutionPayloadEnvelope",
            run_one::<ExecutionPayloadEnvelope>,
        ),
        Self::new("ExecutionRequests", run_one::<ExecutionRequests>),
        Self::new("Fork", run_one::<Fork>),
        Self::new("ForkData", run_one::<ForkData>),
        Self::new("HistoricalSummary", run_one::<HistoricalSummary>),
        Self::new("IndexedAttestation", run_one::<IndexedAttestation>),
        Self::new(
            "IndexedPayloadAttestation",
            run_one::<IndexedPayloadAttestation>,
        ),
        Self::new("PayloadAttestation", run_one::<PayloadAttestation>),
        Self::new("PayloadAttestationData", run_one::<PayloadAttestationData>),
        Self::new(
            "PayloadAttestationMessage",
            run_one::<PayloadAttestationMessage>,
        ),
        Self::new("PendingConsolidation", run_one::<PendingConsolidation>),
        Self::new("PendingDeposit", run_one::<PendingDeposit>),
        Self::new(
            "PendingPartialWithdrawal",
            run_one::<PendingPartialWithdrawal>,
        ),
        Self::new("ProposerSlashing", run_one::<ProposerSlashing>),
        Self::new("SignedBeaconBlock", run_one::<SignedBeaconBlock>),
        Self::new(
            "SignedBeaconBlockHeader",
            run_one::<SignedBeaconBlockHeader>,
        ),
        Self::new(
            "SignedBLSToExecutionChange",
            run_one::<SignedBLSToExecutionChange>,
        ),
        Self::new(
            "SignedExecutionPayloadBid",
            run_one::<SignedExecutionPayloadBid>,
        ),
        Self::new(
            "SignedExecutionPayloadEnvelope",
            run_one::<SignedExecutionPayloadEnvelope>,
        ),
        Self::new("SignedVoluntaryExit", run_one::<SignedVoluntaryExit>),
        Self::new("SigningData", run_one::<SigningData>),
        Self::new("SingleAttestation", run_one::<SingleAttestation>),
        Self::new("SyncAggregate", run_one::<SyncAggregate>),
        Self::new("SyncCommittee", run_one::<SyncCommittee>),
        Self::new("Validator", run_one::<Validator>),
        Self::new("VoluntaryExit", run_one::<VoluntaryExit>),
        Self::new("Withdrawal", run_one::<Withdrawal>),
        Self::new("WithdrawalRequest", run_one::<WithdrawalRequest>),
    ];

    fn as_str(self) -> &'static str {
        self.name
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Roots {
    root: String,
}

#[must_use]
fn run(case: &Case, container: StaticContainer) -> Outcome {
    let files = CaseFiles::new(case);
    let roots_path = files.path(ROOTS);
    let expected = match read_expected_root(&roots_path) {
        Ok(e) => {
            trace_pass("ssz_static roots", "read expected root");
            e
        }
        Err(e) => {
            let detail = format!("read roots.yaml: {e:#}");
            trace_fail("ssz_static roots", &detail);
            return Outcome::Fail(detail);
        }
    };
    let bytes = match files.read_snappy(SERIALIZED) {
        Ok(b) => {
            trace_pass(
                "ssz_static serialized",
                format_args!("decoded {} bytes", b.len()),
            );
            b
        }
        Err(e) => {
            let detail = format!("snappy decode: {e:#}");
            trace_fail("ssz_static serialized", &detail);
            return Outcome::Fail(detail);
        }
    };
    container.run(&bytes, &expected)
}

fn run_one<T>(bytes: &[u8], expected: &[u8; 32], container: &'static str) -> Outcome
where
    T: ssz_rs::Deserialize + ssz_rs::Serialize + ssz_rs::Merkleized,
{
    let mut value = match T::deserialize(bytes) {
        Ok(v) => {
            trace_pass(format_args!("ssz decode {container}"), "decoded value");
            v
        }
        Err(e) => {
            let detail = format!("ssz decode: {e}");
            trace_fail(format_args!("ssz decode {container}"), &detail);
            return Outcome::Fail(detail);
        }
    };
    let mut reencoded = Vec::with_capacity(bytes.len());
    if let Err(e) = ssz_rs::Serialize::serialize(&value, &mut reencoded) {
        let detail = format!("ssz re-encode: {e}");
        trace_fail(format_args!("ssz re-encode {container}"), &detail);
        return Outcome::Fail(detail);
    }
    if reencoded != bytes {
        let detail = format!(
            "ssz re-encode mismatch: got {} bytes, want {} bytes",
            reencoded.len(),
            bytes.len()
        );
        trace_fail(format_args!("ssz re-encode {container}"), &detail);
        return Outcome::Fail(detail);
    }
    trace_pass(
        format_args!("ssz re-encode {container}"),
        format_args!("{} bytes", reencoded.len()),
    );
    let node = match ssz_rs::Merkleized::hash_tree_root(&mut value) {
        Ok(r) => {
            trace_pass(format_args!("hash_tree_root {container}"), "computed root");
            r
        }
        Err(e) => {
            let detail = format!("hash_tree_root: {e}");
            trace_fail(format_args!("hash_tree_root {container}"), &detail);
            return Outcome::Fail(detail);
        }
    };
    let got = Root::from(node).0;
    if got == *expected {
        trace_pass("ssz_static root", "root matches roots.yaml");
        Outcome::Pass
    } else {
        let detail = format!(
            "root mismatch: got 0x{}, want 0x{}",
            encode_hex(&got),
            encode_hex(expected)
        );
        trace_fail("ssz_static root", &detail);
        Outcome::Fail(detail)
    }
}

fn read_expected_root(path: &Path) -> Result<[u8; 32]> {
    let roots: Roots = read_yaml_path(path)?;
    decode_fixed_hex(&roots.root).map_err(|source| StaticError::Hex {
        path: path.to_path_buf(),
        source,
    })
}
