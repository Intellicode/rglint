# Contributing

## Required checks

The `CI` workflow is the source of truth for pull-request validation. Branch
protection should require these check names:

- `fmt (ubuntu-latest)`, `fmt (macos-latest)`, `fmt (windows-latest)`
- `clippy (ubuntu-latest)`, `clippy (macos-latest)`, `clippy (windows-latest)`
- `deny (ubuntu-latest)`, `deny (macos-latest)`, `deny (windows-latest)`
- `test (ubuntu-latest)`, `test (macos-latest)`, `test (windows-latest)`
- `bench-compile (ubuntu-latest)`, `bench-compile (macos-latest)`,
  `bench-compile (windows-latest)`
- `coverage`, `parity`, and `docs-check`

Required checks must remain blocking. Do not use `continue-on-error` or
manually bypass branch protection for an implementation branch.

## Local validation

Run the same locked commands locally before opening a pull request:

```text
cargo fmt --all --check
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo deny check
cargo test --locked --workspace
cargo bench --locked --workspace --no-run
```

The `coverage`, `parity`, and `docs-check` jobs require their respective
tooling and are run in GitHub Actions until the matching xtask commands are
available locally.

Coverage uses Tarpaulin's LLVM engine so executions from separately linked rule
fixture binaries are merged reliably. It currently ratchets from a 60%
workspace line-rate floor while the cross-cutting suites are established.
`scripts/coverage-gate.sh` also keeps a 90% floor for each covered
`rglint-rules` source module. Raise the workspace
floor deliberately as coverage improves; both floors can be overridden for a
local experiment with `RGLINT_COVERAGE_WORKSPACE_MIN` and
`RGLINT_COVERAGE_RULES_MODULE_MIN`, but CI uses the checked-in defaults.
