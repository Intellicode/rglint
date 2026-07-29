# Rust Development Checklist

## Investigation

- Identify the active workspace with `Cargo.toml` and `cargo metadata` when crate membership or feature flags matter.
- Read nearby tests before inventing new test style.
- Inspect feature declarations, optional dependencies, `build.rs`, workspace lints, and toolchain files when compilation behavior is surprising.
- Use `rg` for call sites, trait impls, feature gates, and error messages.

## Design

- Prefer explicit domain types over loosely typed strings or tuples at public boundaries.
- Keep conversions near boundaries; avoid scattering `as` casts or lossy conversions.
- Return `Result` for expected failures and reserve `panic!` for invariant violations or tests.
- Use `thiserror`, `anyhow`, or local error enums only when already present in the crate.
- Choose trait bounds that express the caller need; avoid strengthening bounds accidentally in public APIs.
- Preserve `Send`, `Sync`, `Unpin`, lifetime, and feature-gated behavior when changing public structs or async code.

## Implementation

- Let the compiler drive iteration: fix type errors from the earliest meaningful command before widening the test scope.
- Prefer `Path`/`PathBuf`, `OsStr`/`OsString`, and UTF-8 conversions deliberately in filesystem code.
- Avoid unnecessary allocation in hot paths, but favor clarity unless profiling or existing code indicates performance sensitivity.
- Keep macros small and test their expanded behavior through public usage.
- Add comments for non-obvious invariants, unsafe contracts, or protocol details; skip comments that restate the code.

## Testing

- Add regression tests for bug fixes and edge cases.
- Test feature-gated behavior with the relevant `cargo test --features ...` or `--no-default-features` command when touched.
- Use snapshot tests only if the repo already uses snapshot tooling.
- For parser, formatter, linter, or compiler-like crates, include invalid input and diagnostic-location cases.
- For concurrency or async changes, include cancellation, ordering, and error propagation cases where meaningful.

## Verification Commands

Start narrow, then broaden:

```bash
cargo fmt
cargo test -p <crate> <test_name>
cargo test -p <crate>
cargo clippy -p <crate> --all-targets
```

For workspace-level or release-sensitive changes, consider:

```bash
cargo build --workspace --all-targets
cargo test --workspace
cargo clippy --workspace --all-targets
cargo doc --workspace --no-deps
```

Respect repository instructions when they prescribe a specific sequence.

## Code Review Focus

- Unsound `unsafe`, missing safety comments, or invalid aliasing/threading assumptions.
- Panics, unwraps, indexing, integer overflow, or unchecked conversions on user-controlled input.
- Broken feature combinations or target-specific code paths.
- Public API breakage, semver-sensitive changes, or trait bound regressions.
- Error messages that lose actionable context.
- Tests that only cover the happy path.
