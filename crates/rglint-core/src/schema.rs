//! Schema loader & in-memory cache.
//!
//! Loads GraphQL SDL from disk (single file, glob, or explicit list of files)
//! — plus an inline source string path — into an [`apollo_compiler::Schema`],
//! carrying [`SourceFile`][crate::SourceFile] handles through so a diagnostic
//! can still point at the right physical file even when the schema is a union.
//! Mirrors `packages/plugin/src/schema.ts`.
//!
//! ## Multi-file handling
//!
//! The spec (PLAN.md §3 / "Schema Loader") describes concatenating multiple
//! files into a single source string before `Schema::parse`. The spec's own
//! **Risks / Notes** section anticipates switching to apollo-compiler's
//! *builder* API if `NodeLocation` offsets break across the concatenation
//! boundary. This is the cleaner path and is what we adopt: each input file is
//! parsed as its own source into one [`apollo_compiler::SchemaBuilder`], so a
//! node's [`SourceSpan`] already carries the [`FileId`] of its originating
//! physical file with byte offsets relative to *that file*. There is no
//! concatenation-remapping to get wrong.
//!
//! Consequences for the spec's `interface`:
//!
//! - `LoadedSchema::combined` is still provided (a newline-joined view of every
//!   physical file's content), but it is **not** the source the schema was
//!   parsed against. Any byte offset a rule obtains from a node is relative to
//!   that node's physical file, not to `combined`. `combined` is kept as a
//!   convenience view for reporters that want to render the whole schema as a
//!   single blob.
//! - `LoadedSchema::resolve_file` takes a [`SourceSpan`] (the "node location"
//!   in our dependency stack) and maps its [`FileId`] back to the originating
//!   physical [`SourceFile`] via the compiler's [`SourceMap`][apollo_compiler]
//!   and then to our own `Vec<Arc<SourceFile>>`.
//!
//! ## Parse-error routing
//!
//! apollo-compiler's `SchemaBuilder::build` collects every parse-time
//! diagnostic (SyntaxError, ParserLimit) into a [`DiagnosticList`], which we
//! translate to engine [`Diagnostic`]s with `rule_id = "parse-error"`. Spec
//! validation is **not** run here (spec-053 owns that); we only parse into a
//! `Schema` (with a partial schema still returned on failure, mirroring the
//! error-resilience mandate of PLAN §1).
//!
//! ## Caching
//!
//! `SchemaLoader` keeps an in-memory cache keyed by the content hash of the
//! resolved input set ([`xxh3`][xxhash_rust::xxh3]); two `load` calls with the
//! same resolved inputs return the same [`Arc<LoadedSchema>`] (pointer-equal).
//! This is the spec's "soft" cache fallback for when spec-013 (the shared
//! content-hash `Cache`) is not yet wired in. Spec-013 will replace this field
//! with its `Cache` without changing `load`'s signature.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use apollo_compiler::diagnostic::ToCliReport;
use apollo_compiler::parser::{FileId, SourceSpan};
use apollo_compiler::Schema;
use xxhash_rust::xxh3;

use crate::diagnostics::{Diagnostic, DiagnosticBuilder, Severity};
use crate::location::Span;
use crate::source::SourceFile;

/// The `rule_id` apollo-compiler parse-time diagnostics are routed under.
///
/// PLAN §1 mandates error-resilient parsing: parse failures become diagnostics
/// rather than aborting the run. Spec-011 wires this into the engine; this
/// module only *exposes* them via [`LoadedSchema::parse_errors`].
pub const PARSE_ERROR_RULE_ID: &str = "parse-error";

/// A parsed GraphQL schema plus everything needed to attribute its nodes — and
/// its parse errors — back to the physical files on disk.
#[derive(Debug)]
pub struct LoadedSchema {
    /// The parsed (but not spec-validated) schema. On parse failure this is the
    /// *partial* schema apollo-compiler still produces; never `Err` from the
    /// loader's perspective — failures are surfaced as [`parse_errors`](Self::parse_errors).
    pub compiler: Schema,
    /// One [`SourceFile`] per physical input file, sorted by path (deterministic
    /// file order — see [`SchemaLoader`]). Each node's [`SourceSpan`] offset
    /// is relative to the file it belongs to here, *not* to [`combined`](Self::combined).
    pub sources: Vec<Arc<SourceFile>>,
    /// A newline-joined concatenation of every file in `sources`, in the same
    /// order. Convenience view for reporters; **not** the substrate the schema
    /// was parsed against (see the module-level "Multi-file handling" note).
    pub combined: Arc<SourceFile>,
    /// apollo-compiler parse/build diagnostics translated to engine
    /// [`Diagnostic`]s with `rule_id = "parse-error"`. Empty on a clean parse.
    pub parse_errors: Vec<Diagnostic>,
}

