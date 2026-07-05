//! The parity record (`ExpectedError`) and the [`Comparator`] that checks the
//! engine's actual diagnostics against the fixture's `expected.json`.
//!
//! ## The relaxed byte-offset rule (PLAN §6.3)
//!
//! `graphql-eslint` reports `loc.column` as a 0-based byte offset derived from
//! `graphql-js` token `start`. We mirror that via
//! [`SourceFile::location_eslint`][rglint_core::SourceFile::location_eslint]
//! (byte-based, 0-based column). What the harness deliberately does **not**
//! compare is the raw byte `offset`: two parsers may differ by a few bytes on
//! where a node "starts" (e.g. whether leading trivia counts). PLAN §6.3
//! therefore mandates comparing **line + column** only, and that is what
//! [`Comparator::compare`] does.
//!
//! ## `loose_message` (PLAN §6.3, spec-053)
//!
//! Message-verbatim parity is the strictest assertion. Some `graphql-js` spec
//! rules phrase messages differently from `graphql-eslint`; spec-053 marks
//! those fixtures with `loose_message = true` in `config.toml`, which makes the
//! comparator skip the message and compare rule + location only. The flag is
//! recorded on [`Comparator::loose_message`] and honored by
//! [`Comparator::compare`].

use std::path::Path;

use rglint_core::{Diagnostic, SourceFile};

/// One expected parity error, mirroring PLAN §6.1's `expected.json` shape:
/// `{ rule, message, line, column }`.
///
/// `line` is 1-based; `column` is 0-based (`graphql-eslint` style), matching
/// what [`SourceFile::location_eslint`][rglint_core::SourceFile::location_eslint]
/// returns for an actual diagnostic's span.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ExpectedError {
    /// The rule id that should have produced this diagnostic
    /// (e.g. `"no-anonymous-operations"` or `"parse-error"`).
    pub rule: String,
    /// The verbatim message. Ignored when [`Comparator::loose_message`] is true.
    pub message: String,
    /// 1-based line number.
    pub line: usize,
    /// 0-based column (byte offset within the line), `graphql-eslint` style.
    pub column: usize,
}

/// A structured diff between expected and actual parity records, rendered as a
/// readable multi-line string by [`ParityDiff::render`] (used by
/// [`run_fixture`][crate::run_fixture] on mismatch).
///
/// Construct with [`ParityDiff::new`]; the diff is computed eagerly so callers
/// can both render it and inspect its [`summary`][Self::summary] in tests.
#[derive(Debug)]
pub struct ParityDiff {
    lines: Vec<String>,
}

impl ParityDiff {
    /// Build a diff for a single `(line, column)` slot whose expected and
    /// actual records disagree. The `index` is the 0-based position in the
    /// parity sequence (used as a label).
    pub fn slot(
        index: usize,
        expected: &ExpectedError,
        actual: Option<&ActualError>,
        loose_message: bool,
    ) -> Self {
        let mut lines = Vec::new();
        lines.push(format!("parity slot {index} mismatch:"));
        lines.push(format!(
            "  expected: rule={rule} line={line} column={column} message={msg:?}",
            rule = expected.rule,
            line = expected.line,
            column = expected.column,
            msg = if loose_message {
                "<loose_message: ignored>"
            } else {
                expected.message.as_str()
            },
        ));
        match actual {
            Some(a) => {
                lines.push(format!(
                    "  actual:   rule={rule} line={line} column={column} message={msg:?}",
                    rule = a.rule,
                    line = a.line,
                    column = a.column,
                    msg = a.message,
                ));
                if expected.rule != a.rule {
                    lines.push("  - rule id differs".to_owned());
                }
                if !loose_message && expected.message != a.message {
                    lines.push("  - message differs (verbatim parity required)".to_owned());
                }
                if expected.line != a.line || expected.column != a.column {
                    lines.push("  - location differs (line/column)".to_owned());
                }
            }
            None => lines.push("  actual:   <missing>".to_owned()),
        }
        Self { lines }
    }

    /// Build a count-only diff (extra or missing diagnostics with no slot pairing).
    pub fn count(expected: usize, actual: usize) -> Self {
        Self {
            lines: vec![
                format!("diagnostic count mismatch: expected {expected}, got {actual}"),
                "  (cannot slot-pair differing counts)".to_owned(),
            ],
        }
    }

