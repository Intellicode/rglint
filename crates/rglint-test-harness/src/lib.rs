//! The reusable test harness that drives rule **parity** against `graphql-eslint`
//! fixtures, plus the `insta` snapshot scaffolding and `proptest` property-test
//! helpers — spec-014 (PLAN §3 `crates/rglint-test-harness/`, §6 Testing
//! Strategy).
//!
//! What lives here, at a glance:
//!
//! - [`fixture`] — parse a `rules-fixtures/<rule-id>/{valid,invalid}/NN/`
//!   triplet (`graphql` + `config.toml` + `expected.json`) into an in-memory
//!   [`FixtureCase`][fixture::FixtureCase].
//! - [`expected`] — the [`ExpectedError`][expected::ExpectedError] parity record
//!   and the [`Comparator`][expected::Comparator] that checks actual diagnostics
//!   against expected with the relaxed byte-offset rule (PLAN §6.3: compare
//!   line + column only, not raw offsets).
//! - [`runner`] — [`load_fixture`][fixture::load_fixture`] /
//!   [`run_fixture`][runner::run_fixture`] that lints the case and asserts
//!   parity, producing a readable diff on mismatch via `pretty_assertions`. The
//!   [`rglint_test_suite!`][runner::rglint_test_suite] macro discovers all
//!   fixtures under `rules-fixtures/<rule-id>/` and runs one case per entry.
//! - [`snapshot`] — [`assert_diagnostic_snapshot`][snapshot::assert_diagnostic_snapshot],
//!   an `insta` helper rendering the source with `^^^` carets + messages (the
//!   format the `pretty` reporter uses, spec-057).
//! - [`property`] — [`prop_parse_roundtrip`][property::prop_parse_roundtrip]
//!   and the [`assert_no_panic`][property::assert_no_panic] negative-path helper.
//!
//! The harness is engine-agnostic: it builds a fresh [`LintEngine`] (or accepts
//! a caller-built one) per case, constructs the inline [`Project`] from the
//! case's source/schema, and compares the engine's emitted diagnostics to the
//! case's `expected.json` (or asserts zero diagnostics for `valid` cases).
//!
//! [`LintEngine`]: rglint_core::LintEngine
//! [`Project`]: rglint_core::Project

#![allow(dead_code)]

pub mod expected;
pub mod fixture;
pub mod property;
pub mod runner;
pub mod snapshot;

pub use expected::{find_source, project_actual, Comparator, ExpectedError, ParityDiff};
pub use fixture::{load_fixture, DocKind, FixtureCase, FixtureConfig};
pub use property::{assert_no_panic, prop_parse_roundtrip};
pub use runner::{
    build_project, build_project_with, engine_for, run_fixture, run_suite, BuildProjectError,
    HarnessError, RunOutcome,
};
pub use snapshot::{assert_diagnostic_snapshot, render_snapshot};
