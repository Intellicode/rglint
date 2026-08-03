//! Command-line parsing and orchestration for `rglint` (spec-062).

use std::collections::HashSet;
use std::io::{self, IsTerminal, Write};
use std::path::{Path, PathBuf};

use clap::{Parser, ValueEnum};
use rglint_config::{
    discover, discover_graphql_config, load, load_graphql_config, Config, DocumentSpecRaw, Format,
    ProjectConfigRaw,
};
use rglint_core::{Fixer, LintEngine, Project, ProjectConfig, RulesConfig, Severity};

use crate::exit::ExitCode;
use crate::reporter::{GithubReporter, JsonReporter, PrettyReporter, Reporter, SarifReporter};

/// The `rglint` command-line interface.
#[derive(Debug, Parser)]
#[command(
    name = "rglint",
    version,
    about = "Lint GraphQL schemas and operations"
)]
pub struct Cli {
    /// Files or directories to lint. These override configured documents.
    #[arg(value_name = "PATH")]
    pub paths: Vec<PathBuf>,
    /// Use this configuration file instead of discovery.
    #[arg(long, value_name = "FILE")]
    pub config: Option<PathBuf>,
    /// Output format. When omitted, the configuration's format is used.
    #[arg(long, value_enum)]
    pub format: Option<OutputFormat>,
    /// Apply machine-applicable fixes in place.
    #[arg(long, conflicts_with = "fix_dry_run")]
    pub fix: bool,
    /// Print unified fixes without changing files.
    #[arg(long, conflicts_with = "fix")]
    pub fix_dry_run: bool,
    /// Disable ANSI color in pretty output.
    #[arg(long)]
    pub no_color: bool,
    /// Suppress progress and human-readable summaries.
    #[arg(long)]
    pub quiet: bool,
    /// Fail when the warning count exceeds this number.
    #[arg(long, value_name = "N")]
    pub max_warnings: Option<usize>,
    /// Enable a rule, replacing the configured rules. May be repeated.
    #[arg(long, value_name = "RULE", action = clap::ArgAction::Append)]
    pub rule: Vec<String>,
    /// Reserved for future external rule directories.
    #[arg(long, value_name = "DIR")]
    pub rulesdir: Option<PathBuf>,
    /// Create a starter .rglintrc.toml in the current directory.
    #[arg(long)]
    pub init: bool,
}

/// Formats accepted by `--format`.
#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub enum OutputFormat {
    Pretty,
    Json,
    Sarif,
    Github,
}

impl From<OutputFormat> for Format {
    fn from(value: OutputFormat) -> Self {
        match value {
            OutputFormat::Pretty => Self::Pretty,
            OutputFormat::Json => Self::Json,
            OutputFormat::Sarif => Self::Sarif,
            OutputFormat::Github => Self::Github,
        }
    }
}

