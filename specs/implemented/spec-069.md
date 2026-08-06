# Spec-069: xtask check-parity

> Plan reference: §3 (`xtask/src/check_parity.rs`), §6.4 (snapshot files), §9 ("xtask check-parity")

## Goal

End-to-end parity harness: compare the checked-in graphql-eslint fixture oracle
with rglint over the same cases, optionally replacing the checked-in oracle
with a command backed by a pinned graphql-eslint checkout. The command writes
deterministic JSON artifacts and fails on unknown divergence. This is the
repository's executable form of PLAN §6.2's parity harness.

## Scope

**In scope:**

- `xtask check-parity`:
  1. Discover fixture cases from `rules-fixtures/` and select one rule per
     case from its manifest. The graphql-js validation fixture's valid cases
     run against all registered graphql-js rules because that fixture contains
     several rule ids.
  2. Use each case's checked-in `expected.json` as the captured graphql-eslint
     oracle. This is the offline/default mode and keeps CI reproducible without
     a second checkout or a network download.
  3. When `--ts-command <program>` (or `RGLINT_PARITY_TS_COMMAND`) is supplied,
     invoke that adapter once per case. It receives `RGLINT_PARITY_RULE`,
     `RGLINT_PARITY_CASE`, and `RGLINT_PARITY_SOURCE` and writes an ESLint JSON
     result to stdout. The adapter is responsible for pinning its graphql-eslint
     revision.
  4. Run rglint's engine over the same in-memory fixture and write normalized
     `(rule, message, line, column)` records to
     `parity/rust-output/<rule>/<case>.json`.
  5. Normalize the oracle output to the same record shape, write it to
     `parity/ts-output/<rule>/<case>.json`, and produce `parity/diff.md`.
  6. Exit non-zero for any divergence outside
     `parity/known-divergences.json`. A fixture's `loose_message = true` setting
     intentionally compares only rule id and location, matching spec-053.
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
- `parity/README.md` documenting the offline oracle and adapter contract.
- CI integration via spec-067's `parity` job, using the offline mode.

## Interface / API

```
xtask check-parity                    # full run, exit non-zero on unknown divergence
xtask check-parity --rule <id>        # single rule
xtask check-parity --update-known     # append new divergences to known-divergences.json
xtask check-parity --ts-command <program> # use an external graphql-eslint adapter
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

- The repository no longer contains the graphql-eslint package workspace, so a
  moving `pnpm test` invocation would make parity non-reproducible. The
  checked-in fixture output is the default oracle; a pinned external adapter is
  an explicit opt-in for live upstream verification.
- This is the spec that ultimately proves PLAN §6's "oracle parity" claim;
  the offline run covers every checked-in fixture, while the adapter contract
  allows Phase 1-3 rules to be checked against a live upstream checkout.
