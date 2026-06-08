//! YAML parser for the `fork_choice` reference-test `steps.yaml` schema.

// Fields below are deserialized from YAML; not every field is read at runtime
// (some are kept for schema parity with reference-test fixtures and future handlers).
#![allow(dead_code)]

use std::path::Path;

use serde::Deserialize;

#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub(super) enum Step {
    Tick(TickStep),
    Block(BlockStep),
    Attestation(AttestationStep),
    AttesterSlashing(AttesterSlashingStep),
    PayloadEnvelope(PayloadEnvelopeStep),
    PayloadAttestation(PayloadAttestationStep),
    Checks(ChecksStep),
    /// Catch-all for step kinds the runner does not yet recognise.
    ///
    /// Kept last so serde tries every known variant first. The runner
    /// surfaces a clear "unknown step kind" error listing the YAML keys.
    Other(serde_yaml::Value),
}

#[derive(Debug, Deserialize)]
pub(super) struct TickStep {
    pub tick: u64,
}

#[derive(Debug, Deserialize)]
pub(super) struct BlockStep {
    pub block: String,
    #[serde(default = "yes")]
    pub valid: bool,
}

#[derive(Debug, Deserialize)]
pub(super) struct AttestationStep {
    pub attestation: String,
    #[serde(default = "yes")]
    pub valid: bool,
}

#[derive(Debug, Deserialize)]
pub(super) struct AttesterSlashingStep {
    pub attester_slashing: String,
    #[serde(default = "yes")]
    pub valid: bool,
}

#[derive(Debug, Deserialize)]
pub(super) struct PayloadEnvelopeStep {
    pub execution_payload: String,
    #[serde(default = "yes")]
    pub valid: bool,
}

#[derive(Debug, Deserialize)]
pub(super) struct PayloadAttestationStep {
    pub payload_attestation_message: String,
    #[serde(default = "yes")]
    pub valid: bool,
}

#[derive(Debug, Deserialize)]
pub(super) struct ChecksStep {
    pub checks: Checks,
}

#[derive(Debug, Default, Deserialize)]
pub(super) struct Checks {
    pub time: Option<u64>,
    pub head: Option<HeadCheck>,
    pub justified_checkpoint: Option<CheckpointHex>,
    pub finalized_checkpoint: Option<CheckpointHex>,
    pub proposer_boost_root: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(super) struct HeadCheck {
    pub slot: u64,
    pub root: String,
    #[serde(default)]
    pub payload_status: Option<u8>,
}

#[derive(Debug, Deserialize)]
pub(super) struct CheckpointHex {
    pub epoch: u64,
    pub root: String,
}

fn yes() -> bool {
    true
}

pub(super) fn parse_steps(path: &Path) -> anyhow::Result<Vec<Step>> {
    let raw = std::fs::read_to_string(path)?;
    let steps: Vec<Step> = serde_yaml::from_str(&raw)?;
    Ok(steps)
}
