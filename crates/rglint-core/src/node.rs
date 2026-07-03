//! A thin wrapper over a GraphQL AST node, passed to `Handler::on_node` by the
//! engine (spec-011).
//!
//! This is a deliberately minimal placeholder landed by spec-008 so the
//! `Handler` / `Rule` trait signatures compile; spec-012 (`node_name`, utils)
//! and spec-011 (engine walk) flesh out the real payload (a typed view over
//! the `apollo_compiler` CST carrying the node's [`SyntaxKind`], name, span,
//! and parent link).

use std::marker::PhantomData;

use apollo_parser::SyntaxKind;

/// A borrowed AST node handed to rule handlers during the walk.
///
/// The lifetime `'a` is bound to the originating document / CST. Rules that
/// need typed access to the underlying `apollo_compiler` node will, in later
/// specs, downcast through a method added here; spec-008 only requires the
/// type to exist in the `Handler` signature.
pub struct Node<'a> {
    /// The CST kind of this node (mirrors [`apollo_parser::SyntaxKind`]).
    pub kind: SyntaxKind,
    _phantom: PhantomData<&'a ()>,
}

impl<'a> Node<'a> {
    /// Construct a placeholder node of `kind`. Intended for tests; the engine
    /// (spec-011) constructs the real node view.
    pub const fn new(kind: SyntaxKind) -> Self {
        Self {
            kind,
            _phantom: PhantomData,
        }
    }
}
