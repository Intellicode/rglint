//! The `Rule` / `Handler` traits, the `RuleMeta` descriptor, the `Category`
//! enum, the `RuleEntry` registry record, and the `ALL_RULES` `linkme`
//! distributed slice that aggregates every `#[derive(Rule)]` submission across
//! the workspace — spec-008.
//!
//! `RuleMeta` lives in `rglint-core` (not `rglint-rules`) so that both
//! `rglint-rules` and `rglint-graphql-spec` can reference the trait /
//! descriptor without depending on each other (PLAN §3 cross-crate layout).
//!
//! The registry is a `linkme` distributed slice rather than an explicit array
//! literal so a rule struct annotated with `#[derive(Rule)]` is automatically
//! discoverable by `all_rules()` — no manual list maintenance, no runtime
//! `inventory` init order, and WASM-friendly (no global ctor ordering hazard).
//! See `ARCHITECTURE.md` for the decision record.

use std::sync::OnceLock;

use apollo_parser::SyntaxKind;
use serde::{Deserialize, Serialize};

use crate::context::RuleContext;
use crate::node::Node;
use crate::Severity;

/// Rule category, mirroring graphql-eslint's `Category` (schema / operations /
/// other). Used by docs generation (PLAN §8) and config grouping.
#[derive(Copy, Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub enum Category {
    /// Rules over the type system / schema definition language.
    #[default]
    Schema,
    /// Rules over executable documents (queries / mutations / subscriptions /
    /// fragments).
    Operations,
    /// Rules that don't fit either bucket (e.g. filename conventions, relay
    /// graph specs that span both).
    Other,
}

impl Category {
    /// Parse a category from its lowercase attribute spelling (`"schema"` /
    /// `"operations"` / `"other"`), as used by `#[rule(category = ...)]`.
    pub fn from_kebab(s: &str) -> Option<Self> {
        match s {
            "schema" => Some(Category::Schema),
            "operations" => Some(Category::Operations),
            "other" => Some(Category::Other),
            _ => None,
        }
    }
}

/// A lint rule: produced once at registration, asked for its [`RuleMeta`] and
/// to instantiate a fresh [`Handler`] per document via [`Rule::create`].
///
/// `Send + Sync + 'static` so the engine (spec-011) can hold the registry
/// behind a shared reference and (later, spec-064) parallelize across files.
///
/// `create` returns a `Box<dyn Handler>` (implicitly `+'static`): handlers
/// **do not** borrow from the [`RuleContext`] (or the rule itself) across
/// their lifetime. Rules read what they need at `create` time (e.g. options
/// via [`RuleContext::option`]) and **copy** it into the handler; the spec
/// (Risks / Notes) calls this out — "design handlers to receive node data by
/// value (copy what they need)". This keeps the engine's per-file storage a
/// plain `Vec<ActiveRule>` with no self-referential borrows, and is the
/// template all rules follow.
pub trait Rule: Send + Sync + 'static {
    /// The rule's static metadata descriptor.
    fn meta(&self) -> &'static RuleMeta;
    /// Build a fresh per-document [`Handler`]. The handler does not borrow
    /// from `ctx`; rules copy whatever they need at create time.
    fn create(&self, ctx: &mut RuleContext) -> Box<dyn Handler>;
}

/// A per-document rule handler built by [`Rule::create`]. The engine (spec-011)
/// walks the AST once and calls [`Handler::on_node`] for nodes whose
/// [`SyntaxKind`] is in the rule's [`RuleEntry::interested_kinds`], then
/// [`Handler::finalize`] after the walk.
pub trait Handler {
    /// Called for each AST node the rule declared interest in; the handler
    /// decides whether it actually matches. Default no-op so rules only
    /// override what they need.
    fn on_node(&mut self, _node: &Node<'_>, _parent: Option<&Node<'_>>) {}
    /// Called once after the walk for document-global checks
    /// (e.g. siblings-spanning rules). Default no-op.
    fn finalize(&mut self, _ctx: &mut RuleContext) {}
}

