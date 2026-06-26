# Spec-002: SourceFile & Location/Span types

> Plan reference: §3 (`crates/rglint-core/src/source.rs`, `location.rs`), §4.5, §6.3, §8 (off-by-one risk)

## Goal

Implement the foundational source-text abstractions: `SourceFile` (path +
content + line table) and `Span`/`Location` (byte offsets + 1-based line/col).
These are the substrate every diagnostic and rule carries, and the place where
the line/column normalization between apollo-parser (1/1-based) and graphql-eslint
(1-based line / 0-based column) happens.

## Scope

**In scope:**

- `rglint-core::source::SourceFile` — owns path + source string, builds a
  line-start offset table at construction.
- `rglint-core::location::{Span, Location, LineColumn}` — span = byte offset +
  length; `Location` resolves to 1-based line, 1-based column internally with a
  helper to emit graphql-eslint-style (line 1-based, column 0-based).
- Line table: `Vec<usize>` of byte offsets where each line begins (handles `\n`,
  `\r\n`; treat `\r` alone as line break for safety).
- `SourceFile::slice(span) -> &str` and `SourceFile::line_col(span) -> Location`.
- Conversion helpers from `apollo_parser::NodeLocation` → `Span`.

**Out of scope:**

- Diagnostics struct (spec-003).
- Schema/Document loaders (specs 004, 005).

## Dependencies

- spec-001 (workspace).

## Deliverables

- `crates/rglint-core/src/source.rs`
- `crates/rglint-core/src/location.rs`
- Re-exports from `crates/rglint-core/src/lib.rs`.
- Unit tests covering: empty file, no trailing newline, CRLF, unicode (UTF-8
  multi-byte chars — column must be char-based or byte-based? **Decision:
  byte-based to match graphql-eslint's `loc.column` which is byte offset within
  line** — document this).

## Interface / API

```rust
pub struct SourceFile {
    path: PathBuf,
    source: String,
    line_starts: Vec<usize>,
}

impl SourceFile {
    pub fn new(path: PathBuf, source: String) -> Arc<Self>; // Arc for cheap sharing
    pub fn path(&self) -> &Path;
    pub fn source(&self) -> &str;
    pub fn slice(&self, span: Span) -> &str;
    pub fn line_col(&self, offset: usize) -> LineColumn;     // 1-based line, 1-based col
    pub fn location(&self, span: Span) -> Location;
}

pub struct Span { pub offset: usize, pub len: usize }
impl Span {
    pub fn new(offset: usize, len: usize) -> Self;
    pub fn end(&self) -> usize;
    pub fn from_node_location(loc: &NodeLocation) -> Self; // offset+length
}

pub struct LineColumn { pub line: usize, pub column: usize } // both 1-based

pub struct Location {
    pub line: usize,    // 1-based
    pub column: usize,  // 1-based (internal); use `column_eslint()` for 0-based
    pub end_line: usize,
    pub end_column: usize,
}
impl Location {
    pub fn column_eslint(&self) -> usize { self.column.saturating_sub(1) }
}

impl SourceFile {
    pub fn location_eslint(&self, span: Span) -> (usize, usize, usize, usize)
        // returns (line, col_0based, end_line, end_col_0based) for parity tests
}
```

## Behavior

- Building `SourceFile` precomputes `line_starts` in O(n).
- `line_col` is O(log n) via binary search over `line_starts`.
- Offsets out of range clamp to the last valid position (never panic).
- `Span::from_node_location` uses `NodeLocation::offset()` + `NodeLocation::node()`
  text length (or `end_offset()` if exposed).

## Testing

- Unit tests in `location.rs` / `source.rs` `#[cfg(test)]` mod:
  - CRLF and LF line tables.
  - UTF-8: a line with `é` (2 bytes) — verify column reported as byte offset.
  - Round-trip: `offset → line_col → back` within a line.
- A doctest showing `location_eslint` returns `(2, 9, 2, 13)` for a known span.

## Risks / Notes

- §8 risk "off-by-one in line/column matching graphql-eslint": this spec is the
  single place that normalizes. All downstream code uses `Location` and only the
  parity test harness compares via `location_eslint()`.
- Verify whether apollo-parser exposes `end_offset` or if we must compute from
  `SyntaxKind` token length; if the latter, cache the `SyntaxNode` reference
  temporarily during span construction.
