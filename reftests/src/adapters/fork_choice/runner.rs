//! Driver for `fork_choice` reference-test fixtures.

use std::fmt;

use moonglass::containers::{
    Attestation, AttesterSlashing, BeaconBlock, BeaconState, PayloadAttestationMessage,
    SignedBeaconBlock, SignedExecutionPayloadEnvelope,
};
use moonglass::fork_choice::{Store, get_forkchoice_store};

use super::checks::{StepContext, assert_checks};
use super::steps::{Step, parse_steps};
use crate::adapters::{
    Outcome, TraceData, root_hex, trace_block_snapshot, trace_enabled, trace_fail, trace_pass,
    trace_state_snapshot, trace_step_fail, trace_step_info, trace_step_pass, trace_step_pass_item,
};
use crate::fixtures::{CaseFiles, FixtureFile, FixtureStem};
use crate::inventory::Case;

const ANCHOR_STATE: FixtureFile = FixtureFile::new("anchor_state.ssz_snappy");
const ANCHOR_BLOCK: FixtureFile = FixtureFile::new("anchor_block.ssz_snappy");
const STEPS: FixtureFile = FixtureFile::new("steps.yaml");

pub(super) fn run_case(case: &Case) -> Outcome {
    let files = CaseFiles::new(case);
    let anchor_state: BeaconState = match files.decode_ssz_snappy(ANCHOR_STATE) {
        Ok(s) => {
            trace_pass("decode anchor_state", "decoded anchor_state.ssz_snappy");
            trace_state_snapshot("anchor state", &s);
            s
        }
        Err(e) => {
            let detail = format!("decode anchor_state: {e}");
            trace_fail("fork_choice anchor_state", &detail);
            return Outcome::Fail(detail);
        }
    };
    let anchor_block: BeaconBlock = match files.decode_ssz_snappy(ANCHOR_BLOCK) {
        Ok(b) => {
            trace_pass("decode anchor_block", "decoded anchor_block.ssz_snappy");
            trace_block_snapshot("anchor block", &b);
            b
        }
        Err(e) => {
            let detail = format!("decode anchor_block: {e}");
            trace_fail("fork_choice anchor_block", &detail);
            return Outcome::Fail(detail);
        }
    };

    let mut store = match get_forkchoice_store(anchor_state, &anchor_block) {
        Ok(s) => {
            trace_pass("get_forkchoice_store", "initialized store");
            trace_store_snapshot("store", &s);
            s
        }
        Err(e) => {
            let detail = format!("get_forkchoice_store: {e}");
            trace_fail("fork_choice store", &detail);
            return Outcome::Fail(detail);
        }
    };

    let steps = match parse_steps(&files.path(STEPS)) {
        Ok(s) => s,
        Err(e) => {
            let detail = format!("parse steps.yaml: {e:#}");
            trace_fail("fork_choice steps", &detail);
            return Outcome::Fail(detail);
        }
    };

    for (idx, step) in steps.into_iter().enumerate() {
        let tag = step.tag();
        if trace_enabled() {
            trace_step_info(idx, tag, step_detail(&step));
        }
        match drive_step(&mut store, files, step, idx, tag) {
            Ok(Some(note)) => {
                if trace_enabled() {
                    trace_step_pass(idx, tag, note);
                    trace_step_pass_item(idx, tag, "store", store_data(&store));
                }
            }
            Ok(None) => {
                if trace_enabled() {
                    trace_step_pass(idx, tag, "completed");
                    trace_step_pass_item(idx, tag, "store", store_data(&store));
                }
            }
            Err(err) => {
                let msg = err.to_string();
                trace_step_fail(idx, tag, &msg);
                return Outcome::Fail(format!("step {idx} [{tag}]: {msg}"));
            }
        }
        // Every accepted step must leave the store structurally well formed.
        if let Err(violation) = store.check_invariants() {
            return Outcome::Fail(format!(
                "step {idx} [{tag}] left store invariant broken: {violation}"
            ));
        }
    }
    Outcome::Pass
}

fn step_detail(step: &Step) -> String {
    match step {
        Step::Tick(s) => format!("tick={} valid={}", s.tick, s.valid),
        Step::Block(s) => format!("block={} valid={}", s.block, s.valid),
        Step::Attestation(s) => format!("attestation={} valid={}", s.attestation, s.valid),
        Step::AttesterSlashing(s) => {
            format!(
                "attester_slashing={} valid={}",
                s.attester_slashing, s.valid
            )
        }
        Step::PayloadEnvelope(s) => {
            format!(
                "execution_payload={} valid={}",
                s.execution_payload, s.valid
            )
        }
        Step::PayloadAttestation(s) => format!(
            "payload_attestation_message={} valid={}",
            s.payload_attestation_message, s.valid
        ),
        Step::Checks(s) => format!("checks={}", s.checks.labels().join(",")),
        Step::Other(value) => format!("unknown step kind: {}", describe_step(value)),
    }
}

