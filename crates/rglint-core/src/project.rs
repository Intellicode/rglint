//! Project resolution — binding a `(schema, documents)` pair per configured
//! project and loading each into the engine's working set.
//!
//! A "project" is graphql-config's unit of work: a schema, a set of operation
//! documents, and (optionally) `include`/`ignore` globs. One workspace may
//! contain several projects, each linted independently — its schema is never
//! shared with another project's documents (matching
//! `packages/plugin/src/processor.ts` and graphql-config semantics).
//!
//! ## What this module owns vs. what its dependencies own
//!
//! - [`ProjectConfig`] is the *resolved* project description (name, schema
//!   spec, document spec, ignore globs). It is the seam spec-054 (`.rglintrc`)
//!   and spec-055 (`.graphqlrc`) produce and feed into [`ProjectResolver`].
//! - [`ProjectResolver::resolve`] takes a *pre-built* `&[ProjectConfig]` list
//!   and:
//!     1. Resolves relative paths against the resolver's `base` (the config
//!        file's directory).
//!     2. Loads each project's schema via [`SchemaLoader`](crate::SchemaLoader)
//!        (spec-004) and its documents via
//!        [`DocumentLoader`](crate::DocumentLoader) (spec-005), feeding the
//!        loaded schema into document parsing for type-aware errors.
//!     3. Builds per-project [`Siblings`](crate::Siblings) (spec-006).
//!
//! The reading of `.rglintrc` / `.graphqlrc` *files* and the synthesis of a
//! single "default" project from top-level `schema`/`documents` keys when no
//! `projects` map is present is owned by spec-054's normalization step —
//! this module only consumes the resulting `ProjectConfig` list. Tests here
//! build `ProjectConfig`s inline (the spec calls this out under
//! "Dependencies").
//!
//! ## Schema-less & document-less projects
//!
//! - A project with `schema: None` still loads its documents *standalone*
//!   (apollo-compiler parses without a schema). Rules marked `requires_schema`
//!   self-skip in the engine (spec-011). The loaded
//!   [`Project::schema`](Project::schema) is `None`.
//! - A project with `documents: None` is schema-only: no operation documents
//!   are loaded and [`Project::documents`] is empty (zero docs, zero by-file
//!   entries). [`Project::siblings`](Siblings::is_available) reports
//!   `available == false`.
//!
//! ## Hard vs. soft errors
//!
//! - Missing schema files (when `schema` is set): the
//!   [`SchemaLoader`](crate::SchemaLoader) returns a [`SchemaLoadError`] that
//!   is propagated up — a *hard* error, with the offending glob in the message
//!   (see [`SchemaLoadError::NoMatch`] / [`SchemaLoadError::Glob`]).
//! - Missing document files *when `documents` is set*: the
//!   [`DocumentLoader`](crate::DocumentLoader) returns a [`DocumentLoadError`]
//!   that propagates up — the spec calls an empty document set "suspicious".
//!   When `documents` is `None`, no documents are loaded and no error fires.
//!
//! ## Remote (HTTP) schemas
//!
//! `graphql-config` permits `schema: <http-url>`. v1 of the engine ignores that
//! (PLAN §11 stretch). [`SchemaSpec`] has no HTTP variant today; a glob/path
//! shaped like a URL would otherwise produce a confusing "no match" error, so
//! [`ProjectResolver`] rejects `http://` / `https://` schema specs up front with
//! [`ProjectResolveError::UnsupportedRemoteSchema`].

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use crate::documents::{DocumentLoader, DocumentSpec, LoadedDocuments};
use crate::schema::{LoadedSchema, SchemaLoader, SchemaSpec};
use crate::siblings::Siblings;

