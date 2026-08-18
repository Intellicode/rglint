//! Content-hash cache for incremental lint runs (spec-013).
//!
//! Caches the prior [`Diagnostic`] set for a `(file_path, content_hash)` key so
//! unchanged files skip re-linting on incremental runs (Phase 9 wiring lands in
//! spec-064). Mirrors `packages/plugin/src/cache.ts` from graphql-eslint.
//!
//! v1 caches **diagnostics only**; re-parsing on a hit is wasted but cheap
//! (see `ARCHITECTURE.md`). The store is either in-memory (tests) or a single
//! file at `<target-dir>/rglint-cache.bin` serialized under a magic header +
//! version byte. A missing/corrupt/version-mismatched cache file is never a
//! hard error — caching is a perf optimization, so [`Cache::load`] degrades to
//! an empty memory cache and emits a `tracing` warn.
//!
//! ## Serialization format
//!
//! The spec suggested `bincode` (compact), but [`Diagnostic::data`] is a
//! `serde_json::Value`, which drives the serde Deserializer via
//! `deserialize_any` — unsupported by bincode (and postcard). We therefore
//! serialize the entry map as **JSON** after the magic+version header. JSON is
//! self-describing, round-trips `serde_json::Value` natively, and reuses the
//! existing `serde_json` dep (no new crate). The size overhead vs bincode is
//! acceptable: cache files are small and `flush` is rare (once per run).

use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, RwLock};

use ahash::AHashMap;
use serde::{Deserialize, Serialize};

use crate::diagnostics::Diagnostic;

/// Magic header (`b"rglint"` followed by version `1`).
const MAGIC: &[u8] = b"rglint";
const VERSION: u8 = 1;

/// Lookup key: absolute/canonical file path plus an xxh3-64 content hash.
///
/// Two files with identical content but different paths are distinct keys (no
/// false hits); two versions of the same path with different content hash to
/// different keys (a miss, so the file re-lints).
#[derive(Clone, Debug, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub struct CacheKey {
    /// Path the cache entry is keyed on. Stored verbatim (no canonicalization)
    /// so callers control path normalization.
    pub path: PathBuf,
    /// xxh3-64 hash of the file content (see [`Cache::hash`]).
    pub hash: u64,
}

/// The cached value: the prior [`Diagnostic`] set for a key.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CachedResult {
    /// Diagnostics produced on the prior run for this key's file.
    pub diagnostics: Vec<Diagnostic>,
}

/// Backing store for a [`Cache`]: either purely in-memory (tests, or when no
/// on-disk path is configured) or backed by a file that [`Cache::flush`] writes
/// atomically. The map is behind a read/write lock because lint workers share
/// one cache while they process files in parallel.
#[derive(Debug)]
enum CacheStore {
    Memory {
        mem: RwLock<AHashMap<CacheKey, CachedResult>>,
    },
    File {
        path: PathBuf,
        mem: RwLock<AHashMap<CacheKey, CachedResult>>,
    },
}

/// Content-hash cache. See the module docs for the v1 tradeoffs.
///
/// Construct with [`Cache::memory`] (no persistence) or [`Cache::load`] (load
/// from disk, degrading to empty on any error). Both modes are live stores;
/// `memory` simply skips persistence. Insert entries during a run and call
/// [`Cache::flush`] at the end to persist a file-backed store.
#[derive(Debug)]
pub struct Cache {
    store: CacheStore,
    flush_lock: Mutex<()>,
}

impl Cache {
    /// New in-memory cache (never persisted). Useful for tests and for callers
    /// that want contention-safe caching without a cache file.
    pub fn memory() -> Self {
        Self {
            store: CacheStore::Memory {
                mem: RwLock::new(AHashMap::new()),
            },
            flush_lock: Mutex::new(()),
        }
    }

    /// Load a cache from `path`. **Never fails**: a missing, corrupt, or
    /// version-mismatched file yields an empty in-memory cache with a
    /// `tracing::warn!` (caching is a perf optimization, not correctness).
    ///
    /// The returned cache remembers `path` so a subsequent [`Cache::flush`]
    /// writes back to the same file.
    pub fn load(path: &Path) -> Self {
        match Self::read(path) {
            Ok(mem) => Self {
                store: CacheStore::File {
                    path: path.to_path_buf(),
                    mem: RwLock::new(mem),
                },
                flush_lock: Mutex::new(()),
            },
            Err(e) => {
                tracing::warn!(%e, path = %path.display(), "rglint cache: ignoring unreadable cache file");
                Self {
                    store: CacheStore::File {
                        path: path.to_path_buf(),
                        mem: RwLock::new(AHashMap::new()),
                    },
                    flush_lock: Mutex::new(()),
                }
            }
        }
    }

    fn read(path: &Path) -> io::Result<AHashMap<CacheKey, CachedResult>> {
        let bytes = fs::read(path)?;
        decode(&bytes).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
    }

    /// Look up a prior result for `key`. Returns `None` on a miss.
    pub fn get(&self, key: &CacheKey) -> Option<CachedResult> {
        match &self.store {
            CacheStore::Memory { mem } | CacheStore::File { mem, .. } => {
                read_lock(mem).get(key).cloned()
            }
        }
    }

