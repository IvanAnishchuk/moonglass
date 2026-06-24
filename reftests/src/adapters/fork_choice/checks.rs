//! Assertions for `fork_choice` reference-test `checks` steps.
//!
//! Store-field checks compare the public store state directly. Derived
//! fork-choice checks call back into Moonglass public read APIs, so the
//! harness owns only fixture decoding and assertion formatting.

use moonglass::containers::Checkpoint;
use moonglass::fork_choice::{PayloadStatus, Store, diagnostics::get_viable_for_head_nodes};
use moonglass::primitives::{Epoch, Root};

use super::steps::{
    CheckpointHex, Checks, HeadCheck, PayloadStatusCode, PayloadVoteCheck, ProposerHeadCheck,
    ViableForHeadCheck,
};
use crate::adapters::{trace_enabled, trace_step_check_fail, trace_step_check_pass};
use crate::fixtures::{decode_fixed_hex, encode_hex};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct ViableForHeadNode {
    root: Root,
    weight: u64,
    payload_status: PayloadStatusCode,
}

#[derive(Clone, Copy)]
pub(super) struct StepContext<'a> {
    index: usize,
    tag: &'a str,
}

impl<'a> StepContext<'a> {
    pub(super) const fn new(index: usize, tag: &'a str) -> Self {
        Self { index, tag }
    }
}

#[derive(Clone, Copy)]
struct CheckTrace<'a> {
    step: StepContext<'a>,
}

impl<'a> CheckTrace<'a> {
    const fn new(step: StepContext<'a>) -> Self {
        Self { step }
    }

    fn pass(self, label: &'static str, detail: impl FnOnce() -> String) {
        if trace_enabled() {
            trace_step_check_pass(self.step.index, self.step.tag, label, detail());
        }
    }

    fn pass_dynamic(self, label: &str, detail: impl FnOnce() -> String) {
        if trace_enabled() {
            trace_step_check_pass(self.step.index, self.step.tag, label, detail());
        }
    }

    fn fail<T>(self, label: &str, detail: String) -> Result<T, String> {
        trace_step_check_fail(self.step.index, self.step.tag, label, &detail);
        Err(detail)
    }
}

pub(super) fn assert_checks(
    store: &Store,
    checks: &Checks,
    step: StepContext<'_>,
) -> Result<(), String> {
    let trace = CheckTrace::new(step);
    if let Some(time) = checks.time {
        if store.time == time {
            trace.pass("time", || format!("got {time}"));
        } else {
            return trace.fail("time", format!("got {} want {}", store.time, time));
        }
    }
    if let Some(genesis_time) = checks.genesis_time {
        if store.genesis_time == genesis_time {
            trace.pass("genesis_time", || format!("got {genesis_time}"));
        } else {
            return trace.fail(
                "genesis_time",
                format!("got {} want {}", store.genesis_time, genesis_time),
            );
        }
    }
    if let Some(head_check) = &checks.head {
        check_head(trace, store, head_check)?;
    }
    if let Some(cp) = &checks.justified_checkpoint {
        let want = parse_checkpoint(trace, "justified_checkpoint", cp)?;
        if store.justified_checkpoint != want {
            return trace.fail(
                "justified_checkpoint",
                format!(
                    "justified_checkpoint mismatch: got {:?} want {:?}",
                    store.justified_checkpoint, want
                ),
            );
        }
        trace.pass("justified_checkpoint", || {
            format!("got epoch {:?} root {}", want.epoch, root_hex(&want.root))
        });
    }
    if let Some(cp) = &checks.finalized_checkpoint {
        let want = parse_checkpoint(trace, "finalized_checkpoint", cp)?;
        if store.finalized_checkpoint != want {
            return trace.fail(
                "finalized_checkpoint",
                format!(
                    "finalized_checkpoint mismatch: got {:?} want {:?}",
                    store.finalized_checkpoint, want
                ),
            );
        }
        trace.pass("finalized_checkpoint", || {
            format!("got epoch {:?} root {}", want.epoch, root_hex(&want.root))
        });
    }
    if let Some(root) = &checks.proposer_boost_root {
        let want = parse_root(trace, "proposer_boost_root", root)?;
        if store.proposer_boost_root != want {
            return trace.fail(
                "proposer_boost_root",
                format!(
                    "proposer_boost_root mismatch: got {:?} want {:?}",
                    store.proposer_boost_root, want
                ),
            );
        }
        trace.pass("proposer_boost_root", || format!("got {}", root_hex(&want)));
    }
    if let Some(check) = &checks.viable_for_head_roots_and_weights {
        check_viable_for_head_roots_and_weights(trace, store, check)?;
    }
    if let Some(check) = &checks.payload_timeliness_vote {
        check_payload_votes(
            trace,
            "payload_timeliness_vote",
            &store.payload_timeliness_vote,
            check,
        )?;
    }
    if let Some(check) = &checks.payload_data_availability_vote {
        check_payload_votes(
            trace,
            "payload_data_availability_vote",
            &store.payload_data_availability_vote,
            check,
        )?;
    }
    if let Some(want) = &checks.get_proposer_head {
        check_get_proposer_head(trace, store, want)?;
    }
    Ok(())
}

