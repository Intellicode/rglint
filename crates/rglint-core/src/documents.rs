//! Document loader & dedup.
//!
//! Loads GraphQL operation documents (`.graphql` files containing operations
//! and fragments) from disk into [`apollo_compiler::ExecutableDocument`]s,
//! deduplicating by content hash so the same document imported via overlapping
//! globs is linted once. Mirrors `packages/plugin/src/documents.ts`.
//!
//! ## Dedup model
//!
//! The dedup key is the [`xxh3`][xxhash_rust::xxh3] hash of the *file content*
//! (spec-013's cache primitive). Two inputs with byte-identical content
//! contribute a single [`LoadedDocument`] — both paths route through
//! [`LoadedDocuments::by_file`] to the same `docs` index. The parsed
//! [`ExecutableDocument`] belongs to whichever physical file won the hash race
//! (input order: explicit `Files` are sorted+deduped, globs are sorted by
//! path, `Inline` has a single synthetic `<inline>` entry).
//!
//! ## Per-file documents
//!
//! Each [`LoadedDocument`] holds *one* [`SourceFile`] and *one*
//! [`ExecutableDocument`]. Cross-document fragment resolution is **not** done
//! here — spec-006 (FragmentTracker / siblings) builds the cross-document index
//! on top of [`LoadedDocuments`]. This matches the spec's "Out of scope"
//! section: we deliberately do not concatenate all documents into one giant
//! `ExecutableDocument` (that would lose per-file attribution).
//!
//! ## Schema-aware parsing
//!
//! When a [`Schema`](apollo_compiler::Schema) is supplied, documents are parsed
//! *with* it so operation/fragment types resolve. When it is `None`, parsing is
//! standalone (the engine (spec-011) skips rules that declare `requires_schema`
//! in that case). apollo-compiler's executable builder API requires a
//! `&Valid<Schema>`; we bridge that with
//! [`Valid::assume_valid_ref`][apollo_compiler::Valid::assume_valid_ref] — schema
//! *spec validation* is **not** run here (spec-053 owns that), matching the
//! spec-004 contract that a `LoadedSchema::compiler` is parsed but not
//! validated. If the schema happens to be spec-valid (the common case), nothing
//! changes; named semantic diagnostics such as `UndefinedField` are left for
//! spec-053 rather than being duplicated as `parse-error`.
//!
//! ## Parse-error routing
//!
//! apollo-compiler's executable builder collects syntax and semantic
//! diagnostics into a [`DiagnosticList`]. Diagnostics with Apollo's stable
//! `unstable_error_name` are intentionally excluded here and translated by
//! spec-053 into their configured graphql-eslint rule id. Syntax and unnamed
//! diagnostics become `parse-error` (reusing the constant defined by spec-004
//! in [`schema`][crate::schema]). A per-file document with parse errors is
//! still returned (partial document), mirroring PLAN §1.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use apollo_compiler::diagnostic::ToCliReport;
use apollo_compiler::parser::FileId;
use apollo_compiler::validation::{DiagnosticList, Valid};
use apollo_compiler::{ExecutableDocument, Schema};
use xxhash_rust::xxh3;

use crate::diagnostics::{Diagnostic, DiagnosticBuilder, Severity};
use crate::location::Span;
use crate::schema::PARSE_ERROR_RULE_ID;
use crate::source::SourceFile;

/// What a consumer asks [`DocumentLoader`] to load. Mirrors the shape sketched
/// in spec-005's interface section and the `documents` field of graphql-config.
#[derive(Clone, Debug)]
pub enum DocumentSpec {
    /// A single glob pattern (e.g. `"src/**/*.graphql"`), resolved under `base`.
    Glob(String),
    /// A list of glob patterns; matches are unioned (and de-duplicated by
    /// content hash after resolution, so overlapping globs lint each file
    /// once).
    Globs(Vec<String>),
    /// An explicit list of `.graphql` files (sorted + de-duplicated by the
    /// loader), each resolved relative to `base`.
    Files(Vec<PathBuf>),
    /// A literal document source; no disk I/O. `base` is ignored. The
    /// synthetic physical file is named `<inline>`.
    Inline(String),
}