    /// Insert (or replace) the cached result for `key`.
    pub fn insert(&self, key: CacheKey, result: CachedResult) {
        match &self.store {
            CacheStore::Memory { mem } | CacheStore::File { mem, .. } => {
                write_lock(mem).insert(key, result);
            }
        }
    }

    /// Number of cached entries.
    pub fn len(&self) -> usize {
        match &self.store {
            CacheStore::Memory { mem } | CacheStore::File { mem, .. } => read_lock(mem).len(),
        }
    }

    /// Whether the cache holds no entries.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Snapshot the entries. Order is unspecified; the snapshot releases the
    /// cache lock before the caller iterates.
    pub fn iter(&self) -> CacheIter {
        let entries = match &self.store {
            CacheStore::Memory { mem } | CacheStore::File { mem, .. } => {
                let entries: Vec<_> = read_lock(mem)
                    .iter()
                    .map(|(key, result)| (key.clone(), result.clone()))
                    .collect();
                entries.into_iter()
            }
        };
        CacheIter { inner: entries }
    }

    /// Persist the cache to disk. Atomic: writes `<path>.tmp` then renames
    /// over the target. No-op (returns `Ok(())`) for in-memory caches.
    pub fn flush(&self) -> io::Result<()> {
        let (path, mem) = match &self.store {
            CacheStore::Memory { .. } => return Ok(()),
            CacheStore::File { path, mem } => (path, mem),
        };
        let _flush_guard = mutex_lock(&self.flush_lock);
        let snapshot = read_lock(mem).clone();
        let bytes = encode(&snapshot);
        let mut tmp = path.to_path_buf();
        tmp.set_extension("tmp");
        {
            let mut f = fs::File::create(&tmp)?;
            f.write_all(&bytes)?;
            f.sync_all().unwrap_or(());
        }
        fs::rename(&tmp, path)?;
        Ok(())
    }

    /// xxh3-64 content hash. Used by callers to build [`CacheKey`]s.
    pub fn hash(content: &[u8]) -> u64 {
        xxhash_rust::xxh3::xxh3_64(content)
    }
}

/// Snapshot iterator over a [`Cache`]'s entries.
pub struct CacheIter {
    inner: std::vec::IntoIter<(CacheKey, CachedResult)>,
}

impl Iterator for CacheIter {
    type Item = (CacheKey, CachedResult);

    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next()
    }
}

impl Default for Cache {
    fn default() -> Self {
        Self::memory()
    }
}

/// Serialize the entries with magic header + version + JSON payload (see
/// module docs for why JSON over bincode, and why a `Vec` of entries rather
/// than a `Map`: JSON object keys must be strings, so a struct key cannot be a
/// JSON map key — we store an array of `(key, value)` pairs instead).
fn encode(mem: &AHashMap<CacheKey, CachedResult>) -> Vec<u8> {
    let mut out = Vec::with_capacity(mem.len() * 64 + 8);
    out.extend_from_slice(MAGIC);
    out.push(VERSION);
    let entries: Vec<(&CacheKey, &CachedResult)> = mem.iter().collect();
    if let Ok(payload) = serde_json::to_vec(&entries) {
        out.extend_from_slice(&payload);
    }
    out
}

/// Decode a previously-encoded payload. Errors on bad magic, version
/// mismatch, or corrupt JSON — all of which [`Cache::load`] turns into an
/// empty cache.
fn decode(bytes: &[u8]) -> Result<AHashMap<CacheKey, CachedResult>, String> {
    if bytes.len() < MAGIC.len() + 1 {
        return Err("truncated header".to_owned());
    }
    let (magic, rest) = bytes.split_at(MAGIC.len());
    if magic != MAGIC {
        return Err("bad magic".to_owned());
    }
    let (&version, payload) = rest
        .split_first()
        .ok_or_else(|| "missing version".to_owned())?;
    if version != VERSION {
        return Err(format!("version mismatch: got {version}, want {VERSION}"));
    }
    if payload.is_empty() {
        return Ok(AHashMap::new());
    }
    let entries: Vec<(CacheKey, CachedResult)> =
        serde_json::from_slice(payload).map_err(|e| e.to_string())?;
    Ok(entries.into_iter().collect())
}

fn read_lock<T>(lock: &RwLock<T>) -> std::sync::RwLockReadGuard<'_, T> {
    lock.read()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

fn write_lock<T>(lock: &RwLock<T>) -> std::sync::RwLockWriteGuard<'_, T> {
    lock.write()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

