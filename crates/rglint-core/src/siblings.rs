//! Sibling operations & fragments index (FragmentTracker).
//!
//! Builds the cross-document index of GraphQL operations and fragment
//! definitions across *all* documents in a project so rules can answer
//! "which fragments does this operation use?" and "where is fragment `X`
//! defined?". Mirrors `packages/plugin/src/siblings.ts` and is the contract
//! every rule flagged `requires_siblings: true` (Phase 4 rules, spec-017 / 018,
//! …) depends on.
//!
//! ## Construction & ownership
//!
//! [`Siblings::from_documents`] walks a [`LoadedDocuments`] bundle once and
//! collects, per physical document, every top-level operation and fragment
//! into flat owned lists. Each [`OperationDef`] / [`FragmentDef`] carries:
//!
//! - a cheap `Arc`-cloned [`SourceFile`] (the document it came from — the
//!   cross-file back-ref the spec calls out);
//! - a [`Span`] extracted from the apollo-compiler node location;
//! - an owned clone of the type-annotated AST node
//!   ([`Node<Operation>`][apollo_compiler::Node] /
//!   [`Node<Fragment>`][apollo_compiler::Node]).
//!
//! ### Lifetime note (the spec's "decide during spike")
//!
//! The spec interface sketched storing `ast::FragmentDefinition`, which borrows
//! from an [`ExecutableDocument`]. `apollo_compiler`'s executable AST uses
//! [`Node<T>`][apollo_compiler::Node] — a reference-counted (triomphe `Arc`)
//! smart pointer — so cloning an operation/fragment node is a cheap ref-count
//! bump that carries its own [`SourceSpan`][apollo_compiler::parser::SourceSpan]
//! and is fully owned (no borrow tying us to the source `ExecutableDocument`).
//! We therefore *copy the nodes out* rather than hold
//! `Arc<ExecutableDocument>`s alongside, sidestepping the lifetime gymnastics
//! the spec worried about without sacrificing attribution (the `Arc<SourceFile>`
//! per def still tells us which file each def came from).
//!
//! ## Fragment name collisions
//!
//! Two documents defining a fragment with the same name is a
//! `unique-fragment-name` violation (spec-017). That rule fires separately;
//! here we just index, with **last-wins** semantics for the indexed
//! [`FragmentDef`] — the later document in iteration order overwrites the
//! earlier entry. No warning is logged here: there is no logger in
//! `rglint-core` today, and spec-017 owns duplicate detection. If/when core
//! gains logger infra, surface a `warn` here at construction time.
//!
//! ## `get_fragments_in_use` semantics
//!
//! A recursive walk over an operation's selection set, collecting every
//! [`FragmentSpread`][apollo_compiler::executable::FragmentSpread] and, when
//! its target is indexed, descending into *that* fragment's selection set in
//! turn (recursive by default, matching `siblings.ts`). A `HashSet<String>`
//! visited guard terminates cycles (`A → B → A`), and the result preserves
//! *insertion order of first encounter* (topologically stable) — relying on
//! `IndexMap` ordering inside `ExecutableDocument::fragments` being stable and
//! on our recursion order matching the source spread order.

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;

use apollo_compiler::executable::{Fragment, Operation, Selection, SelectionSet};
use apollo_compiler::Node;

use crate::documents::{LoadedDocument, LoadedDocuments};
use crate::location::Span;
use crate::source::SourceFile;

/// An indexed fragment definition: its name, source file, span, and the
/// type-annotated AST node it was parsed from.
///
/// Cloning the [`Node`] is a cheap ref-count bump; the node is owned, so a
/// [`FragmentDef`] is not tied to the lifetime of the document it came from.
#[derive(Debug, Clone)]
pub struct FragmentDef {
    /// The fragment's GraphQL name.
    pub name: String,
    /// The physical source file the fragment was parsed from.
    pub source: Arc<SourceFile>,
    /// Byte span of the fragment definition node within `source`.
    pub span: Span,
    /// The type-annotated AST node.
    pub node: Node<Fragment>,
}

/// An indexed operation definition: its (optional) name, source file, span,
/// and the type-annotated AST node it was parsed from.
#[derive(Debug, Clone)]
pub struct OperationDef {
    /// The operation's GraphQL name (`None` for an anonymous operation).
    pub name: Option<String>,
    /// The physical source file the operation was parsed from.
    pub source: Arc<SourceFile>,
    /// Byte span of the operation definition node within `source`.
    pub span: Span,
    /// The type-annotated AST node.
    pub node: Node<Operation>,
}

