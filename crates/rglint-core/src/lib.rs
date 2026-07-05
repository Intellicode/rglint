#![allow(dead_code)]

mod context;
mod diagnostics;
mod documents;
mod engine;
mod location;
mod node;
mod node_name;
mod project;
mod rule;
mod schema;
mod selector;
mod siblings;
mod source;
mod utils;

pub use context::{RuleContext, RuleContextError};
pub use diagnostics::{Diagnostic, DiagnosticBuilder, Fix, Severity, Suggestion};
pub use documents::{
    DocumentLoadError, DocumentLoader, DocumentSpec, LoadedDocument, LoadedDocuments,
};
pub use engine::{
    EnabledRule, LintEngine, LintEngineError, ProjectLintResult, RuleConfig, RulesConfig,
};
pub use location::{LineColumn, Location, Span};
pub use node::Node;
pub use node_name::node_name;
pub use project::{Project, ProjectConfig, ProjectResolveError, ProjectResolver};
pub use rule::{Category, Handler, Rule, RuleEntry, RuleMeta, ALL_RULES};
pub use schema::{LoadedSchema, SchemaLoadError, SchemaLoader, SchemaSpec, PARSE_ERROR_RULE_ID};
pub use selector::{
    compile as compile_selector, parse as parse_selector, AttrKind, AttrOp, AttrValue, Matcher,
    SelectorError, SelectorNode,
};
pub use siblings::{FragmentDef, OperationDef, Siblings};
pub use source::SourceFile;
pub use utils::{
    array_default_options, get_document_type, is_field_definition, is_object_type_definition,
    strip_leading_slash, DocumentKind,
};