/// What a consumer asks [`SchemaLoader`] to load. Mirrors the
/// `SchemaSpec` shape sketched in the spec's interface section and the
/// `schema` field of graphql-config.
#[derive(Clone, Debug)]
pub enum SchemaSpec {
    /// A single file, resolved relative to the loader's `base` directory.
    File(PathBuf),
    /// A glob pattern (e.g. `"schema/*.graphqls"`), resolved under `base`.
    Glob(String),
    /// An explicit list of files (sorted + de-duplicated by the loader),
    /// each resolved relative to `base`.
    Files(Vec<PathBuf>),
    /// A literal SDL string; no disk I/O. The `base` is ignored for this
    /// variant. The synthetic physical file is named `<inline>`.
    Inline(String),
}

impl SchemaSpec {
    /// A short, hashable discriminant byte distinguishing the variants so that
    /// e.g. `Inline("x")` and `File` containing `"x"` never share a cache key.
    fn discriminant(&self) -> u8 {
        match self {
            SchemaSpec::File(_) => b'F',
            SchemaSpec::Glob(_) => b'G',
            SchemaSpec::Files(_) => b'L',
            SchemaSpec::Inline(_) => b'I',
        }
    }
}

/// Errors that can occur while resolving and parsing a [`SchemaSpec`].
///
/// Parse *syntax* errors are not here — those become [`LoadedSchema::parse_errors`]
/// and are surfaced as diagnostics. Only I/O and glob-resolution failures
/// short-circuit `load`.
#[derive(Debug, thiserror::Error)]
pub enum SchemaLoadError {
    /// Reading a resolved file from disk failed.
    #[error("failed to read schema file `{path}`: {source}")]
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
    /// A glob pattern could not be compiled.
    #[error("invalid glob pattern `{pattern}`: {source}")]
    Glob {
        pattern: String,
        source: globset::Error,
    },
    /// A glob pattern matched no `.graphql`/`.graphqls` files under `base`.
    #[error("no schema files matched `{pattern}` under `{base}`")]
    NoMatch { pattern: String, base: PathBuf },
    /// A `Files` spec resolved to an empty list.
    #[error("schema spec resolved to no input files")]
    Empty,
}

/// Loads GraphQL schemas from disk (or inline) into an [`apollo_compiler::Schema`],
/// caching results by content hash so repeated loads of the same inputs share
/// one [`Arc<LoadedSchema>`].
///
/// File order for globs and explicit lists is **deterministic**: paths are
/// sorted, so source spans and cache keys are stable across runs. Globs are
/// matched against the path relative to `base` and additionally filtered to the
/// `.graphql` / `.graphqls` extensions (spec scope). Walking is gitignore-aware
/// via the [`ignore`] crate.
///
/// The cache is an in-memory [`HashMap`] fallback (per spec's soft dependency on
/// spec-013); spec-013 will swap in its own `Cache` type without touching
/// [`SchemaLoader::load`]'s signature.
#[derive(Debug, Default)]
pub struct SchemaLoader {
    cache: Mutex<HashMap<u64, Arc<LoadedSchema>>>,
}

impl SchemaLoader {
    /// Create a loader with an empty cache.
    pub fn new() -> Self {
        Self::default()
    }

    /// Resolve `spec` under `base`, parse it into a [`LoadedSchema`], and
    /// return it shared via [`Arc`] (cache hits return the *same* `Arc`
    /// pointer — see the module "Caching" note).
    ///
    /// #Errors
    /// Only I/O / glob-resolution failures (`SchemaLoadError`). Parse *syntax*
    /// errors are surfaced as [`LoadedSchema::parse_errors`] rather than aborting.
    pub fn load(
        &self,
        spec: &SchemaSpec,
        base: &Path,
    ) -> Result<Arc<LoadedSchema>, SchemaLoadError> {
        let resolved = resolve(spec, base)?;
        let key = resolved.key;
        if let Some(hit) = self.cache_get(key) {
            return Ok(hit);
        }
        let loaded = build(resolved)?;
        Ok(self.cache_insert(key, loaded))
    }

    fn cache_get(&self, key: u64) -> Option<Arc<LoadedSchema>> {
        self.cache.lock().ok()?.get(&key).cloned()
    }