/// The cross-document index of operations and fragments across every document
/// in a project.
///
/// Build one with [`Siblings::from_documents`]; query with
/// [`get_fragments_in_use`][Self::get_fragments_in_use],
/// [`get_operation_by_name`][Self::get_operation_by_name],
/// [`get_fragment_by_name`][Self::get_fragment_by_name],
/// [`operations`][Self::operations], or [`fragments`][Self::fragments].
#[derive(Debug)]
pub struct Siblings {
    operations: Vec<OperationDef>,
    fragments: HashMap<String, FragmentDef>,
    /// `path -> index into [`Self::sources`]` for every resolved input file
    /// (before content-hash dedup; multiple paths may share one index when
    /// their content hashes collided during loading).
    doc_by_file: HashMap<PathBuf, usize>,
    /// One [`Arc<SourceFile>`] per loaded document, in the same order as
    /// [`LoadedDocuments::docs`] at construction time. Indexed by
    /// [`Self::doc_by_file`]'s values so the file→document mapping is
    /// resolvable from `self` alone.
    sources: Vec<Arc<SourceFile>>,
}

impl Siblings {
    /// Build the sibling index from a [`LoadedDocuments`] bundle.
    ///
    /// Walks every loaded document, flattening its operations and fragments
    /// into owned [`OperationDef`] / [`FragmentDef`] lists. Iteration order
    /// follows [`LoadedDocuments::docs`] (which is the loader's deterministic
    /// input order), and is the basis for the *first-encounter* ordering of
    /// [`Self::get_fragments_in_use`] and for last-wins collision resolution.
    pub fn from_documents(docs: &LoadedDocuments) -> Self {
        let mut operations: Vec<OperationDef> = Vec::new();
        let mut fragments: HashMap<String, FragmentDef> = HashMap::new();
        let mut sources: Vec<Arc<SourceFile>> = Vec::with_capacity(docs.docs.len());

        for LoadedDocument {
            source, document, ..
        } in &docs.docs
        {
            let source = source.clone();
            // Operations: anonymous first (if present) then named, in document
            // order — matches the GraphQL spec's "single anonymous operation"
            // invariant and `OperationMap`'s layout.
            if let Some(op) = document.operations.anonymous.as_ref() {
                operations.push(OperationDef {
                    name: None,
                    source: source.clone(),
                    span: span_of(op),
                    node: op.clone(),
                });
            }
            for (_name, op) in &document.operations.named {
                operations.push(OperationDef {
                    name: op.name.as_ref().map(|n| n.as_str().to_owned()),
                    source: source.clone(),
                    span: span_of(op),
                    node: op.clone(),
                });
            }
            // Fragments: last-wins on name collision (see module docs).
            for (_name, frag) in &document.fragments {
                fragments.insert(
                    frag.name.as_str().to_owned(),
                    FragmentDef {
                        name: frag.name.as_str().to_owned(),
                        source: source.clone(),
                        span: span_of(frag),
                        node: frag.clone(),
                    },
                );
            }

            sources.push(source);
        }

        // `LoadedDocuments::by_file` already maps every resolved input path
        // to its document's index (the very index we use here into `sources`),
        // so we lift it verbatim.
        let doc_by_file = docs.by_file.clone();

        Self {
            operations,
            fragments,
            doc_by_file,
            sources,
        }
    }

    /// Resolve the fragments transitively used by `op`, in first-encounter
    /// (source spread) order. Cycles (`A → B → A`) are terminated by a
    /// visited-set guard. Returns empty when `op` spreads no indexed
    /// fragments.
    pub fn get_fragments_in_use(&self, op: &Operation) -> Vec<&FragmentDef> {
        let mut visited: HashSet<String> = HashSet::new();
        let mut out: Vec<&FragmentDef> = Vec::new();
        self.collect_fragments(&op.selection_set, &mut visited, &mut out);
        out
    }

    /// Like [`Self::get_fragments_in_use`] but starting from a fragment's
    /// selection set — useful for rules asking "which fragments does fragment
    /// `X` (transitively) pull in?".
    pub fn get_fragments_in_use_for_fragment(&self, frag: &Fragment) -> Vec<&FragmentDef> {
        let mut visited: HashSet<String> = HashSet::new();
        // The fragment itself is implicitly "in use"; mark it visited so a
        // self-referential cycle terminates without re-emitting.
        visited.insert(frag.name.as_str().to_owned());
        let mut out: Vec<&FragmentDef> = Vec::new();
        self.collect_fragments(&frag.selection_set, &mut visited, &mut out);
        out
    }

