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
