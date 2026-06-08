//! Driver for `fork_choice` reference-test fixtures.

use std::path::Path;

use moonglass::containers::{
    Attestation, AttesterSlashing, BeaconBlock, BeaconState, Checkpoint, PayloadAttestationMessage,
    SignedBeaconBlock, SignedExecutionPayloadEnvelope,
};
use moonglass::fork_choice::{
    ForkChoiceNode, PayloadStatus, Store, get_forkchoice_store, get_head, on_attestation,
    on_attester_slashing, on_block, on_execution_payload_envelope, on_payload_attestation_message,
    on_tick,
};
use moonglass::primitives::{Epoch, Root};

use super::steps::{CheckpointHex, Checks, HeadCheck, Step, parse_steps};
use crate::adapters::Outcome;
use crate::discover::Case;
use crate::fixture::decode_ssz_snappy;
use crate::hex::decode_prefixed_fixed;

pub(super) fn run_case(case: &Case) -> Outcome {
    let anchor_state: BeaconState =
        match decode_ssz_snappy(&case.root.join("anchor_state.ssz_snappy")) {
            Ok(s) => s,
            Err(e) => return Outcome::Fail(format!("decode anchor_state: {e:#}")),
        };
    let anchor_block: BeaconBlock =
        match decode_ssz_snappy(&case.root.join("anchor_block.ssz_snappy")) {
            Ok(b) => b,
            Err(e) => return Outcome::Fail(format!("decode anchor_block: {e:#}")),
        };

    let mut store = match get_forkchoice_store(anchor_state, &anchor_block) {
        Ok(s) => s,
        Err(e) => return Outcome::Fail(format!("get_forkchoice_store: {e}")),
    };

    let steps_path = case.root.join("steps.yaml");
    let steps = match parse_steps(&steps_path) {
        Ok(s) => s,
        Err(e) => return Outcome::Fail(format!("parse steps.yaml: {e:#}")),
    };

    for (idx, step) in steps.into_iter().enumerate() {
        let tag = step_tag(&step);
        if let Err(msg) = drive_step(&mut store, &case.root, step) {
            return Outcome::Fail(format!("step {idx} [{tag}]: {msg}"));
        }
    }
    Outcome::Pass
}

fn step_tag(step: &Step) -> &'static str {
    match step {
        Step::Tick(_) => "Tick",
        Step::Block(_) => "Block",
        Step::Attestation(_) => "Attestation",
        Step::AttesterSlashing(_) => "AttesterSlashing",
        Step::PayloadEnvelope(_) => "PayloadEnvelope",
        Step::PayloadAttestation(_) => "PayloadAttestation",
        Step::Checks(_) => "Checks",
        Step::Other(_) => "Other",
    }
}