    /// Recursive core of the fragment collector. Walks `selection_set`,
    /// resolving [`FragmentSpread`][apollo_compiler::executable::FragmentSpread]s
    /// against the index and descending into matching fragments. Handles fields
    /// and inline fragments (which carry their own selection sets) so nested
    /// spreads resolve transitively. The `visited`/`out` pair guarantees
    /// cycle termination and first-encounter ordering.
    fn collect_fragments<'a>(
        &'a self,
        selection_set: &SelectionSet,
        visited: &mut HashSet<String>,
        out: &mut Vec<&'a FragmentDef>,
    ) {
        for selection in &selection_set.selections {
            match selection {
                Selection::FragmentSpread(spread) => {
                    let fragment_name = spread.fragment_name.as_str();
                    if visited.contains(fragment_name) {
                        continue;
                    }
                    // An unresolved spread (fragment not in the index — e.g.
                    // defined in an unloaded file, or a typo) contributes
                    // nothing; `unique-fragment-name` / other rules own those.
                    let Some(fragment) = self.fragments.get(fragment_name) else {
                        continue;
                    };
                    visited.insert(fragment_name.to_owned());
                    out.push(fragment);
                    // Recurse into the spread fragment's selection set so
                    // transitively-referenced fragments resolve too.
                    self.collect_fragments(&fragment.node.selection_set, visited, out);
                }
                Selection::InlineFragment(inline) => {
                    self.collect_fragments(&inline.selection_set, visited, out);
                }
                Selection::Field(field) => {
                    self.collect_fragments(&field.selection_set, visited, out);
                }
            }
        }
    }

    /// Find an operation by name. Anonymous operations are *not* addressable
    /// by name (they have none); this returns `None` for them.
    pub fn get_operation_by_name(&self, name: &str) -> Option<&OperationDef> {
        self.operations
            .iter()
            .find(|op| op.name.as_deref() == Some(name))
    }

    /// Find an indexed fragment by name (last-wins on cross-file collisions;
    /// see module docs).
    pub fn get_fragment_by_name(&self, name: &str) -> Option<&FragmentDef> {
        self.fragments.get(name)
    }

    /// All indexed operations, in construction (document, then
    /// anonymous→named) order.
    pub fn operations(&self) -> &[OperationDef] {
        &self.operations
    }

    /// Iterate all indexed fragments as `(name, def)` pairs.
    pub fn fragments(&self) -> impl Iterator<Item = (&String, &FragmentDef)> {
        self.fragments.iter()
    }

    /// Look up the [`SourceFile`] for a resolved input path. Returns `None`
    /// when `path` was not part of the document set `self` was built from.
    /// Useful for attributing a diagnostic back to a file via
    /// [`LoadedDocuments::by_file`]'s dedup semantics.
    pub fn source_for_file(&self, path: &std::path::Path) -> Option<Arc<SourceFile>> {
        self.doc_by_file
            .get(path)
            .map(|&idx| self.sources[idx].clone())
    }

    /// True iff no documents were loaded (no operations, no fragments).
    /// Mirrors `siblings.ts`'s `available` flag: when `false`, rules with
    /// `requires_siblings` self-skip.
    pub fn is_available(&self) -> bool {
        !self.sources.is_empty()
    }
}