/// Execute one parsed command and return its stable process status.
pub fn run(cli: Cli) -> ExitCode {
    if cli.init {
        return init_config();
    }

    let cwd = match std::env::current_dir() {
        Ok(cwd) => cwd,
        Err(error) => {
            return fail(
                ExitCode::InternalError,
                format!("failed to find current directory: {error}"),
            )
        }
    };
    let (config, config_dir) = match read_config(&cli, &cwd) {
        Ok(value) => value,
        Err(error) => return fail(ExitCode::ConfigError, error),
    };

    let mut project_configs = config.project_configs();
    if !cli.paths.is_empty() {
        let files = match collect_paths(&cwd, &cli.paths) {
            Ok(files) => files,
            Err(error) => return fail(ExitCode::ConfigError, error),
        };
        if project_configs.is_empty() {
            project_configs.push(ProjectConfig {
                name: "default".to_owned(),
                schema: None,
                documents: None,
                ignore: Vec::new(),
            });
        }
        project_configs[0].documents = Some(rglint_core::DocumentSpec::Files(files));
    }

    let rules = match configured_rules(&config, &cli.rule) {
        Ok(rules) => rules,
        Err(error) => return fail(ExitCode::ConfigError, error),
    };
    let engine = match LintEngine::new(&rules) {
        Ok(engine) => engine,
        Err(error) => return fail(ExitCode::ConfigError, error.to_string()),
    };
    let resolver = rglint_core::ProjectResolver::new(config_dir);
    let mut projects = match resolver.resolve(&project_configs) {
        Ok(projects) => projects,
        Err(error) => return fail(ExitCode::ConfigError, error.to_string()),
    };

    if cli.fix_dry_run {
        let fixer = Fixer::new(&engine);
        let mut stdout = io::stdout().lock();
        for project in &projects {
            match fixer.dry_run(project) {
                Ok(diffs) => {
                    for diff in diffs {
                        if let Err(error) = writeln!(stdout, "{}", diff.unified_diff) {
                            return fail(
                                ExitCode::InternalError,
                                format!("failed to write fix diff: {error}"),
                            );
                        }
                    }
                }
                Err(error) => return fail(ExitCode::InternalError, error.to_string()),
            }
        }
        return ExitCode::Clean;
    }

    let mut results = Vec::with_capacity(projects.len());
    for project in &mut projects {
        progress(project, cli.quiet);
        if cli.fix {
            match Fixer::new(&engine).fix(project) {
                Ok(summary) if !cli.quiet => {
                    eprintln!(
                        "Fixed {} file(s) in {} pass(es); {} diagnostic(s) remain.",
                        summary.files_changed, summary.passes, summary.remaining
                    );
                }
                Ok(_) => {}
                Err(error) => return fail(ExitCode::InternalError, error.to_string()),
            }
        }
        match engine.lint(project) {
            Ok(result) => results.push(result),
            Err(error) => return fail(ExitCode::InternalError, error.to_string()),
        }
    }

    let format = cli.format.map(Format::from).unwrap_or(config.format);
    if let Err(error) = render(format, !cli.no_color, !cli.quiet, &results) {
        return fail(
            ExitCode::InternalError,
            format!("failed to render diagnostics: {error}"),
        );
    }

    let (errors, warnings) = counts(&results);
    if errors > 0 || cli.max_warnings.is_some_and(|max| warnings > max) {
        ExitCode::LintError
    } else {
        ExitCode::Clean
    }
}

fn read_config(cli: &Cli, cwd: &Path) -> Result<(Config, PathBuf), String> {
    let path = cli
        .config
        .as_ref()
        .map(|path| absolute(cwd, path))
        .or_else(|| discover(cwd))
        .or_else(|| discover_graphql_config(cwd));
    let Some(path) = path else {
        let projects = if cli.paths.is_empty() {
            vec![ProjectConfigRaw {
                name: "default".to_owned(),
                schema: None,
                documents: None,
                ignore: Vec::new(),
            }]
        } else {
            vec![ProjectConfigRaw {
                name: "default".to_owned(),
                schema: None,
                documents: Some(DocumentSpecRaw::Multiple(Vec::new())),
                ignore: Vec::new(),
            }]
        };
        return Ok((
            Config {
                projects,
                rules: Default::default(),
                ignore: Vec::new(),
                format: Format::default(),
            },
            cwd.to_path_buf(),
        ));
    };
    let config = if path
        .file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.starts_with(".graphql"))
        || path.file_name().and_then(|name| name.to_str()) == Some(".graphqlconfig")
    {
        load_graphql_config(&path)
    } else {
        load(&path)
    }
    .map_err(|error| error.to_string())?;
    Ok((config, path.parent().unwrap_or(cwd).to_path_buf()))
}

fn configured_rules(config: &Config, overrides: &[String]) -> Result<RulesConfig, String> {
    if !overrides.is_empty() {
        return Ok(RulesConfig {
            rules: overrides
                .iter()
                .map(|id| rglint_core::RuleConfig {
                    id: id.clone(),
                    severity: Severity::Warn,
                    options: serde_json::Map::new().into(),
                })
                .collect(),
        });
    }
    // Keep both built-in registries linked: the recommended presets contain
    // graphql-js validation ids as well as rglint-native rules.
    let _ = rglint_rules::all_rules();
    let _ = rglint_graphql_spec::all_spec_rules();
    let entries: Vec<_> = rglint_core::ALL_RULES.iter().collect();
    config
        .validate(&entries)
        .map_err(|error| error.to_string())?;
    Ok(config.rules_config())
}

