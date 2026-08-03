//! `LintEngine` — the orchestrator that, given a [`Project`], runs all enabled
//! rules over all documents, walking each document's AST once and multiplexing
//! to subscribed rule handlers, then collecting diagnostics — spec-011.
//!
//! This is the single entry point the CLI (spec-062) calls. Per document it:
//!
//! 1. Emits `parse-error` diagnostics the loader already surfaced (specs 004 /
//!    005 — `parse_errors` live on [`LoadedSchema`](crate::LoadedSchema) and
//!    [`LoadedDocument`](crate::LoadedDocument)).
//! 2. Builds a [`RuleContext`] + [`Handler`] per enabled rule via
//!    [`Rule::create`](crate::Rule::create), skipping rules whose
//!    `requires_schema` / `requires_siblings` preconditions aren't met by the
//!    project (skip, not error — matches graphql-eslint).
//! 3. Walks the CST of the file **once** with `apollo-parser`, calling
//!    [`Handler::on_node`](crate::Handler::on_node) for each node whose
//!    [`SyntaxKind`] is in **any** rule's `interested_kinds` set, iterating
//!    only the handlers subscribed to that kind. Parent node is passed where
//!    available.
//! 4. Calls [`Handler::finalize`](crate::Handler::finalize) for each rule.
//! 5. Drains [`RuleContext::take_diagnostics`] into the per-file result.
//!
//! "Documents" here is the union of the project's loaded *operation* documents
//! (spec-005) and its *schema* source files (spec-004): operation rules fire on
//! operation files, schema rules fire on schema files, and the kind-based
//! dispatch filters each rule's interest naturally (an `OPERATION_DEFINITION`
//! never appears in a `.graphqls` CST, a `FIELD_DEFINITION` never in a
//! `.graphql` operation CST). A rule whose `requires_schema` is true is skipped
//! on a schema-less project (and gets the project's schema attached to its
//! context elsewhere); a `requires_siblings` rule skips when no documents were
//! loaded.
//!
//! Severity: diagnostics carrying [`Severity::Off`] are dropped before
//! reporting (config can downgrade a rule to off). The final `all` vec is
//! sorted by `(file, line, column, rule_id)` for stable reporter output.
//!
//! The engine is single-threaded today but `Send + Sync` so spec-064 (rayon)
//! drops in unchanged.
//!
//! ## Why re-parse with `apollo-parser` instead of walking `apollo-compiler`?
//!
//! `apollo-compiler`'s typed AST does not expose a generic walk over every CST
//! node — its nodes are typed handles (e.g. `ObjectTypeDefinition`) you have
//! to dispatch on by hand. `apollo-parser`'s rowan-backed CST, on the other
//! hand, gives us the untyped `SyntaxNode` tree we can descend generically and
//! dispatch by `SyntaxKind` to whichever rule declared interest. We re-parse
//! each source file with `apollo-parser::Parser` purely to obtain this CST;
//! the parsed-typed `ExecutableDocument` / `Schema` from the loaders (specs
//! 004/005) is what rules still read through [`RuleContext`] (e.g.
//! `ctx.schema().get_object(...)`), so the re-parse has no semantic cost
//! beyond a second pass over the bytes.

use std::collections::HashMap;
use std::path::PathBuf;

use apollo_parser::cst::CstNode;
use apollo_parser::{Parser, SyntaxKind, SyntaxNode};

use crate::diagnostics::{Diagnostic, Severity};
use crate::location::Span;
use crate::node::Node;
use crate::project::Project;
use crate::rule::{Handler, RuleEntry, ALL_RULES};
use crate::source::SourceFile;
use crate::RuleContext;

/// A rule name resolution failure from [`LintEngine::new`].
#[derive(Debug, thiserror::Error)]
pub enum LintEngineError {
    /// The config referenced a rule id that is not in the
    /// [`ALL_RULES`](crate::ALL_RULES) registry.
    #[error("unknown rule id `{rule_id}`; not in the rule registry")]
    UnknownRule {
        /// The unresolved rule id.
        rule_id: String,
    },
}

