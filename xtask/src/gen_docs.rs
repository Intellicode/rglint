//! `xtask gen-docs` — generate the rule reference (spec-068).
//!
//! The generator deliberately consumes the live registries and fixture corpus
//! instead of maintaining a second hand-written list.  `--check` renders to a
//! temporary directory and compares the complete file set with `docs/rules/`,
//! making stale generated output a normal, actionable validation failure.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, bail, Context, Result};
use clap::Args;
use rglint_core::{Category, RuleEntry, RuleMeta, Severity};
use serde_json::Value;

#[derive(Debug, Args)]
#[command(name = "gen-docs", about = "Generate rule reference documentation")]
pub struct GenDocsArgs {
    /// Check committed docs/rules without modifying the working tree.
    #[arg(long)]
    pub check: bool,
}

struct GeneratedFile {
    relative_path: PathBuf,
    contents: String,
}

pub fn run(args: GenDocsArgs) -> Result<()> {
    let root = workspace_root()?;
    let files = generate_files(&root)?;

    if args.check {
        check_files(&root.join("docs/rules"), &files)
    } else {
        write_files(&root.join("docs/rules"), &files)?;
        println!(
            "generated {} rule documentation files in {}",
            files.len() - 1,
            root.join("docs/rules").display()
        );
        Ok(())
    }
}

fn workspace_root() -> Result<PathBuf> {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .map(Path::to_path_buf)
        .ok_or_else(|| anyhow!("cannot locate workspace root"))
}

fn entries() -> Vec<&'static RuleEntry> {
    let mut by_id = BTreeMap::new();

    // The spec-rule crate also contributes entries to ALL_RULES through
    // linkme.  Insert both sources explicitly as the spec requires, while
    // deduplicating their shared registry records by stable rule id.
    for entry in rglint_rules::all_rules()
        .iter()
        .chain(rglint_graphql_spec::all_spec_rules().iter())
    {
        by_id.entry(entry.meta.id).or_insert(entry);
    }

    by_id.into_values().collect()
}

fn generate_files(root: &Path) -> Result<Vec<GeneratedFile>> {
    let mut rules = entries();
    rules.sort_by_key(|entry| (category_order(entry.meta.category), entry.meta.id));

    let mut files = Vec::with_capacity(rules.len() + 1);
    for entry in &rules {
        files.push(GeneratedFile {
            relative_path: PathBuf::from(format!("{}.md", entry.meta.id)),
            contents: render_rule(root, entry)?,
        });
    }
    files.push(GeneratedFile {
        relative_path: PathBuf::from("README.md"),
        contents: render_index(&rules),
    });
    Ok(files)
}

fn category_order(category: Category) -> u8 {
    match category {
        Category::Schema => 0,
        Category::Operations => 1,
        Category::Other => 2,
    }
}

fn category_name(category: Category) -> &'static str {
    match category {
        Category::Schema => "schema",
        Category::Operations => "operations",
        Category::Other => "other",
    }
}

fn severity_name(severity: Severity) -> &'static str {
    match severity {
        Severity::Off => "off",
        Severity::Warn => "warn",
        Severity::Error => "error",
    }
}

fn render_rule(root: &Path, entry: &RuleEntry) -> Result<String> {
    let meta = entry.meta;
    let mut out = String::new();
    out.push_str(&format!("# `{}`\n\n", meta.id));

    if meta.deprecated {
        out.push_str("> **Deprecated.**");
        if let Some(replaced_by) = meta.replaced_by {
            out.push_str(&format!(" Use `{replaced_by}` instead."));
        }
        out.push_str("\n\n");
    }

    out.push_str("| Property | Value |\n| --- | --- |\n");
    out.push_str(&format!(
        "| Category | `{}` |\n",
        category_name(meta.category)
    ));
    out.push_str(&format!(
        "| Default severity | `{}` |\n",
        severity_name(meta.severity)
    ));
    out.push_str(&format!(
        "| Requires schema | `{}` |\n",
        meta.requires_schema
    ));
    out.push_str(&format!(
        "| Requires siblings | `{}` |\n",
        meta.requires_siblings
    ));
    out.push_str(&format!(
        "| Has suggestions | `{}` |\n\n",
        meta.has_suggestions
    ));

    out.push_str("## Description\n\n");
    if meta.docs.trim().is_empty() {
        out.push_str("_No description is provided for this rule._\n\n");
    } else {
        out.push_str(meta.docs.trim());
        out.push_str("\n\n");
    }

    out.push_str("## Options\n\n");
    match meta.option_schema_source() {
        None => out.push_str("This rule has no options.\n\n"),
        Some(source) => {
            let schema: Value = serde_json::from_str(source)
                .with_context(|| format!("invalid option schema for `{}`", meta.id))?;
            if meta.option_schema().is_none() {
                bail!("option schema for `{}` does not compile", meta.id);
            }
            out.push_str(&render_options(meta, &schema)?);
        }
    }

    out.push_str("## Examples\n\n");
    let examples = fixture_examples(root, meta.id)?;
    if examples.is_empty() {
        out.push_str("_No dedicated valid fixture is available for this rule._\n");
    } else {
        for (label, source) in examples {
            out.push_str(&format!("### `{label}`\n\n```graphql\n{source}\n```\n\n"));
        }
    }

    while out.ends_with('\n') {
        out.pop();
    }
    out.push('\n');
    Ok(out)
}

