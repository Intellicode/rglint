//! A thin wrapper over a GraphQL AST node, passed to `Handler::on_node` by the
//! engine (spec-011) and inspected by the selector engine (spec-010).
//!
//! ## Status
//!
//! spec-008 landed a minimal placeholder so the `Rule` / `Handler` trait
//! signatures compile. spec-010 extends `Node` with just enough surface for
//! the selector [`Matcher`](crate::selector::Matcher) to do its work: the
//! CST [`SyntaxKind`], the node's name text (for `[name.value=...]`), an
//! optional description string (for `[description.value=...]`), an optional
//! raw value string (for `[value=...]`), and a back-pointer to the parent
//! node (so `Child` / `Descendant` combinators can walk the ancestor chain
//! from a single `&Node`).
//!
//! spec-011 (engine walk) and spec-012 (`node_name` / typed utils) will
//! replace this with the fully-typed view over the `apollo_compiler` CST —
//! carrying the real AST node, span, source file, etc. The fields added here
//! are chosen to keep that future migration mechanical: each is `Option` and
//! defaults to `None` via [`Node::new`], so existing call sites
//! (`Node::new(SyntaxKind::NAME)` in `context.rs` tests) keep working
//! unchanged.

use std::marker::PhantomData;

use apollo_parser::SyntaxKind;

use crate::location::Span;

/// A borrowed AST node handed to rule handlers during the walk, and to the
/// selector [`Matcher`](crate::selector::Matcher) when testing whether a
/// compiled selector matches.
///
/// The lifetime `'a` is bound to the originating document / CST. The
/// `parent` link — when set — points at another `Node<'a>` with the same
/// lifetime, so the matcher can walk ancestors without re-borrowing.
pub struct Node<'a> {
    /// The CST kind of this node (mirrors [`apollo_parser::SyntaxKind`]).
    pub kind: SyntaxKind,
    /// The node's name text, if it has one (e.g. `"Query"` for an
    /// `OBJECT_TYPE_DEFINITION` named `Query`). `None` for nodes without a
    /// name (e.g. anonymous `OperationDefinition`, `SelectionSet`).
    ///
    /// Owned (rather than `&'a str`) so the engine walk can extract the
    /// identifier from `apollo-parser`'s rowan-backed CST — the textual data
    /// lives in a reference-counted green tree whose borrow is awkward to
    /// surface through `&'a str` across the `SyntaxToken` accessor. The
    /// identifier is short, so the per-node `String` allocation is
    /// negligible; `node_name` widens it again to an owned `String` for
    /// caller convenience.
    pub name: Option<String>,
    /// The node's description text (the string literal preceding it, without
    /// surrounding quotes / block-stripping), if any. Selector engine only
    /// needs *some* text to match `[description.value=...]` against; the
    /// precise trimming rules live in spec-023 (`description-style`).
    pub description: Option<&'a str>,
    /// The node's raw value text (e.g. the literal `42`, `"foo"`, `[1, 2]`)
    /// for value-bearing nodes (`Argument`, `EnumValue`, `DefaultValue`,
    /// …). `None` for non-value nodes.
    pub value_raw: Option<&'a str>,
    /// Back-pointer to the parent node in the walk, set by the engine
    /// (spec-011) when it constructs the `Node` view. The selector
    /// [`Matcher`](crate::selector::Matcher) walks this for `Child` /
    /// `Descendant` combinators.
    pub parent: Option<&'a Node<'a>>,
    /// The byte span of this node in its source file, set by the engine
    /// (spec-011) for every visited node. `None` only on placeholder nodes
    /// built outside the engine (tests, the selector matcher's synthetic
    /// roots); the engine always sets it from the CST `SyntaxNode`.
    pub span: Option<Span>,
    _phantom: PhantomData<&'a ()>,
}

impl<'a> Node<'a> {
    /// Construct a placeholder node of `kind` with no name / description /
    /// value / parent. Intended for tests; the engine (spec-011) constructs
    /// the real node view with the other fields populated.
    pub const fn new(kind: SyntaxKind) -> Self {
        Self {
            kind,
            name: None,
            description: None,
            value_raw: None,
            parent: None,
            span: None,
            _phantom: PhantomData,
        }
    }

    /// Builder: attach a name text to this node.
    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    /// Builder: attach a description text to this node.
    pub const fn with_description(mut self, description: &'a str) -> Self {
        self.description = Some(description);
        self
    }

    /// Builder: attach a raw value text to this node.
    pub const fn with_value_raw(mut self, value_raw: &'a str) -> Self {
        self.value_raw = Some(value_raw);
        self
    }

    /// Builder: attach a parent node (used by the selector matcher for
    /// `Child` / `Descendant` combinators).
    pub const fn with_parent(mut self, parent: &'a Node<'a>) -> Self {
        self.parent = Some(parent);
        self
    }

    /// Builder: conditionally attach a parent node (`None` leaves it unset).
    /// Used by the engine walk (spec-011) at the CST root where there is no
    /// parent.
    pub const fn with_parent_opt(mut self, parent: Option<&'a Node<'a>>) -> Self {
        self.parent = parent;
        self
    }

    /// Builder: attach a span. Set by the engine walk (spec-011) for every
    /// visited node from its CST `SyntaxNode`.
    pub const fn with_span(mut self, span: Span) -> Self {
        self.span = Some(span);
        self
    }
}
