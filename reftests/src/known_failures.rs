//! Allowlist of reftest cases that the moonglass implementation is known to
//! fail. Listing a case here lets the reftests job stay green for it; the
//! failure is still reported separately so it remains visible.
//!
//! ## This list should disappear
//!
//! Every entry is an open bug. The goal is an empty [`KNOWN_FAILURES`] list.
//! When a fix lands, remove the corresponding entry. If a fix accidentally
//! makes a previously failing case pass, the runner reports an
//! "unexpected pass" so the list gets cleaned up.

/// One allowlisted case (or group of cases) and the reason it is allowed to
/// fail.
pub(crate) struct KnownFailure {
    /// Prefix matched against [`Case::display_path`]. A trailing `/` matches
    /// every case under that subpath; otherwise the match is exact.
    ///
    /// [`Case::display_path`]: crate::discover::Case::display_path
    pub case: &'static str,
    pub reason: &'static str,
}

/// TODO: empty this list.
///
/// Each entry is a bug to fix. New entries should be added with a clear
/// reason and removed as the underlying bug is fixed.
pub(crate) const KNOWN_FAILURES: &[KnownFailure] = &[
    KnownFailure {
        case: "minimal/gloas/fork_choice/on_attestation/",
        reason: "TODO: payload_status not propagated through on_attestation",
    },
    KnownFailure {
        case: "minimal/gloas/fork_choice/on_execution_payload_envelope/",
        reason: "TODO: payload_status not propagated through on_execution_payload_envelope",
    },
    KnownFailure {
        case: "minimal/gloas/fork_choice/on_payload_attestation_message/",
        reason: "TODO: indexed attestation index uniqueness check rejects valid attestations",
    },
    KnownFailure {
        case: "minimal/gloas/operations/payload_attestation/",
        reason: "TODO: same indexed attestation sorting/uniqueness bug under operations",
    },
    KnownFailure {
        case: "minimal/gloas/random/random/pyspec_tests/randomized_1",
        reason: "TODO: blocked on the indexed attestation fix above",
    },
];

/// Returns the matching [`KnownFailure`] for `case_path`, if any.
#[must_use]
pub(crate) fn matches(case_path: &str) -> Option<&'static KnownFailure> {
    KNOWN_FAILURES
        .iter()
        .find(|kf| case_path.starts_with(kf.case))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn directory_prefix_matches_descendants() {
        let hit = matches("minimal/gloas/fork_choice/on_attestation/pyspec_tests/some_case");
        assert!(hit.is_some());
    }

    #[test]
    fn exact_pattern_matches_only_that_case() {
        assert!(matches("minimal/gloas/random/random/pyspec_tests/randomized_1").is_some());
        assert!(matches("minimal/gloas/random/random/pyspec_tests/randomized_2").is_none());
    }

    #[test]
    fn unrelated_path_does_not_match() {
        assert!(matches("minimal/gloas/operations/attestation/pyspec_tests/foo").is_none());
    }
}
