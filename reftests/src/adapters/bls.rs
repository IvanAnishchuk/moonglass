//! Adapter for `bls` reference-test fixtures.

use serde::Deserialize;

use moonglass::error::SignatureError;
use moonglass::primitives::{BLSPubkey, BLSSignature, Root};
use moonglass::state_transition::{aggregate_pubkeys, fast_aggregate_verify, verify_signature};

use crate::adapters::Outcome;
use crate::discover::Case;
use crate::hex;

const DATA_FILENAME: &str = "data.yaml";

#[must_use]
pub(super) fn run(case: &Case) -> Outcome {
    let data_path = case.root.join(DATA_FILENAME);
    if !data_path.exists() {
        return Outcome::Fail(format!("missing {DATA_FILENAME}"));
    }
    let text = match std::fs::read_to_string(&data_path) {
        Ok(t) => t,
        Err(e) => return Outcome::Fail(format!("read {DATA_FILENAME}: {e}")),
    };
    match handler(case.handler.as_str()) {
        Some(run) => run(&text),
        None => Outcome::Fail(format!(
            "bls handler '{}' not wired in moonglass",
            case.handler
        )),
    }
}

#[must_use]
pub(super) fn supports(handler: &str) -> bool {
    self::handler(handler).is_some()
}

fn handler(name: &str) -> Option<fn(&str) -> Outcome> {
    match name {
        "verify" => Some(handle_verify),
        "eth_aggregate_pubkeys" => Some(handle_aggregate_pubkeys),
        "eth_fast_aggregate_verify" => Some(handle_eth_fast_aggregate_verify),
        _ => None,
    }
}

#[derive(Deserialize)]
struct VerifyCase {
    input: VerifyInput,
    output: bool,
}

#[derive(Deserialize)]
struct VerifyInput {
    pubkey: String,
    message: String,
    signature: String,
}

fn handle_verify(text: &str) -> Outcome {
    let case: VerifyCase = match serde_yaml::from_str(text) {
        Ok(c) => c,
        Err(e) => return Outcome::Fail(format!("parse data.yaml: {e}")),
    };
    let pubkey = match parse_pubkey(&case.input.pubkey) {
        Ok(p) => p,
        Err(e) => return decode_failure(&e, case.output),
    };
    let signing_root = match parse_root(&case.input.message) {
        Ok(r) => r,
        Err(e) => return decode_failure(&e, case.output),
    };
    let signature = match parse_signature(&case.input.signature) {
        Ok(s) => s,
        Err(e) => return decode_failure(&e, case.output),
    };
    let result = verify_signature(
        &pubkey,
        signing_root,
        &signature,
        SignatureError::RandaoReveal,
    )
    .is_ok();
    check_bool(result, case.output)
}

#[derive(Deserialize)]
struct AggregatePubkeysCase {
    input: Vec<String>,
    output: Option<String>,
}

fn handle_aggregate_pubkeys(text: &str) -> Outcome {
    let case: AggregatePubkeysCase = match serde_yaml::from_str(text) {
        Ok(c) => c,
        Err(e) => return Outcome::Fail(format!("parse data.yaml: {e}")),
    };
    let mut pubkeys: Vec<BLSPubkey> = Vec::with_capacity(case.input.len());
    let mut decode_error: Option<String> = None;
    for hex in &case.input {
        match parse_pubkey(hex) {
            Ok(p) => pubkeys.push(p),
            Err(e) => {
                decode_error = Some(e);
                break;
            }
        }
    }
    let expected_bytes = match case.output.as_deref().map(parse_pubkey).transpose() {
        Ok(o) => o,
        Err(e) => return Outcome::Fail(format!("decode expected output: {e}")),
    };
    if let Some(err) = decode_error {
        return match expected_bytes {
            None => Outcome::Pass,
            Some(_) => Outcome::Fail(format!("input pubkey decode failed: {err}")),
        };
    }
    let result = aggregate_pubkeys(&pubkeys);
    match (result, expected_bytes) {
        (Ok(got), Some(want)) if got.0 == want.0 => Outcome::Pass,
        (Ok(got), Some(want)) => Outcome::Fail(format!(
            "aggregate mismatch: got 0x{}, want 0x{}",
            hex::encode(&got.0),
            hex::encode(&want.0)
        )),
        (Ok(got), None) => {
            Outcome::Fail(format!("expected failure, got 0x{}", hex::encode(&got.0)))
        }
        (Err(_), None) => Outcome::Pass,
        (Err(e), Some(_)) => Outcome::Fail(format!("aggregate failed: {e}")),
    }
}

#[derive(Deserialize)]
struct FastAggregateVerifyCase {
    input: FastAggregateVerifyInput,
    output: bool,
}

#[derive(Deserialize)]
struct FastAggregateVerifyInput {
    pubkeys: Vec<String>,
    message: String,
    signature: String,
}

fn handle_eth_fast_aggregate_verify(text: &str) -> Outcome {
    let case: FastAggregateVerifyCase = match serde_yaml::from_str(text) {
        Ok(c) => c,
        Err(e) => return Outcome::Fail(format!("parse data.yaml: {e}")),
    };
    let mut pubkeys: Vec<BLSPubkey> = Vec::with_capacity(case.input.pubkeys.len());
    for hex in &case.input.pubkeys {
        match parse_pubkey(hex) {
            Ok(p) => pubkeys.push(p),
            Err(e) => return decode_failure(&e, case.output),
        }
    }
    let signing_root = match parse_root(&case.input.message) {
        Ok(r) => r,
        Err(e) => return decode_failure(&e, case.output),
    };
    let signature = match parse_signature(&case.input.signature) {
        Ok(s) => s,
        Err(e) => return decode_failure(&e, case.output),
    };
    let result = fast_aggregate_verify(
        &pubkeys,
        signing_root,
        &signature,
        SignatureError::SyncAggregate,
    )
    .is_ok();
    check_bool(result, case.output)
}

fn decode_failure(err: &str, expected: bool) -> Outcome {
    if expected {
        Outcome::Fail(format!("input decode failed: {err}"))
    } else {
        Outcome::Pass
    }
}

fn check_bool(got: bool, want: bool) -> Outcome {
    if got == want {
        Outcome::Pass
    } else {
        Outcome::Fail(format!("expected {want}, got {got}"))
    }
}

fn parse_pubkey(hex: &str) -> Result<BLSPubkey, String> {
    hex::decode_prefixed_fixed(hex)
        .map(BLSPubkey)
        .map_err(|e| e.to_string())
}

fn parse_signature(hex: &str) -> Result<BLSSignature, String> {
    hex::decode_prefixed_fixed(hex)
        .map(BLSSignature)
        .map_err(|e| e.to_string())
}

fn parse_root(hex: &str) -> Result<Root, String> {
    hex::decode_prefixed_fixed(hex)
        .map(Root)
        .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn aggregate_pubkeys_expected_failure_counts_as_pass() {
        let text = "input: []\noutput: null\n";
        assert!(matches!(handle_aggregate_pubkeys(text), Outcome::Pass));
    }
}