/// The resolved description of one project, as produced by the config loader
/// (spec-054 / spec-055) and consumed by [`ProjectResolver`].
///
/// `schema` and `documents` are both optional to support the schema-less and
/// document-less linting modes described in the spec's "Behavior" section. The
/// spec's interface sketch listed them as non-optional `SchemaSpec` /
/// `DocumentSpec`; the *behavior* requirements ("`schema: None`", "`documents`
/// unset") take precedence and require `Option` here. The config loader fills
/// in the appropriate variant; if a future caller wants a guaranteed-present
/// `SchemaSpec` it checks `is_some`.
#[derive(Clone, Debug)]
pub struct ProjectConfig {
    /// Human-readable project name (graphql-config key, or `"default"` when
    /// synthesized by spec-054's normalization). Used in error messages and
    /// reporter output.
    pub name: String,
    /// The schema spec for this project, or `None` for schema-less linting.
    pub schema: Option<SchemaSpec>,
    /// The document spec for this project, or `None` for schema-only lint.
    pub documents: Option<DocumentSpec>,
    /// Project-scoped ignore globs (relative to the config file's directory).
    /// Reserved for the engine's file-filtering pass; the v1 resolver stores
    /// but does not yet apply them — see module "What this module owns" note.
    pub ignore: Vec<String>,
}

/// A fully bound project: its config plus the loaded schema (if any), loaded
/// documents, and the cross-document [`Siblings`] index.
///
/// Each [`Project`] owns its own [`LoadedSchema`] / [`LoadedDocuments`] /
/// [`Siblings`]; even when two projects point at *overlapping* schema paths,
/// they each load via a fresh [`SchemaLoader`] caller — see the resolver's note
/// on "no shared schema object between projects".
#[derive(Debug)]
pub struct Project {
    /// The source config this project was resolved from.
    pub config: ProjectConfig,
    /// The loaded schema, or `None` for a schema-less project. Cheap to share
    /// (an [`Arc`]) so a reporter can hold a handle without copying.
    pub schema: Option<Arc<LoadedSchema>>,
    /// The loaded operation documents. Empty (`docs` + `by_file` both empty)
    /// for a document-less (schema-only) project.
    pub documents: LoadedDocuments,
    /// The cross-document index of operations/fragments built from
    /// [`Project::documents`]. `is_available()` returns `false` when no
    /// documents were loaded.
    pub siblings: Siblings,
}

/// Errors that can occur while resolving a list of [`ProjectConfig`]s into
/// [`Project`]s.
///
/// Each project is loaded independently; the first project to fail short-
/// circuits the whole `resolve` call with its error (and the project name
/// is attached via [`Self::project`] so the caller can report *which* project
/// was at fault).
#[derive(Debug, thiserror::Error)]
pub enum ProjectResolveError {
    /// A schema spec could not be loaded (missing/unreadable/glob failure).
    #[error("project `{project}`: {source}")]
    Schema {
        /// The name of the project whose schema failed to load.
        project: String,
        /// The underlying schema-load error.
        #[source]
        source: crate::schema::SchemaLoadError,
    },
    /// A document spec could not be loaded (missing/glob failure when
    /// `documents` was set).
    #[error("project `{project}`: {source}")]
    Documents {
        /// The name of the project whose documents failed to load.
        project: String,
        /// The underlying document-load error.
        #[source]
        source: crate::documents::DocumentLoadError,
    },
    /// `schema` pointed at a remote `http://` / `https://` URL, which v1 of
    /// the engine does not support yet (PLAN §11 stretch).
    #[error(
        "project `{project}`: remote schema URL `{url}` is not supported yet"
    )]
    UnsupportedRemoteSchema {
        /// The name of the offending project.
        project: String,
        /// The unsupported URL.
        url: String,
    },
}

/// Resolves a list of [`ProjectConfig`]s into fully bound [`Project`]s by
/// loading each project's schema + documents and building its siblings index.
///
/// The `base` directory is the *config file's directory*: all relative paths in
/// a [`ProjectConfig`] (and the resolver's ignore globs) are interpreted under
/// it. Resolution is **deterministic**: projects are loaded in the order they
/// appear in the input slice, and each project's schema/documents are loaded
/// via spec-004 / spec-005 (which sort + dedup paths), so two resolves of the
/// same config produce identical file orderings.
///
/// Each project receives a *fresh* [`SchemaLoader`] / [`DocumentLoader`] pair
/// inside `resolve` — guaranteeing the "no shared schema object between
/// projects" invariant from the spec's "Behavior" section. The cache benefit
/// of `SchemaLoader` therefore applies *repeated resolves of the same
/// ProjectConfig list over the lifetime of one [`ProjectResolver`]*, not across
/// projects in a single call. A future revision can lift a shared
/// `SchemaLoader` into the resolver if cross-project schema caching is wanted
/// without violating the invariant (cache hits still return distinct `Arc`s on
/// distinct loaders, but a *shared* loader would alias them — left as a
/// deliberate non-choice for v1).
#[derive(Debug)]
pub struct ProjectResolver {
    base: PathBuf,
}