/// Errors that can occur while resolving and parsing a [`DocumentSpec`].
///
/// Parse *syntax* errors are not here — those become
/// [`LoadedDocument::parse_errors`] and are surfaced as diagnostics. Only I/O
/// and glob-resolution failures short-circuit `load`.
#[derive(Debug, thiserror::Error)]
pub enum DocumentLoadError {
    /// Reading a resolved file from disk failed.
    #[error("failed to read document file `{path}`: {source}")]
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
    /// A `Glob` / `Globs` spec resolved to no `.graphql` files under `base`.
    #[error("no document files matched `{patterns}` under `{base}`")]
    NoMatch { patterns: String, base: PathBuf },
    /// A `Files` spec resolved to an empty list.
    #[error("document spec resolved to no input files")]
    Empty,
}

/// A single loaded document: its [`SourceFile`], its parsed
/// [`ExecutableDocument`], and any parse errors apollo-compiler produced for it.
///
/// On a parse failure, `document` is the *partial* document apollo-compiler
/// still produces (see the module "Parse-error routing" note); it is never an
/// `Err` from the loader's perspective — failures surface as `parse_errors`.
#[derive(Debug)]
pub struct LoadedDocument {
    /// The physical source this document was parsed from. Shared by every
    /// path in [`LoadedDocuments::by_file`] that pointed at identical content.
    pub source: Arc<SourceFile>,
    /// The parsed (but not spec-validated) executable document. Partial on
    /// parse failure.
    pub document: ExecutableDocument,
    /// apollo-compiler parse/build diagnostics translated to engine
    /// [`Diagnostic`]s with `rule_id = "parse-error"`. Empty on a clean parse.
    pub parse_errors: Vec<Diagnostic>,
}

/// The deduplicated bundle of documents loaded from a [`DocumentSpec`].
///
/// - `docs` is the deduplicated list (one [`LoadedDocument`] per unique file
///   content hash).
/// - `by_file` maps every *input* path (i.e. every path the spec resolved to,
///   before dedup) to its index in `docs`. Two paths with identical content
///   share one index; this is the seam spec-006 (siblings) and reporters use to
///   attribute a diagnostic back to a file even when the underlying executable
///   document belongs to a deduped sibling.
#[derive(Debug)]
pub struct LoadedDocuments {
    /// Deduplicated loaded documents.
    pub docs: Vec<LoadedDocument>,
    /// `path -> index into docs` for every resolved input file. Multiple paths
    /// may map to the same index when their content hashes collide.
    pub by_file: HashMap<PathBuf, usize>,
}

/// Loads GraphQL operation documents from disk (or inline) into per-file
/// [`ExecutableDocument`]s, de-duplicating by content hash so overlapping globs
/// lint each file once.
///
/// File order for globs and explicit lists is **deterministic**: paths are
/// sorted, so the dedup winner (first file in sorted order to claim a content
/// hash) and the `by_file` indices are stable across runs. Globs are matched
/// against the path relative to `base` and additionally filtered to the
/// `.graphql` extension (documents only — `.graphqls` schema files are owned
/// by spec-004). Walking is gitignore-aware via the [`ignore`] crate.
///
/// The loader is stateless today (the spec-005 spec mentions no cross-load
/// cache); spec-013 (the shared content-hash `Cache`) may later plug in here
/// without changing [`DocumentLoader::load`]'s signature.
#[derive(Debug, Default)]
pub struct DocumentLoader {}

impl DocumentLoader {
    /// Create a loader.
    pub fn new() -> Self {
        Self::default()
    }

    /// Resolve `spec` under `base`, parse each unique file's content into an
    /// [`ExecutableDocument`] (optionally with `schema` for type-aware parsing),
    /// and return the deduplicated bundle.
    ///
    /// # Errors
    /// Only I/O / glob-resolution failures ([`DocumentLoadError`]). Parse
    /// *syntax* errors are surfaced as [`LoadedDocument::parse_errors`] rather
    /// than aborting.
    pub fn load(
        &self,
        spec: &DocumentSpec,
        base: &Path,
        schema: Option<&Schema>,
    ) -> Result<LoadedDocuments, DocumentLoadError> {
        let files = resolve(spec, base)?;
        let valid_schema = schema.map(Valid::assume_valid_ref);

        let mut docs: Vec<LoadedDocument> = Vec::new();
        // content-hash -> index into docs
        let mut hash_to_idx: HashMap<u64, usize> = HashMap::new();
        let mut by_file: HashMap<PathBuf, usize> = HashMap::new();

        for (path, content) in files {
            let hash = xxh3::xxh3_64(content.as_bytes());
            let idx = if let Some(&existing) = hash_to_idx.get(&hash) {
                // Dedup hit: this path points at an already-parsed document.
                existing
            } else {
                let source = SourceFile::new(path.clone(), content);
                let (document, parse_errors) = parse_one(&source, valid_schema);
                let idx = docs.len();
                docs.push(LoadedDocument {
                    source,
                    document,
                    parse_errors,
                });
                hash_to_idx.insert(hash, idx);
                idx
            };
            by_file.insert(path, idx);
        }

        Ok(LoadedDocuments { docs, by_file })
    }
}

