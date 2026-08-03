//! Process exit codes for the `rglint` command.

/// Stable exit-code mapping for scripts and CI integrations.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExitCode {
    /// Lint completed without errors and within the warning budget.
    Clean,
    /// Lint found an error or exceeded `--max-warnings`.
    LintError,
    /// Configuration or command-line usage was invalid.
    ConfigError,
    /// An unexpected I/O, engine, or reporter failure occurred.
    InternalError,
}

impl ExitCode {
    /// Return the numeric process status represented by this value.
    pub const fn code(self) -> u8 {
        match self {
            Self::Clean => 0,
            Self::LintError => 1,
            Self::ConfigError => 2,
            Self::InternalError => 3,
        }
    }
}