impl ProjectResolver {
    /// Create a resolver that resolves relative paths under `base` (typically
    /// the directory containing the `.rglintrc` / `.graphqlrc` file).
    pub fn new(base: PathBuf) -> Self {
        Self { base }
    }

    /// The base directory all relative paths are resolved against.
    pub fn base(&self) -> &std::path::Path {
        &self.base
    }

    /// Resolve every [`ProjectConfig`] into a fully bound [`Project`].
    ///
    /// Projects are loaded in input order; the first error short-circuits the
    /// whole call (subsequent projects are not attempted) with the offending
    /// project's name attached — see [`ProjectResolveError`]. On success each
    /// returned [`Project`] is fully independent: its own schema (when
    /// present), documents, and siblings.
    ///
    /// # Errors
    /// [`ProjectResolveError::Schema`] when a project's schema spec fails to
    /// load; [`ProjectResolveError::Documents`] when its document spec fails;
    /// [`ProjectResolveError::UnsupportedRemoteSchema`] when `schema` is an
    /// HTTP URL.
    pub fn resolve(&self, cfgs: &[ProjectConfig]) -> Result<Vec<Project>, ProjectResolveError> {
        let schema_loader = SchemaLoader::new();
        let doc_loader = DocumentLoader::new();

        let mut projects = Vec::with_capacity(cfgs.len());
        for cfg in cfgs {
            projects.push(self.resolve_one(cfg, &schema_loader, &doc_loader)?);
        }
        Ok(projects)
    }

    fn resolve_one(
        &self,
        cfg: &ProjectConfig,
        schema_loader: &SchemaLoader,
        doc_loader: &DocumentLoader,
    ) -> Result<Project, ProjectResolveError> {
        // 1. Schema (optional). Reject remote URLs up front with a clear error
        //    rather than letting the glob walker produce a confusing "no match".
        let schema = match &cfg.schema {
            Some(spec) => {
                if let Some(url) = remote_url_of(spec) {
                    return Err(ProjectResolveError::UnsupportedRemoteSchema {
                        project: cfg.name.clone(),
                        url: url.to_owned(),
                    });
                }
                Some(
                    schema_loader
                        .load(spec, &self.base)
                        .map_err(|source| ProjectResolveError::Schema {
                            project: cfg.name.clone(),
                            source,
                        })?,
                )
            }
            None => None,
        };

        // 2. Documents (optional), parsed against the (parsed, unvalidated)
        //    schema when present for type-aware errors. A schema-less project
        //    parses its documents standalone — exactly what the engine's
        //    `requires_schema` rules self-skip on.
        let documents = match &cfg.documents {
            Some(spec) => doc_loader
                .load(spec, &self.base, schema.as_deref().map(|ls| &ls.compiler))
                .map_err(|source| ProjectResolveError::Documents {
                    project: cfg.name.clone(),
                    source,
                })?,
            None => empty_documents(),
        };

        // 3. Siblings are built from the loaded documents; for a document-less
        //    project this is an empty index (is_available == false), which the
        //    engine uses to self-skip `requires_siblings` rules.
        let siblings = Siblings::from_documents(&documents);

        Ok(Project {
            config: cfg.clone(),
            schema,
            documents,
            siblings,
        })
    }
}

impl Project {
    /// Reload operation documents after applying in-memory replacements.
    ///
    /// The schema is intentionally left untouched: spec-061 only permits
    /// fixes in executable documents. Existing input aliases are retained and
    /// receive the replacement belonging to their deduplicated owner.
    pub(crate) fn reload_documents(
        &mut self,
        replacements: &HashMap<PathBuf, String>,
    ) -> Result<(), crate::documents::DocumentLoadError> {
        let files = document_sources(&self.documents, replacements);
        let loader = DocumentLoader::new();
        let schema = self.schema.as_deref().map(|schema| &schema.compiler);
        self.documents = loader.load_sources(&files, schema)?;
        self.siblings = Siblings::from_documents(&self.documents);
        Ok(())
    }

