//! The selector engine (spec-010 / PLAN §4.3).
//!
//! graphql-eslint rules declare which AST nodes they care about with an
//! esquery-style selector string — e.g.
//! `ObjectTypeDefinition > FieldDefinition[name.value=/^_/]`. This module
//! compiles such a string once per rule into a [`Matcher`] predicate the
//! engine (spec-011) applies during the AST walk.
//!
//! ## Pipeline
//!
//! `src` → [`lexer`] → [`parser`] → [`SelectorNode`] → [`matcher`] → [`Matcher`].
//! [`compile`] is the single entry point that runs the whole pipeline; the
//! submodule functions are `pub(crate)` so the engine and tests can exercise
//! individual stages.
//!
//! ## Scope (v1)
//!
//! Five features: kind matchers, attribute predicates (`name`, `kind`,
//! `description`, `value`), child (`>`) and descendant (whitespace)
//! combinators, and the `:matches` / `:not` pseudo-classes. PLAN §8
//! defers features no rule uses (sibling / adjacent combinators, other
//! pseudo-classes) until a rule needs them — see `ARCHITECTURE.md`.

mod ast;
mod lexer;
mod matcher;
mod parser;

pub use ast::{AttrKind, AttrOp, AttrValue, SelectorError, SelectorNode};
pub use matcher::Matcher;

#[cfg(test)]
mod tests;

/// Compile a selector source string into a [`Matcher`].
///
/// Errors are returned with a byte offset into `src` (see
/// [`SelectorError::span`]) so the config-error reporter (spec-054) can
/// underline the offending fragment.
pub fn compile(src: &str) -> Result<Matcher, SelectorError> {
    matcher::compile(src)
}

/// Parse a selector source string into a [`SelectorNode`] tree. Exposed for
/// tests / snapshots; rules go through [`compile`].
pub fn parse(src: &str) -> Result<SelectorNode, SelectorError> {
    parser::parse(src)
}
