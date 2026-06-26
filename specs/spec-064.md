# Spec-064: Rayon parallelization

> Plan reference: §5 Phase 9 ("Rayon parallelisation per file"), §1 (engine Send + Sync)

## Goal

Parallelize per-file linting with `rayon` so multi-file projects lint on all
cores. The engine (spec-011) is already `Send + Sync`; this spec adds the
parallel dispatch + cache locking.

## Scope

**In scope:**

- Replace the single-threaded per-file loop in `LintEngine::lint` with a
  `rayon::par_iter` over documents.
- Thread-safe cache (spec-013): wrap the in-memory map in a `DashMap` (or
  `Mutex<HashMap>`) so parallel workers can read cached diagnostics without
  contention.
- Per-file results collected into `ProjectLintResult` with deterministic order
  (sort by path after parallel collect — parallel iteration order is
  nondeterministic; the final sort in spec-011 is preserved).
- `--jobs <N>` CLI flag (spec-062 adds) to cap rayon's thread pool; default =
  number of CPUs.
- Schema loading stays serial (one schema per project, shared `Arc`).

**Out of scope:**

- Parallel rule execution within a single document (rules share a walk;
  spec-011's design is single-walk-multiplexed — parallelizing would
  complicate handler borrowing for marginal gain).
- WASM (no threads; this spec's parallel path is cfg-gated off for WASM).

## Dependencies

- spec-011 (engine — modify its loop).
- spec-013 (cache — make thread-safe).
- spec-062 (`--jobs` flag).
- `rayon`, `dashmap` (or `parking_lot::Mutex`) deps.

## Deliverables

- Modified `crates/rglint-core/src/engine.rs`.
- Modified `crates/rglint-core/src/cache.rs` (thread-safe store).
- A benchmark (feeds into spec-065) showing ≥2× speedup on a 50-file project
  with 4+ cores.

## Interface / API

```rust
impl LintEngine {
    pub fn lint(&self, project: &Project) -> Result<ProjectLintResult> {
        let results: Vec<(PathBuf, Vec<Diagnostic>)> = project.documents.docs
            .par_iter()
            .map(|doc| self.lint_one(doc, project))
            .collect();
        // sort + assemble
    }
    pub fn set_thread_pool(&self, n: usize);  // or use rayon::ThreadPoolBuilder
}
```

## Behavior

- Determinism: the final `ProjectLintResult.all` is sorted by (file, line,
  column, rule_id) regardless of thread order — verified by a test that runs
  the same lint 10× and asserts identical output.
- Cache hits under contention never panic (`DashMap` or granular locking).
- A worker panic is caught and converted to an internal-error diagnostic
  (don't let one bad file abort the run).

## Testing

- Determinism test (above).
- Performance: spec-065 benches gate against regression.

## Risks / Notes

- `apollo_compiler` types: verify `Schema` and `ExecutableDocument` are
  `Sync` (they should be — they're ASTs behind `Arc`). If not, the parallel
  path shares clones rather than refs.