// `Ok(None)` means the step did its job with nothing to report. `Ok(Some(msg))`
// means the step passed because something was correctly rejected and `msg`
// records why. `Err(msg)` is a case failure.
fn drive_step(
    store: &mut Store,
    files: CaseFiles<'_>,
    step: Step,
    index: usize,
    tag: &str,
) -> Result<Option<String>, StepFailure> {
    match step {
        Step::Tick(s) => expect_step_result(store.on_tick(s.tick), s.valid, "tick"),
        Step::Block(s) => apply_block(store, files, &s.block, s.valid, index, tag),
        Step::Attestation(s) => apply::<Attestation, _>(
            store,
            files,
            &s.attestation,
            s.valid,
            index,
            tag,
            |store, att| store.on_attestation(att, false),
        ),
        Step::AttesterSlashing(s) => apply::<AttesterSlashing, _>(
            store,
            files,
            &s.attester_slashing,
            s.valid,
            index,
            tag,
            Store::on_attester_slashing,
        ),
        Step::PayloadEnvelope(s) => apply::<SignedExecutionPayloadEnvelope, _>(
            store,
            files,
            &s.execution_payload,
            s.valid,
            index,
            tag,
            Store::on_execution_payload_envelope,
        ),
        Step::PayloadAttestation(s) => apply::<PayloadAttestationMessage, _>(
            store,
            files,
            &s.payload_attestation_message,
            s.valid,
            index,
            tag,
            |store, msg| store.on_payload_attestation_message(msg, false),
        ),
        Step::Checks(s) => assert_checks(store, &s.checks, StepContext::new(index, tag))
            .map(|()| None)
            .map_err(StepFailure::Check),
        Step::Other(value) => Err(StepFailure::UnknownStep(describe_step(&value))),
    }
}

fn expect_step_result(
    result: Result<(), moonglass::error::ForkChoiceError>,
    expect_valid: bool,
    label: &str,
) -> Result<Option<String>, StepFailure> {
    match (result, expect_valid) {
        (Ok(()), true) => Ok(None),
        (Err(e), false) => Ok(Some(format!("rejected as expected: {e}"))),
        (Ok(()), false) => Err(StepFailure::UnexpectedSuccess {
            label: label.to_owned(),
        }),
        (Err(e), true) => Err(StepFailure::UnexpectedFailure {
            label: label.to_owned(),
            source: e.to_string(),
        }),
    }
}

#[derive(Debug)]
enum StepFailure {
    Decode { fixture: String, source: String },
    Check(String),
    UnknownStep(String),
    UnexpectedSuccess { label: String },
    UnexpectedFailure { label: String, source: String },
}

impl StepFailure {
    fn decode(fixture: &FixtureStem, source: impl fmt::Display) -> Self {
        Self::Decode {
            fixture: fixture.to_string(),
            source: source.to_string(),
        }
    }
}

impl fmt::Display for StepFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Decode { fixture, source } => write!(f, "decode {fixture}: {source}"),
            Self::Check(detail) | Self::UnknownStep(detail) => f.write_str(detail),
            Self::UnexpectedSuccess { label } => {
                write!(f, "expected invalid, {label} returned Ok")
            }
            Self::UnexpectedFailure { label, source } => {
                write!(f, "expected valid, {label} returned: {source}")
            }
        }
    }
}

fn describe_step(value: &serde_yaml::Value) -> String {
    match value {
        serde_yaml::Value::Mapping(map) => {
            let mut keys: Vec<String> = map
                .keys()
                .map(|k| match k {
                    serde_yaml::Value::String(s) => s.clone(),
                    other => format!("{other:?}"),
                })
                .collect();
            keys.sort();
            if keys.is_empty() {
                "<empty mapping>".to_owned()
            } else {
                keys.join(", ")
            }
        }
        other => format!("{other:?}"),
    }
}

fn trace_store_snapshot(label: &str, store: &Store) {
    if trace_enabled() {
        trace_pass(label, store_data(store));
    }
}

fn store_data(store: &Store) -> String {
    format!(
        "time={} blocks={} states={} latest_messages={} payloads={} timeliness_votes={} data_votes={} justified={}/{} finalized={}/{} proposer_boost_root={}",
        store.time,
        store.blocks.len(),
        store.block_states.len(),
        store.latest_messages.len(),
        store.payloads.len(),
        store.payload_timeliness_vote.len(),
        store.payload_data_availability_vote.len(),
        store.justified_checkpoint.epoch.as_u64(),
        root_hex(&store.justified_checkpoint.root),
        store.finalized_checkpoint.epoch.as_u64(),
        root_hex(&store.finalized_checkpoint.root),
        root_hex(&store.proposer_boost_root),
    )
}

fn apply_block(
    store: &mut Store,
    files: CaseFiles<'_>,
    file_stem: &FixtureStem,
    expect_valid: bool,
    index: usize,
    tag: &str,
) -> Result<Option<String>, StepFailure> {
    let signed: SignedBeaconBlock = files
        .decode_ssz_snappy_stem(file_stem)
        .map_err(|e| StepFailure::decode(file_stem, e))?;
    if trace_enabled() {
        trace_step_pass_item(
            index,
            tag,
            "decode",
            format_args!("decoded {file_stem}.ssz_snappy"),
        );
        trace_step_pass_item(index, tag, "input", signed.trace_data());
    }
    expect_step_result(
        store.on_block_with_embedded_messages(&signed),
        expect_valid,
        file_stem.as_str(),
    )
}

fn apply<T, F>(
    store: &mut Store,
    files: CaseFiles<'_>,
    file_stem: &FixtureStem,
    expect_valid: bool,
    index: usize,
    tag: &str,
    apply_fn: F,
) -> Result<Option<String>, StepFailure>
where
    T: ssz_rs::Deserialize + TraceData,
    F: FnOnce(&mut Store, &T) -> Result<(), moonglass::error::ForkChoiceError>,
{
    let value: T = files
        .decode_ssz_snappy_stem(file_stem)
        .map_err(|e| StepFailure::decode(file_stem, e))?;
    if trace_enabled() {
        trace_step_pass_item(
            index,
            tag,
            "decode",
            format_args!("decoded {file_stem}.ssz_snappy"),
        );
        trace_step_pass_item(index, tag, "input", value.trace_data());
    }
    let result = apply_fn(store, &value);
    expect_step_result(result, expect_valid, file_stem.as_str())
}
