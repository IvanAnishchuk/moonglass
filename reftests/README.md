# reftests

Conformance runner for moonglass against the generated fixtures published on
the hardcoded [`ethereum/consensus-specs`](https://github.com/ethereum/consensus-specs)
release tag.

## Usage

```bash
cargo build --release -p reftests
target/release/reftests
```

That single runner builds and runs both presets. The default release binary runs
the mainnet preset plus the shared `general` fixtures directly, builds the
minimal preset runner into `target/reftests-minimal/`, then runs minimal.
Discovery includes every fixture family currently wired to moonglass adapters;
unsupported upstream families are not selected.

The release tag and fork target live in code as constants. Users do not pass
tags, configs, filters, or subcommands.

The runner auto-fetches the hardcoded test-vector release into
`reftests/vectors/` when the required fixtures are missing.

`target/release/reftests` exits `0` only when both presets match at least one
wired case and every matched case passes.
