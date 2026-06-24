//! YAML parser for the `fork_choice` reference-test `steps.yaml` schema.
//!
//! Fork-choice tests are imperative: each YAML item names exactly one step to
//! apply to a mutable store. The parser dispatches by the known top-level key
//! instead of using an untagged enum, because untagged parsing can hide extra
//! fields by falling through to a catch-all variant. Every known step and nested
//! check uses `deny_unknown_fields` so a new upstream check becomes a clear
//! harness failure.

use std::path::Path;

use serde::{Deserialize, Deserializer, de};

use crate::error::FixtureError;
use crate::fixtures::{FixtureStem, read_yaml_path};

#[derive(Debug)]
pub(super) enum Step {
    /// Advance store time.
    Tick(TickStep),
    /// Apply a signed beacon block fixture.
    Block(BlockStep),
    /// Apply a standalone beacon attestation fixture.
    Attestation(AttestationStep),
    /// Apply a standalone attester slashing fixture.
    AttesterSlashing(AttesterSlashingStep),
    /// Apply a signed execution payload envelope fixture.
    PayloadEnvelope(PayloadEnvelopeStep),
    /// Apply a standalone payload-attestation gossip message fixture.
    PayloadAttestation(PayloadAttestationStep),
    /// Assert one or more store checks.
    Checks(Box<ChecksStep>),
    /// Catch-all for step kinds the runner does not yet recognise.
    ///
    /// Kept last so serde tries every known variant first. The runner
    /// surfaces a clear "unknown step kind" error listing the YAML keys.
    Other(serde_yaml::Value),
}

#[derive(Clone, Copy, Debug)]
pub(super) enum StepKind {
    Tick,
    Block,
    Attestation,
    AttesterSlashing,
    PayloadEnvelope,
    PayloadAttestation,
    Checks,
}

impl StepKind {
    const ALL: &[Self] = &[
        Self::Tick,
        Self::Block,
        Self::Attestation,
        Self::AttesterSlashing,
        Self::PayloadEnvelope,
        Self::PayloadAttestation,
        Self::Checks,
    ];

    const fn wire_key(self) -> &'static str {
        match self {
            Self::Tick => "tick",
            Self::Block => "block",
            Self::Attestation => "attestation",
            Self::AttesterSlashing => "attester_slashing",
            Self::PayloadEnvelope => "execution_payload",
            Self::PayloadAttestation => "payload_attestation_message",
            Self::Checks => "checks",
        }
    }

    pub(super) const fn tag(self) -> &'static str {
        match self {
            Self::Tick => "Tick",
            Self::Block => "Block",
            Self::Attestation => "Attestation",
            Self::AttesterSlashing => "AttesterSlashing",
            Self::PayloadEnvelope => "PayloadEnvelope",
            Self::PayloadAttestation => "PayloadAttestation",
            Self::Checks => "Checks",
        }
    }

    fn parse_value<E>(self, value: serde_yaml::Value) -> Result<Step, E>
    where
        E: de::Error,
    {
        match self {
            Self::Tick => parse_step_value(value, Step::Tick),
            Self::Block => parse_step_value(value, Step::Block),
            Self::Attestation => parse_step_value(value, Step::Attestation),
            Self::AttesterSlashing => parse_step_value(value, Step::AttesterSlashing),
            Self::PayloadEnvelope => parse_step_value(value, Step::PayloadEnvelope),
            Self::PayloadAttestation => parse_step_value(value, Step::PayloadAttestation),
            Self::Checks => parse_step_value(value, |step| Step::Checks(Box::new(step))),
        }
    }
}

impl Step {
    pub(super) const fn tag(&self) -> &'static str {
        match self {
            Self::Tick(_) => StepKind::Tick.tag(),
            Self::Block(_) => StepKind::Block.tag(),
            Self::Attestation(_) => StepKind::Attestation.tag(),
            Self::AttesterSlashing(_) => StepKind::AttesterSlashing.tag(),
            Self::PayloadEnvelope(_) => StepKind::PayloadEnvelope.tag(),
            Self::PayloadAttestation(_) => StepKind::PayloadAttestation.tag(),
            Self::Checks(_) => StepKind::Checks.tag(),
            Self::Other(_) => "Other",
        }
    }
}

impl<'de> Deserialize<'de> for Step {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = serde_yaml::Value::deserialize(deserializer)?;
        let Some(map) = value.as_mapping() else {
            return Ok(Self::Other(value));
        };