fn check_get_proposer_head(
    trace: CheckTrace<'_>,
    store: &Store,
    want: &ProposerHeadCheck,
) -> Result<(), String> {
    let want_root = parse_root(trace, "get_proposer_head", &want.root)?;
    let head = match store.get_head() {
        Ok(head) => head,
        Err(e) => return trace.fail("get_proposer_head", format!("get_head: {e}")),
    };
    let node = match store.get_proposer_head(head, store.get_current_slot()) {
        Ok(node) => node,
        Err(e) => return trace.fail("get_proposer_head", format!("get_proposer_head: {e}")),
    };
    if node.root != want_root {
        return trace.fail(
            "get_proposer_head",
            format!(
                "get_proposer_head mismatch: got {} want {}",
                root_hex(&node.root),
                root_hex(&want_root)
            ),
        );
    }
    // The result is a complete ForkChoiceNode, so the fixture carries the expected
    // payload status alongside the root and both must match.
    let got_status = payload_status_code(node.payload_status);
    if got_status != want.payload_status {
        return trace.fail(
            "get_proposer_head",
            format!(
                "get_proposer_head payload_status mismatch: got {got_status:?} want {:?}",
                want.payload_status
            ),
        );
    }
    trace.pass("get_proposer_head", || {
        format!("got {} status {got_status:?}", root_hex(&node.root))
    });
    Ok(())
}

fn parse_root(trace: CheckTrace<'_>, label: &str, s: &str) -> Result<Root, String> {
    let bytes: [u8; 32] = match decode_fixed_hex(s) {
        Ok(bytes) => bytes,
        Err(e) => return trace.fail(label, format!("invalid root {s}: {e}")),
    };
    Ok(Root(bytes))
}

fn parse_checkpoint(
    trace: CheckTrace<'_>,
    label: &str,
    cp: &CheckpointHex,
) -> Result<Checkpoint, String> {
    Ok(Checkpoint {
        epoch: Epoch::new(cp.epoch),
        root: parse_root(trace, label, &cp.root)?,
    })
}

fn check_payload_votes(
    trace: CheckTrace<'_>,
    label: &str,
    actual_by_root: &std::collections::HashMap<Root, Vec<Option<bool>>>,
    check: &PayloadVoteCheck,
) -> Result<(), String> {
    let root = parse_root(trace, label, &check.block_root)?;
    let Some(actual) = actual_by_root.get(&root) else {
        return trace.fail(
            label,
            format!("{label}: missing vote vector for {}", root_hex(&root)),
        );
    };
    if actual != &check.votes {
        return trace.fail(
            label,
            format!(
                "{label}: got {:?} want {:?} for {:?}",
                actual, check.votes, root
            ),
        );
    }
    trace.pass_dynamic(label, || format!("got {actual:?} for {root:?}"));
    Ok(())
}

