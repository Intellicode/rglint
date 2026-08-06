# Spec-067: CI pipeline (GitHub Actions)

> Plan reference: §9 (CI Pipeline), §6.7 (coverage gate)

## Goal

Implement the GitHub Actions CI matrix (Linux/mac/Windows) running the full
PLAN §9 pipeline: fmt, clippy, deny, test, coverage, bench-compile, parity,
docs-check.

## Scope

**In scope:**

- `.github/workflows/ci.yml` — matrix (`ubuntu-latest`, `macos-latest`,
  `windows-latest`) with jobs:
  1. `fmt` — `cargo fmt --all --check`.
  2. `clippy` — `cargo clippy --workspace --all-targets -- -D warnings`.
  3. `deny` — `cargo deny check` (uses spec-001's `deny.toml`).
  4. `test` — `cargo test --workspace` (unit + fixture + snapshot).
  5. `coverage` — `cargo tarpaulin --workspace --out xml` (Linux only;
     tarpaulin doesn't support mac/win) → upload to codecov + enforce 85%
     floor (spec-070 owns the gate logic; this job runs tarpaulin and posts
     the report).
  6. `bench-compile` — `cargo bench --workspace --no-run` (compile-only per
     PLAN §9).
  7. `parity` — `xtask check-parity` (spec-069; installs pnpm + node,
     runs graphql-eslint tests, diffs Rust output). Linux only.
  8. `docs-check` — `xtask gen-docs --check` (spec-068). Linux only.
- Concurrency: cancel in-progress on new push to the same PR.
- Cache: `Swatinem/rust-action` or `actions/cache` for `~/.cargo` +
  `target/`.
- Required status checks: fmt, clippy, deny, test (all OSes), coverage,
  parity, docs-check.

**Out of scope:**

- The release workflow (spec-066).
- The nightly bench run (spec-065).

## Dependencies

- spec-001 (deny.toml), spec-065 (bench), spec-069 (parity), spec-068
  (docs-check), spec-070 (coverage gate).

## Deliverables

- `.github/workflows/ci.yml`.
- `.github/dependabot.yml` (keep actions + cargo deps current).
- `README.md` CI badge section.

## Interface / API

None (workflow YAML).

## Behavior

- A failing required check blocks merge (branch protection assumed; document
  in `docs/contributing.md`).
- `parity` and `docs-check` are Linux-only (Node/xtask convenience).
- Coverage floor enforced in the `coverage` job via spec-070's script.

## Testing

- Self-test: push a deliberate lint error → `clippy` fails; revert → passes.
- Matrix: all three OSes green on `main`.

## Risks / Notes

- Windows path handling in fixtures: graphql-eslint fixtures use `/`; ensure
  the test harness (spec-014) normalizes path separators so Windows CI is
  green. Address in spec-014 if it surfaces.
- tarpaulin is Linux-only; mac/win coverage is best-effort (skip the gate on
  those OSes).