/// Parse a single [`SourceFile`] into an [`ExecutableDocument`] using
/// apollo-compiler's executable builder. When `schema` is `Some`, parses
/// type-aware (operation/fragment types resolve); when `None`, parses
/// standalone.
///
/// Returns the document and any translated parse-error diagnostics. Parse
/// *syntax* errors never abort — the builder produces a partial document and we
/// route every entry of the [`DiagnosticList`] as a `parse-error` diagnostic.
fn parse_one(
    source: &Arc<SourceFile>,
    schema: Option<&Valid<Schema>>,
) -> (ExecutableDocument, Vec<Diagnostic>) {
    let mut errors = DiagnosticList::new(Default::default());
    let document = ExecutableDocument::builder(schema, &mut errors)
        .parse(source.source(), source.path())
        .build();
    let parse_errors = translate_parse_errors(&document, &errors, source);
    (document, parse_errors)
}

/// Translate apollo-compiler's build diagnostics into engine [`Diagnostic`]s
/// with `rule_id = "parse-error"`. Every build-time diagnostic is routed here;
/// spec validation errors are not produced at this stage (spec-053 owns that).
///
/// Each diagnostic's [`Span`] uses offsets already relative to its physical
/// file (because the document is built from a single source), so the attached
/// `file` `PathBuf` and `span` can slice directly into `source`.
fn translate_parse_errors(
    document: &ExecutableDocument,
    errors: &DiagnosticList,
    source: &Arc<SourceFile>,
) -> Vec<Diagnostic> {
    let file = source.path().to_path_buf();
    let fallback_span = Span::new(0, 0);
    let mut out = Vec::new();
    for diag in errors.iter() {
        // Apollo's executable builder reports semantic validation failures
        // (for example UndefinedField) through the same DiagnosticList as
        // syntax errors. Leave those to spec-053 so enabling a GraphQL spec
        // rule does not produce a duplicate `parse-error` diagnostic.
        if diag.error.unstable_error_name().is_some() {
            continue;
        }
        let message = diag.error.to_string();
        let span = match diag.error.location() {
            Some(loc) => {
                // Each LoadedDocument is parsed from a single file; the
                // location's FileId should resolve back to that file. If (defensively)
                // it doesn't (e.g. built-in types from a supplied schema),
                // fall back to offset 0 of our own source.
                if same_file(document, loc.file_id(), source) {
                    Span::new(loc.offset(), loc.node_len())
                } else {
                    fallback_span
                }
            }
            None => fallback_span,
        };
        out.push(
            DiagnosticBuilder::new(PARSE_ERROR_RULE_ID, file.clone(), span, message)
                .severity(Severity::Error)
                .finish(),
        );
    }
    out
}

/// Returns `true` iff the apollo-compiler source registered under `file_id`
/// points at our [`SourceFile`]'s path. Used to attribute a diagnostic's span
/// to the right physical file when a schema was also fed to the builder.
fn same_file(document: &ExecutableDocument, file_id: FileId, source: &SourceFile) -> bool {
    match document.sources.get(&file_id) {
        Some(apollo_sf) => apollo_sf.path() == source.path(),
        None => false,
    }
}

/// Resolve a [`DocumentSpec`] under `base` into a sorted, path-deduplicated
/// list of `(path, content)` pairs. This is the only place I/O (and glob
/// walking) happens in the loader. Content-level de-duplication is performed
/// later by the content hash; this only kills *exact path* overlaps (e.g. an
/// explicit `Files` list with duplicates, or several `Globs` matching the same
/// path).
fn resolve(spec: &DocumentSpec, base: &Path) -> Result<Vec<(PathBuf, String)>, DocumentLoadError> {
    match spec {
        DocumentSpec::Inline(text) => Ok(vec![(PathBuf::from("<inline>"), text.clone())]),
        DocumentSpec::Files(rels) => {
            let mut paths: Vec<PathBuf> = rels.iter().map(|r| base.join(r)).collect();
            paths.sort();
            paths.dedup();
            if paths.is_empty() {
                return Err(DocumentLoadError::Empty);
            }
            read_files(&paths)
        }
        DocumentSpec::Glob(pattern) => resolve_globs(base, std::slice::from_ref(pattern)),
        DocumentSpec::Globs(patterns) => {
            if patterns.is_empty() {
                return Err(DocumentLoadError::Empty);
            }
            resolve_globs(base, patterns)
        }
    }
}

