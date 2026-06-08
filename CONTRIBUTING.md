# Contributing

Thanks for taking the time. Moonglass is a readability-focused implementation of Ethereum consensus, so contributions should keep behavior clear, traceable, and easy to audit.

## PR titles

PRs are squash-merged. Your PR title becomes the single commit on `master`, so it must follow [Conventional Commits](https://www.conventionalcommits.org/):

```
<type>(<optional scope>): <subject>
```

CI rejects titles that don't match. Subject starts lowercase, no trailing period.

### Allowed types

| Type | When to use |
| --- | --- |
| `feat` | New behavior or new public API |
| `fix` | Bug fix |
| `docs` | Doc-only change (`///`, `//!`, README) |
| `refactor` | Internal restructure with no behavior change |
| `perf` | Performance improvement |
| `test` | Adding or fixing tests only |
| `chore` | Tooling, dependencies, repo housekeeping |
| `ci` | Workflow or CI configuration |
| `build` | Build system or external build deps |
| `style` | Formatting only (`cargo fmt`, whitespace) |

### Optional scope

A scope is the subsystem the change touches. Useful but not required. Examples: `fork_choice`, `state_transition`, `reftests`, `crypto`, `primitives`, `containers`, `constants`, `error`, `ci`.

### Examples

```
feat(fork_choice): add proposer boost reset on slot rollover
fix(state_transition): reject attestation with future target epoch
docs(constants): document bit-40 builder index flag
refactor(epoch): split process_epoch into phase-named helpers
test(primitives): add round-trip for BuilderIndex encoding
chore: bump ssz_rs to 0.10
ci: cache reftest vectors per consensus-specs tag
```

Bad examples and why:

```
Fix bug                       # missing type, capital F, vague subject
feat: Added new helper.       # capital A, trailing period
feat(ForkChoice): ...         # scope should be lowercase
update                        # not a conventional-commit type
```

## Local checks

Before opening a PR, run:

```
cargo fmt --all
cargo clippy --all-targets -- -D warnings
cargo test --workspace
cargo build -p moonglass --no-default-features --features minimal
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps
```

All five must pass cleanly. The fourth catches breakage of the minimal preset, which CI also exercises. The fifth catches broken intra-doc links in `///` and `//!` comments.

CI runs the same checks plus the consensus-specs reference tests (minimal preset). Timeouts are reported but do not block merges.

## Review

Every PR needs **one approving review** before it can be squash-merged. CI must be green: `build`, `doc`, `reftests`, and `conventional-commit`.

## Reftests allowlist

`reftests/src/known_failures.rs` lists cases the implementation is known to fail. They show up as `todo` in the summary and do not fail CI. When you fix a bug, remove its entry. If a case in the allowlist starts passing, the runner reports it as an "unexpected pass" so the list gets cleaned.

## Testing convention

Tests live inline at the bottom of each module in a `#[cfg(test)] mod tests` block. See `moonglass/src/primitives/numeric.rs` for the canonical example. New helpers should ship with at least one test covering boundary or sentinel behavior.
