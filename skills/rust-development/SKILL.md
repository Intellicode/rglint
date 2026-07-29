---
name: rust-development
description: Develop, debug, refactor, review, and verify Rust codebases and Cargo workspaces. Use when Codex is asked to implement Rust features or fixes, work with crates/modules/traits/lifetimes/async/unsafe code, diagnose compiler or Clippy failures, improve tests or benchmarks, or prepare Rust changes for commit or review.
---

# Rust Development

## Overview

Use this skill to make Rust changes with compiler-guided iteration, idiomatic ownership choices, and verification that matches the risk of the edit. Prefer the repository's existing architecture, lint settings, and test style over introducing new patterns.

## Workflow

1. Inspect the workspace before editing.
   - Read `Cargo.toml`, relevant crate manifests, nearby modules, tests, and repo instructions such as `AGENTS.md`.
   - Use `rg` and `cargo metadata` when module boundaries, feature flags, or crate relationships are unclear.
   - Check `git status --short` and preserve unrelated user changes.

2. Let the existing Rust shape guide the design.
   - Prefer local types, error handling style, visibility, naming, and iterator patterns.
   - Keep ownership simple: borrow when practical, clone only when the data is cheap or ownership clarity is worth it.
   - Avoid broad generic abstractions until duplication or API shape makes them clearly useful.
   - Treat `unsafe` as a design constraint: justify invariants in comments and add focused tests around the boundary.

3. Edit in small, compiler-friendly steps.
   - Update the narrowest module surface that satisfies the request.
   - Add or adjust tests near the behavior being changed.
   - Run `cargo fmt` after nontrivial edits, then compile and test.

4. Verify intentionally.
   - Prefer the smallest meaningful test command first, such as `cargo test -p crate_name test_name`.
   - For shared code or public APIs, broaden to crate or workspace tests.
   - Run `cargo clippy` when the repo requires it or when the edit touches patterns Clippy commonly catches.
   - Report any skipped checks and why.

## Common Tasks

### Implement Features

Trace the requested behavior from public entry points inward. Add the data model, parsing, trait impl, or API changes where callers already expect related behavior. Prefer tests that demonstrate the user-visible behavior instead of testing incidental private structure.

### Fix Bugs

Reproduce the failure when possible before editing. If the issue is a compiler error, fix the root mismatch rather than layering conversions until it compiles. If the issue is runtime behavior, add a regression test that fails before the change.

### Refactor Safely

Keep behavioral changes separate from mechanical movement when possible. Use `cargo test` or narrower tests between risky steps. Preserve public API compatibility unless the user asked for a breaking change.

### Review Rust Code

Prioritize correctness, unsoundness, panic paths, lifetime or aliasing bugs, feature-flag regressions, error swallowing, API compatibility, and missing tests. Cite file and line references for findings.

## Reference

Read `references/rust-checklist.md` when the change is nontrivial, touches public APIs, involves unsafe/async/macros/features, or the initial compile/test run fails in a way that needs structured debugging.