/// Extract a [`Span`] from an apollo-compiler node's optional location,
/// falling back to a zero span when the node carries no source location
/// (e.g. programmatically constructed nodes).
fn span_of<T: ?Sized>(node: &Node<T>) -> Span {
    node.location()
        .map(|loc| Span::from_node_location(&loc))
        .unwrap_or(Span::new(0, 0))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::documents::{DocumentLoader, DocumentSpec};
    use std::path::{Path, PathBuf};

    fn fixture_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/siblings")
    }

    fn transitive_dir() -> PathBuf {
        fixture_root().join("transitive")
    }

    fn cyclic_dir() -> PathBuf {
        fixture_root().join("cyclic")
    }

    fn load_dir(dir: &Path) -> LoadedDocuments {
        let loader = DocumentLoader::new();
        loader
            .load(&DocumentSpec::Glob("*.graphql".to_owned()), dir, None)
            .expect("fixture load")
    }

    #[test]
    fn transitive_resolution_returns_a_then_b() {
        // spec-006 Testing: `op` uses `A` and `B`; `A` uses `B`. Expect
        // `[A, B]` in first-encounter order.
        let loaded = load_dir(&transitive_dir());
        let siblings = Siblings::from_documents(&loaded);

        // Three documents loaded.
        assert_eq!(loaded.docs.len(), 3);
        // One named operation across the set (GetOp).
        assert_eq!(siblings.operations().len(), 1);
        assert_eq!(siblings.operations()[0].name.as_deref(), Some("GetOp"));
        // Two fragments indexed.
        assert_eq!(siblings.fragments().count(), 2);

        let op = &siblings.operations()[0].node;
        let in_use = siblings.get_fragments_in_use(op);
        let names: Vec<&str> = in_use.iter().map(|f| f.name.as_str()).collect();
        assert_eq!(names, vec!["A", "B"], "transitive resolution order");
    }

    #[test]
    fn cyclic_fragments_terminate() {
        // spec-006 Testing: `A` uses `B`, `B` uses `A` → terminates, returns
        // `[A, B]`.
        let loaded = load_dir(&cyclic_dir());
        let siblings = Siblings::from_documents(&loaded);

        let op = &siblings.operations()[0].node;
        let in_use = siblings.get_fragments_in_use(op);
        let names: Vec<&str> = in_use.iter().map(|f| f.name.as_str()).collect();
        assert_eq!(names, vec!["A", "B"], "cycle must terminate, no repeats");
    }

    #[test]
    fn get_operation_by_name_round_trips() {
        let loaded = load_dir(&transitive_dir());
        let siblings = Siblings::from_documents(&loaded);
        assert!(siblings.get_operation_by_name("GetOp").is_some());
        assert!(siblings.get_operation_by_name("DoesNotExist").is_none());
    }

    #[test]
    fn get_fragment_by_name_round_trips() {
        let loaded = load_dir(&transitive_dir());
        let siblings = Siblings::from_documents(&loaded);
        let a = siblings
            .get_fragment_by_name("A")
            .expect("fragment A indexed");
        assert_eq!(a.name, "A");
        // Source attribution points back at fragA.graphql.
        assert_eq!(a.source.path(), transitive_dir().join("fragA.graphql"));
        assert!(siblings.get_fragment_by_name("Unknown").is_none());
    }

    #[test]
    fn doc_by_file_resolves_source() {
        let loaded = load_dir(&transitive_dir());
        let siblings = Siblings::from_documents(&loaded);
        let op_path = transitive_dir().join("op.graphql");
        let source = siblings
            .source_for_file(&op_path)
            .expect("op.graphql known to siblings");
        assert_eq!(source.path(), op_path);
    }

    #[test]
    fn inline_single_op_resolves_its_only_spread() {
        // A self-contained inline case: a single document spreading one
        // fragment defined in the same inline doc.
        let loader = DocumentLoader::new();
        let src = "fragment U on User { id } query Q { user { ...U } }".to_owned();
        let loaded = loader
            .load(&DocumentSpec::Inline(src), Path::new("ignored"), None)
            .expect("inline load");
        let siblings = Siblings::from_documents(&loaded);
        assert!(siblings.is_available());
        let op = siblings
            .get_operation_by_name("Q")
            .expect("operation Q indexed");
        let in_use = siblings.get_fragments_in_use(&op.node);
        let names: Vec<&str> = in_use.iter().map(|f| f.name.as_str()).collect();
        assert_eq!(names, vec!["U"]);
    }

    #[test]
    fn empty_bundle_is_unavailable() {
        // No documents at all: `is_available` is false, matching siblings.ts's
        // `available` flag and the engine's self-skip for `requires_siblings`.
        let docs = LoadedDocuments {
            docs: Vec::new(),
            by_file: HashMap::new(),
        };
        let siblings = Siblings::from_documents(&docs);
        assert!(!siblings.is_available());
        assert!(siblings.operations().is_empty());
        assert_eq!(siblings.fragments().count(), 0);
    }

    #[test]
    fn unresolved_spread_is_silently_skipped() {
        // A spread whose target isn't indexed (defined in an unloaded file)
        // contributes nothing rather than panicking — `unique-fragment-name`
        // and friends own surfacing that.
        let loader = DocumentLoader::new();
        let src = "query Q { user { ...Missing } }".to_owned();
        let loaded = loader
            .load(&DocumentSpec::Inline(src), Path::new("ignored"), None)
            .expect("inline load");
        let siblings = Siblings::from_documents(&loaded);
        let op = &siblings.operations()[0].node;
        let in_use = siblings.get_fragments_in_use(op);
        assert!(in_use.is_empty(), "unresolved spread yields no fragment");
    }

    #[test]
    fn siblings_and_defs_are_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<Siblings>();
        assert_send_sync::<OperationDef>();
        assert_send_sync::<FragmentDef>();
    }
}