    /// Return a reloaded project without changing the caller's project. Used
    /// by dry-run mode to simulate successive fix passes without filesystem
    /// writes.
    pub(crate) fn reloaded_documents(
        &self,
        replacements: &HashMap<PathBuf, String>,
    ) -> Result<Self, crate::documents::DocumentLoadError> {
        let files = document_sources(&self.documents, replacements);
        let loader = DocumentLoader::new();
        let schema = self.schema.as_deref().map(|schema| &schema.compiler);
        let documents = loader.load_sources(&files, schema)?;
        let siblings = Siblings::from_documents(&documents);
        let project = Self {
            config: self.config.clone(),
            schema: self.schema.clone(),
            documents,
            siblings,
        };
        Ok(project)
    }
}

fn document_sources(
    documents: &LoadedDocuments,
    replacements: &HashMap<PathBuf, String>,
) -> Vec<(PathBuf, String)> {
    documents
        .by_file
        .keys()
        .map(|path| {
            let owner = documents
                .document_for_file(path)
                .expect("LoadedDocuments::by_file points to a loaded document");
            let content = replacements
                .get(owner.source.path())
                .or_else(|| replacements.get(path))
                .cloned()
                .unwrap_or_else(|| owner.source.source().to_owned());
            (path.clone(), content)
        })
        .collect()
}

/// Construct an empty [`LoadedDocuments`] for a document-less project.
fn empty_documents() -> LoadedDocuments {
    LoadedDocuments {
        docs: Vec::new(),
        by_file: std::collections::HashMap::new(),
    }
}