    /// Build a diff from an arbitrary list of pre-rendered lines. Used by the
    /// runner to layer a `pretty_assertions` body on top of a structured diff.
    pub(crate) fn from_lines(lines: Vec<String>) -> Self {
        Self { lines }
    }

    /// A one-line summary (the first line of the diff). Used for quick assertions.
    pub fn summary(&self) -> &str {
        self.lines.first().map(|s| s.as_str()).unwrap_or("")
    }

    /// Render the full multi-line diff. Ends with a trailing newline so it
    /// composes cleanly into error messages.
    pub fn render(&self) -> String {
        let mut out = self.lines.join("\n");
        out.push('\n');
        out
    }
}

impl std::fmt::Display for ParityDiff {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.render())
    }
}

/// The actual diagnostic, projected to the parity fields for comparison. This
/// is the mirror of [`ExpectedError`] computed from a real [`Diagnostic`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ActualError {
    pub rule: String,
    pub message: String,
    pub line: usize,
    pub column: usize,
}

/// The fixture's expected vs. actual comparison state. Built from the expected
/// slice + the engine's actual diagnostics resolved through their [`SourceFile`]
/// if any; otherwise projected to `(line=0, column=0)` for unfiled diagnostics.
pub struct Comparator<'a> {
    /// The expected parity records, in `expected.json` order.
    pub expected: &'a [ExpectedError],
    /// When true, skip the message comparison (spec-053 `loose_message`).
    pub loose_message: bool,
}

impl<'a> Comparator<'a> {
    /// Construct a comparator over `expected` with `loose_message` from the
    /// case's [`FixtureConfig`][crate::FixtureConfig].
    pub fn new(expected: &'a [ExpectedError], loose_message: bool) -> Self {
        Self {
            expected,
            loose_message,
        }
    }

    /// Compare `actual` (already sorted the way the engine emits them) against
    /// [`Self::expected`], slot by slot. Returns `Ok(())` on full parity, or
    /// the first [`ParityDiff`] that disagrees.
    ///
    /// The actual slice is compared **in order**: the engine already stable-sorts
    /// diagnostics by `(file, line, column, rule_id)`, and `graphql-eslint`
    /// `expected.json` lists errors in line/column order, so slot `i` of one
    /// lines up with slot `i` of the other. Count must also match.
    pub fn compare(&self, actual: &[ActualError]) -> Result<(), ParityDiff> {
        if self.expected.len() != actual.len() {
            return Err(ParityDiff::count(self.expected.len(), actual.len()));
        }
        for (i, (exp, act)) in self.expected.iter().zip(actual.iter()).enumerate() {
            if exp.rule != act.rule
                || exp.line != act.line
                || exp.column != act.column
                || (!self.loose_message && exp.message != act.message)
            {
                return Err(ParityDiff::slot(i, exp, Some(act), self.loose_message));
            }
        }
        Ok(())
    }

    /// Compare against zero actual diagnostics — the `valid`-case assertion
    /// (expected is empty and the engine produced nothing).
    pub fn compare_valid(&self, actual: &[ActualError]) -> Result<(), ParityDiff> {
        if actual.is_empty() {
            return Ok(());
        }
        // Report the first stray diagnostic (the engine emitted something on a
        // case expected to be clean).
        let stray = &actual[0];
        Err(ParityDiff {
            lines: vec![
                "valid case produced unexpected diagnostics:".to_owned(),
                format!(
                    "  rule={rule} line={line} column={column} message={msg:?}",
                    rule = stray.rule,
                    line = stray.line,
                    column = stray.column,
                    msg = stray.message,
                ),
                format!("  expected 0 diagnostics, got {}", actual.len()),
            ],
        })
    }
}