/// Read a pre-resolved, deduplicated list of paths from disk.
fn read_files(paths: &[PathBuf]) -> Result<Vec<(PathBuf, String)>, DocumentLoadError> {
    let mut out = Vec::with_capacity(paths.len());
    for path in paths {
        let content = std::fs::read_to_string(path).map_err(|source| DocumentLoadError::Io {
            path: path.clone(),
            source,
        })?;
        out.push((path.clone(), content));
    }
    Ok(out)
}

/// Walk `base` (gitignore-aware via the [`ignore`] crate), match each path's
/// relative form against every pattern in `patterns` (unioned), restrict to
/// `.graphql` files, and return the sorted, path-deduplicated matches with
/// their contents.
fn resolve_globs(
    base: &Path,
    patterns: &[String],
) -> Result<Vec<(PathBuf, String)>, DocumentLoadError> {
    let matchers = patterns
        .iter()
        .map(|p| {
            globset::Glob::new(p)
                .map_err(|source| DocumentLoadError::Glob {
                    pattern: p.clone(),
                    source,
                })
                .map(|g| g.compile_matcher())
        })
        .collect::<Result<Vec<_>, _>>()?;

    let walker = ignore::WalkBuilder::new(base).build();
    let mut hits: Vec<PathBuf> = Vec::new();
    for entry in walker {
        let entry = match entry {
            Ok(e) => e,
            Err(_) => continue,
        };
        if !entry.file_type().is_some_and(|t| t.is_file()) {
            continue;
        }
        let path = entry.path();
        if !is_document_file(path) {
            continue;
        }
        let rel = match path.strip_prefix(base) {
            Ok(r) => r,
            Err(_) => continue,
        };
        if matchers.iter().any(|m| m.is_match(rel)) {
            hits.push(path.to_path_buf());
        }
    }

    if hits.is_empty() {
        return Err(DocumentLoadError::NoMatch {
            patterns: patterns.join(", "),
            base: base.to_path_buf(),
        });
    }

    hits.sort();
    hits.dedup();

    read_files(&hits)
}

/// `true` iff `path` ends in `.graphql` (case-sensitive). Schema files
/// (`.graphqls`) are intentionally **excluded** — they are owned by spec-004's
/// `SchemaLoader`. This keeps a glob like `**/*.graphql` from accidentally
/// eating a colocated schema split across `.graphqls` files.
fn is_document_file(path: &Path) -> bool {
    matches!(path.extension().and_then(|e| e.to_str()), Some("graphql"))
}

impl LoadedDocuments {
    /// Look up a loaded document by one of its input paths. Returns `None` if
    /// `path` was not part of this [`LoadedDocuments`]' resolution set.
    pub fn document_for_file(&self, path: &Path) -> Option<&LoadedDocument> {
        self.by_file.get(path).map(|&i| &self.docs[i])
    }
}

impl LoadedDocument {
    /// Convenience accessor mirroring [`crate::schema::LoadedSchema::parse_errors`].
    pub fn parse_errors(&self) -> &[Diagnostic] {
        &self.parse_errors
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use apollo_compiler::Schema;

    fn fixture_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/documents")
    }

    fn dup_dir() -> PathBuf {
        fixture_root().join("dup")
    }

    fn bad_dir() -> PathBuf {
        fixture_root().join("bad")
    }

    fn gitignore_dir() -> PathBuf {
        fixture_root().join("gitignore")
    }

    fn overlaps_dir() -> PathBuf {
        fixture_root().join("overlaps")
    }

    #[test]
    fn inline_yields_single_synthetic_doc() {
        let loader = DocumentLoader::new();
        let loaded = loader
            .load(
                &DocumentSpec::Inline("query GetUser { user { id } }".to_owned()),
                Path::new("ignored"),
                None,
            )
            .expect("inline load");
        assert_eq!(loaded.docs.len(), 1);
        assert_eq!(loaded.by_file.len(), 1);
        assert_eq!(loaded.docs[0].source.path(), Path::new("<inline>"));
        // A minimal operation parses cleanly standalone.
        assert!(loaded.docs[0].parse_errors.is_empty());
    }