/// A resolved configuration for a single rule (id + severity + options), as
/// consumed by [`LintEngine::new`]. The engine maps this to an
/// [`EnabledRule`] by looking up the rule id in [`ALL_RULES`](crate::ALL_RULES).
#[derive(Clone, Debug)]
pub struct RuleConfig {
    /// Stable rule id, matching [`RuleMeta::id`](crate::RuleMeta::id).
    pub id: String,
    /// Configured severity for this rule. [`Severity::Off`] disables the rule's
    /// diagnostics (the engine still constructs handlers but drops their
    /// output); see "Severity filtering" in the spec.
    pub severity: Severity,
    /// The rule's options, raw JSON. The engine passes them verbatim into
    /// [`RuleContext::options_raw`](crate::RuleContext::options_raw); rules
    /// deserialize via [`RuleContext::option`](crate::RuleContext::option).
    pub options: serde_json::Value,
}

impl Default for RuleConfig {
    fn default() -> Self {
        Self {
            id: String::new(),
            severity: Severity::Warn,
            options: serde_json::Value::Null,
        }
    }
}

/// The resolved `rules:` block of the user's config — a list of
/// [`RuleConfig`]s. Spec-054 / spec-055 build this from `.rglintrc` /
/// `.graphqlrc`; the engine consumes the already-parsed shape so it stays
/// independent of file format.
#[derive(Clone, Debug, Default)]
pub struct RulesConfig {
    /// Enabled rule configs, in declaration order.
    pub rules: Vec<RuleConfig>,
}

/// A rule the engine has resolved from a [`RuleConfig`] to a static
/// [`RuleEntry`] in the registry, carrying the runtime severity + options to
/// apply for this project.
#[derive(Clone, Debug)]
pub struct EnabledRule {
    /// Static registry record: metadata + factory + `interested_kinds`.
    pub entry: &'static RuleEntry,
    /// Configured severity (overrides the rule's `RuleMeta::severity`).
    pub severity: Severity,
    /// Configured options, raw JSON.
    pub options: serde_json::Value,
}

/// The lint output for one project: per-file diagnostics and the full
/// sorted-by-`(file, line, column, rule_id)` aggregate.
#[derive(Debug, Default)]
pub struct ProjectLintResult {
    /// The project name (mirrors [`ProjectConfig::name`](crate::ProjectConfig::name)).
    pub project_name: String,
    /// Diagnostics grouped by their attributed file path. A path that was
    /// deduped to the same content hash as another shares the same
    /// `Vec<Diagnostic>` (cloned per path to keep the API simple); the
    /// diagnostic's own [`Diagnostic::file`] always carries the physical
    /// source path the offending node lived in.
    pub by_file: HashMap<PathBuf, Vec<Diagnostic>>,
    /// All diagnostics, sorted by `(file, line, column, rule_id)`.
    pub all: Vec<Diagnostic>,
    /// Source text for every file that the engine visited, keyed by its
    /// physical path. Reporters use this index to render snippets without
    /// rereading files (and so inline sources remain renderable).
    pub sources: HashMap<PathBuf, std::sync::Arc<SourceFile>>,
}

/// The lint engine: holds the compiled rule registry + resolved config
/// (which rules enabled, severities, options). Construct once with
/// [`LintEngine::new`], then [`LintEngine::lint`] each project.
#[derive(Debug)]
pub struct LintEngine {
    rules: Vec<EnabledRule>,
}

impl LintEngine {
    /// Resolve the configured rule ids against the [`ALL_RULES`](crate::ALL_RULES)
    /// registry and return a ready-to-run engine.
    ///
    /// # Errors
    /// [`LintEngineError::UnknownRule`] for the first rule id in `config` that
    /// is not present in the registry.
    pub fn new(config: &RulesConfig) -> Result<Self, LintEngineError> {
        let mut rules = Vec::with_capacity(config.rules.len());
        for rc in &config.rules {
            let entry = ALL_RULES
                .iter()
                .find(|e| e.meta.id == rc.id)
                .ok_or_else(|| LintEngineError::UnknownRule {
                    rule_id: rc.id.clone(),
                })?;
            rules.push(EnabledRule {
                entry,
                severity: rc.severity,
                options: rc.options.clone(),
            });
        }
        Ok(Self { rules })
    }

    /// Build an engine from already-resolved [`EnabledRule`]s, bypassing the
    /// registry lookup. Useful for tests that construct [`RuleEntry`]s manually
    /// (e.g. to set `interested_kinds`) without going through `#[derive(Rule)]`.
    pub fn from_enabled_rules(rules: Vec<EnabledRule>) -> Self {
        Self { rules }
    }

    /// Return the resolved rules for engine adapters such as [`crate::Fixer`].
    pub(crate) fn enabled_rules(&self) -> &[EnabledRule] {
        &self.rules
    }