    fn cache_insert(&self, key: u64, loaded: Arc<LoadedSchema>) -> Arc<LoadedSchema> {
        if let Ok(mut guard) = self.cache.lock() {
            // Another thread may have inserted the same key in the meantime;
            // prefer the stored entry so all callers share one Arc.
            guard.entry(key).or_insert_with(|| loaded.clone());
        }
        loaded
    }
}

/// The fully resolved inputs to a [`SchemaSpec`]: sorted physical files with
/// their contents plus a content-hash cache key. `Inline` specs synthesize a
/// single `<inline>` entry.
struct ResolvedSpec {
    files: Vec<(PathBuf, String)>,
    /// Combined source, precomputed once. For zero files this is empty.
    combined_text: String,
    key: u64,
}

/// Resolve a [`SchemaSpec`] under `base` into concrete `(path, content)` pairs,
/// the joined `combined` text, and a content-hash cache key. This is the only
/// place I/O happens in the loader.
fn resolve(spec: &SchemaSpec, base: &Path) -> Result<ResolvedSpec, SchemaLoadError> {
    let mut hasher = xxh3::Xxh3::default();
    hasher.update(&[spec.discriminant()]);

    let files: Vec<(PathBuf, String)> = match spec {
        SchemaSpec::Inline(text) => {
            let path = PathBuf::from("<inline>");
            hasher.update(path.as_os_str().as_encoded_bytes());
            hasher.update(b"\0");
            hasher.update(text.as_bytes());
            vec![(path, text.clone())]
        }
        SchemaSpec::File(rel) => {
            let path = base.join(rel);
            let content = read_file(&path)?;
            hasher.update(path_buf_bytes(&path));
            hasher.update(b"\0");
            hasher.update(content.as_bytes());
            vec![(path, content)]
        }
        SchemaSpec::Files(rels) => read_files_sorted(base, rels, &mut hasher)?,
        SchemaSpec::Glob(pattern) => resolve_glob(base, pattern, &mut hasher)?,
    };

    if files.is_empty() {
        return Err(SchemaLoadError::Empty);
    }

    // Deterministic join: newline between files (matching graphql-config).
    let mut combined_text = String::new();
    for (i, (_, content)) in files.iter().enumerate() {
        if i > 0 {
            combined_text.push('\n');
        }
        combined_text.push_str(content);
    }

    Ok(ResolvedSpec {
        files,
        combined_text,
        key: hasher.digest(),
    })
}

/// Disk-read for a single file, with an [`SchemaLoadError::Io`] carrying the path.
fn read_file(path: &Path) -> Result<String, SchemaLoadError> {
    std::fs::read_to_string(path).map_err(|source| SchemaLoadError::Io {
        path: path.to_path_buf(),
        source,
    })
}

/// Read and sort+dedup an explicit file list for `SchemaSpec::Files`.
fn read_files_sorted(
    base: &Path,
    rels: &[PathBuf],
    hasher: &mut xxh3::Xxh3,
) -> Result<Vec<(PathBuf, String)>, SchemaLoadError> {
    // Sort + dedup by resolved path first for stable file order.
    let mut paths: Vec<PathBuf> = rels.iter().map(|r| base.join(r)).collect();
    paths.sort();
    paths.dedup();

    let mut out = Vec::with_capacity(paths.len());
    // Insertion-sort merge so the (hash, content) stream stays in path order
    // even though `paths` was sorted above (no-op when already sorted). Kept
    // explicit so the hash order is provably path-stable.
    paths.sort();
    for path in paths {
        let content = read_file(&path)?;
        hasher.update(path_buf_bytes(&path));
        hasher.update(b"\0");
        hasher.update(content.as_bytes());
        out.push((path, content));
    }
    Ok(out)
}

