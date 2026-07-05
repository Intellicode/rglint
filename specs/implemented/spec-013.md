# Spec-013: Content-hash cache

> Plan reference: §3 (`crates/rglint-core/src/cache.rs`), §1 (caching), §2 (`xxhash-rust`), §5 Phase 9

## Goal

Implement a content-hash cache so unchanged files skip re-parsing and
re-linting on incremental runs. Mirrors `packages/plugin/src/cache.ts`. Built
early (Phase 0) because the loaders (specs 004/005) reference it, even though
the incremental-run wiring lands in Phase 9 (spec-064).

## Scope

**In scope:**

- `Cache` struct: persisted key/value store mapping
  `(file_path, content_hash) -> CachedResult` where `CachedResult` holds the
  prior `Vec<Diagnostic>` (and optionally a serialized `LoadedSchema`/`Siblings`
  handle, but v1 caches only **diagnostics** — re-parsing is cheap enough).
- `xxhash-rust::xxh3` for hashing file content.
- On-disk persistence at `<target-dir>/rglint-cache.bin` (postcard or bincode
  serialization). In-memory-only mode for tests.
- `Cache::get(path, hash) -> Option<CachedResult>`.
- `Cache::insert(path, hash, result)`.
- `Cache::flush()` writes to disk; `Cache::load(path) -> Result<Self>`.
- Cache invalidation: a missing or corrupt cache file is treated as empty
  (never a hard error — caching is a perf optimization).

**Out of scope:**

- Parallel-aware locking (spec-064).
- LRU eviction (v1 unbounded; project caches are small).

## Dependencies

- spec-001 (workspace + `xxhash-rust` dep).
- spec-003 (Diagnostic — the cached value).

## Deliverables

- `crates/rglint-core/src/cache.rs`.
- Unit + property tests: insert then get round-trips; corrupt-file recovery.

## Interface / API

```rust
pub struct CacheKey { pub path: PathBuf, pub hash: u64 }
pub struct CachedResult { pub diagnostics: Vec<Diagnostic> }

pub enum CacheStore { Memory(AHashMap<CacheKey, CachedResult>), File { path: PathBuf, mem: AHashMap<CacheKey, CachedResult> } }

pub struct Cache { store: CacheStore }
impl Cache {
    pub fn memory() -> Self;
    pub fn load(path: &Path) -> Self;          // never fails; empty on error
    pub fn get(&self, key: &CacheKey) -> Option<&CachedResult>;
    pub fn insert(&mut self, key: CacheKey, result: CachedResult);
    pub fn flush(&self) -> Result<()>;
    pub fn hash(content: &[u8]) -> u64;
}
```

## Behavior

- `hash` uses `xxh3::xxh3_64`.
- File serialization uses `bincode` (compact) with a magic header + version
  byte; version mismatch → treat as empty (no downgrade path v1).
- `flush` is atomic: write to `*.tmp` then rename.
- `load` swallows IO/decode errors and returns an empty memory cache (with a
  `tracing` warn).

## Testing

- Insert 3 entries, `flush`, drop, `load`, assert all 3 retrievable.
- Corrupt the file (truncate) → `load` returns empty cache, no panic.
- Same content different path → different key (no false hits).
- Different content same path → hash differs → miss.

## Risks / Notes

- v1 caches diagnostics only. Re-parsing on a cache hit is wasted work but
  acceptable; revisit if profiling (spec-065) shows parse dominates. Document
  this tradeoff in `ARCHITECTURE.md`.