    /// Run every enabled rule over every document in `project`, returning the
    /// collected diagnostics grouped per file and sorted across the project.
    ///
    /// Rules whose `requires_schema` precondition is unmet (project has no
    /// schema) are skipped silently, as are `requires_siblings` rules when no
    /// document siblings were loaded. Parallelism (spec-064) and `--fix`
    /// (spec-061) are out of scope here; this is a single-threaded walk today.
    pub fn lint(&self, project: &Project) -> Result<ProjectLintResult, LintEngineError> {
        let schema_ref = project.schema.as_deref().map(|ls| &ls.compiler);
        let siblings = if project.siblings.is_available() {
            Some(&project.siblings)
        } else {
            None
        };

        // Path -> SourceFile handle, used to resolve (line, column) for the
        // final sort. Built once across both the schema sources and the
        // operation document sources so any diagnostic's file resolves.
        let mut source_index: HashMap<PathBuf, std::sync::Arc<SourceFile>> = HashMap::new();
        if let Some(schema) = project.schema.as_deref() {
            for sf in &schema.sources {
                source_index.insert(sf.path().to_path_buf(), sf.clone());
            }
        }
        for doc in &project.documents.docs {
            source_index.insert(doc.source.path().to_path_buf(), doc.source.clone());
        }

        // Each entry: (file_path, file_diags). We collect first per source
        // file (so `by_file` reflects the file the diagnostics point at), then
        // merge into the result's `all` and `by_file`.
        let mut all: Vec<Diagnostic> = Vec::new();
        // `by_file` collects per physical input path; an input path that
        // dedups to another path's content has its own clone of the doc's
        // diagnostics (see ProjectLintResult docs).
        let mut by_file: HashMap<PathBuf, Vec<Diagnostic>> =
            HashMap::with_capacity(source_index.len());

        // 1. Schema source files (walked for schema-rules; operation rules
        //    self-filter by kind since CST has no OPERATION_DEFINITION here).
        if let Some(schema) = project.schema.as_deref() {
            for sf in &schema.sources {
                let file_diags = lint_one_file(
                    sf.as_ref(),
                    &self.rules,
                    schema_ref,
                    siblings,
                    &project.config,
                    &[],
                );
                index_into(&mut by_file, &mut all, sf.path(), file_diags);
            }
        }

        // 2. Operation documents. Parse errors from each LoadedDocument are
        //    emitted *before* rule handlers run (spec step 1).
        for doc in &project.documents.docs {
            let sf = doc.source.as_ref();
            let file_diags = lint_one_file(
                sf,
                &self.rules,
                schema_ref,
                siblings,
                &project.config,
                &doc.parse_errors,
            );
            index_into(&mut by_file, &mut all, sf.path(), file_diags);
        }

        // 3. Aliased paths (deduped to the same content hash) share the same
        //    diagnostics as their owning doc. Mirror the owning file's vec
        //    into every alias slot in `project.documents.by_file`.
        for (path, &idx) in &project.documents.by_file {
            if let Some(owner) = project.documents.docs.get(idx) {
                let owner_path = owner.source.path().to_path_buf();
                if &owner_path != path {
                    if let Some(v) = by_file.get(&owner_path).cloned() {
                        by_file.insert(path.clone(), v);
                    }
                }
            }
        }

        // 4. Severity filter: drop `Severity::Off` diagnostics. Done late so
        //    a rule that reports `Off`-stamped diagnostics (e.g. via a
        //    severity override) still drains cleanly; the filter is on the
        //    final aggregate. (Off-configured rules also still run — a rule
        //    id revoked from config never appears here because LintEngine::new
        //    only enabled rules in the config; a rule enabled with severity
        //    `Off` is intentionally run-but-silenced.)
        all.retain(|d| d.severity != Severity::Off);
        for v in by_file.values_mut() {
            v.retain(|d| d.severity != Severity::Off);
        }

        // 5. Stable sort by (file, line, column, rule_id). Line/column derive
        //    from the file's SourceFile, so resolve via the source index. Use
        //    a stable sort key (path / line / column / rule_id bytes) so
        //    configurations ordering equal-key diagnostics deterministically
        //    by emission order.
        all.sort_by_key(|d| sort_key(&source_index, d));

        Ok(ProjectLintResult {
            project_name: project.config.name.clone(),
            by_file,
            all,
            sources: source_index,
        })
    }
}

