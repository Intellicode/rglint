//! Source location primitives: [`Span`] (byte offset + length) and
//! [`Location`]/[`LineColumn`] (1-based line/column).
//!
//! ## Column semantics: byte-based
//!
//! Columns are **byte offsets within the line**, not character counts. This
//! matches `graphql-eslint`'s `loc.column`, which is derived from the
//! underlying `graphql-js` token `start`/`end` byte offsets. A multi-byte
//! UTF-8 sequence such as `é` (2 bytes) therefore advances the column by 2.
//! `apollo-compiler`'s own [`apollo_compiler::parser::LineColumn`] is
//! character-based and is **not** used for parity comparisons; the
//! [`SourceFile::location_eslint`][crate::SourceFile::location_eslint] helper
//! is the single normalization point used by the parity test harness.
//!
//! ## `NodeLocation` mapping
//!
//! The spec (PLAN.md §3 / §4.5 / §6.3 / §8) references a `NodeLocation` type.
//! Neither `apollo-parser` 0.8 nor `apollo-compiler` 1.32 exposes a type by
//! that name. The canonical "node location" in our dependency stack is
//! [`apollo_compiler::parser::SourceSpan`] (file id + byte range); rules
//! obtain it from AST nodes via `node.location()`. [`Span::from_node_location`]
//! accepts a `&SourceSpan` and drops the file id — file context is carried by
//! [`SourceFile`][crate::SourceFile] separately. For CST-level access where
//! only a [`apollo_parser::SyntaxNode`] is available, use
//! [`Span::from_syntax_node`].

use apollo_compiler::parser::SourceSpan;
use apollo_parser::SyntaxNode;

/// A byte range in a source file: `offset` is the 0-based start byte, `len` is
/// the length in bytes. [`Span::end`] (`offset + len`) is exclusive.
///
/// `Span` is `Serialize`/`Deserialize` (as `{"offset":..,"len":..}`) so that
/// [`Diagnostic`][crate::diagnostics::Diagnostic] and
/// [`Fix`][crate::diagnostics::Fix] — which embed a `Span` — can round-trip
/// through JSON for the JSON reporter (spec-058) and parity harness.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct Span {
    /// 0-based byte offset from the start of the file.
    pub offset: usize,
    /// Length in bytes.
    pub len: usize,
}

impl Span {
    /// Create a new span from a 0-based byte offset and a byte length.
    pub const fn new(offset: usize, len: usize) -> Self {
        Self { offset, len }
    }

    /// The exclusive end byte offset (`offset + len`).
    pub const fn end(&self) -> usize {
        self.offset + self.len
    }

    /// Build a [`Span`] from an [`apollo_compiler::parser::SourceSpan`] — the
    /// "node location" type rules get from `node.location()`. The file id is
    /// dropped: file context is carried by
    /// [`SourceFile`][crate::SourceFile] separately.
    ///
    /// This is the primary conversion used by rule code that holds
    /// `apollo-compiler` AST nodes.
    pub fn from_node_location(loc: &SourceSpan) -> Self {
        Self::new(loc.offset(), loc.node_len())
    }

    /// Build a [`Span`] from a raw [`apollo_parser::SyntaxNode`] (CST-level),
    /// using the node's `text_range()`. Use this when only the untyped CST
    /// node is available (e.g. trivia scanning); prefer
    /// [`from_node_location`][Self::from_node_location] when a
    /// [`SourceSpan`] is at hand.
    pub fn from_syntax_node(node: &SyntaxNode) -> Self {
        let range = node.text_range();
        let start: usize = u32::from(range.start()) as usize;
        let end: usize = u32::from(range.end()) as usize;
        Self::new(start, end - start)
    }
}

/// A 1-based line and 1-based (byte) column pair.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct LineColumn {
    /// 1-based line number.
    pub line: usize,
    /// 1-based column (byte offset within the line + 1).
    pub column: usize,
}

/// A resolved source location: 1-based line/column for both the start and the
/// exclusive end of a [`Span`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Location {
    /// 1-based start line.
    pub line: usize,
    /// 1-based start column (byte-based).
    pub column: usize,
    /// 1-based end line.
    pub end_line: usize,
    /// 1-based end column (byte-based, exclusive).
    pub end_column: usize,
}

impl Location {
    /// The 0-based start column, matching `graphql-eslint`'s `loc.column`.
    pub fn column_eslint(&self) -> usize {
        self.column.saturating_sub(1)
    }

    /// The 0-based exclusive end column, matching `graphql-eslint`'s
    /// `loc.endColumn`.
    pub fn end_column_eslint(&self) -> usize {
        self.end_column.saturating_sub(1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn span_end_is_offset_plus_len() {
        let s = Span::new(5, 3);
        assert_eq!(s.offset, 5);
        assert_eq!(s.len, 3);
        assert_eq!(s.end(), 8);
    }

    #[test]
    fn span_new_is_const_constructible() {
        const S: Span = Span::new(0, 0);
        assert_eq!(S, Span::new(0, 0));
    }

    #[test]
    fn column_eslint_is_zero_based() {
        let loc = Location {
            line: 2,
            column: 10,
            end_line: 2,
            end_column: 14,
        };
        assert_eq!(loc.column_eslint(), 9);
        assert_eq!(loc.end_column_eslint(), 13);
    }

    #[test]
    fn column_eslint_clamps_at_one() {
        let loc = Location {
            line: 1,
            column: 0,
            end_line: 1,
            end_column: 0,
        };
        assert_eq!(loc.column_eslint(), 0);
        assert_eq!(loc.end_column_eslint(), 0);
    }

    #[test]
    fn from_node_location_round_trips_source_span() {
        // Obtain a real SourceSpan from an apollo-compiler AST node, then
        // verify Span::from_node_location faithfully extracts offset+length.
        use apollo_compiler::Schema;
        let src = "type Query { x: Int }";
        let schema = Schema::parse(src, "test.graphql").unwrap();
        let query = schema.types.get("Query").expect("Query type should exist");
        let loc = query.location().expect("Query type has a location");
        let span = Span::from_node_location(&loc);
        assert_eq!(span.offset, loc.offset());
        assert_eq!(span.end(), loc.end_offset());
        assert_eq!(span.len, loc.node_len());
        assert_eq!(span.offset, 0);
        assert_eq!(span.end(), src.len());
    }

    #[test]
    fn from_syntax_node_covers_root_text_range() {
        // Parse with apollo-parser (CST-level) and confirm the root node's
        // span covers the entire input.
        use apollo_parser::cst::CstNode;
        let src = "type Query { x: Int }";
        let parser = apollo_parser::Parser::new(src);
        let tree = parser.parse();
        assert!(tree.errors().next().is_none(), "parse should be error-free");
        let doc = tree.document();
        let span = Span::from_syntax_node(doc.syntax());
        assert_eq!(span.offset, 0);
        assert_eq!(span.end(), src.len());
        assert_eq!(span.len, src.len());
    }
}
