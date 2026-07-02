//! The in-engine diagnostic data model: [`Diagnostic`], [`Severity`],
//! [`Suggestion`], [`Fix`], and [`DiagnosticBuilder`].
//!
//! Rules produce diagnostics via `RuleContext::report` (spec-009) using
//! [`DiagnosticBuilder`]; reporters (pretty/miette spec-057, JSON spec-058,
//! SARIF spec-059) consume [`Diagnostic`]. The model is deliberately kept
//! independent of `miette` — miette is a *renderer*, not the model; spec-057
//! adapts `Diagnostic` into a `miette::Report`. This keeps `rglint-core`
//! compilable without miette in later WASM builds (PLAN §2).
//!
//! `Diagnostic` is `Clone + Send + Sync + serde::Serialize/Deserialize` so the
//! JSON reporter and the parity harness can round-trip it. The
//! `01.expected.json` parity shape (`{rule, message, line, column}`, PLAN §6.1)
//! is produced by `Diagnostic::to_parity_json(source)`, intentionally deferred
//! to the test harness (spec-014) to keep core test-agnostic; this spec only
//! provides `Serialize`.

use std::path::PathBuf;

use crate::location::Span;

/// Severity of a [`Diagnostic`], mirroring eslint's severity levels
/// (PLAN §7: `severity: "off" | "warn" | "error"`).
///
/// `Off` is permitted in the model so configuration can downgrade a rule; the
/// engine (spec-011) filters out `Off` diagnostics before reporting, but the
/// model carries them so config can downgrade. [`Severity::default`] is
/// [`Severity::Warn`], used by [`DiagnosticBuilder`] when a rule does not set
/// an explicit severity — the engine applies the configured / rule-meta
/// severity on top.
#[derive(Copy, Clone, Debug, Default, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum Severity {
    /// Diagnostic suppressed; the engine drops these before reporting.
    Off,
    /// A warning: reported but does not fail the run by default.
    #[default]
    Warn,
    /// An error: reported and fails the run (non-zero exit code).
    Error,
}

/// A machine-applicable fix attached to a [`Suggestion`]. Maps to eslint's
/// `fix`/`suggest` ranges.
///
/// `Replace` with empty `text` is equivalent to [`Fix::Remove`] (see the
/// spec-003 "Behavior" section).
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub enum Fix {
    /// Replace the byte range `span` with `text`.
    Replace {
        /// Byte range to overwrite.
        span: Span,
        /// Replacement text. May be empty (equivalent to [`Fix::Remove`]).
        text: String,
    },
    /// Insert `text` at the byte `offset`.
    Insert {
        /// 0-based byte offset where text is inserted.
        offset: usize,
        /// Text to insert.
        text: String,
    },
    /// Remove the byte range `span`.
    Remove {
        /// Byte range to delete.
        span: Span,
    },
}

/// A human-readable suggestion carrying a [`Fix`]. Reporters render `desc`;
/// `--fix` mode (spec-061) applies `fix`.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct Suggestion {
    /// Description of the suggested change, shown to the user.
    pub desc: String,
    /// The machine-applicable fix.
    pub fix: Fix,
}

/// A lint diagnostic: the unit rules produce and reporters consume.
///
/// Build with [`DiagnosticBuilder`] inside rule code:
///
/// ```
/// # use rglint_core::{DiagnosticBuilder, Fix, Severity, Span};
/// # use std::path::PathBuf;
/// let diag = DiagnosticBuilder::new(
///     "no-deprecated",
///     PathBuf::from("schema.graphql"),
///     Span::new(0, 0),
///     "Field is deprecated",
/// )
/// .severity(Severity::Error)
/// .suggestion("remove it", Fix::Remove { span: Span::new(0, 0) })
/// .finish();
/// assert_eq!(diag.severity, Severity::Error);
/// assert_eq!(diag.suggestions.len(), 1);
/// ```
///
/// `Diagnostic` is `Clone + Send + Sync + serde::Serialize/Deserialize`. The
/// `span` always carries a real (possibly zero-length) span; rules without a
/// node use a zero-length span at file offset 0 (`Span::new(0, 0)`) — column
/// parity for such spans is a TODO handled by the parity harness (spec-014).
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct Diagnostic {
    /// The rule id that produced this diagnostic (e.g. `"no-deprecated"`).
    pub rule_id: String,
    /// Path (or identifier) of the source file the diagnostic belongs to.
    pub file: PathBuf,
    /// Byte span of the offending source range.
    pub span: Span,
    /// Human-readable message, verbatim from the rule (parity-asserted).
    pub message: String,
    /// Severity level.
    pub severity: Severity,
    /// Suggested fixes; may be empty.
    pub suggestions: Vec<Suggestion>,
    /// Rule-specific payload (e.g. a deprecation reason, expected naming
    /// style). Free-form JSON so rules can carry structured detail without the
    /// core model growing a per-rule field.
    pub data: serde_json::Value,
}

