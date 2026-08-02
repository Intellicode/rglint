//! Output formatters for lint results.

use std::io::{self, Write};

use rglint_core::ProjectLintResult;

pub mod pretty;

/// A formatter for one or more project lint results.
pub trait Reporter {
    /// Render all results into `out`.
    fn render(&self, results: &[ProjectLintResult], out: &mut dyn Write) -> io::Result<()>;
}

pub use pretty::PrettyReporter;