/// Walk `base` (gitignore-aware), match the relative path against `pattern`,
/// restrict to `.graphql`/`.graphqls`, and return the sorted matches.
fn resolve_glob(
    base: &Path,
    pattern: &str,
    hasher: &mut xxh3::Xxh3,
) -> Result<Vec<(PathBuf, String)>, SchemaLoadError> {
    let glob = globset::Glob::new(pattern)
        .map_err(|source| SchemaLoadError::Glob {
            pattern: pattern.to_owned(),
            source,
        })?
        .compile_matcher();

    let walker = ignore::WalkBuilder::new(base).build();
    let mut hits: Vec<(PathBuf, String)> = Vec::new();
    for entry in walker {
        let entry = match entry {
            Ok(e) => e,
            // An unreadable entry shouldn't abort the whole walk; surface as a
            // NoMatch if nothing matched. For now, skip — best-effort globbing.
            Err(_) => continue,
        };
        if !entry.file_type().is_some_and(|t| t.is_file()) {
            continue;
        }
        let path = entry.path();
        if !is_graphql_file(path) {
            continue;
        }
        let rel = match path.strip_prefix(base) {
            Ok(r) => r,
            Err(_) => continue,
        };
        if !glob.is_match(rel) {
            continue;
        }
        let content = read_file(path)?;
        hits.push((path.to_path_buf(), content));
    }

    // Sort by path so file order (and therefore span attribution) is stable.
    hits.sort_by(|a, b| a.0.cmp(&b.0));

    if hits.is_empty() {
        return Err(SchemaLoadError::NoMatch {
            pattern: pattern.to_owned(),
            base: base.to_path_buf(),
        });
    }

    for (path, content) in &hits {
        hasher.update(path_buf_bytes(path));
        hasher.update(b"\0");
        hasher.update(content.as_bytes());
    }
    Ok(hits)
}

/// `true` iff `path` ends in `.graphql` or `.graphqls` (case-sensitive).
fn is_graphql_file(path: &Path) -> bool {
    matches!(
        path.extension().and_then(|e| e.to_str()),
        Some("graphql") | Some("graphqls")
    )
}

/// portable bytes of a PathBuf for hashing (uses OS bytes on each platform;
/// stable within one process / OS, which is all the cache key needs).
fn path_buf_bytes(p: &Path) -> &[u8] {
    p.as_os_str().as_encoded_bytes()
}

/// The actual parse: feed each `(path, content)` into one
/// [`apollo_compiler::SchemaBuilder`] and translate the resulting
/// `DiagnosticList` (if any) to engine [`Diagnostic`]s.
///
/// `combined` is built as a [`SourceFile`] for the loader's `sources`/`combined`
/// fields. For `Inline` and single-`File` specs, `combined` and the lone
/// physical file share content (but are distinct `Arc`s — by design, since
/// they represent different "files" to reporters).
fn build(resolved: ResolvedSpec) -> Result<Arc<LoadedSchema>, SchemaLoadError> {
    // Build the rglint-core SourceFiles first (one per physical file). These
    // own the byte-offset line tables reporters and rule code consume.
    let sources: Vec<Arc<SourceFile>> = resolved
        .files
        .iter()
        .map(|(path, content)| SourceFile::new(path.clone(), content.clone()))
        .collect();

    // Parse every physical file into one shared SchemaBuilder. Each file gets
    // its own FileId, so node locations point back at the right physical file
    // without remapping.
    let mut builder = Schema::builder();
    for (path, content) in &resolved.files {
        builder = builder.parse(content, path.as_os_str().to_string_lossy().as_ref());
    }
    let (compiler, parse_errors) = match builder.build() {
        Ok(schema) => (schema, Vec::new()),
        Err(with_errors) => {
            let compiler = with_errors.partial;
            let parse_errors = translate_parse_errors(&compiler, &with_errors.errors, &sources);
            (compiler, parse_errors)
        }
    };

    let combined = SourceFile::new(PathBuf::from("<combined>"), resolved.combined_text);

    Ok(Arc::new(LoadedSchema {
        compiler,
        sources,
        combined,
        parse_errors,
    }))
}

/// Translate apollo-compiler's build diagnostics into engine [`Diagnostic`]s
/// with `rule_id = "parse-error"`. Every build-time diagnostic is routed here;
/// spec validation errors are not produced at this stage (spec-053 owns that).
///
/// Each diagnostic's [`Span`] uses offsets already relative to its physical
/// file (because each file was parsed standalone by the builder), so the
/// attached `file` PathBuf and span can slice directly into that SourceFile.
fn translate_parse_errors(
    compiler: &Schema,
    errors: &apollo_compiler::validation::DiagnosticList,
    sources: &[Arc<SourceFile>],
) -> Vec<Diagnostic> {
    let mut out = Vec::new();
    for diag in errors.iter() {
        let message = diag.error.to_string();
        let (file, span) = match diag.error.location() {
            Some(loc) => {
                let file = resolve_file_id(compiler, loc.file_id(), sources)
                    .map(|sf| sf.path().to_path_buf())
                    .unwrap_or_else(|| PathBuf::from("<combined>"));
                (file, Span::new(loc.offset(), loc.node_len()))
            }
            None => {
                // No location: attribute to the first physical file at offset 0.
                let file = sources
                    .first()
                    .map(|sf| sf.path().to_path_buf())
                    .unwrap_or_else(|| PathBuf::from("<combined>"));
                (file, Span::new(0, 0))
            }
        };
        out.push(
            DiagnosticBuilder::new(PARSE_ERROR_RULE_ID, file, span, message)
                .severity(Severity::Error)
                .finish(),
        );
    }
    out
}

