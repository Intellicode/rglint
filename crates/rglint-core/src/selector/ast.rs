//! The selector AST: [`SelectorNode`] and its attribute payload types.
//!
//! Produced by the [`parser`](super::parser) from a selector string and
//! compiled into a [`Matcher`](super::Matcher) by the
//! [`matcher`](super::matcher) module.
//!
//! The shape mirrors graphql-eslint's esquery-style selectors (PLAN §4.3):
//!
//! - `Kind("ObjectTypeDefinition")` — match a node of a given CST kind.
//! - `Child(A, B)` — match a `B` whose **direct** parent matches `A`.
//! - `Descendant(A, B)` — match a `B` with **some** ancestor matching `A`.
//! - `Attribute { target, op, value }` — match a node whose attribute
//!   (`name`, `kind`, `description`, `value`) satisfies the predicate.
//! - `Matches([..])` — `:matches(...)`; matches if any inner selector does.
//! - `Not([..])` — `:not(...)`; matches if none of the inner selectors do.

use regex::Regex;

/// A single parsed selector expression.
#[derive(Debug)]
pub enum SelectorNode {
    /// Match a node of the given CST kind. The string is the graphql-eslint
    /// (camelCase) spelling, e.g. `"ObjectTypeDefinition"`; the matcher
    /// resolves it to an [`apollo_parser::SyntaxKind`] at compile time and
    /// errors on unknown kind names.
    Kind(String),
    /// `A > B` — `B` whose direct parent matches `A`.
    Child(Box<SelectorNode>, Box<SelectorNode>),
    /// `A B` (whitespace) — `B` with some ancestor matching `A`.
    Descendant(Box<SelectorNode>, Box<SelectorNode>),
    /// `[target op value]` — attribute predicate.
    Attribute {
        target: AttrKind,
        op: AttrOp,
        value: AttrValue,
    },
    /// `:matches(...)` — matches if any inner selector matches.
    Matches(Vec<SelectorNode>),
    /// `:not(...)` — matches if none of the inner selectors match.
    Not(Vec<SelectorNode>),
}

/// Which attribute of a node a selector predicate tests. PLAN §4.3 lists
/// these as the four attribute kinds the engine supports in v1.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AttrKind {
    /// The node's name text (e.g. `"Query"` for `type Query { ... }`).
    /// Selector spelling: `name.value`.
    NameValue,
    /// The node's kind, expressed as a camelCase name (e.g.
    /// `"ObjectTypeDefinition"`). Selector spelling: `kind`. Redundant with
    /// [`SelectorNode::Kind`] but supported for parity with esquery's
    /// `[kind=...]` form.
    Kind,
    /// The node's description text. Selector spelling: `description.value`.
    DescriptionValue,
    /// The node's raw value text (e.g. the literal `42`, `"foo"`).
    /// Selector spelling: `value`.
    ValueRaw,
}

/// The comparison a selector attribute predicate applies.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AttrOp {
    /// `=` — string equality.
    Eq,
    /// `=~` — regex match (the value is a [`AttrValue::Regex`]).
    RegexMatch,
}

/// The right-hand side of a selector attribute predicate.
#[derive(Debug, Clone)]
pub enum AttrValue {
    /// A plain string literal (single- or double-quoted in the source).
    Str(String),
    /// A regex literal (`/pattern/` in the source). Compiled eagerly at
    /// parse time so a malformed regex becomes a [`SelectorError`] with the
    /// selector-string offset of the offending literal.
    Regex(Regex),
}

/// Errors raised while compiling a selector string into a [`Matcher`].
///
/// Every variant carries a `span` — the 0-based byte offset into the original
/// selector string where the error was detected — so the config-error
/// reporter (spec-054) can underline the offending fragment. `message` is a
/// short, human-readable description (no source snippet; the reporter adds
/// that from the span).
#[derive(Debug, thiserror::Error)]
pub enum SelectorError {
    /// Lexing failed (e.g. an unterminated string / regex literal).
    #[error("selector lex error at offset {span}: {message}")]
    Lex { span: usize, message: String },
    /// Parsing failed (unexpected token, unclosed `:not(` / `[`, ...).
    #[error("selector parse error at offset {span}: {message}")]
    Parse { span: usize, message: String },
    /// A regex literal could not be compiled.
    #[error("invalid regex at offset {span}: {message}")]
    Regex { span: usize, message: String },
    /// A `Kind(...)` or `[kind=...]` referenced an unknown CST kind name.
    #[error("unknown node kind `{kind}` at offset {span}")]
    UnknownKind { span: usize, kind: String },
}

impl SelectorError {
    /// The 0-based byte offset into the selector string where the error was
    /// detected. Guaranteed present on every variant.
    pub fn span(&self) -> usize {
        match self {
            SelectorError::Lex { span, .. }
            | SelectorError::Parse { span, .. }
            | SelectorError::Regex { span, .. }
            | SelectorError::UnknownKind { span, .. } => *span,
        }
    }
}
