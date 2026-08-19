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

Coverage currently ratchets from a 60% workspace line-rate floor while the
cross-cutting suites are established. `scripts/coverage-gate.sh` targets 90%
for each `rglint-rules` source module. Modules that predate the gate below that
target have reviewed floors in `scripts/coverage-baseline.json`; they must not
regress in either covered-line count or percentage. Two Tarpaulin attribution
gaps are pinned by exact line count, so edits require an explicit baseline
review. A passing run publishes the Cobertura report as the required
`tarpaulin-report` GitHub artifact. Raise or remove baselines deliberately as
coverage improves. The workspace and default module targets can be overridden
for a local experiment with `RGLINT_COVERAGE_WORKSPACE_MIN` and
`RGLINT_COVERAGE_RULES_MODULE_MIN`, but CI uses the checked-in defaults.
