//! [`SourceFile`]: owns a source path + text + a precomputed line-start
//! byte-offset table. This is the substrate every diagnostic and rule carries,
//! and the place where line/column normalization between apollo-parser
//! (1/1-based) and graphql-eslint (1-based line / 0-based column) happens.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::location::{LineColumn, Location, Span};

/// A source file: path + text + precomputed line-start byte-offset table.
///
/// Construction is O(n) in the source length (it builds the line table).
/// [`SourceFile::line_col`] is O(log n) via binary search over the table.
/// Construct with [`SourceFile::new`], which returns an [`Arc`] for cheap
/// sharing across rules, diagnostics, and threads.
#[derive(Debug)]
pub struct SourceFile {
    path: PathBuf,
    source: String,
    line_starts: Vec<usize>,
}

impl SourceFile {
    /// Create a new `SourceFile`, wrapped in an [`Arc`] for cheap sharing.
    /// Builds the line-start table in O(n).
    pub fn new(path: PathBuf, source: String) -> Arc<Self> {
        let line_starts = build_line_starts(&source);
        Arc::new(Self {
            path,
            source,
            line_starts,
        })
    }

    /// The filesystem path (or arbitrary identifier) for this source.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// The full source text.
    pub fn source(&self) -> &str {
        &self.source
    }

    /// Returns the source text covered by `span`. Out-of-range offsets are
    /// clamped to the source bounds and to UTF-8 char boundaries; never panics.
    pub fn slice(&self, span: Span) -> &str {
        let end_idx = span.end().min(self.source.len());
        let start_idx = span.offset.min(end_idx);
        let start = self.clamp_to_char_boundary(start_idx);
        let end = self.clamp_to_char_boundary(end_idx);
        let start = start.min(end);
        &self.source[start..end]
    }

    /// Resolves a 0-based byte `offset` to a 1-based [`LineColumn`].
    /// Offsets past end-of-file clamp to the last valid position; never panics.
    pub fn line_col(&self, offset: usize) -> LineColumn {
        let offset = offset.min(self.source.len());
        // Find the last line start <= offset. binary_search returns Ok(i) when
        // offset lands exactly on a line start, Err(i) where i is the index of
        // the next line start (so the containing line is i-1).
        let line_idx = match self.line_starts.binary_search(&offset) {
            Ok(i) => i,
            Err(i) => i.saturating_sub(1),
        };
        let line_start = self.line_starts[line_idx];
        LineColumn {
            line: line_idx + 1,
            column: offset - line_start + 1,
        }
    }

    /// Resolves a [`Span`] to a 1-based [`Location`] (start + exclusive end).
    pub fn location(&self, span: Span) -> Location {
        let start = self.line_col(span.offset);
        let end = self.line_col(span.end());
        Location {
            line: start.line,
            column: start.column,
            end_line: end.line,
            end_column: end.column,
        }
    }

    /// Returns `(line, column_0_based, end_line, end_column_0_based)` for
    /// `graphql-eslint` parity. This is the single normalization point used
    /// by the parity test harness.
    ///
    /// ```rust
    /// use rglint_core::{SourceFile, Span};
    /// use std::path::PathBuf;
    /// // line 1: "type Query {\n"            (13 bytes incl. trailing \n)
    /// // line 2: "         user: String\n"   (9 spaces + "user" + ": String\n")
    /// //          0-based cols: 0123456789...
    /// //          'user' occupies 0-based cols 9..13 (exclusive) on line 2.
    /// let src = "type Query {\n         user: String\n}\n";
    /// let file = SourceFile::new(PathBuf::from("test.graphql"), src.to_string());
    /// // 'user' starts at byte offset 13 (line 2 start) + 9 = 22, length 4.
    /// let span = Span::new(22, 4);
    /// assert_eq!(file.location_eslint(span), (2, 9, 2, 13));
    /// ```
    pub fn location_eslint(&self, span: Span) -> (usize, usize, usize, usize) {
        let loc = self.location(span);
        (
            loc.line,
            loc.column_eslint(),
            loc.end_line,
            loc.end_column_eslint(),
        )
    }

    /// Walk `idx` back to the nearest UTF-8 char boundary, clamped to source
    /// length. Spans from apollo-parser/apollo-compiler are always on char
    /// boundaries, so this is a defensive no-op for real spans.
    fn clamp_to_char_boundary(&self, idx: usize) -> usize {
        let mut idx = idx.min(self.source.len());
        while idx > 0 && !self.source.is_char_boundary(idx) {
            idx -= 1;
        }
        idx
    }
}

/// Build the line-start byte-offset table for `source` in a single O(n) pass.
///
/// - `\n` starts a new line at the byte after it.
/// - `\r\n` starts a new line at the byte after the `\n`.
/// - A lone `\r` (not followed by `\n`) starts a new line at the byte after
///   it (for safety with classic Mac line endings).
/// - Line 1 always starts at offset 0.
fn build_line_starts(source: &str) -> Vec<usize> {
    let bytes = source.as_bytes();
    let mut starts = Vec::new();
    starts.push(0);
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'\n' => {
                starts.push(i + 1);
                i += 1;
            }
            b'\r' => {
                let next = i + 1;
                if next < bytes.len() && bytes[next] == b'\n' {
                    starts.push(next + 1);
                    i = next + 1;
                } else {
                    starts.push(next);
                    i = next;
                }
            }
            _ => i += 1,
        }
    }
    starts
}

#[cfg(test)]
mod tests {
    use super::*;

    fn file(src: &str) -> Arc<SourceFile> {
        SourceFile::new(PathBuf::from("test.graphql"), src.to_string())
    }

