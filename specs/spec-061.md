# Spec-061: --fix mode

> Plan reference: §5 Phase 8 ("--fix mode applying suggestions"), §11 (stretch: "--fix rewrites in place")

## Goal

Implement `--fix`: apply rule suggestions (`Fix`) to source files in place,
re-linting iteratively until no fixable diagnostics remain (or a fixed
iteration cap). Rules with `hasSuggestions: true` contribute fixes; `--fix`
edits the files on disk.

## Scope

**In scope:**

- A `Fixer` that, given a `ProjectLintResult` + the source files, groups
  `Diagnostic.suggestions[].fix` by file, applies non-overlapping fixes
  (rightmost-first to preserve offsets), writes the new source, and re-runs
  the engine.
- Iteration cap (default 10) to avoid fix loops.
- `--fix-dry-run` flag: print the diff (`unified_diff`) without writing.
- `--fix` only applies fixes whose rule is enabled and `hasSuggestions: true`.
- Conflict resolution: overlapping fix spans → keep the first (lowest offset),
  skip the rest in that pass, retry next iteration.
- Back up original files? No (git is the user's safety net) — but print a
  summary of changed files.

**Out of scope:**

- Fixing schema files (v1: operations only; schema fixes risk cascading —
  add a `--fix-schema` flag later).
- VSCode LSP integration (stretch).

## Dependencies

- spec-003 (Fix, Suggestion).
- spec-011 (LintEngine — re-run after each pass).
- spec-062 (CLI flags).

## Deliverables

- `crates/rglint-core/src/fixer.rs` (or `crates/rglint/src/fixer.rs` —
  decide; core keeps it testable without CLI).
- Integration test: a fixture with 2 fixable `alphabetize` swaps → after
  `--fix`, re-lint yields zero diagnostics.
- `--fix-dry-run` snapshot test (unified diff).

## Interface / API

```rust
pub struct Fixer<'e> { engine: &'e LintEngine, max_passes: usize }
impl<'e> Fixer<'e> {
    pub fn fix(&self, project: &mut Project) -> Result<FixSummary>;
    pub fn dry_run(&self, project: &Project) -> Result<Vec<FileDiff>>;
}
pub struct FixSummary { pub passes: usize, pub files_changed: usize, pub remaining: usize }
pub struct FileDiff { pub path: PathBuf, pub unified_diff: String }
```

## Behavior

- Each pass: lint → collect suggestions → apply per-file (sort desc by offset,
  skip overlaps) → write → next pass.
- Stop when a pass applies zero fixes or `max_passes` hit.
- `dry_run` never writes; produces a unified diff per file.
- A rule that emits a fix which doesn't resolve its own diagnostic → loop
  caught by `max_passes`; log a warning naming the rule.

## Testing

- Integration: `alphabetize` 3-field swap → one pass, file sorted, zero
  remaining.
- Loop guard: a malicious fixture where a fix re-triggers itself → stops at
  `max_passes`, no infinite loop, warning logged.
- Dry-run diff snapshot.

## Risks / Notes

- PLAN §11 lists `--fix` as a stretch goal but Phase 8 mentions it; treat as
  **in scope for 1.0** but only for operation-side fixes. Schema fixes
  deferred. Record this scoping decision in `ARCHITECTURE.md`.