/// Fluent builder for [`Diagnostic`], the type rules pass to
/// `RuleContext::report` (spec-009).
///
/// Defaults: severity = [`Severity::Warn`] ([`Severity::default`]), no
/// suggestions, `data = serde_json::Value::Null`. Every builder method consumes
/// and returns `self` for chaining; call [`finish`](Self::finish) to produce
/// the [`Diagnostic`].
pub struct DiagnosticBuilder {
    diag: Diagnostic,
}

impl DiagnosticBuilder {
    /// Begin building a diagnostic for `rule_id` in `file` at `span` with
    /// `message`. `message: impl Into<String>` accepts `&str` or `String`.
    pub fn new(rule_id: &str, file: PathBuf, span: Span, message: impl Into<String>) -> Self {
        Self {
            diag: Diagnostic {
                rule_id: rule_id.to_owned(),
                file,
                span,
                message: message.into(),
                severity: Severity::default(),
                suggestions: Vec::new(),
                data: serde_json::Value::Null,
            },
        }
    }

    /// Set the severity. If unset, defaults to [`Severity::default`] (`Warn`).
    pub fn severity(mut self, s: Severity) -> Self {
        self.diag.severity = s;
        self
    }

    /// Add a suggestion (description + fix).
    pub fn suggestion(mut self, desc: impl Into<String>, fix: Fix) -> Self {
        self.diag.suggestions.push(Suggestion {
            desc: desc.into(),
            fix,
        });
        self
    }

    /// Set the rule-specific `data` payload. If unset, defaults to JSON null.
    pub fn data(mut self, v: serde_json::Value) -> Self {
        self.diag.data = v;
        self
    }