    #[test]
    fn empty_file_has_single_line() {
        let f = file("");
        assert_eq!(f.source(), "");
        assert_eq!(f.line_col(0), LineColumn { line: 1, column: 1 });
    }

    #[test]
    fn no_trailing_newline_reports_eof_on_last_line() {
        // "abc" — no newline. EOF (offset 3) should report line 1, col 4
        // (one past the last char, i.e. the exclusive end position).
        let f = file("abc");
        assert_eq!(f.line_col(0), LineColumn { line: 1, column: 1 });
        assert_eq!(f.line_col(2), LineColumn { line: 1, column: 3 });
        assert_eq!(f.line_col(3), LineColumn { line: 1, column: 4 });
    }

    #[test]
    fn lf_line_table() {
        // "ab\ncd\n" -> line starts [0, 3, 6]
        let f = file("ab\ncd\n");
        // line 1: a(0) b(1) \n(2)   -> line 2 starts at 3
        // line 2: c(3) d(4) \n(5)   -> line 3 starts at 6 (empty trailing line)
        assert_eq!(f.line_col(0), LineColumn { line: 1, column: 1 });
        assert_eq!(f.line_col(1), LineColumn { line: 1, column: 2 });
        assert_eq!(f.line_col(3), LineColumn { line: 2, column: 1 });
        assert_eq!(f.line_col(4), LineColumn { line: 2, column: 2 });
        assert_eq!(f.line_col(6), LineColumn { line: 3, column: 1 });
    }

    #[test]
    fn crlf_line_table_does_not_double_count() {
        // "ab\r\ncd" -> line starts [0, 4]. The \r\n is treated as one break,
        // so column math after it is byte-based (not counting \r as a column).
        let f = file("ab\r\ncd");
        assert_eq!(f.line_col(0), LineColumn { line: 1, column: 1 });
        assert_eq!(f.line_col(1), LineColumn { line: 1, column: 2 });
        assert_eq!(f.line_col(4), LineColumn { line: 2, column: 1 });
        assert_eq!(f.line_col(5), LineColumn { line: 2, column: 2 });
    }

    #[test]
    fn lone_cr_is_a_line_break() {
        // "ab\rcd" -> line starts [0, 3] (classic Mac ending treated as a break).
        let f = file("ab\rcd");
        assert_eq!(f.line_col(0), LineColumn { line: 1, column: 1 });
        assert_eq!(f.line_col(3), LineColumn { line: 2, column: 1 });
        assert_eq!(f.line_col(5), LineColumn { line: 2, column: 3 });
    }

    #[test]
    fn utf8_column_is_byte_based() {
        // "é" is 2 bytes (0xC3 0xA9) followed by "\n" (byte offset 2).
        // The \n belongs to line 1 (col 3); line 2 starts at offset 3 (EOF).
        // Note é is ONE character but its second byte reports column 2 — byte,
        // not char, based — matching graphql-eslint's loc.column.
        let f = file("é\n");
        assert_eq!(f.line_col(0), LineColumn { line: 1, column: 1 });
        assert_eq!(f.line_col(1), LineColumn { line: 1, column: 2 });
        assert_eq!(f.line_col(2), LineColumn { line: 1, column: 3 });
        assert_eq!(f.line_col(3), LineColumn { line: 2, column: 1 });
    }

    #[test]
    fn round_trip_offset_to_line_col_within_a_line() {
        let f = file("type Query {\n  hero: String\n}\n");
        // Pick offset 16 (inside line 2: "  hero: String" starts at 13).
        let lc = f.line_col(16);
        assert_eq!(lc, LineColumn { line: 2, column: 4 });
        // The span [13, 13 + line_len) should slice back to the whole line.
        let line_span = Span::new(13, "  hero: String".len());
        assert_eq!(f.slice(line_span), "  hero: String");
    }

    #[test]
    fn slice_clamps_out_of_range_offsets() {
        let f = file("abc");
        // Span past EOF clamps to the whole source.
        assert_eq!(f.slice(Span::new(0, 100)), "abc");
        // Span starting past EOF yields empty.
        assert_eq!(f.slice(Span::new(50, 5)), "");
        // Zero-length span yields empty, never the char at that offset.
        assert_eq!(f.slice(Span::new(2, 0)), "");
        // Single-byte span at the end yields just that char.
        assert_eq!(f.slice(Span::new(2, 1)), "c");
        assert_eq!(f.slice(Span::new(3, 0)), "");
    }

    #[test]
    fn slice_clamps_to_char_boundary() {
        // "é" is 2 bytes. A span starting at byte 1 (mid-char) should clamp
        // back to 0 rather than panic on a non-char-boundary slice.
        let f = file("é");
        assert_eq!(f.slice(Span::new(1, 1)), "é");
    }

    #[test]
    fn location_resolves_start_and_end() {
        // "type Query {\n         user: String\n}\n" — see location_eslint doctest.
        let f = file("type Query {\n         user: String\n}\n");
        let span = Span::new(22, 4); // 'user'
        let loc = f.location(span);
        assert_eq!(
            loc,
            Location {
                line: 2,
                column: 10,
                end_line: 2,
                end_column: 14,
            }
        );
        assert_eq!(f.location_eslint(span), (2, 9, 2, 13));
    }

    #[test]
    fn location_spanning_multiple_lines() {
        // "ab\ncd" — span from offset 1 ('b') to offset 4 ('d').
        let f = file("ab\ncd");
        let span = Span::new(1, 3); // 'b\nc'
        let loc = f.location(span);
        assert_eq!(
            loc,
            Location {
                line: 1,
                column: 2,
                end_line: 2,
                end_column: 2,
            }
        );
        assert_eq!(f.location_eslint(span), (1, 1, 2, 1));
    }
}