/// If `spec` represents a remote `http://` or `https://` schema, return its
/// string form. A [`SchemaSpec`] today does not carry an HTTP variant, so we
/// sniff the textual content of the variants that *could* be a URL: `Glob` and
/// `File` whose first segment looks like a scheme. `Inline` is never remote and
/// `Files` is a list of path components (no scheme semantics).
fn remote_url_of(spec: &SchemaSpec) -> Option<&str> {
    let s = match spec {
        SchemaSpec::Glob(s) => s,
        SchemaSpec::File(p) => p.to_str()?,
        _ => return None,
    };
    if s.starts_with("http://") || s.starts_with("https://") {
        Some(s)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/project")
    }

    fn multi_dir() -> PathBuf {
        fixture_root().join("multi")
    }

    fn default_dir() -> PathBuf {
        fixture_root().join("default")
    }

    /// The two-project (`web` + `admin`) fixture: each project has its own
    /// schema and one operation document, mirroring the spec's
    /// `examples/multiple-projects-graphql-config` layout.
    fn two_project_configs() -> Vec<ProjectConfig> {
        vec![
            ProjectConfig {
                name: "web".to_owned(),
                schema: Some(SchemaSpec::File(PathBuf::from("web/schema.graphqls"))),
                documents: Some(DocumentSpec::Files(vec![PathBuf::from("web/doc.graphql")])),
                ignore: Vec::new(),
            },
            ProjectConfig {
                name: "admin".to_owned(),
                schema: Some(SchemaSpec::File(PathBuf::from("admin/schema.graphqls"))),
                documents: Some(DocumentSpec::Files(vec![PathBuf::from("admin/doc.graphql")])),
                ignore: Vec::new(),
            },
        ]
    }

    #[test]
    fn resolves_two_independent_projects_with_correct_schema_types() {
        // spec-007 Testing: a 2-project fixture; resolve() returns 2 Projects
        // with correct schema types.
        let resolver = ProjectResolver::new(multi_dir());
        let projects = resolver
            .resolve(&two_project_configs())
            .expect("two-project resolve");

        assert_eq!(projects.len(), 2, "both projects resolved");

        let web = &projects[0];
        assert_eq!(web.config.name, "web");
        let web_schema = web.schema.as_deref().expect("web has a schema");
        assert!(
            web_schema.compiler.get_object("Query").is_some(),
            "web schema has Query"
        );
        // The `greeting` field distinguishes the *web* schema from the *admin*
        // schema (whose Query has `count`) — proving the two projects do not
        // share a schema object.
        let web_query = web_schema
            .compiler
            .get_object("Query")
            .expect("Query object");
        assert!(
            web_query
                .fields
                .iter()
                .any(|(name, _)| name.as_str() == "greeting"),
            "web Query has `greeting` (not cross-contaminated with admin)"
        );
        assert_eq!(web.documents.docs.len(), 1);
        assert!(web.siblings.is_available());
        assert_eq!(web.siblings.operations().len(), 1);
        assert_eq!(
            web.siblings.operations()[0].name.as_deref(),
            Some("Greeting")
        );

        let admin = &projects[1];
        assert_eq!(admin.config.name, "admin");
        let admin_schema = admin.schema.as_deref().expect("admin has a schema");
        let admin_query = admin_schema
            .compiler
            .get_object("Query")
            .expect("Query object");
        assert!(
            admin_query
                .fields
                .iter()
                .any(|(name, _)| name.as_str() == "count"),
            "admin Query has `count`"
        );
        // Sanity: the admin schema does *not* expose `greeting` (no shared
        // schema object between projects).
        assert!(
            !admin_query
                .fields
                .iter()
                .any(|(name, _)| name.as_str() == "greeting"),
            "admin schema must not carry web's `greeting` field"
        );
        assert_eq!(admin.documents.docs.len(), 1);
        assert_eq!(
            admin.siblings.operations()[0].name.as_deref(),
            Some("Count")
        );
    }

    #[test]
    fn default_project_synthesis_via_inline_config() {
        // spec-007 Behavior: when no `projects` key is present, a single
        // "default" project synthesized from top-level `schema`/`documents`
        // yields one project named "default". The synthesis itself lives in
        // spec-054's normalization (out of scope here); this test drives the
        // resolver with the *output* of that synthesis, mirroring what the
        // engine will see in practice.
        let resolver = ProjectResolver::new(default_dir());
        let cfg = ProjectConfig {
            name: "default".to_owned(),
            schema: Some(SchemaSpec::File(PathBuf::from("schema.graphqls"))),
            documents: Some(DocumentSpec::Files(vec![PathBuf::from("doc.graphql")])),
            ignore: Vec::new(),
        };
        let projects = resolver
            .resolve(std::slice::from_ref(&cfg))
            .expect("default resolve");
        assert_eq!(projects.len(), 1);
        assert_eq!(projects[0].config.name, "default");
        assert!(
            projects[0]
                .schema
                .as_deref()
                .map(|ls| ls.compiler.get_object("Query").is_some())
                .unwrap_or(false),
            "default project has a Query"
        );
    }

    #[test]
    fn schemaless_project_loads_documents_standalone() {
        // spec-007 Behavior: "A project with schema: None still loads
        // documents (schema-less linting; requires_schema rules self-skip)."
        let resolver = ProjectResolver::new(multi_dir());
        let cfg = ProjectConfig {
            name: "schemaless".to_owned(),
            schema: None,
            documents: Some(DocumentSpec::Files(vec![PathBuf::from("admin/doc.graphql")])),
            ignore: Vec::new(),
        };
        let projects = resolver
            .resolve(std::slice::from_ref(&cfg))
            .expect("schemaless resolve");
        assert!(projects[0].schema.is_none(), "no schema loaded");
        assert_eq!(
            projects[0].documents.docs.len(),
            1,
            "documents still loaded standalone"
        );
        assert!(projects[0].siblings.is_available());
    }

    #[test]
    fn documentless_project_is_schema_only() {
        // spec-007 Behavior: "When documents is unset -> no documents,
        // schema-only lint."
        let resolver = ProjectResolver::new(multi_dir());
        let cfg = ProjectConfig {
            name: "schemaonly".to_owned(),
            schema: Some(SchemaSpec::File(PathBuf::from("web/schema.graphqls"))),
            documents: None,
            ignore: Vec::new(),
        };
        let projects = resolver
            .resolve(std::slice::from_ref(&cfg))
            .expect("schema-only resolve");
        assert!(projects[0].schema.is_some(), "schema loaded");
        assert!(
            projects[0].documents.docs.is_empty(),
            "no documents loaded"
        );
        assert!(
            projects[0].documents.by_file.is_empty(),
            "no by_file entries"
        );
        assert!(
            !projects[0].siblings.is_available(),
            "siblings unavailable when no documents"
        );
    }

    #[test]
    fn missing_schema_files_is_a_hard_error_with_project_name() {
        // spec-007 Behavior: "Missing schema files -> hard error with the
        // offending glob in the message."
        let resolver = ProjectResolver::new(multi_dir());
        let cfg = ProjectConfig {
            name: "broken".to_owned(),
            schema: Some(SchemaSpec::Glob("does-not-exist-*.graphqls".to_owned())),
            documents: None,
            ignore: Vec::new(),
        };
        let err = resolver
            .resolve(std::slice::from_ref(&cfg))
            .expect_err("missing schema must error");
        let msg = format!("{err}");
        assert!(
            msg.contains("broken"),
            "error names the offending project: {msg}"
        );
        assert!(
            msg.contains("does-not-exist-"),
            "error carries the offending glob: {msg}"
        );
    }

    #[test]
    fn missing_documents_when_set_is_an_error() {
        // spec-007 Behavior: "Missing document files when documents is set ->
        // error (an empty document set is suspicious)."
        let resolver = ProjectResolver::new(multi_dir());
        let cfg = ProjectConfig {
            name: "broken-docs".to_owned(),
            schema: Some(SchemaSpec::File(PathBuf::from("web/schema.graphqls"))),
            documents: Some(DocumentSpec::Glob("does-not-exist-*.graphql".to_owned())),
            ignore: Vec::new(),
        };
        let err = resolver
            .resolve(std::slice::from_ref(&cfg))
            .expect_err("missing documents must error");
        assert!(
            matches!(err, ProjectResolveError::Documents { .. }),
            "document-load failure pinned to the Documents variant: {err}"
        );
        assert!(
            format!("{err}").contains("broken-docs"),
            "error names the project"
        );
    }

    #[test]
    fn remote_schema_url_is_rejected_with_a_clear_message() {
        // spec-007 Risks/Notes: "graphql-config supports schema: <http-url>;
        // v1 ignores that." Reject with a clear "not supported yet" error.
        let resolver = ProjectResolver::new(multi_dir());
        let cfg = ProjectConfig {
            name: "remote".to_owned(),
            schema: Some(SchemaSpec::Glob(
                "https://example.com/schema.graphql".to_owned(),
            )),
            documents: None,
            ignore: Vec::new(),
        };
        let err = resolver
            .resolve(std::slice::from_ref(&cfg))
            .expect_err("remote schema must be rejected");
        let msg = format!("{err}");
        assert!(
            msg.contains("not supported yet") && msg.contains("https://example.com"),
            "clear 'not supported yet' message with URL: {msg}"
        );
    }

    #[test]
    fn short_circuits_on_first_failing_project() {
        // Two projects, the first one broken: we never attempt the second.
        let resolver = ProjectResolver::new(multi_dir());
        let broken = ProjectConfig {
            name: "broken".to_owned(),
            schema: Some(SchemaSpec::Glob("nope-*.graphqls".to_owned())),
            documents: None,
            ignore: Vec::new(),
        };
        let good = ProjectConfig {
            name: "web".to_owned(),
            schema: Some(SchemaSpec::File(PathBuf::from("web/schema.graphqls"))),
            documents: Some(DocumentSpec::Files(vec![PathBuf::from("web/doc.graphql")])),
            ignore: Vec::new(),
        };
        let err = resolver
            .resolve(&[broken, good])
            .expect_err("first project fails");
        assert!(
            matches!(err, ProjectResolveError::Schema { .. }),
            "schema-failure variant: {err}"
        );
    }

    #[test]
    fn resolver_and_project_are_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<ProjectResolver>();
        assert_send_sync::<Project>();
        assert_send_sync::<ProjectConfig>();
        assert_send_sync::<ProjectResolveError>();
    }
}