    /// Finalize into a [`Diagnostic`].
    pub fn finish(self) -> Diagnostic {
        self.diag
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn severity_default_is_warn() {
        assert_eq!(Severity::default(), Severity::Warn);
    }

    #[test]
    fn severity_serializes_as_variant_name() {
        assert_eq!(serde_json::to_string(&Severity::Off).unwrap(), "\"Off\"");
        assert_eq!(serde_json::to_string(&Severity::Warn).unwrap(), "\"Warn\"");
        assert_eq!(
            serde_json::to_string(&Severity::Error).unwrap(),
            "\"Error\""
        );
    }

    #[test]
    fn severity_round_trips() {
        for s in [Severity::Off, Severity::Warn, Severity::Error] {
            let json = serde_json::to_string(&s).unwrap();
            let back: Severity = serde_json::from_str(&json).unwrap();
            assert_eq!(back, s);
        }
    }

    #[test]
    fn span_serializes_as_offset_and_len() {
        // Span gained Serialize/Deserialize in this spec so Diagnostic/Fix
        // (which embed it) can round-trip through JSON.
        let s = Span::new(42, 7);
        let json = serde_json::to_string(&s).unwrap();
        assert_eq!(json, "{\"offset\":42,\"len\":7}");
        let back: Span = serde_json::from_str(&json).unwrap();
        assert_eq!(back, s);
    }

    /// A representative diagnostic exercising every field, including both a
    /// `Remove` and a `Replace` suggestion and a non-null `data` payload.
    fn sample_diagnostic() -> Diagnostic {
        DiagnosticBuilder::new(
            "no-deprecated",
            PathBuf::from("/src/schema.graphql"),
            Span::new(10, 4),
            "Field \"name\" is deprecated",
        )
        .severity(Severity::Error)
        .suggestion(
            "Remove the field",
            Fix::Remove {
                span: Span::new(10, 4),
            },
        )
        .suggestion(
            "Rename to fullName",
            Fix::Replace {
                span: Span::new(10, 4),
                text: "fullName".to_owned(),
            },
        )
        .data(serde_json::json!({ "reason": "old", "since": "v2" }))
        .finish()
    }

    #[test]
    fn diagnostic_json_has_expected_field_names() {
        // spec-003 Testing: assert the model's own serde field names. The
        // parity `01.expected.json` shape ({rule,message,line,column}) is
        // produced by `to_parity_json`, deferred to spec-014.
        let val: serde_json::Value = serde_json::to_value(sample_diagnostic()).unwrap();
        let obj = val.as_object().expect("Diagnostic serializes as object");
        for key in [
            "rule_id",
            "file",
            "span",
            "message",
            "severity",
            "suggestions",
            "data",
        ] {
            assert!(obj.contains_key(key), "missing top-level field `{key}`");
        }
        let span = obj.get("span").unwrap().as_object().unwrap();
        assert!(span.contains_key("offset") && span.contains_key("len"));
        let sugg = obj.get("suggestions").unwrap().as_array().unwrap();
        assert_eq!(sugg.len(), 2);
        let first = sugg[0].as_object().unwrap();
        assert!(first.contains_key("desc") && first.contains_key("fix"));
        // serde's default external tagging: { "Remove": { "span": .. } }.
        let fix = first.get("fix").unwrap().as_object().unwrap();
        assert!(fix.contains_key("Remove"), "Fix is externally tagged");
        let data = obj.get("data").unwrap().as_object().unwrap();
        assert!(data.contains_key("reason") && data.contains_key("since"));
    }

    #[test]
    fn diagnostic_round_trips_json() {
        let diag = sample_diagnostic();
        let json = serde_json::to_string(&diag).unwrap();
        let back: Diagnostic = serde_json::from_str(&json).unwrap();
        // Diagnostic has no PartialEq by design; compare canonical JSON to
        // assert full structural round-trip (including suggestions/fixes).
        let json2 = serde_json::to_string(&back).unwrap();
        assert_eq!(json, json2);

        // Field-by-field checks for readability.
        assert_eq!(back.rule_id, diag.rule_id);
        assert_eq!(back.file, diag.file);
        assert_eq!(back.span, diag.span);
        assert_eq!(back.message, diag.message);
        assert_eq!(back.severity, diag.severity);
        assert_eq!(back.suggestions.len(), diag.suggestions.len());
        assert_eq!(back.data, diag.data);
    }

    #[test]
    fn fix_variants_round_trip() {
        let cases = [
            Fix::Replace {
                span: Span::new(1, 2),
                text: "x".to_owned(),
            },
            Fix::Insert {
                offset: 5,
                text: "y".to_owned(),
            },
            Fix::Remove {
                span: Span::new(9, 3),
            },
        ];
        for fix in &cases {
            let json = serde_json::to_string(fix).unwrap();
            let back: Fix = serde_json::from_str(&json).unwrap();
            let json2 = serde_json::to_string(&back).unwrap();
            assert_eq!(json, json2, "Fix round-trip mismatch for {json}");
        }
    }

    #[test]
    fn fix_replace_empty_text_is_valid() {
        // Behavior: `Replace` with empty text is valid (equivalent to `Remove`).
        let fix = Fix::Replace {
            span: Span::new(0, 3),
            text: String::new(),
        };
        let json = serde_json::to_string(&fix).unwrap();
        let back: Fix = serde_json::from_str(&json).unwrap();
        let json2 = serde_json::to_string(&back).unwrap();
        assert_eq!(json, json2);
    }

    #[test]
    fn builder_defaults() {
        let d =
            DiagnosticBuilder::new("r", PathBuf::from("f.graphql"), Span::new(0, 0), "m").finish();
        assert_eq!(d.rule_id, "r");
        assert_eq!(d.file, PathBuf::from("f.graphql"));
        assert_eq!(d.span, Span::new(0, 0));
        assert_eq!(d.message, "m");
        assert_eq!(d.severity, Severity::Warn);
        assert!(d.suggestions.is_empty());
        assert!(d.data.is_null());
    }

    #[test]
    fn builder_fluent_methods_apply() {
        let d = DiagnosticBuilder::new("r", PathBuf::from("f"), Span::new(3, 1), "m")
            .severity(Severity::Error)
            .suggestion(
                "s1",
                Fix::Insert {
                    offset: 0,
                    text: "t".to_owned(),
                },
            )
            .data(serde_json::json!({ "k": 1 }))
            .finish();
        assert_eq!(d.severity, Severity::Error);
        assert_eq!(d.suggestions.len(), 1);
        assert_eq!(d.suggestions[0].desc, "s1");
        assert_eq!(d.data, serde_json::json!({ "k": 1 }));
    }

    #[test]
    fn diagnostic_is_send_sync() {
        // spec-003: Diagnostic is Send + Sync (auto traits); assert at compile
        // time so a future field cannot silently break it.
        fn check<T: Send + Sync>() {}
        check::<Diagnostic>();
    }

    #[test]
    fn zero_length_span_at_offset_zero_is_representable() {
        // Behavior: rules without a node use a zero-length span at offset 0.
        let d = DiagnosticBuilder::new("r", PathBuf::from("f"), Span::new(0, 0), "m").finish();
        assert_eq!(d.span, Span::new(0, 0));
        let json = serde_json::to_string(&d).unwrap();
        assert!(json.contains("\"offset\":0"));
        assert!(json.contains("\"len\":0"));
    }
}