fn render_options(meta: &RuleMeta, schema: &Value) -> Result<String> {
    let Some(properties) = schema.get("properties").and_then(Value::as_object) else {
        return Ok(
            "This rule accepts options, but does not publish property-level documentation.\n\n"
                .into(),
        );
    };
    if properties.is_empty() {
        return Ok("This rule accepts an empty options object.\n\n".into());
    }

    let defaults = meta.default_options().and_then(Value::as_object);
    let mut out =
        String::from("| Option | Type | Default | Description |\n| --- | --- | --- | --- |\n");
    for (name, property) in properties {
        let ty = schema_type(property);
        let default = property
            .get("default")
            .or_else(|| defaults.and_then(|values| values.get(name)))
            .map(format_json_value)
            .unwrap_or_else(|| "—".into());
        let description = property
            .get("description")
            .and_then(Value::as_str)
            .unwrap_or("—");
        out.push_str(&format!(
            "| `{}` | `{}` | `{}` | {} |\n",
            escape_cell(name),
            escape_cell(&ty),
            escape_cell(&default),
            escape_cell(description),
        ));
    }
    out.push('\n');
    Ok(out)
}

fn schema_type(schema: &Value) -> String {
    if let Some(one_of) = schema.get("oneOf").and_then(Value::as_array) {
        return one_of
            .iter()
            .map(schema_type)
            .collect::<Vec<_>>()
            .join(" or ");
    }
    if let Some(any_of) = schema.get("anyOf").and_then(Value::as_array) {
        return any_of
            .iter()
            .map(schema_type)
            .collect::<Vec<_>>()
            .join(" or ");
    }
    match schema.get("type").and_then(Value::as_str) {
        Some("array") => schema
            .get("items")
            .map(|items| format!("array of {}", schema_type(items)))
            .unwrap_or_else(|| "array".into()),
        Some(kind) => kind.into(),
        None if schema.get("enum").is_some() => "enum".into(),
        None => "any".into(),
    }
}

fn format_json_value(value: &Value) -> String {
    match value {
        Value::String(value) => value.clone(),
        _ => serde_json::to_string(value).unwrap_or_else(|_| "—".into()),
    }
}

fn escape_cell(value: &str) -> String {
    value.replace('|', "\\|").replace('\n', " ")
}

fn fixture_examples(root: &Path, rule_id: &str) -> Result<Vec<(String, String)>> {
    let valid_dir = root.join("rules-fixtures").join(rule_id).join("valid");
    if !valid_dir.is_dir() {
        return Ok(Vec::new());
    }

    let mut cases = Vec::new();
    let mut directories = fs::read_dir(&valid_dir)
        .with_context(|| format!("reading {}", valid_dir.display()))?
        .collect::<std::io::Result<Vec<_>>>()?;
    directories.sort_by_key(|entry| entry.file_name());
    for directory in directories {
        if !directory.file_type()?.is_dir() {
            continue;
        }
        let mut sources = fs::read_dir(directory.path())
            .with_context(|| format!("reading {}", directory.path().display()))?
            .collect::<std::io::Result<Vec<_>>>()?;
        sources.sort_by_key(|entry| entry.file_name());
        for source in sources {
            let path = source.path();
            let extension = path.extension().and_then(|value| value.to_str());
            if !matches!(extension, Some("graphql" | "gql")) {
                continue;
            }
            let text = fs::read_to_string(&path)
                .with_context(|| format!("reading fixture {}", path.display()))?;
            let label = path
                .strip_prefix(root)
                .unwrap_or(&path)
                .display()
                .to_string();
            cases.push((label, text.trim_end().to_owned()));
            if cases.len() == 3 {
                return Ok(cases);
            }
        }
    }
    Ok(cases)
}