        for kind in StepKind::ALL {
            if map.contains_key(serde_yaml::Value::String(kind.wire_key().to_owned())) {
                return kind.parse_value(value);
            }
        }
        Ok(Self::Other(value))
    }
}

fn parse_step_value<T, E>(value: serde_yaml::Value, wrap: impl FnOnce(T) -> Step) -> Result<Step, E>
where
    T: for<'de> Deserialize<'de>,
    E: de::Error,
{
    serde_yaml::from_value(value).map(wrap).map_err(E::custom)
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct TickStep {
    pub tick: u64,
    #[serde(default = "yes")]
    pub valid: bool,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct BlockStep {
    pub block: FixtureStem,
    #[serde(default = "yes")]
    pub valid: bool,
    // Optional data-availability inputs in the fixture format: `blobs`/`proofs`
    // (blob commitments) and `columns` (data columns). Data availability
    // is mocked in this harness, so these are accepted to satisfy
    // `deny_unknown_fields` and then ignored.
    #[serde(default)]
    #[allow(dead_code)]
    blobs: Option<serde_yaml::Value>,
    #[serde(default)]
    #[allow(dead_code)]
    proofs: Option<serde_yaml::Value>,
    #[serde(default)]
    #[allow(dead_code)]
    columns: Option<serde_yaml::Value>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct AttestationStep {
    pub attestation: FixtureStem,
    #[serde(default = "yes")]
    pub valid: bool,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct AttesterSlashingStep {
    pub attester_slashing: FixtureStem,
    #[serde(default = "yes")]
    pub valid: bool,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct PayloadEnvelopeStep {
    pub execution_payload: FixtureStem,
    #[serde(default = "yes")]
    pub valid: bool,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct PayloadAttestationStep {
    pub payload_attestation_message: FixtureStem,
    #[serde(default = "yes")]
    pub valid: bool,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ChecksStep {
    pub checks: Checks,
}

// Unknown keys are rejected so a check the harness does not model cannot be
// dropped without notice. A `checks` block carrying an unmodeled key fails
// parsing immediately because `checks` is a known top-level step.
#[derive(Debug, Default)]
pub(super) struct Checks {
    /// Expected `store.time`.
    pub time: Option<u64>,
    /// Expected `store.genesis_time`.
    pub genesis_time: Option<u64>,
    /// Expected `Store::get_head` result.
    pub head: Option<HeadCheck>,
    /// Expected `store.justified_checkpoint`.
    pub justified_checkpoint: Option<CheckpointHex>,
    /// Expected `store.finalized_checkpoint`.
    pub finalized_checkpoint: Option<CheckpointHex>,
    /// Expected `store.proposer_boost_root`.
    pub proposer_boost_root: Option<String>,
    /// Expected viable filtered-tree leaves and their weights.
    pub viable_for_head_roots_and_weights: Option<Vec<ViableForHeadCheck>>,
    /// Expected PTC timeliness vote vector for one block root.
    pub payload_timeliness_vote: Option<PayloadVoteCheck>,
    /// Expected PTC data-availability vote vector for one block root.
    pub payload_data_availability_vote: Option<PayloadVoteCheck>,
    /// Expected `Store::get_proposer_head` returned node.
    pub get_proposer_head: Option<ProposerHeadCheck>,
}

impl<'de> Deserialize<'de> for Checks {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Wire {
            time: Option<u64>,
            genesis_time: Option<u64>,
            head: Option<HeadCheck>,
            justified_checkpoint: Option<CheckpointHex>,
            finalized_checkpoint: Option<CheckpointHex>,
            proposer_boost_root: Option<String>,
            viable_for_head_roots_and_weights: Option<Vec<ViableForHeadCheck>>,
            payload_timeliness_vote: Option<PayloadVoteCheck>,
            payload_data_availability_vote: Option<PayloadVoteCheck>,
            get_proposer_head: Option<ProposerHeadCheck>,
        }

        let wire = Wire::deserialize(deserializer)?;
        let checks = Self {
            time: wire.time,
            genesis_time: wire.genesis_time,
            head: wire.head,
            justified_checkpoint: wire.justified_checkpoint,
            finalized_checkpoint: wire.finalized_checkpoint,
            proposer_boost_root: wire.proposer_boost_root,
            viable_for_head_roots_and_weights: wire.viable_for_head_roots_and_weights,
            payload_timeliness_vote: wire.payload_timeliness_vote,
            payload_data_availability_vote: wire.payload_data_availability_vote,
            get_proposer_head: wire.get_proposer_head,
        };
        if checks.is_empty() {
            return Err(de::Error::custom(
                "checks block must contain at least one check",
            ));
        }
        Ok(checks)
    }
}

impl Checks {
    pub(super) fn labels(&self) -> Vec<&'static str> {
        let mut labels = Vec::new();
        if self.time.is_some() {
            labels.push("time");
        }
        if self.genesis_time.is_some() {
            labels.push("genesis_time");
        }
        if self.head.is_some() {
            labels.push("head");
        }
        if self.justified_checkpoint.is_some() {
            labels.push("justified_checkpoint");
        }
        if self.finalized_checkpoint.is_some() {
            labels.push("finalized_checkpoint");
        }
        if self.proposer_boost_root.is_some() {
            labels.push("proposer_boost_root");
        }
        if self.viable_for_head_roots_and_weights.is_some() {
            labels.push("viable_for_head_roots_and_weights");
        }
        if self.payload_timeliness_vote.is_some() {
            labels.push("payload_timeliness_vote");
        }
        if self.payload_data_availability_vote.is_some() {
            labels.push("payload_data_availability_vote");
        }
        if self.get_proposer_head.is_some() {
            labels.push("get_proposer_head");
        }
        labels
    }

    fn is_empty(&self) -> bool {
        self.time.is_none()
            && self.genesis_time.is_none()
            && self.head.is_none()
            && self.justified_checkpoint.is_none()
            && self.finalized_checkpoint.is_none()
            && self.proposer_boost_root.is_none()
            && self.viable_for_head_roots_and_weights.is_none()
            && self.payload_timeliness_vote.is_none()
            && self.payload_data_availability_vote.is_none()
            && self.get_proposer_head.is_none()
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct HeadCheck {
    pub slot: u64,
    pub root: String,
    #[serde(default)]
    pub payload_status: Option<PayloadStatusCode>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct CheckpointHex {
    pub epoch: u64,
    pub root: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct PayloadVoteCheck {
    pub block_root: String,
    pub votes: Vec<Option<bool>>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ViableForHeadCheck {
    pub root: String,
    pub weight: u64,
    pub payload_status: PayloadStatusCode,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ProposerHeadCheck {
    pub root: String,
    pub payload_status: PayloadStatusCode,
}

/// Numeric fork-choice payload-status code used by consensus-spec YAML.
///
/// The upstream schema encodes `Empty = 0`, `Full = 1`, and `Pending = 2`.
/// Values outside that range are fixture-schema errors, not failed checks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[repr(u8)]
pub(super) enum PayloadStatusCode {
    Empty = 0,
    Full = 1,
    Pending = 2,
}

impl PayloadStatusCode {
    pub(super) const fn as_u8(self) -> u8 {
        self as u8
    }
}

impl<'de> Deserialize<'de> for PayloadStatusCode {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        match u8::deserialize(deserializer)? {
            0 => Ok(Self::Empty),
            1 => Ok(Self::Full),
            2 => Ok(Self::Pending),
            other => Err(de::Error::custom(format!(
                "invalid payload_status {other}; expected 0, 1, or 2"
            ))),
        }
    }
}

fn yes() -> bool {
    true
}

pub(super) fn parse_steps(path: &Path) -> std::result::Result<Vec<Step>, FixtureError> {
    read_yaml_path(path)
}

#[cfg(test)]
mod tests {
    use super::{Step, parse_steps};

    #[test]
    fn parses_checked_in_target_fork_get_head_steps() {
        let path = crate::testing::GET_HEAD_GENESIS.file("steps.yaml");
        let steps = parse_steps(&path).expect("parse steps");

        let [Step::Checks(step)] = steps.as_slice() else {
            panic!("expected checks step");
        };
        assert_eq!(step.checks.genesis_time, Some(0));
        let head = step.checks.head.as_ref().expect("head check");
        assert_eq!(head.slot, 0);
        assert_eq!(
            head.root,
            "0x2a9cb69f4de117f55f3cd886f2765427572b62e31666bd9ba53d19a69e148418"
        );
        assert_eq!(
            head.payload_status.map(super::PayloadStatusCode::as_u8),
            Some(0)
        );
    }
}