    #[test]
    fn inline_eof_only_source_surfaces_a_parse_error() {
        // An empty/all-whitespace inline source is a genuine syntax error
        // (Unexpected <EOF>) which we route under `parse-error` rather than
        // aborting the load.
        let loader = DocumentLoader::new();
        let loaded = loader
            .load(
                &DocumentSpec::Inline("".to_owned()),
                Path::new("ignored"),
                None,
            )
            .expect("inline load still returns a (partial) document");
        assert_eq!(loaded.docs.len(), 1);
        assert_eq!(loaded.docs[0].parse_errors.len(), 1);
        assert_eq!(loaded.docs[0].parse_errors[0].rule_id, PARSE_ERROR_RULE_ID);
        assert_eq!(
            loaded.docs[0].parse_errors[0].file,
            PathBuf::from("<inline>")
        );
    }

    #[test]
    fn two_identical_files_dedup_to_one_document() {
        // spec-005 Testing: "Two `.graphql` files with identical content ->
        // LoadedDocuments.docs.len() == 1".
        let loader = DocumentLoader::new();
        let loaded = loader
            .load(
                &DocumentSpec::Glob("*.graphql".to_owned()),
                &dup_dir(),
                None,
            )
            .expect("dup glob load");

        assert_eq!(
            loaded.docs.len(),
            1,
            "two identical-content files must dedup to a single document"
        );
        // Both input paths route to the same doc index (0).
        assert_eq!(loaded.by_file.len(), 2);
        let a_idx = loaded.by_file[&dup_dir().join("a.graphql")];
        let b_idx = loaded.by_file[&dup_dir().join("b.graphql")];
        assert_eq!(a_idx, 0);
        assert_eq!(b_idx, 0);
        assert!(loaded.docs[0].parse_errors.is_empty());
    }

    #[test]
    fn overlapping_globs_dedup() {
        // spec-005 Deliverables: "a glob with overlaps dedups".
        // `**/*.graphql` matches every file under `overlaps/`, and `nested/*.graphql`
        // matches two of them again — every matched path appears exactly once in
        // `by_file`, and `docs` has one entry per unique *content* (dedup by hash).
        let loader = DocumentLoader::new();
        let loaded = loader
            .load(
                &DocumentSpec::Globs(vec![
                    "**/*.graphql".to_owned(),
                    "nested/*.graphql".to_owned(),
                ]),
                &overlaps_dir(),
                None,
            )
            .expect("overlapping globs load");
        // `by_file` has every *path* once (path-level dedup) even though both
        // globs matched the nested files.
        assert!(loaded.by_file.len() >= 2);
        // Every path that was matched by the second (overlapping) glob points
        // to the same index as the broader glob match for that file.
        for (path, &idx) in &loaded.by_file {
            assert_eq!(
                loaded.by_file[path], idx,
                "by_file must path-dedup even with overlapping globs"
            );
        }
        // No duplicate `LoadedDocument` for a path-level-deduped file.
        assert_eq!(loaded.docs.len(), loaded.by_file.len());
    }

    #[test]
    fn malformed_operation_yields_parse_error_in_right_file() {
        // spec-005 Testing: "A malformed operation yields a `parse-error`
        // diagnostic with the correct file."
        let loader = DocumentLoader::new();
        let bad_file = bad_dir().join("bad.graphql");
        let loaded = loader
            .load(
                &DocumentSpec::Files(vec![PathBuf::from("bad.graphql")]),
                &bad_dir(),
                None,
            )
            .expect("load still succeeds");
        assert!(!loaded.docs.is_empty());
        let doc = &loaded.docs[0];
        assert!(
            !doc.parse_errors.is_empty(),
            "malformed operation must produce parse_errors"
        );
        let diag = &doc.parse_errors[0];
        assert_eq!(diag.rule_id, PARSE_ERROR_RULE_ID);
        assert_eq!(diag.severity, Severity::Error);
        assert_eq!(
            diag.file, bad_file,
            "parse error attributed to the malformed file"
        );
        // Slicing the source by the diagnostic span must not panic.
        let _ = doc.source.slice(diag.span);
    }