fn drive_step(store: &mut Store, root: &Path, step: Step) -> Result<(), String> {
    match step {
        Step::Tick(s) => on_tick(store, s.tick).map_err(|e| format!("on_tick({}): {e}", s.tick)),
        Step::Block(s) => apply_block(store, root, &s.block, s.valid),
        Step::Attestation(s) => {
            apply::<Attestation, _>(store, root, &s.attestation, s.valid, |store, att| {
                on_attestation(store, att, false)
            })
        }
        Step::AttesterSlashing(s) => apply::<AttesterSlashing, _>(
            store,
            root,
            &s.attester_slashing,
            s.valid,
            on_attester_slashing,
        ),
        Step::PayloadEnvelope(s) => apply::<SignedExecutionPayloadEnvelope, _>(
            store,
            root,
            &s.execution_payload,
            s.valid,
            on_execution_payload_envelope,
        ),
        Step::PayloadAttestation(s) => apply::<PayloadAttestationMessage, _>(
            store,
            root,
            &s.payload_attestation_message,
            s.valid,
            |store, msg| on_payload_attestation_message(store, msg, false),
        ),
        Step::Checks(s) => assert_checks(store, &s.checks),
        Step::Other(value) => Err(format!("unknown step kind: {}", describe_step(&value))),
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

// After a successful `on_block`, the reference-test helper replays the
// block's embedded attestations and attester slashings against the store with
// `is_from_block=true`. Moonglass's `on_block` deliberately does not replay
// these (matching the spec), so the harness replays them here.
fn apply_block(
    store: &mut Store,
    case_root: &Path,
    file_stem: &str,
    expect_valid: bool,
) -> Result<(), String> {
    let path = case_root.join(format!("{file_stem}.ssz_snappy"));
    let signed: SignedBeaconBlock =
        decode_ssz_snappy(&path).map_err(|e| format!("decode {file_stem}: {e:#}"))?;
    let result = on_block(store, &signed);
    match (result, expect_valid) {
        (Ok(()), true) => {
            for attestation in signed.message.body.attestations.iter() {
                if let Err(e) = on_attestation(store, attestation, true) {
                    return Err(format!(
                        "block-embedded attestation failed on_attestation: {e}"
                    ));
                }
            }
            for slashing in signed.message.body.attester_slashings.iter() {
                if let Err(e) = on_attester_slashing(store, slashing) {
                    return Err(format!(
                        "block-embedded attester_slashing failed on_attester_slashing: {e}"
                    ));
                }
            }
            Ok(())
        }
        (Err(_), false) => Ok(()),
        (Ok(()), false) => Err(format!("expected invalid, {file_stem} returned Ok")),
        (Err(e), true) => Err(format!("expected valid, {file_stem} returned: {e}")),
    }
}

fn apply<T, F>(
    store: &mut Store,
    case_root: &Path,
    file_stem: &str,
    expect_valid: bool,
    apply_fn: F,
) -> Result<(), String>
where
    T: ssz_rs::Deserialize,
    F: FnOnce(&mut Store, &T) -> Result<(), moonglass::error::ForkChoiceError>,
{
    let path = case_root.join(format!("{file_stem}.ssz_snappy"));
    let value: T = decode_ssz_snappy(&path).map_err(|e| format!("decode {file_stem}: {e:#}"))?;
    let result = apply_fn(store, &value);
    match (result, expect_valid) {
        (Ok(()), true) | (Err(_), false) => Ok(()),
        (Ok(()), false) => Err(format!("expected invalid, {file_stem} returned Ok")),
        (Err(e), true) => Err(format!("expected valid, {file_stem} returned: {e}")),
    }
}

fn parse_root(s: &str) -> Result<Root, String> {
    let bytes: [u8; 32] = decode_prefixed_fixed(s).map_err(|e| e.to_string())?;
    Ok(Root(bytes))
}

fn parse_checkpoint(cp: &CheckpointHex) -> Result<Checkpoint, String> {
    Ok(Checkpoint {
        epoch: Epoch::new(cp.epoch),
        root: parse_root(&cp.root)?,
    })
}

fn assert_checks(store: &Store, checks: &Checks) -> Result<(), String> {
    if let Some(time) = checks.time
        && store.time != time
    {
        return Err(format!("time: got {} want {}", store.time, time));
    }
    if let Some(head_check) = &checks.head {
        check_head(store, head_check)?;
    }
    if let Some(cp) = &checks.justified_checkpoint {
        let want = parse_checkpoint(cp)?;
        if store.justified_checkpoint != want {
            return Err(format!(
                "justified_checkpoint mismatch: got {:?} want {:?}",
                store.justified_checkpoint, want
            ));
        }
    }
    if let Some(cp) = &checks.finalized_checkpoint {
        let want = parse_checkpoint(cp)?;
        if store.finalized_checkpoint != want {
            return Err(format!(
                "finalized_checkpoint mismatch: got {:?} want {:?}",
                store.finalized_checkpoint, want
            ));
        }
    }
    if let Some(root_hex) = &checks.proposer_boost_root {
        let want = parse_root(root_hex)?;
        if store.proposer_boost_root != want {
            return Err(format!(
                "proposer_boost_root mismatch: got {:?} want {:?}",
                store.proposer_boost_root, want
            ));
        }
    }
    Ok(())
}

fn check_head(store: &Store, head_check: &HeadCheck) -> Result<(), String> {
    let head: ForkChoiceNode = get_head(store).map_err(|e| format!("get_head: {e}"))?;
    let want_root = parse_root(&head_check.root)?;
    if head.root != want_root {
        return Err(format!(
            "head root: got {:?} want {:?}",
            head.root, want_root
        ));
    }
    let block = store
        .blocks
        .get(&head.root)
        .ok_or_else(|| format!("head block {:?} missing from store", head.root))?;
    if block.slot.as_u64() != head_check.slot {
        return Err(format!(
            "head slot: got {} want {}",
            block.slot.as_u64(),
            head_check.slot
        ));
    }
    if let Some(want_status) = head_check.payload_status {
        let got = match head.payload_status {
            PayloadStatus::Empty => 0u8,
            PayloadStatus::Full => 1,
            PayloadStatus::Pending => 2,
        };
        if got != want_status {
            return Err(format!("head payload_status: got {got} want {want_status}"));
        }
    }
    Ok(())
}