/// Build the sort key `(file_path, line, column, rule_id)` for a diagnostic,
/// resolving `(line, column)` through `source_index` so 1-based line/column is
/// used (matching the spec's "(file, line, column, rule_id)" ordering).
fn sort_key(
    source_index: &HashMap<PathBuf, std::sync::Arc<SourceFile>>,
    d: &Diagnostic,
) -> (PathBuf, usize, usize, String) {
    let (line, column) = source_index
        .get(&d.file)
        .map(|sf| {
            let lc = sf.line_col(d.span.offset);
            (lc.line, lc.column)
        })
        .unwrap_or((0, 0));
    (d.file.clone(), line, column, d.rule_id.clone())
}

/// Insert a file's diagnostics both into `by_file` under the file's path and
/// extend the aggregate `all`.
fn index_into(
    by_file: &mut HashMap<PathBuf, Vec<Diagnostic>>,
    all: &mut Vec<Diagnostic>,
    path: &std::path::Path,
    file_diags: Vec<Diagnostic>,
) {
    all.extend(file_diags.iter().cloned());
    by_file.insert(path.to_path_buf(), file_diags);
}

/// Walk one source file's CST once, dispatching to every rule whose
/// `interested_kinds` contains the visited node's kind. Emits `parse_errors`
/// as the first diagnostics of the file (spec step 1). Returns the file's
/// diagnostics in their emission order (rule emission order; the global sort
/// happens in [`LintEngine::lint`]).
///
/// Operation files pass their own pre-computed `parse_errors` (from
/// [`LoadedDocument::parse_errors`]); schema source files pass `&[]` (their
/// parse errors are surfaced separately as the schema is loaded — spec-004 —
/// and radiate out via the schema's own `parse_errors` field if needed by a
/// later spec).
///
/// The `schema` argument is what `RuleContext::schema` will report for this
/// file — the *project's* schema, regardless of which source file is being
/// walked (so a `requires_schema` rule still sees it when walking an operation
/// document). Rules decide what to do with `ctx.schema`.
#[allow(clippy::too_many_arguments)]
fn lint_one_file(
    file: &SourceFile,
    rules: &[EnabledRule],
    schema: Option<&apollo_compiler::Schema>,
    siblings: Option<&crate::siblings::Siblings>,
    project_config: &crate::project::ProjectConfig,
    parse_errors: &[Diagnostic],
) -> Vec<Diagnostic> {
    let mut diags: Vec<Diagnostic> = parse_errors.to_vec();

    // Skip rules whose preconditions are unmet *for this project*.
    let active: Vec<&EnabledRule> = rules
        .iter()
        .filter(|r| {
            if r.entry.meta.requires_schema && schema.is_none() {
                return false;
            }
            if r.entry.meta.requires_siblings && siblings.is_none() {
                return false;
            }
            true
        })
        .collect();

    if active.is_empty() {
        return diags;
    }

    // The handlers + contexts are constructed here, before the walk, so each
    // rule gets one handler per file. Each `(handler, ctx)` pair is held in a
    // `ActiveRule` so we can drain ctx into the file's diagnostics after the
    // walk + finalize.
    // Build a fresh `(handler, ctx)` per active rule, then drop the boxed
    // `Rule` instance — handlers copy what they need at `create` time so they
    // don't borrow from the rule (or the context) across the walk.
    let mut actives: Vec<ActiveRule> = Vec::with_capacity(active.len());
    let mut interested_set: Vec<SyntaxKind> = Vec::new();
    for r in active {
        let mut ctx = RuleContext::new(
            file,
            schema,
            siblings,
            project_config,
            &r.options,
            r.entry.meta.id,
            r.severity,
        );
        let rule = (r.entry.factory)();
        let handler = rule.create(&mut ctx);
        for &k in r.entry.interested_kinds {
            if !interested_set.contains(&k) {
                interested_set.push(k);
            }
        }
        actives.push(ActiveRule {
            entry: r.entry,
            handler,
            ctx,
        });
    }

    // Re-parse the source so we have a rowan CST to walk generically. The
    // pre-parsed apollo-compiler document is what rules read; this CST is only
    // for dispatch by SyntaxKind. Errors here are ignored — any genuine parse
    // error was already surfaced by spec-004/005 as a `parse-error` diagnostic.
    let tree = Parser::new(file.source()).parse();
    let root = tree.document();
    let root_node = root.syntax().clone();

    walk_node(&root_node, None, &mut actives, &interested_set);

    // Finalize each handler and drain its diagnostics into the file's vec.
    for mut a in actives {
        a.handler.finalize(&mut a.ctx);
        diags.extend(a.ctx.take_diagnostics());
    }

    diags
}