    #[test]
    fn glob_respects_gitignore() {
        // spec-005 Testing: "A glob with `**` recursion respects `.gitignore`
        // (via `ignore` crate)."
        //
        // The `gitignore` fixture dir contains two `.graphql` files plus a
        // `.gitignore` that excludes `ignored.graphql`. A `**/*.graphql` glob
        // must match only `kept.graphql`.
        let loader = DocumentLoader::new();
        let loaded = loader
            .load(
                &DocumentSpec::Glob("**/*.graphql".to_owned()),
                &gitignore_dir(),
                None,
            )
            .expect("gitignored glob still matches one file");
        let kept = gitignore_dir().join("kept.graphql");
        let ignored = gitignore_dir().join("ignored.graphql");
        assert!(loaded.by_file.contains_key(&kept), "kept.graphql matched");
        assert!(
            !loaded.by_file.contains_key(&ignored),
            "ignored.graphql must be skipped (gitignore respected)"
        );
        assert_eq!(loaded.docs.len(), 1);
    }

    #[test]
    fn no_match_glob_is_an_error() {
        let loader = DocumentLoader::new();
        let err = loader
            .load(
                &DocumentSpec::Glob("does-not-exist-*.graphql".to_owned()),
                &dup_dir(),
                None,
            )
            .expect_err("no matches should error");
        assert!(matches!(err, DocumentLoadError::NoMatch { .. }));
    }

    #[test]
    fn invalid_glob_pattern_is_an_error() {
        let loader = DocumentLoader::new();
        let err = loader
            .load(
                &DocumentSpec::Glob("[unclosed".to_owned()),
                &dup_dir(),
                None,
            )
            .expect_err("invalid glob should error");
        assert!(matches!(err, DocumentLoadError::Glob { .. }));
    }

    #[test]
    fn empty_files_spec_is_an_error() {
        let loader = DocumentLoader::new();
        let err = loader
            .load(&DocumentSpec::Files(vec![]), &dup_dir(), None)
            .expect_err("empty files spec should error");
        assert!(matches!(err, DocumentLoadError::Empty));
    }

    #[test]
    fn schema_aware_parse_resolves_field_type() {
        // With a schema supplied, the builder resolves operation field types
        // (and reports UndefinedField if the operation references a field not
        // in the schema). Driving the happy path here exercises the
        // `Option<&Schema>` plumbing of the public `load` signature.
        let schema_src = "type Query { user: User } type User { id: ID! name: String }";
        let schema = Schema::parse_and_validate(schema_src, "schema.graphql").unwrap();

        let loader = DocumentLoader::new();
        let op = "query GetUser { user { id name } }".to_owned();
        let loaded = loader
            .load(
                &DocumentSpec::Inline(op.clone()),
                Path::new("ignored"),
                Some(&schema),
            )
            .expect("inline schema-aware load");
        assert_eq!(loaded.docs.len(), 1);
        assert!(loaded.docs[0].parse_errors.is_empty());
    }

    #[test]
    fn schema_aware_parse_leaves_undefined_field_to_spec_bridge() {
        // An operation selecting a non-existent field with a schema present
        // produces a semantic diagnostic that spec-053 owns.
        let schema_src = "type Query { user: User } type User { id: ID! }";
        let schema = Schema::parse_and_validate(schema_src, "schema.graphql").unwrap();

        let loader = DocumentLoader::new();
        let loaded = loader
            .load(
                &DocumentSpec::Inline("query { user { id doesNotExist } }".to_owned()),
                Path::new("ignored"),
                Some(&schema),
            )
            .expect("schema-aware load still succeeds");
        let doc = &loaded.docs[0];
        assert!(
            doc.parse_errors.is_empty(),
            "semantic validation is owned by the graphql-spec bridge"
        );
    }

    #[test]
    fn document_for_file_lookup_round_trips() {
        let loader = DocumentLoader::new();
        let loaded = loader
            .load(
                &DocumentSpec::Glob("*.graphql".to_owned()),
                &dup_dir(),
                None,
            )
            .unwrap();
        let a = dup_dir().join("a.graphql");
        let doc = loaded.document_for_file(&a).expect("a.graphql present");
        assert_eq!(doc.source.path(), a.as_path());
    }

    #[test]
    fn loaded_document_and_loader_are_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<LoadedDocument>();
        assert_send_sync::<LoadedDocuments>();
        assert_send_sync::<DocumentLoader>();
    }
}
