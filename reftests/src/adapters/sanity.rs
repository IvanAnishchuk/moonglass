//! Adapter for `sanity` reference-test fixtures (full slot and block transitions).

use moonglass::containers::{BeaconState, SignedBeaconBlock};
use moonglass::error::TransitionError;
use moonglass::primitives::Slot;

use super::{Outcome, finish_state, load_pre_state};
use crate::discover::Case;
use crate::fixture;

const SLOTS_FILENAME: &str = "slots.yaml";

/// Outer `Err` carries a harness-side failure (missing file, decode error)
/// distinct from a real transition error. The outer-Ok inner-Result then
/// holds the actual transition outcome that gets compared against the post
/// fixture.
type Applied = Result<Result<(), TransitionError>, String>;

#[derive(Clone, Copy)]
enum Handler {
    Blocks,
    Slots,
}

/// Sanity, finality, and random runners share the apply-blocks-and-compare shape.
#[must_use]
pub(super) fn run(case: &Case) -> Outcome {
    let mut state = match load_pre_state(case) {
        Ok(state) => state,
        Err(msg) => return Outcome::Fail(msg),
    };

    let applied = match handler(case.handler.as_str()) {
        Some(Handler::Blocks) => apply_blocks(case, &mut state),
        Some(Handler::Slots) => apply_slots(case, &mut state),
        None => {
            return Outcome::Fail(format!(
                "sanity handler '{}' not wired in moonglass",
                case.handler
            ));
        }
    };

    match applied {
        Ok(result) => finish_state(case, &mut state, result, "transition"),
        Err(harness_msg) => Outcome::Fail(harness_msg),
    }
}

#[must_use]
pub(super) fn supports(handler: &str) -> bool {
    matches!(handler, "blocks" | "slots")
}

fn handler(name: &str) -> Option<Handler> {
    Some(match name {
        "blocks" | "finality" | "random" => Handler::Blocks,
        "slots" => Handler::Slots,
        _ => return None,
    })
}

fn apply_blocks(case: &Case, state: &mut BeaconState) -> Applied {
    let meta = fixture::read_meta(&case.root).map_err(|e| format!("read meta.yaml: {e:#}"))?;
    let count = meta
        .blocks_count
        .ok_or_else(|| "meta.yaml missing `blocks_count` for blocks/finality case".to_owned())?;
    for i in 0..count {
        let path = case.root.join(format!("blocks_{i}.ssz_snappy"));
        if !path.exists() {
            return Err(format!("missing blocks_{i}.ssz_snappy"));
        }
        let block = fixture::decode_ssz_snappy::<SignedBeaconBlock>(&path)
            .map_err(|e| format!("decode blocks_{i}: {e:#}"))?;
        if let Err(e) = state.apply_signed_block(&block) {
            return Ok(Err(e));
        }
    }
    Ok(Ok(()))
}

fn apply_slots(case: &Case, state: &mut BeaconState) -> Applied {
    let slots_path = case.root.join(SLOTS_FILENAME);
    if !slots_path.exists() {
        return Err(format!("missing {SLOTS_FILENAME}"));
    }
    let text =
        std::fs::read_to_string(&slots_path).map_err(|e| format!("read {SLOTS_FILENAME}: {e}"))?;
    let advance: u64 =
        serde_yaml::from_str(text.trim()).map_err(|e| format!("parse {SLOTS_FILENAME}: {e}"))?;
    let target = Slot(state.slot.0.saturating_add(advance));
    Ok(state.process_slots(target))
}