fn mutex_lock<T>(lock: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    lock.lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::tempdir;

    use super::*;
    use crate::{location::Span, DiagnosticBuilder};

    fn diag(msg: &str) -> Diagnostic {
        DiagnosticBuilder::new("test", PathBuf::from("a.graphql"), Span::new(0, 0), msg).finish()
    }

    #[test]
    fn insert_then_get_round_trip() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("rglint-cache.bin");
        {
            let cache = Cache::load(&path);
            assert_eq!(cache.len(), 0);

            let k1 = CacheKey {
                path: PathBuf::from("a.graphql"),
                hash: 1,
            };
            let k2 = CacheKey {
                path: PathBuf::from("b.graphql"),
                hash: 2,
            };
            let k3 = CacheKey {
                path: PathBuf::from("c.graphql"),
                hash: 3,
            };
            cache.insert(
                k1.clone(),
                CachedResult {
                    diagnostics: vec![diag("m1")],
                },
            );
            cache.insert(
                k2.clone(),
                CachedResult {
                    diagnostics: vec![diag("m2")],
                },
            );
            cache.insert(
                k3.clone(),
                CachedResult {
                    diagnostics: vec![diag("m3")],
                },
            );
            assert_eq!(cache.len(), 3);
            cache.flush().unwrap();
        }
        {
            let cache = Cache::load(&path);
            assert_eq!(cache.len(), 3);
            let k = CacheKey {
                path: PathBuf::from("b.graphql"),
                hash: 2,
            };
            let got = cache.get(&k).expect("hit");
            assert_eq!(got.diagnostics.len(), 1);
            assert_eq!(got.diagnostics[0].message, "m2");
        }
    }

    #[test]
    fn corrupt_file_recovers_empty() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("rglint-cache.bin");
        fs::write(&path, b"not a cache at all").unwrap();
        let cache = Cache::load(&path);
        assert_eq!(cache.len(), 0);
        // flushing an empty cache should not panic
        cache.flush().unwrap();
    }

    #[test]
    fn truncated_file_recovers_empty() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("rglint-cache.bin");
        // Just the magic + version, no bincode payload.
        fs::write(&path, [MAGIC, &[VERSION]].concat()).unwrap();
        let cache = Cache::load(&path);
        assert_eq!(cache.len(), 0);
    }

    #[test]
    fn version_mismatch_recovers_empty() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("rglint-cache.bin");
        fs::write(&path, [MAGIC, &[99u8]].concat()).unwrap();
        let cache = Cache::load(&path);
        assert_eq!(cache.len(), 0);
    }

    #[test]
    fn missing_file_recovers_empty() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("does-not-exist.bin");
        let cache = Cache::load(&path);
        assert_eq!(cache.len(), 0);
    }

    #[test]
    fn same_content_different_path_no_false_hit() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("rglint-cache.bin");
        let content = b"type Query { x: Int }";
        let h = Cache::hash(content);
        {
            let cache = Cache::load(&path);
            cache.insert(
                CacheKey {
                    path: PathBuf::from("a.graphql"),
                    hash: h,
                },
                CachedResult {
                    diagnostics: vec![diag("a")],
                },
            );
            cache.flush().unwrap();
        }
        let cache = Cache::load(&path);
        // Same content hash, different path -> miss.
        let miss = cache.get(&CacheKey {
            path: PathBuf::from("b.graphql"),
            hash: h,
        });
        assert!(miss.is_none());
    }

    #[test]
    fn different_content_same_path_miss() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("rglint-cache.bin");
        {
            let cache = Cache::load(&path);
            cache.insert(
                CacheKey {
                    path: PathBuf::from("a.graphql"),
                    hash: Cache::hash(b"v1"),
                },
                CachedResult {
                    diagnostics: vec![diag("v1")],
                },
            );
            cache.flush().unwrap();
        }
        let cache = Cache::load(&path);
        // Same path, different content hash -> miss.
        let miss = cache.get(&CacheKey {
            path: PathBuf::from("a.graphql"),
            hash: Cache::hash(b"v2"),
        });
        assert!(miss.is_none());
    }

    #[test]
    fn memory_cache_stores_entries() {
        let cache = Cache::memory();
        let k = CacheKey {
            path: PathBuf::from("a.graphql"),
            hash: 1,
        };
        cache.insert(
            k.clone(),
            CachedResult {
                diagnostics: vec![diag("x")],
            },
        );
        assert_eq!(cache.get(&k).unwrap().diagnostics[0].message, "x");
        assert_eq!(cache.len(), 1);
        // flush on a memory cache is a no-op.
        cache.flush().unwrap();
    }

    #[test]
    fn hash_is_xxh3_64() {
        // xxh3_64 of empty input is a fixed, well-known constant:
        assert_eq!(Cache::hash(b""), 0x2d06800538d394c2);
    }

    #[test]
    fn concurrent_reads_and_writes_do_not_panic() {
        use std::sync::Arc;
        use std::thread;

        let cache = Arc::new(Cache::memory());
        let mut workers = Vec::new();
        for i in 0..8 {
            let cache = Arc::clone(&cache);
            workers.push(thread::spawn(move || {
                for j in 0..100 {
                    let key = CacheKey {
                        path: PathBuf::from(format!("{i}-{j}.graphql")),
                        hash: j,
                    };
                    cache.insert(
                        key.clone(),
                        CachedResult {
                            diagnostics: vec![diag("concurrent")],
                        },
                    );
                    assert!(cache.get(&key).is_some());
                }
            }));
        }
        for worker in workers {
            worker.join().expect("cache worker must not panic");
        }
        assert_eq!(cache.len(), 800);
    }
}