/// Static descriptor for a rule (PLAN §4.1). All fields are `const`-friendly
/// so a `static` instance can be declared by the `Rule` derive; the non-const
/// bits — the compiled JSON-Schema `option_schema` and the parsed
/// `default_options` — are built lazily through [`OnceLock`]s (PLAN §4.1:
/// *"option_schema is built OnceCell-lazy because jsonschema::Validator
/// construction is non-const"*).
///
/// `option_schema` is stored as a [`jsonschema::JSONSchema`] (jsonschema 0.18
/// renamed the public type from `Validator` to `JSONSchema`; the spec wording
/// "Validator" refers to the same concept).
pub struct RuleMeta {
    /// Stable rule identifier, e.g. `"no-anonymous-operations"`.
    pub id: &'static str,
    /// Which document family the rule targets.
    pub category: Category,
    /// Default severity before configuration overrides. The engine (spec-011)
    /// applies the configured severity on top.
    pub severity: Severity,
    /// Human-readable documentation; surfaced by `xtask gen-docs` (PLAN §8).
    pub docs: &'static str,
    /// Raw JSON-Schema source for the rule's `options` object, or `None` when
    /// the rule takes no options. Compiled into `option_schema` lazily.
    option_schema_src: Option<&'static str>,
    /// Lazily-compiled JSON-Schema validator for `options`. Built from
    /// `option_schema_src` on first access via [`RuleMeta::option_schema`].
    option_schema: OnceLock<Option<jsonschema::JSONSchema>>,
    /// Raw JSON source for the rule's default options, or `None`.
    default_options_src: Option<&'static str>,
    /// Lazily-parsed `default_options`. Built from `default_options_src` on
    /// first access via [`RuleMeta::default_options`].
    default_options: OnceLock<Option<serde_json::Value>>,
    /// Whether the rule needs a schema to run; the engine skips it on
    /// schema-less projects.
    pub requires_schema: bool,
    /// Whether the rule needs sibling operations to run; the engine skips it
    /// when no siblings are loaded.
    pub requires_siblings: bool,
    /// Whether the rule is deprecated; config warns / refuses to enable it.
    pub deprecated: bool,
    /// The rule id that replaces a deprecated rule, if any.
    pub replaced_by: Option<&'static str>,
    /// Whether the rule emits [`crate::Suggestion`]s (drives `--fix` ui).
    pub has_suggestions: bool,
}

impl RuleMeta {
    /// `const` constructor usable in `static` initializers (the `Rule` derive
    /// emits a call to this). The non-const options are passed as their JSON
    /// source strings and compiled lazily.
    #[allow(clippy::too_many_arguments)]
    pub const fn new(
        id: &'static str,
        category: Category,
        severity: Severity,
        docs: &'static str,
        option_schema_src: Option<&'static str>,
        default_options_src: Option<&'static str>,
        requires_schema: bool,
        requires_siblings: bool,
        deprecated: bool,
        replaced_by: Option<&'static str>,
        has_suggestions: bool,
    ) -> Self {
        Self {
            id,
            category,
            severity,
            docs,
            option_schema_src,
            option_schema: OnceLock::new(),
            default_options_src,
            default_options: OnceLock::new(),
            requires_schema,
            requires_siblings,
            deprecated,
            replaced_by,
            has_suggestions,
        }
    }

    /// Lazily compile and return the rule's `options` JSON-Schema validator.
    /// Returns `None` when the rule takes no options.
    pub fn option_schema(&self) -> Option<&jsonschema::JSONSchema> {
        self.option_schema
            .get_or_init(|| {
                self.option_schema_src.and_then(|src| {
                    let schema: serde_json::Value = serde_json::from_str(src).ok()?;
                    jsonschema::JSONSchema::compile(&schema).ok()
                })
            })
            .as_ref()
    }

    /// Lazily parse and return the rule's default options.
    pub fn default_options(&self) -> Option<&serde_json::Value> {
        self.default_options
            .get_or_init(|| {
                self.default_options_src
                    .and_then(|src| serde_json::from_str(src).ok())
            })
            .as_ref()
    }
}

impl std::fmt::Debug for RuleMeta {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RuleMeta")
            .field("id", &self.id)
            .field("category", &self.category)
            .field("severity", &self.severity)
            .field("requires_schema", &self.requires_schema)
            .field("requires_siblings", &self.requires_siblings)
            .field("deprecated", &self.deprecated)
            .field("replaced_by", &self.replaced_by)
            .field("has_suggestions", &self.has_suggestions)
            .field("has_option_schema", &self.option_schema_src.is_some())
            .field("has_default_options", &self.default_options_src.is_some())
            .finish()
    }
}

/// A static registry record describing how to instantiate a rule and which AST
/// kinds it wants to be visited for. Aggregated into [`ALL_RULES`] by the
/// `linkme` distributed slice on every `#[derive(Rule)]` submission.
pub struct RuleEntry {
    /// The rule's static metadata.
    pub meta: &'static RuleMeta,
    /// Zero-arg factory constructing a fresh rule instance (boxed so the slice
    /// is uniform).
    pub factory: fn() -> Box<dyn Rule>,
    /// The CST kinds the rule's handler cares about; the engine (spec-011)
    /// skips `on_node` dispatch for nodes not in this list.
    pub interested_kinds: &'static [SyntaxKind],
}

impl std::fmt::Debug for RuleEntry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RuleEntry")
            .field("id", &self.meta.id)
            .field("interested_kinds", &self.interested_kinds)
            .finish_non_exhaustive()
    }
}

/// `linkme` distributed slice aggregating every `#[derive(Rule)]` submission
/// across the workspace. Read via `rglint_rules::all_rules()`.
#[linkme::distributed_slice]
pub static ALL_RULES: [RuleEntry];
