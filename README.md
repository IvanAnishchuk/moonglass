# Moonglass

[![test][test-badge]][actions-url] [![coverage][coverage-badge]][coverage-url] [![docs][docs-badge]][docs-url] [![license][license-badge]][license-url]

[test-badge]: https://github.com/brech1/moonglass/actions/workflows/test.yml/badge.svg
[actions-url]: https://github.com/brech1/moonglass/actions?query=branch%3Amaster
[coverage-badge]: https://codecov.io/gh/brech1/moonglass/branch/master/graph/badge.svg
[coverage-url]: https://codecov.io/gh/brech1/moonglass
[docs-badge]: https://img.shields.io/badge/docs-online-blue
[docs-url]: https://brech1.github.io/moonglass/
[license-badge]: https://img.shields.io/badge/license-AGPL--3.0-blue
[license-url]: https://github.com/brech1/moonglass/blob/master/LICENSE

> [!WARNING]
> Moonglass is experimental.

Moonglass is a Rust implementation of selected Ethereum consensus paths. It is
built for traceable reading: each path should make clear which object enters,
which rule owns it, what state changes, and where local validation stops.

## Scope

Moonglass focuses on:

- State-transition and fork-choice behavior.
- Typed consensus objects, constants, and errors that stay close to the spec
  shape.
- Rustdoc as the primary explanation layer.

Moonglass does not try to be a full beacon node. Networking, sync, validator
duties, execution-engine driving, persistence, and production operations are
outside the current library boundary.

## Validation

Consensus validation runs through the accessory `reftests` crate, which checks
Moonglass against generated [consensus-specs] fixtures. If upstream fixtures
cover new behavior, add the matching reftest adapter and checks, then run the
relevant lane. Passing a lane only proves its wired fixture inventory.
Unsupported fixture families are not coverage. See
[`reftests/README.md`](reftests/README.md) for runner details.

## Reading Model

Start with the state boundary, not the directory tree. `BeaconState` is durable
consensus state, advanced by state transition. `Store` is one node's local
fork-choice view, updated after accepted blocks and messages. `Store::payloads`
is local payload evidence, not an execution-engine verdict. `ForkChoiceNode`
includes payload status because head selection may choose between pending,
empty, and full payload branches.

For any consensus path, ask:

1. What object enters the path?
2. Which rule owns it: state transition, fork choice, or a local verifier?
3. What is read and what is mutated: `BeaconState`, `Store`, both, or neither?
4. What is verified locally, and what external verifier is not modeled?

Useful entry points:

- Blocks: `BeaconState::apply_signed_block`, then `Store::on_block`.
- Payload commitments: `process_execution_payload_bid`,
  `Store::on_execution_payload_envelope`, and
  `accept_parent_payload_commitment`.
- Votes and head choice: `process_attestation`, `Store::on_attestation`,
  and `Store::get_head`.

## Repository Layout

| Path | Role |
| --- | --- |
| `moonglass/src/containers/` | SSZ containers carried by consensus paths. |
| `moonglass/src/primitives/` | Typed roots, slots, epochs, indices, and checked arithmetic. |
| `moonglass/src/constants/` | Consumed protocol constants for the active preset. |
| `moonglass/src/state_transition/` | Slot, epoch, block, operation, and builder processing. |
| `moonglass/src/fork_choice/` | Local store updates, filtering, weights, and head selection. |
| `moonglass/src/crypto/` | Hashing, BLS, and KZG wrappers. |
| `moonglass/src/error/` | Centralized rejection reasons. |
| `reftests/` | Accessory consensus-spec fixture runner. |

## Documentation Standard

Rustdoc is the project documentation surface. Published docs are available at
[brech1.github.io/moonglass](https://brech1.github.io/moonglass/).

Library docs should explain protocol ownership, mutations, invariants, and
boundaries where the code makes those decisions.

The workspace denies missing docs, unused code, dead code, unreachable public
items, unsafe code, broken intra-doc links, and Clippy `all` and `pedantic`.
The `missing_errors_doc` lint is allowed because error descriptions are
centralized in the error modules.

See [CONTRIBUTING.md](CONTRIBUTING.md) for review, testing, and documentation
policy.

## Contributing

Contribution policy, local checks, and current contribution areas live in
[CONTRIBUTING.md](CONTRIBUTING.md). Consensus changes should include the
matching `reftests` adapter and checks when upstream fixtures exist.

## License

Moonglass is licensed under [AGPL-3.0-only](LICENSE).

[consensus-specs]: https://github.com/ethereum/consensus-specs