fn check_head(trace: CheckTrace<'_>, store: &Store, head_check: &HeadCheck) -> Result<(), String> {
    let head = match store.get_head() {
        Ok(head) => {
            trace.pass("head.get_head", || format!("root {}", root_hex(&head.root)));
            head
        }
        Err(e) => return trace.fail("head.get_head", format!("get_head: {e}")),
    };
    let want_root = parse_root(trace, "head.root", &head_check.root)?;
    if head.root != want_root {
        return trace.fail(
            "head.root",
            format!("head root: got {:?} want {:?}", head.root, want_root),
        );
    }
    trace.pass("head.root", || format!("got {}", root_hex(&head.root)));
    let Some(block) = store.blocks.get(&head.root) else {
        return trace.fail(
            "head.block",
            format!("head block {:?} missing from store", head.root),
        );
    };
    if block.slot.as_u64() != head_check.slot {
        return trace.fail(
            "head.slot",
            format!(
                "head slot: got {} want {}",
                block.slot.as_u64(),
                head_check.slot
            ),
        );
    }
    trace.pass("head.slot", || format!("got {}", block.slot.as_u64()));
    if let Some(want_status) = head_check.payload_status {
        let got = payload_status_code(head.payload_status);
        if got != want_status {
            return trace.fail(
                "head.payload_status",
                format!(
                    "head payload_status: got {} want {}",
                    got.as_u8(),
                    want_status.as_u8()
                ),
            );
        }
        trace.pass("head.payload_status", || format!("got {}", got.as_u8()));
    }
    Ok(())
}

fn check_viable_for_head_roots_and_weights(
    trace: CheckTrace<'_>,
    store: &Store,
    check: &[ViableForHeadCheck],
) -> Result<(), String> {
    let mut want = check
        .iter()
        .map(|node| {
            Ok(ViableForHeadNode {
                root: parse_root(trace, "viable_for_head_roots_and_weights", &node.root)?,
                weight: node.weight,
                payload_status: node.payload_status,
            })
        })
        .collect::<Result<Vec<_>, String>>()?;
    let mut got = match viable_for_head_roots_and_weights(store) {
        Ok(nodes) => nodes,
        Err(e) => return trace.fail("viable_for_head_roots_and_weights", e),
    };

    // The Python compliance runner compares this check as an unordered set.
    // Sorting preserves duplicate detection while avoiding traversal-order
    // differences between HashMap-backed stores.
    want.sort_unstable();
    got.sort_unstable();

    if got != want {
        return trace.fail(
            "viable_for_head_roots_and_weights",
            format!("viable_for_head_roots_and_weights mismatch: got {got:?} want {want:?}"),
        );
    }
    trace.pass("viable_for_head_roots_and_weights", || {
        format!("{} nodes matched", got.len())
    });
    Ok(())
}

fn viable_for_head_roots_and_weights(store: &Store) -> Result<Vec<ViableForHeadNode>, String> {
    let nodes =
        get_viable_for_head_nodes(store).map_err(|e| format!("get_viable_for_head_nodes: {e}"))?;
    Ok(nodes
        .into_iter()
        .map(|node| ViableForHeadNode {
            root: node.root,
            weight: node.weight.as_u64(),
            payload_status: payload_status_code(node.payload_status),
        })
        .collect())
}

fn payload_status_code(status: PayloadStatus) -> PayloadStatusCode {
    match status {
        PayloadStatus::Empty => PayloadStatusCode::Empty,
        PayloadStatus::Full => PayloadStatusCode::Full,
        PayloadStatus::Pending => PayloadStatusCode::Pending,
    }
}

fn root_hex(root: &Root) -> String {
    format!("0x{}", encode_hex(&root.0))
}

#[cfg(test)]
mod tests {
    use moonglass::fork_choice::PayloadStatus;

    use super::payload_status_code;

    #[test]
    fn payload_status_codes_match_fork_choice_format() {
        assert_eq!(payload_status_code(PayloadStatus::Empty).as_u8(), 0);
        assert_eq!(payload_status_code(PayloadStatus::Full).as_u8(), 1);
        assert_eq!(payload_status_code(PayloadStatus::Pending).as_u8(), 2);
    }
}