/// Project a [`Diagnostic`] into an [`ActualError`] using `source` to resolve
/// its span to `(line, 0-based column)`. If `source` is `None` or its path
/// doesn't match the diagnostic's `file`, the location falls back to
/// `(0, 0)` — the comparator will then naturally flag the mismatch unless the
/// expected fixture also records `(0, 0)` (which it never does for real
/// diagnostics, so this is a faithful "unresolved" sentinel).
///
/// This is the single place the parity projection happens; runner + snapshot
/// share it.
pub fn project_actual(diag: &Diagnostic, source: Option<&SourceFile>) -> ActualError {
    let (line, column) = match source {
        Some(sf) if sf.path() == diag.file.as_path() => {
            let (line, col, _end_line, _end_col) = sf.location_eslint(diag.span);
            (line, col)
        }
        _ => (0, 0),
    };
    ActualError {
        rule: diag.rule_id.clone(),
        message: diag.message.clone(),
        line,
        column,
    }
}

/// Look up the [`SourceFile`] (from the project's loaded schema + documents)
/// whose path matches `file`, so [`project_actual`] can resolve a diagnostic's
/// location. Returns `None` when no source matches (e.g. a diagnostic attributed
/// to `<combined>` which has no line table that lines up with a real node).
pub fn find_source<'s>(
    sources: &'s [std::sync::Arc<SourceFile>],
    file: &Path,
) -> Option<&'s SourceFile> {
    sources
        .iter()
        .find(|sf| sf.path() == file)
        .map(|arc| arc.as_ref())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ex(rule: &str, line: usize, col: usize, msg: &str) -> ExpectedError {
        ExpectedError {
            rule: rule.to_owned(),
            line,
            column: col,
            message: msg.to_owned(),
        }
    }

    fn act(rule: &str, line: usize, col: usize, msg: &str) -> ActualError {
        ActualError {
            rule: rule.to_owned(),
            line,
            column: col,
            message: msg.to_owned(),
        }
    }

    #[test]
    fn equal_sequences_compare_ok() {
        let expected = [ex("r", 1, 0, "m")];
        let actual = [act("r", 1, 0, "m")];
        Comparator::new(&expected, false)
            .compare(&actual)
            .expect("identical -> ok");
    }

    #[test]
    fn count_mismatch_returns_count_diff() {
        let expected = [ex("r", 1, 0, "m")];
        let actual: [ActualError; 0] = [];
        let err = Comparator::new(&expected, false)
            .compare(&actual)
            .expect_err("count mismatch");
        assert!(err.summary().contains("count"));
        assert!(err.render().contains("expected 1"));
    }

    #[test]
    fn message_mismatch_reported_under_strict_mode() {
        let expected = [ex("r", 1, 0, "right")];
        let actual = [act("r", 1, 0, "wrong")];
        let err = Comparator::new(&expected, false)
            .compare(&actual)
            .expect_err("message mismatch");
        assert!(err.render().contains("message differs"));
    }

    #[test]
    fn loose_message_skips_message_field() {
        let expected = [ex("r", 1, 0, "anything")];
        let actual = [act("r", 1, 0, "totally different")];
        Comparator::new(&expected, true)
            .compare(&actual)
            .expect("loose_message ignores message");
    }

    #[test]
    fn location_mismatch_reported_even_under_loose_message() {
        let expected = [ex("r", 1, 0, "x")];
        let actual = [act("r", 2, 0, "x")];
        let err = Comparator::new(&expected, true)
            .compare(&actual)
            .expect_err("location must still match under loose_message");
        assert!(err.render().contains("location differs"));
    }

    #[test]
    fn rule_id_mismatch_reported_even_under_loose_message() {
        let expected = [ex("r1", 1, 0, "x")];
        let actual = [act("r2", 1, 0, "x")];
        let err = Comparator::new(&expected, true)
            .compare(&actual)
            .expect_err("rule id must still match under loose_message");
        assert!(err.render().contains("rule id differs"));
    }

    #[test]
    fn valid_case_with_zero_diagnostics_is_ok() {
        let actual: [ActualError; 0] = [];
        Comparator::new(&[], false)
            .compare_valid(&actual)
            .expect("valid + zero -> ok");
    }

    #[test]
    fn valid_case_with_diagnostics_reports_first() {
        let actual = [act("r", 5, 2, "boom")];
        let err = Comparator::new(&[], false)
            .compare_valid(&actual)
            .expect_err("valid + stray -> err");
        assert!(err.render().contains("unexpected diagnostics"));
        assert!(err.render().contains("boom"));
    }
}