fn render_index(rules: &[&RuleEntry]) -> String {
    let mut out = String::from(
        "# Rule reference\n\nGenerated by `cargo run --locked -p xtask -- gen-docs`; do not edit manually.\n\n",
    );
    out.push_str("| Rule | Category | Severity | Requires schema | Requires siblings | Suggestions |\n| --- | --- | --- | --- | --- | --- |\n");
    for entry in rules {
        let meta = entry.meta;
        out.push_str(&format!(
            "| [`{}`](./{}.md) | `{}` | `{}` | `{}` | `{}` | `{}` |\n",
            meta.id,
            meta.id,
            category_name(meta.category),
            severity_name(meta.severity),
            meta.requires_schema,
            meta.requires_siblings,
            meta.has_suggestions,
        ));
    }
    out
}

fn write_files(output_dir: &Path, files: &[GeneratedFile]) -> Result<()> {
    fs::create_dir_all(output_dir).with_context(|| format!("creating {}", output_dir.display()))?;
    for entry in fs::read_dir(output_dir)? {
        let entry = entry?;
        if entry.path().extension().and_then(|value| value.to_str()) == Some("md") {
            fs::remove_file(entry.path())?;
        }
    }
    for file in files {
        fs::write(output_dir.join(&file.relative_path), &file.contents)
            .with_context(|| format!("writing {}", file.relative_path.display()))?;
    }
    Ok(())
}

fn check_files(output_dir: &Path, expected: &[GeneratedFile]) -> Result<()> {
    let generated_dir = TempDir::new()?;
    write_files(generated_dir.path(), expected)?;
    let actual = read_markdown_files(output_dir)?;
    let generated = read_markdown_files(generated_dir.path())?;
    if actual != generated {
        eprintln!(
            "generated documentation is stale; run `cargo run --locked -p xtask -- gen-docs`"
        );
        for file in expected {
            match actual.get(&file.relative_path) {
                Some(current) if current == generated.get(&file.relative_path).unwrap() => {}
                Some(current) => print_diff(
                    &file.relative_path,
                    current,
                    generated.get(&file.relative_path).unwrap(),
                ),
                None => eprintln!("missing docs/rules/{}", file.relative_path.display()),
            }
        }
        for path in actual.keys().filter(|path| !generated.contains_key(*path)) {
            eprintln!("unexpected docs/rules/{}", path.display());
        }
        bail!("documentation check failed");
    }
    println!("documentation is up to date ({} files)", expected.len());
    Ok(())
}

fn read_markdown_files(directory: &Path) -> Result<BTreeMap<PathBuf, String>> {
    let mut files = BTreeMap::new();
    if !directory.is_dir() {
        return Ok(files);
    }
    for entry in fs::read_dir(directory)? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().and_then(|value| value.to_str()) != Some("md") {
            continue;
        }
        let relative = path
            .strip_prefix(directory)
            .expect("directory entry is beneath output directory")
            .to_path_buf();
        files.insert(relative, fs::read_to_string(path)?);
    }
    Ok(files)
}

struct TempDir(PathBuf);

impl TempDir {
    fn new() -> Result<Self> {
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or_default();
        let path =
            std::env::temp_dir().join(format!("rglint-gen-docs-{}-{nonce}", std::process::id()));
        fs::create_dir(&path).with_context(|| format!("creating {}", path.display()))?;
        Ok(Self(path))
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn print_diff(path: &Path, current: &str, generated: &str) {
    eprintln!("--- docs/rules/{}", path.display());
    eprintln!("+++ generated/{}", path.display());
    for line in current.lines() {
        eprintln!("-{}", line);
    }
    for line in generated.lines() {
        eprintln!("+{}", line);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schema_type_renders_nested_options() {
        let schema = serde_json::json!({
            "oneOf": [
                {"type": "string"},
                {"type": "array", "items": {"type": "string"}}
            ]
        });
        assert_eq!(schema_type(&schema), "string or array of string");
    }

    #[test]
    fn cells_escape_table_delimiters() {
        assert_eq!(escape_cell("a|b\nc"), "a\\|b c");
    }

    #[test]
    fn registry_merge_deduplicates_spec_entries() {
        let entries = entries();
        assert_eq!(
            entries
                .iter()
                .filter(|entry| entry.meta.id == "executable-definitions")
                .count(),
            1
        );
        assert!(entries.iter().any(|entry| entry.meta.id == "alphabetize"));
    }
}
