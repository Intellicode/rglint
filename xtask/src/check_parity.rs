//! xtask check-parity: compare fixture oracle output with rglint (spec-069).

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{anyhow, bail, Context, Result};
use clap::Args;
use rglint_core::{LintEngine, RuleConfig, RulesConfig, Severity};
use rglint_test_harness::{build_project, load_fixture, project_actual, FixtureCase};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Args)]
#[command(
    name = "check-parity",
    about = "Compare fixture oracle output with rglint"
)]
pub struct CheckParityArgs {
    /// Check one fixture directory or one rule id from the graphql-js fixture.
    #[arg(long)]
    pub rule: Option<String>,
    /// Add unknown divergences to parity/known-divergences.json.
    #[arg(long)]
    pub update_known: bool,
    /// Run a live graphql-eslint adapter instead of the checked-in oracle.
    #[arg(long)]
    pub ts_command: Option<PathBuf>,
    /// Write generated output below this directory instead of parity/.
    #[arg(long)]
    pub output: Option<PathBuf>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct KnownDivergences {
    version: u32,
    divergences: Vec<KnownDivergence>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct KnownDivergence {
    rule: String,
    case: String,
    reason: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
struct ParityRecord {
    rule: String,
    message: String,
    line: usize,
    column: usize,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct ParityOutput {
    diagnostics: Vec<ParityRecord>,
}

#[derive(Clone, Debug)]
struct CaseInfo {
    folder_rule: String,
    kind: String,
    id: String,
    key: String,
    path: PathBuf,
    manifest_rule: Option<String>,
}

#[derive(Debug, Deserialize)]
struct FixtureManifest {
    cases: Vec<ManifestCase>,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum ManifestCase {
    Detail {
        kind: String,
        id: String,
        rule: Option<String>,
    },
    Legacy(String),
}

impl ManifestCase {
    fn kind_and_id(&self) -> Option<(&str, &str)> {
        match self {
            Self::Detail { kind, id, .. } => Some((kind, id)),
            Self::Legacy(value) => value.split_once('/'),
        }
    }

    fn rule(&self) -> Option<&str> {
        match self {
            Self::Detail { rule, .. } => rule.as_deref(),
            Self::Legacy(_) => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Comparison {
    Match,
    Known,
    Unknown,
}

pub fn run(args: CheckParityArgs) -> Result<()> {
    let root = workspace_root()?;
    let fixtures_root = root.join("rules-fixtures");
    let output_root = args.output.unwrap_or_else(|| root.join("parity"));
    let known_path = root.join("parity/known-divergences.json");
    let mut known = load_known(&known_path)?;
    let ts_command = args
        .ts_command
        .or_else(|| std::env::var_os("RGLINT_PARITY_TS_COMMAND").map(PathBuf::from));
    let cases = discover_cases(&fixtures_root, args.rule.as_deref())?;
    if cases.is_empty() {
        bail!("no fixture cases matched the requested rule");
    }

    let mut rows = Vec::with_capacity(cases.len());
    let mut unknown = 0usize;
    let mut added = 0usize;

    for case_info in cases {
        let case = load_fixture(&case_info.path)
            .with_context(|| format!("loading fixture case {}", case_info.key))?;
        let rust = rust_output(&case_info, &case)?;
        let oracle = match ts_command.as_deref() {
            Some(command) => live_oracle(command, &case_info, &case)?,
            None => fixture_oracle(&case),
        };

        write_output(&output_root, "rust-output", &case_info, &rust)?;
        write_output(&output_root, "ts-output", &case_info, &oracle)?;

        let mut comparison = compare(&oracle, &rust, case.loose_message, &known, &case_info);
        if comparison == Comparison::Unknown && args.update_known {
            let rule = case_info
                .manifest_rule
                .clone()
                .unwrap_or_else(|| case_info.folder_rule.clone());
            if !known
                .divergences
                .iter()
                .any(|entry| entry.rule == rule && entry.case == case_info.key)
            {
                known.divergences.push(KnownDivergence {
                    rule,
                    case: case_info.key.clone(),
                    reason: "Added by xtask check-parity --update-known; review this entry."
                        .to_owned(),
                });
                added += 1;
            }
            comparison = Comparison::Known;
        }
        if comparison == Comparison::Unknown {
            unknown += 1;
        }
        rows.push(render_row(&case_info, &oracle, &rust, comparison));
    }

    if args.update_known && added > 0 {
        write_known(&known_path, &known)?;
    }
    write_diff(&output_root, &rows, unknown, added)?;

    println!(
        "check-parity: {} case(s), {} unknown divergence(s), {} known update(s)",
        rows.len(),
        unknown,
        added
    );
    if unknown > 0 {
        bail!("parity check found {unknown} unknown divergence(s); see parity/diff.md");
    }
    Ok(())
}

fn workspace_root() -> Result<PathBuf> {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .map(Path::to_path_buf)
        .ok_or_else(|| anyhow!("cannot locate workspace root"))
}

fn load_known(path: &Path) -> Result<KnownDivergences> {
    if !path.is_file() {
        return Ok(KnownDivergences {
            version: 1,
            divergences: Vec::new(),
        });
    }
    let text = fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
    let known =
        serde_json::from_str(&text).with_context(|| format!("parsing {}", path.display()))?;
    Ok(known)
}

fn write_known(path: &Path, known: &KnownDivergences) -> Result<()> {
    let text = format!("{}\n", serde_json::to_string_pretty(known)?);
    fs::write(path, text).with_context(|| format!("writing {}", path.display()))
}

fn discover_cases(root: &Path, filter: Option<&str>) -> Result<Vec<CaseInfo>> {
    let mut cases = Vec::new();
    let mut rule_dirs = sorted_dirs(root)?;
    rule_dirs.retain(|dir| {
        let name = dir
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or_default();
        filter.map_or(true, |wanted| {
            wanted == name || name == "graphql-js-validation"
        })
    });

    for rule_dir in rule_dirs {
        let folder_rule = rule_dir
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(|| anyhow!("fixture directory has a non-UTF-8 name"))?
            .to_owned();
        let manifest = load_manifest(&rule_dir)?;
        for kind in ["valid", "invalid"] {
            let kind_dir = rule_dir.join(kind);
            if !kind_dir.is_dir() {
                continue;
            }
            for case_dir in sorted_dirs(&kind_dir)? {
                let id = case_dir
                    .file_name()
                    .and_then(|name| name.to_str())
                    .ok_or_else(|| anyhow!("fixture case has a non-UTF-8 name"))?
                    .to_owned();
                let manifest_rule = manifest.as_ref().and_then(|manifest| {
                    manifest
                        .cases
                        .iter()
                        .find(|case| {
                            case.kind_and_id().is_some_and(|(case_kind, case_id)| {
                                case_kind == kind && case_id == id
                            })
                        })
                        .and_then(|case| case.rule().map(str::to_owned))
                });
                if let Some(wanted) = filter {
                    let selected =
                        folder_rule == wanted || manifest_rule.as_deref() == Some(wanted);
                    if !selected {
                        continue;
                    }
                }
                cases.push(CaseInfo {
                    folder_rule: folder_rule.clone(),
                    kind: kind.to_owned(),
                    id: id.clone(),
                    key: format!("{folder_rule}/{kind}/{id}"),
                    path: case_dir,
                    manifest_rule,
                });
            }
        }
    }
    cases.sort_by(|left, right| left.key.cmp(&right.key));
    Ok(cases)
}

fn sorted_dirs(root: &Path) -> Result<Vec<PathBuf>> {
    if !root.is_dir() {
        return Ok(Vec::new());
    }
    let mut dirs = fs::read_dir(root)
        .with_context(|| format!("reading {}", root.display()))?
        .filter_map(|entry| entry.ok())
        .filter_map(|entry| {
            entry
                .file_type()
                .ok()
                .filter(|kind| kind.is_dir())
                .map(|_| entry.path())
        })
        .collect::<Vec<_>>();
    dirs.sort();
    Ok(dirs)
}

fn load_manifest(rule_dir: &Path) -> Result<Option<FixtureManifest>> {
    let path = rule_dir.join("manifest.json");
    if !path.is_file() {
        return Ok(None);
    }
    let text = fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;
    Ok(Some(
        serde_json::from_str(&text).with_context(|| format!("parsing {}", path.display()))?,
    ))
}

fn rust_output(info: &CaseInfo, case: &FixtureCase) -> Result<ParityOutput> {
    let _ = rglint_rules::all_rules();
    let _ = rglint_graphql_spec::all_spec_rules();
    let ids = enabled_rule_ids(info);
    let engine = LintEngine::new(&RulesConfig {
        rules: ids
            .iter()
            .map(|id| RuleConfig {
                id: (*id).to_owned(),
                severity: Severity::Error,
                options: case.options.clone(),
            })
            .collect(),
    })?;
    let project = build_project(case)?;
    let result = engine.lint(&project)?;
    let mut diagnostics = result
        .all
        .iter()
        .map(|diagnostic| {
            let source = result
                .sources
                .get(&diagnostic.file)
                .map(|source| source.as_ref());
            let actual = project_actual(diagnostic, source);
            ParityRecord {
                rule: actual.rule,
                message: actual.message,
                line: actual.line,
                column: actual.column,
            }
        })
        .collect::<Vec<_>>();
    sort_records(&mut diagnostics);
    Ok(ParityOutput { diagnostics })
}

fn enabled_rule_ids(info: &CaseInfo) -> Vec<&str> {
    if info.folder_rule == "graphql-js-validation" && info.kind == "valid" {
        return rglint_graphql_spec::all_spec_rules()
            .iter()
            .map(|entry| entry.meta.id)
            .collect();
    }
    vec![info.manifest_rule.as_deref().unwrap_or(&info.folder_rule)]
}

fn fixture_oracle(case: &FixtureCase) -> ParityOutput {
    let mut diagnostics = case
        .expected
        .iter()
        .map(|error| ParityRecord {
            rule: error.rule.clone(),
            message: error.message.clone(),
            line: error.line,
            column: error.column,
        })
        .collect::<Vec<_>>();
    sort_records(&mut diagnostics);
    ParityOutput { diagnostics }
}

fn live_oracle(command: &Path, info: &CaseInfo, case: &FixtureCase) -> Result<ParityOutput> {
    let output = Command::new(command)
        .env("RGLINT_PARITY_RULE", enabled_rule_ids(info).join(","))
        .env("RGLINT_PARITY_CASE", &info.path)
        .env("RGLINT_PARITY_SOURCE", &case.source_path)
        .output()
        .with_context(|| format!("running parity adapter {}", command.display()))?;
    if !output.status.success() {
        bail!(
            "parity adapter {} failed with {}: {}",
            command.display(),
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    normalize_oracle(&output.stdout)
}

fn normalize_oracle(bytes: &[u8]) -> Result<ParityOutput> {
    let value: Value =
        serde_json::from_slice(bytes).context("parity adapter returned invalid JSON")?;
    let mut diagnostics = Vec::new();
    match &value {
        Value::Array(files) => {
            for file in files {
                for message in file
                    .get("messages")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                {
                    diagnostics.push(record_from_value(message, true)?);
                }
            }
        }
        Value::Object(object) if object.get("errors").is_some() => {
            for error in object
                .get("errors")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
            {
                diagnostics.push(record_from_value(error, false)?);
            }
        }
        Value::Object(object) if object.get("diagnostics").is_some() => {
            for error in object
                .get("diagnostics")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
            {
                diagnostics.push(record_from_value(error, false)?);
            }
        }
        _ => bail!("parity adapter JSON must be an ESLint array or an errors object"),
    }
    sort_records(&mut diagnostics);
    Ok(ParityOutput { diagnostics })
}

fn record_from_value(value: &Value, eslint_columns: bool) -> Result<ParityRecord> {
    let object = value
        .as_object()
        .ok_or_else(|| anyhow!("parity diagnostic must be a JSON object"))?;
    let rule = object
        .get("rule")
        .or_else(|| object.get("ruleId"))
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("parity diagnostic is missing rule/ruleId"))?;
    let line = object
        .get("line")
        .and_then(Value::as_u64)
        .ok_or_else(|| anyhow!("parity diagnostic is missing line"))? as usize;
    let raw_column = object
        .get("column")
        .or_else(|| object.get("column0"))
        .and_then(Value::as_u64)
        .ok_or_else(|| anyhow!("parity diagnostic is missing column"))?
        as usize;
    let column = if eslint_columns && object.get("column0").is_none() {
        raw_column.saturating_sub(1)
    } else {
        raw_column
    };
    let message = object
        .get("message")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("parity diagnostic is missing message"))?;
    Ok(ParityRecord {
        rule: rule.to_owned(),
        message: message.to_owned(),
        line,
        column,
    })
}

fn sort_records(records: &mut [ParityRecord]) {
    records.sort_by(|left, right| {
        (left.line, left.column, &left.rule, &left.message).cmp(&(
            right.line,
            right.column,
            &right.rule,
            &right.message,
        ))
    });
}

fn compare(
    oracle: &ParityOutput,
    rust: &ParityOutput,
    loose_message: bool,
    known: &KnownDivergences,
    info: &CaseInfo,
) -> Comparison {
    if oracle.diagnostics.len() == rust.diagnostics.len()
        && oracle
            .diagnostics
            .iter()
            .zip(&rust.diagnostics)
            .all(|(left, right)| {
                left.rule == right.rule
                    && left.line == right.line
                    && left.column == right.column
                    && (loose_message || left.message == right.message)
            })
    {
        return Comparison::Match;
    }
    let rule = info.manifest_rule.as_deref().unwrap_or(&info.folder_rule);
    if known
        .divergences
        .iter()
        .any(|entry| entry.rule == rule && entry.case == info.key)
    {
        Comparison::Known
    } else {
        Comparison::Unknown
    }
}

fn write_output(root: &Path, side: &str, info: &CaseInfo, output: &ParityOutput) -> Result<()> {
    let path = root
        .join(side)
        .join(&info.folder_rule)
        .join(&info.kind)
        .join(format!("{}.json", info.id));
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(
        &path,
        format!("{}\n", serde_json::to_string_pretty(output)?),
    )
    .with_context(|| format!("writing {}", path.display()))
}

fn render_row(
    info: &CaseInfo,
    oracle: &ParityOutput,
    rust: &ParityOutput,
    comparison: Comparison,
) -> String {
    let status = match comparison {
        Comparison::Match => "pass",
        Comparison::Known => "known",
        Comparison::Unknown => "FAIL",
    };
    let diff = if comparison == Comparison::Match {
        String::new()
    } else {
        first_difference(oracle, rust)
    };
    format!(
        "| {} | {} | {} | {} | {} |",
        info.key,
        oracle.diagnostics.len(),
        rust.diagnostics.len(),
        diff,
        status
    )
}

fn first_difference(oracle: &ParityOutput, rust: &ParityOutput) -> String {
    let Some((index, (left, right))) = oracle
        .diagnostics
        .iter()
        .zip(&rust.diagnostics)
        .enumerate()
        .find(|(_, (left, right))| left != right)
    else {
        return format!(
            "count {} vs {}",
            oracle.diagnostics.len(),
            rust.diagnostics.len()
        );
    };
    format!(
        "slot {index}: oracle={} rust={}",
        compact_record(left),
        compact_record(right)
    )
}

fn compact_record(record: &ParityRecord) -> String {
    let message = record.message.replace('|', "\\|").replace('\n', " ");
    format!(
        "{}@{}:{} {}",
        record.rule, record.line, record.column, message
    )
}

fn write_diff(root: &Path, rows: &[String], unknown: usize, added: usize) -> Result<()> {
    let mut text = String::from(
        "# Parity diff\n\n| Case | Oracle | Rust | Diff | Status |\n| --- | ---: | ---: | --- | --- |\n",
    );
    for row in rows {
        text.push_str(row);
        text.push('\n');
    }
    text.push_str(&format!(
        "\nUnknown divergences: {unknown}\nKnown updates: {added}\n"
    ));
    fs::write(root.join("diff.md"), text)
        .with_context(|| format!("writing {}", root.join("diff.md").display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record(rule: &str, message: &str, line: usize, column: usize) -> ParityRecord {
        ParityRecord {
            rule: rule.to_owned(),
            message: message.to_owned(),
            line,
            column,
        }
    }

    #[test]
    fn normalizes_fixture_and_eslint_shapes() {
        let fixture = normalize_oracle(
            br#"{"errors":[{"rule":"demo","message":"bad","line":2,"column":0}]}"#,
        )
        .unwrap();
        assert_eq!(fixture.diagnostics, vec![record("demo", "bad", 2, 0)]);

        let eslint = normalize_oracle(
            br#"[{"filePath":"x.graphql","messages":[{"ruleId":"demo","message":"bad","line":2,"column":1}]}]"#,
        )
        .unwrap();
        assert_eq!(eslint.diagnostics, vec![record("demo", "bad", 2, 0)]);
    }

    #[test]
    fn loose_message_only_relaxes_message() {
        let info = CaseInfo {
            folder_rule: "demo".into(),
            kind: "invalid".into(),
            id: "01".into(),
            key: "demo/invalid/01".into(),
            path: PathBuf::from("demo/invalid/01"),
            manifest_rule: None,
        };
        let oracle = ParityOutput {
            diagnostics: vec![record("demo", "upstream", 1, 3)],
        };
        let rust = ParityOutput {
            diagnostics: vec![record("demo", "apollo", 1, 3)],
        };
        let known = KnownDivergences {
            version: 1,
            divergences: vec![],
        };
        assert_eq!(
            compare(&oracle, &rust, true, &known, &info),
            Comparison::Match
        );
        assert_eq!(
            compare(&oracle, &rust, false, &known, &info),
            Comparison::Unknown
        );
    }

    #[test]
    fn known_divergence_is_scoped_to_rule_and_case() {
        let mut info = CaseInfo {
            folder_rule: "demo".into(),
            kind: "invalid".into(),
            id: "01".into(),
            key: "demo/invalid/01".into(),
            path: PathBuf::from("demo/invalid/01"),
            manifest_rule: None,
        };
        let oracle = ParityOutput {
            diagnostics: vec![record("demo", "one", 1, 0)],
        };
        let rust = ParityOutput {
            diagnostics: vec![record("demo", "two", 1, 0)],
        };
        let known = KnownDivergences {
            version: 1,
            divergences: vec![KnownDivergence {
                rule: "demo".into(),
                case: "demo/invalid/01".into(),
                reason: "test".into(),
            }],
        };
        assert_eq!(
            compare(&oracle, &rust, false, &known, &info),
            Comparison::Known
        );
        info.id = "02".into();
        info.key = "demo/invalid/02".into();
        assert_eq!(
            compare(&oracle, &rust, false, &known, &info),
            Comparison::Unknown
        );
    }
}