fn collect_paths(cwd: &Path, paths: &[PathBuf]) -> Result<Vec<PathBuf>, String> {
    let mut files = Vec::new();
    for path in paths {
        let path = absolute(cwd, path);
        if path.is_dir() {
            for entry in walkdir::WalkDir::new(&path)
                .into_iter()
                .filter_map(Result::ok)
            {
                let candidate = entry.path();
                if candidate.is_file() && is_document(candidate) {
                    files.push(candidate.to_path_buf());
                }
            }
        } else if path.is_file() {
            files.push(path);
        } else {
            return Err(format!("path `{}` does not exist", path.display()));
        }
    }
    files.sort();
    files.dedup();
    if files.is_empty() {
        return Err("no GraphQL documents were found in the requested paths".to_owned());
    }
    Ok(files)
}

fn is_document(path: &Path) -> bool {
    matches!(
        path.extension().and_then(|extension| extension.to_str()),
        Some("graphql" | "gql")
    )
}

fn absolute(cwd: &Path, path: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        cwd.join(path)
    }
}

fn render(
    format: Format,
    color: bool,
    summary: bool,
    results: &[rglint_core::ProjectLintResult],
) -> io::Result<()> {
    let mut stdout = io::stdout().lock();
    match format {
        Format::Pretty => {
            PrettyReporter::new_with_summary(color, summary).render(results, &mut stdout)
        }
        Format::Json => JsonReporter::new(true).render(results, &mut stdout),
        Format::Sarif => SarifReporter::new().render(results, &mut stdout),
        Format::Github => GithubReporter::new(summary).render(results, &mut stdout),
    }
}

fn counts(results: &[rglint_core::ProjectLintResult]) -> (usize, usize) {
    results
        .iter()
        .flat_map(|result| &result.all)
        .fold((0, 0), |(errors, warnings), diagnostic| {
            match diagnostic.severity {
                Severity::Error => (errors + 1, warnings),
                Severity::Warn => (errors, warnings + 1),
                Severity::Off => (errors, warnings),
            }
        })
}

fn progress(project: &Project, quiet: bool) {
    if quiet || !io::stderr().is_terminal() {
        return;
    }
    let mut paths = HashSet::new();
    if let Some(schema) = &project.schema {
        paths.extend(
            schema
                .sources
                .iter()
                .map(|source| source.path().to_path_buf()),
        );
    }
    paths.extend(
        project
            .documents
            .docs
            .iter()
            .map(|document| document.source.path().to_path_buf()),
    );
    let mut paths: Vec<_> = paths.into_iter().collect();
    paths.sort();
    for path in paths {
        eprintln!("Checking {}", path.display());
    }
}

fn init_config() -> ExitCode {
    let path = PathBuf::from(".rglintrc.toml");
    if path.exists() {
        return fail(
            ExitCode::ConfigError,
            format!("refusing to overwrite `{}`", path.display()),
        );
    }
    let template = "# rglint configuration\nextends = \"recommended\"\n\n# [rules]\n# no-anonymous-operations = \"warn\"\n";
    match std::fs::write(&path, template) {
        Ok(()) => ExitCode::Clean,
        Err(error) => fail(
            ExitCode::ConfigError,
            format!("failed to write `{}`: {error}", path.display()),
        ),
    }
}

fn fail(code: ExitCode, message: impl AsRef<str>) -> ExitCode {
    eprintln!("rglint: {}", message.as_ref());
    code
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parser_accepts_all_specified_flags() {
        let cli = Cli::try_parse_from([
            "rglint",
            "--format",
            "json",
            "--fix-dry-run",
            "--max-warnings",
            "0",
            "--rule",
            "alphabetize",
            "file.graphql",
        ])
        .unwrap();
        assert_eq!(cli.format, Some(OutputFormat::Json));
        assert!(cli.fix_dry_run);
        assert_eq!(cli.max_warnings, Some(0));
        assert_eq!(cli.rule, vec!["alphabetize"]);
    }

    #[test]
    fn exit_codes_are_stable() {
        assert_eq!(ExitCode::Clean.code(), 0);
        assert_eq!(ExitCode::LintError.code(), 1);
        assert_eq!(ExitCode::ConfigError.code(), 2);
        assert_eq!(ExitCode::InternalError.code(), 3);
    }
}