/// A per-rule, per-file active handler and its context, held together so the
/// walk can dispatch into the handler and the engine can drain the context
/// after `finalize`.
struct ActiveRule<'a> {
    /// The static registry entry so the walk can check `interested_kinds`.
    entry: &'static RuleEntry,
    handler: Box<dyn Handler + 'a>,
    ctx: RuleContext<'a>,
}

/// Recursive pre-order walk over `syn`. For each node whose kind is in
/// `interested_set`, dispatch `on_node` to every active rule whose
/// `interested_kinds` contains the kind. `parent_view` is the parent
/// [`Node`] view (already constructed by the caller) so rule handlers can
/// walk ancestor links via [`Node::parent`]; `None` at the root.
fn walk_node(
    syn: &SyntaxNode,
    parent_view: Option<&Node<'_>>,
    actives: &mut [ActiveRule<'_>],
    interested_set: &[SyntaxKind],
) {
    let kind = syn.kind();
    // Populate the node view with the CST-derived name (the first NAME token
    // child, if any) and the byte span. The name is owned (`String`) because
    // `apollo-parser`'s rowan-backed `SyntaxToken::text` borrows from a
    // short-lived token handle, so we copy out the identifier we need; the
    // span is `Copy` so it rides for free. Rules consult `node.name` to
    // distinguish named vs anonymous definitions (spec-016
    // `no-anonymous-operations` is the first consumer) and `node.span` to
    // report at the offending location.
    let span = Span::from_syntax_node(syn);
    let name = extract_name(syn);
    let mut node_view = Node::new(kind);
    if let Some(n) = name {
        node_view = node_view.with_name(n);
    }
    node_view = node_view.with_span(span);
    let node_view = node_view.with_parent_opt(parent_view);

    if interested_set.contains(&kind) {
        for a in actives.iter_mut() {
            if a.entry.interested_kinds.contains(&kind) {
                a.handler.on_node(&node_view, parent_view);
            }
        }
    }

    // Recurse into children. Each child's parent view is `&node_view`, which
    // lives in this stack frame for the duration of the recursive call.
    for child in syn.children() {
        walk_node(&child, Some(&node_view), actives, interested_set);
    }
}

/// Extract the identifier text of the first `NAME` child of `syn`, if present,
/// as an owned `String`. Most GraphQL CST kinds that *can* bear a name (every
/// definition kind, `Field`, `FragmentSpread`, `Argument`, `NamedType`, …)
/// attach it as a single `NAME` child node whose `IDENT` token carries the
/// text; anonymous `OperationDefinition` (no `Name`) and inherently nameless
/// kinds (`Selection`, `SelectionSet`, `Arguments`, …) yield `None`.
///
/// The text is owned out (rather than `&'a str` borrowing from `syn`) because
/// `apollo-parser`'s rowan-backed `SyntaxToken::text` borrows from the token
/// handle, not the long-lived green tree; the identifier is short so the
/// allocation is negligible (rules visit at most a few hundred nodes per file).
fn extract_name(syn: &SyntaxNode) -> Option<String> {
    for child in syn.children() {
        if child.kind() == SyntaxKind::NAME {
            // The `NAME` node wraps a single `IDENT` token whose `text()` is
            // the identifier; scan its children-with-tokens for the first
            // `Token` element. A well-formed `NAME` always has exactly one
            // child `IDENT` token, so this loop hits on the first iteration.
            for token in child.children_with_tokens() {
                if let apollo_parser::SyntaxElement::Token(t) = token {
                    return Some(t.text().to_owned());
                }
            }
            return None;
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn engine_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<LintEngine>();
        assert_send_sync::<ProjectLintResult>();
        assert_send_sync::<EnabledRule>();
        assert_send_sync::<RulesConfig>();
        assert_send_sync::<LintEngineError>();
    }

    #[test]
    fn unknown_rule_id_resolves_to_error() {
        let cfg = RulesConfig {
            rules: vec![RuleConfig {
                id: "does-not-exist".to_owned(),
                severity: Severity::Warn,
                options: serde_json::Value::Null,
            }],
        };
        let err = LintEngine::new(&cfg).expect_err("unknown rule must error");
        assert!(matches!(err, LintEngineError::UnknownRule { .. }));
        assert!(format!("{err}").contains("does-not-exist"));
    }

    #[test]
    fn empty_config_yields_engine_with_no_rules() {
        let engine = LintEngine::new(&RulesConfig::default()).expect("empty config ok");
        assert!(engine.rules.is_empty());
    }
}
