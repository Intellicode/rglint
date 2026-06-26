# Spec-001: Project skeleton, Cargo workspace & cargo-deny

> Plan reference: §1, §2, §3, §5 Phase 0.0, §8 (tooling risks)

## Goal

Stand up the empty Cargo workspace with all crates declared in §3, pinned
dependency versions from §2, shared lint/format config, and `cargo-deny` setup.
This is the prerequisite scaffold every subsequent spec builds on.

## Scope

**In scope:**

- Root `Cargo.toml` workspace manifest with members:
  `crates/rglint`, `crates/rglint-core`, `crates/rglint-rules`,
  `crates/rglint-graphql-spec`, `crates/rglint-config`, `crates/rglint-derive`,
  `crates/rglint-test-harness`, `xtask`.
- Each member crate created with `src/lib.rs` (or `main.rs`) stub + `Cargo.toml`
  declaring only the deps it will need (placeholder is fine; refined later).
- Shared `rustfmt.toml`, `clippy.toml`, `deny.toml`.
- `.gitignore` for `target/`, `*.snap.new`, etc.
- Top-level `README.md` (stub pointing to `PLAN.md`), `ARCHITECTURE.md` stub.
- `docs/`, `docs/rules/`, `docs/design-records/` directories (empty).
- Pin all dependency versions exactly as listed in PLAN.md §2.

**Out of scope:**

- Any actual implementation logic (just `lib.rs` with `#![allow(dead_code)]` stubs).
- CI workflow (spec-067).
- Benchmark/test harness crates' internals (specs 014, 065).

## Dependencies

- None (first spec).

## Deliverables

```
rglint/
├── Cargo.toml              # [workspace] + shared [workspace.dependencies]
├── Cargo.lock              # committed
├── rustfmt.toml
├── clippy.toml
├── deny.toml               # licenses + advisories
├── README.md               # stub
├── ARCHITECTURE.md         # stub
├── docs/
│   ├── rules/.gitkeep
│   └── design-records/.gitkeep
├── crates/
│   ├── rglint/             # bin
│   ├── rglint-core/        # lib
│   ├── rglint-rules/       # lib
│   ├── rglint-graphql-spec/# lib
│   ├── rglint-config/      # lib
│   ├── rglint-derive/      # proc-macro lib
│   └── rglint-test-harness/# lib
└── xtask/                  # bin
```

`deny.toml` must:
- Allow licenses: `MIT`, `Apache-2.0`, `BSD-2/3-Clause`, `ISC`, `Unicode-DFS-2016`.
- Deny advisories via `cargo audit` database.
- Ban duplicate versions except allowlisted.

`rustfmt.toml`: edition 2021, max_width 100, imports_granularity = "Item".

`clippy.toml`: set `msrv = "1.75"` (or chosen toolchain), `cognitive-complexity` threshold.

## Interface / API

No public API yet. Each `lib.rs` exports nothing meaningful.

## Behavior

- `cargo build --workspace` succeeds with zero warnings under
  `RUSTFLAGS="-D warnings"`.
- `cargo fmt --check` passes.
- `cargo clippy --workspace -- -D warnings` passes.
- `cargo deny check` passes (requires `cargo install cargo-deny`).

## Testing

- `cargo build --workspace --all-features` clean build.
- `cargo test --workspace` runs (no tests yet, exits 0).

## Risks / Notes

- Choose toolchain MSRV; pin in `rust-toolchain.toml` (recommended: stable).
- `rglint-derive` is a proc-macro crate — must be its own crate per Rust rules.
- Consider `[workspace.dependencies]` to share dep versions across members.
