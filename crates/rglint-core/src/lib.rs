#![allow(dead_code)]

mod context;
mod diagnostics;
mod documents;
mod location;
mod node;
mod project;
mod rule;
mod schema;
mod siblings;
mod source;

pub use context::{RuleContext, RuleContextError};
pub use diagnostics::{Diagnostic, DiagnosticBuilder, Fix, Severity, Suggestion};
pub use documents::{
    DocumentLoadError, DocumentLoader, DocumentSpec, LoadedDocument, LoadedDocuments,
};
pub use location::{LineColumn, Location, Span};
pub use node::Node;
pub use project::{Project, ProjectConfig, ProjectResolveError, ProjectResolver};
pub use rule::{Category, Handler, Rule, RuleEntry, RuleMeta, ALL_RULES};
pub use schema::{LoadedSchema, SchemaLoadError, SchemaLoader, SchemaSpec, PARSE_ERROR_RULE_ID};
pub use siblings::{FragmentDef, OperationDef, Siblings};
pub use source::SourceFile;