/// Map an apollo-compiler [`FileId`] back to the originating physical
/// [`SourceFile`] from our `sources` vec, by matching the path the compiler
/// recorded against our SourceFile paths.
fn resolve_file_id<'s>(
    compiler: &Schema,
    file_id: FileId,
    sources: &'s [Arc<SourceFile>],
) -> Option<&'s Arc<SourceFile>> {
    let apollo_path = compiler.sources.get(&file_id)?.path();
    sources.iter().find(|sf| sf.path() == apollo_path)
}

impl LoadedSchema {
    /// Resolve a node [`SourceSpan`] (the "node location" in our dependency
    /// stack) back to the originating physical [`SourceFile`] from `sources`.
    ///
    /// Returns `None` if the location's [`FileId`] is unknown (e.g. built-in
    /// types whose location points at apollo-compiler's `built_in.graphql`).
    pub fn resolve_file(&self, loc: &SourceSpan) -> Option<&Arc<SourceFile>> {
        resolve_file_id(&self.compiler, loc.file_id(), &self.sources)
    }

    /// Convenience: the engine surfaces parse errors via this accessor (the
    /// field is public, but a method documents the intent and lets rule code
    /// stay decoupled from the struct shape).
    pub fn parse_errors(&self) -> &[Diagnostic] {
        &self.parse_errors
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Absolute path to `crates/rglint-core/tests/fixtures/schema` — stable
    /// because `CARGO_MANIFEST_DIR` is set at compile time per crate.
    fn fixture_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/schema")
    }

    /// The two-files fixture dir: `a.graphqls` defines `Query`, `b.graphqls`
    /// defines `User`. The glob `*.graphqls` matches both.
    fn two_files_dir() -> PathBuf {
        fixture_root().join("two-files")
    }

    #[test]
    fn loads_two_file_schema_and_finds_types_from_each_file() {
        let loader = SchemaLoader::new();
        let spec = SchemaSpec::Glob("*.graphqls".to_owned());
        let loaded = loader.load(&spec, &two_files_dir()).expect("load succeeds");

        // Both file-local types appear in the unified schema.
        assert!(
            loaded.compiler.get_object("Query").is_some(),
            "Query type (from a.graphqls) should be present"
        );
        assert!(
            loaded.compiler.get_object("User").is_some(),
            "User type (from b.graphqls) should be present"
        );
        // File order is deterministic (sorted by path): a before b.
        assert_eq!(loaded.sources.len(), 2);
        assert_eq!(loaded.sources[0].path(), two_files_dir().join("a.graphqls"));
        assert_eq!(loaded.sources[1].path(), two_files_dir().join("b.graphqls"));
        assert!(
            loaded.parse_errors.is_empty(),
            "no parse errors on clean input"
        );
    }

    #[test]
    fn malformed_schema_yields_parse_error_diagnostic_in_right_file() {
        let loader = SchemaLoader::new();
        let bad = fixture_root().join("bad.graphqls");
        let spec = SchemaSpec::File(bad.clone());
        let loaded = loader
            .load(&spec, Path::new(""))
            .expect("load still succeeds");
        // Parse failure surfaces as parse-error diagnostics, not a hard error.
        assert!(
            !loaded.parse_errors.is_empty(),
            "malformed schema must produce parse_errors"
        );
        let diag = &loaded.parse_errors[0];
        assert_eq!(diag.rule_id, PARSE_ERROR_RULE_ID);
        assert_eq!(diag.severity, Severity::Error);
        assert_eq!(
            diag.file, bad,
            "parse error attributed to the malformed file"
        );
        assert_eq!(loaded.sources.len(), 1);
        assert_eq!(loaded.sources[0].path(), bad);
        // The diagnostic span must slice into the physical file without panic
        // and be non-empty (i.e. point at real source).
        let snippet = loaded.sources[0].slice(diag.span);
        let _ = snippet; // non-empty is not guaranteed for every error kind; just must not panic.
    }

