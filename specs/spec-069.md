# Spec-069: xtask check-parity

> Plan reference: §3 (`xtask/src/check_parity.rs`), §6.4 (snapshot files), §9 ("xtask check-parity")

## Goal

End-to-end parity harness: run both the TS graphql-eslint linter and rglint
over the same fixtures/rules, diff their JSON outputs, and fail on
divergence. This is the ultimate oracle check (PLAN §6.2 "Parity harness").

## Scope

**In scope:**

- `xtask check-parity`:
  1. Install pnpm + node (via `actions/setup-node` in CI; locally assumes
     installed).
  2. Run the existing `pnpm test` in this repo (which runs graphql-eslint's
     own tests) but with `--format json` captured per rule → produces
     `parity/ts-output/<rule>.json`.
  3. Run rglint over the same inputs with `--format json` →
     `parity/rust-output/<rule>.json`.
  4. Diff: for each rule, compare the two JSON documents on
     `(ruleId, message, line, column)` per diagnostic (relaxing byte offsets
     per PLAN §6.3). Produce `parity/diff.md` summarizing divergences.
  5. Exit non-zero if any divergence outside the known-divergences allowlist
     (`parity/known-divergences.json`).
- The `known-divergences.json` allowlist: rule ids + cases where divergence
  is documented (e.g. spec-rule message variance per spec-053; column
  normalization rounding). Each entry has a reason.

**Out of scope:**

- Snapshot `.snap` regeneration (separate; `xtask gen-snapshots` could be a
  future spec).
- Fixing divergences (this spec only reports).

## Dependencies

- spec-058 (JSON reporter — contract for the Rust side).
- spec-014 (fixture harness — same fixtures drive both sides).
- spec-053 (known spec-rule divergence).
- spec-015 (fixture source — the TS tests this diffs against).

## Deliverables

- `xtask/src/check_parity.rs`.
- `parity/known-divergences.json` (seeded from spec-053's divergence log).
- CI integration via spec-067 (`parity` job).

## Interface / API

```
xtask check-parity                    # full run, exit non-zero on unknown divergence
xtask check-parity --rule <id>        # single rule
xtask check-parity --update-known     # append new divergences to known-divergences.json
```

## Behavior

- The comparator uses the same `(line, column_0based, ruleId, message)`
  tuple as the fixture harness (spec-014) for consistency.
- `--update-known` is a manual escape hatch; reviewed PRs only (don't run in
  CI).
- Output: a markdown table per rule: `case | ts | rust | diff | status`.

## Testing

- Self-test: on a green commit, `check-parity` exits 0.
- Inject a deliberate Rust-side message change (not in known-divergences) →
  exit non-zero with the case listed in `diff.md`.

## Risks / Notes

- TS-side invocation: this repo's `pnpm test` runs jest, not a raw JSON
  emitter; may need a small jest reporter that dumps per-rule JSON. Add a
  `parity/jest-reporter.cjs` here (tiny). Confirm the repo's test harness
  can be driven headlessly; if not, invoke `eslint --format json` directly
  on the fixtures instead.
- This is the spec that ultimately proves PLAN §6's "oracle parity" claim;
  prioritize getting it green for Phase 1-3 rules before declaring 0.1.