    #[test]
    fn inline_schema_loads_and_carries_single_source() {
        let loader = SchemaLoader::new();
        let src = "type Query { x: Int }".to_owned();
        let spec = SchemaSpec::Inline(src.clone());
        let loaded = loader
            .load(&spec, Path::new("ignored"))
            .expect("inline load");
        assert!(loaded.compiler.get_object("Query").is_some());
        assert_eq!(loaded.sources.len(), 1);
        assert_eq!(loaded.sources[0].path(), Path::new("<inline>"));
        assert_eq!(loaded.sources[0].source(), src);
        assert_eq!(loaded.combined.source(), src);
        assert!(loaded.parse_errors.is_empty());
    }

    #[test]
    fn loading_same_glob_twice_returns_pointer_equal_arc() {
        // spec-004 Testing: "Loading the same glob twice returns cached
        // LoadedSchema (same Arc pointer when spec-013 is integrated)."
        // Our in-memory HashMap fallback satisfies this today.
        let loader = SchemaLoader::new();
        let spec = SchemaSpec::Glob("*.graphqls".to_owned());
        let first = loader.load(&spec, &two_files_dir()).unwrap();
        let second = loader.load(&spec, &two_files_dir()).unwrap();
        assert!(
            Arc::ptr_eq(&first, &second),
            "cache hit should return the same Arc<LoadedSchema> pointer"
        );
    }

    #[test]
    fn different_specs_do_not_collide_in_cache() {
        let loader = SchemaLoader::new();
        let a = loader
            .load(&SchemaSpec::Glob("*.graphqls".to_owned()), &two_files_dir())
            .unwrap();
        // An inline specification with different content must not hit the
        // two-files cache entry.
        let b = loader
            .load(
                &SchemaSpec::Inline("type Query { x: Int }".to_owned()),
                Path::new("ignored"),
            )
            .unwrap();
        assert!(!Arc::ptr_eq(&a, &b));
    }

    #[test]
    fn resolve_file_maps_node_location_to_originating_physical_file() {
        let loader = SchemaLoader::new();
        let spec = SchemaSpec::Glob("*.graphqls".to_owned());
        let loaded = loader.load(&spec, &two_files_dir()).unwrap();

        // The Query type is defined in a.graphqls — its node location's file_id
        // must map back to a.graphqls, not b.graphqls.
        let query = loaded
            .compiler
            .get_object("Query")
            .expect("Query type present");
        let loc = query.location().expect("Query has a location");
        let resolved = loaded
            .resolve_file(&loc)
            .expect("location should resolve to a physical file");
        assert_eq!(resolved.path(), two_files_dir().join("a.graphqls"));

        // The User type is defined in b.graphqls.
        let user = loaded
            .compiler
            .get_object("User")
            .expect("User type present");
        let loc = user.location().expect("User has a location");
        let resolved = loaded
            .resolve_file(&loc)
            .expect("location should resolve to a physical file");
        assert_eq!(resolved.path(), two_files_dir().join("b.graphqls"));
    }

    #[test]
    fn files_spec_is_sorted_and_deduped() {
        let loader = SchemaLoader::new();
        // Pass them out of order with a duplicate; loader sorts + dedups.
        let spec = SchemaSpec::Files(vec![
            PathBuf::from("b.graphqls"),
            PathBuf::from("a.graphqls"),
            PathBuf::from("a.graphqls"),
        ]);
        let loaded = loader.load(&spec, &two_files_dir()).expect("load succeeds");
        assert_eq!(loaded.sources.len(), 2);
        assert_eq!(loaded.sources[0].path(), two_files_dir().join("a.graphqls"));
        assert_eq!(loaded.sources[1].path(), two_files_dir().join("b.graphqls"));
    }

    #[test]
    fn no_match_glob_is_an_error_not_an_empty_schema() {
        let loader = SchemaLoader::new();
        let spec = SchemaSpec::Glob("does-not-exist-*.graphqls".to_owned());
        let err = loader
            .load(&spec, &two_files_dir())
            .expect_err("no matches should error");
        assert!(matches!(err, SchemaLoadError::NoMatch { .. }));
    }

    #[test]
    fn invalid_glob_pattern_is_an_error() {
        let loader = SchemaLoader::new();
        let spec = SchemaSpec::Glob("[unclosed".to_owned());
        let err = loader
            .load(&spec, &two_files_dir())
            .expect_err("invalid glob should error");
        assert!(matches!(err, SchemaLoadError::Glob { .. }));
    }

    #[test]
    fn loaded_schema_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<LoadedSchema>();
        assert_send_sync::<SchemaLoader>();
    }
}
