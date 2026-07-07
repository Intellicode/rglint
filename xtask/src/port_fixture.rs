//! `xtask port-fixture` — turn graphql-eslint's TS test cases into Rust-runnable
//! fixtures (spec-015).
//!
//! ## Purpose
//!
//! Reads `packages/plugin/src/rules/<rule>/index.test.ts`, finds the
//! `ruleTester.run('<id>', rule, { valid: [...], invalid: [...] })` block and
//! emits one `rules-fixtures/<rule-id>/{valid,invalid}/NN/` directory per case
//! containing:
//!
//! - `NN.graphql` (the TS `code` field, verbatim),
//! - `NN.config.toml` (`schema`, `[options]`, `loose_message`, `kind` — the
//!   format locked in spec-015 and consumed by spec-014's `load_fixture`),
//! - `NN.expected.json` (invalid cases only) — `{ "errors": [...] }` with the
//!   `message`, `line`, `column` fields and the rule id filled from the test.
//!
//! A `rules-fixtures/<rule-id>/manifest.json` records the source `.ts` file,
//! its hash, and per-case hashes — so re-runs without `--force` skip work.
//!
//! Best-effort `rules-fixtures/<rule-id>/defaults.json` mirrors
//! `meta.docs.configOptions` (PLAN §8 risk mitigation); not every rule has it.
//!
//! ## Known limitations (do NOT trust beyond scope)
//!
//! - The extractor is a **hand-rolled bracket/string scanner**, not a real
//!   TS parser. It handles the subset graphql-eslint's test files actually use.
//! - Identifier values inside `options` (e.g. `Kind.ENUM`) become JSON strings
//!   `"Kind.ENUM"`. Computed keys like `[Kind.DIRECTIVE_DEFINITION]` are
//!   unrepresentable in TOML; the whole `[options]` block falls back to
//!   `options_json = "<json blob>"` and is left for manual cleanup by each
//!   rule spec.
//! - `code`/`schema` template literals with `${}` interpolations are passed
//!   through verbatim; **no interpolation is evaluated**.
//! - Error `line`/`column` are not in the TS sources (graphql-eslint's
//!   eslint-rule-tester computes them at runtime), so emitted `expected.json`
//!   entries carry `line: 1, column: 0` placeholders. Each rule spec's port
//!   pass manually fixes these by running the harness and diffing. This is
//!   PLAN §5 Phase 0.4's "not perfectly; manual cleanup expected" caveat.
//! - `meta.docs.configOptions` extraction is best-effort: if we can't cleanly
//!   parse the value, `defaults.json` is skipped silently.
//! - We do NOT detect schema-vs-operations kind: schema-only rules' fixtures
//!   will need `kind = "schema"` added by hand by each rule's port pass.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};

// ─────────────────────── CLI ───────────────────────

#[derive(Debug, Clone, clap::Parser)]
#[command(
    name = "port-fixture",
    about = "Convert TS test cases to rglint fixtures (spec-015)"
)]
pub struct PortFixtureArgs {
    /// Port only the rule whose id matches (e.g. "no-anonymous-operations").
    /// Mutually exclusive with `--all`.
    #[arg(long, conflicts_with = "all")]
    rule: Option<String>,
    /// Port every rule directory under the source root.
    #[arg(long, conflicts_with = "rule")]
    all: bool,
    /// Overwrite existing fixtures even when the source hash is unchanged.
    #[arg(long)]
    force: bool,
    /// Path to the graphql-eslint checkout root (the dir containing `packages/`).
    /// Defaults to the parent of this crate's workspace root, since `rglint/`
    /// lives inside the `graphql-eslint` repo.
    #[arg(long)]
    source_root: Option<PathBuf>,
    /// Where to write `rules-fixtures/`. Defaults to `<workspace>/rules-fixtures`.
    #[arg(long)]
    fixtures_dir: Option<PathBuf>,
}

pub fn run(args: PortFixtureArgs) -> Result<()> {
    let workspace_root = workspace_root()?;
    let source_root = match args.source_root {
        Some(p) => p,
        None => workspace_root
            .parent()
            .ok_or_else(|| anyhow!("workspace root has no parent; pass --source-root"))?
            .to_path_buf(),
    };
    let rules_dir = source_root
        .join("packages")
        .join("plugin")
        .join("src")
        .join("rules");
    if !rules_dir.is_dir() {
        return Err(anyhow!(
            "rules dir `{}` not found; pass --source-root <path to graphql-eslint checkout>",
            rules_dir.display()
        ));
    }

    let fixtures_dir = args
        .fixtures_dir
        .unwrap_or_else(|| workspace_root.join("rules-fixtures"));
    fs::create_dir_all(&fixtures_dir)?;

    let rule_ids = if args.all {
        list_rule_dirs(&rules_dir)?
    } else if let Some(id) = &args.rule {
        vec![id.clone()]
    } else {
        return Err(anyhow!("pass --rule <id> or --all"));
    };

    let mut total = 0usize;
    let mut errors = Vec::new();
    for id in &rule_ids {
        match port_one_rule(id, &rules_dir, &fixtures_dir, args.force) {
            Ok(n) => {
                total += n;
                println!("port-fixture: {id}: wrote {n} cases");
            }
            Err(e) => {
                let msg = format!("port-fixture: {id}: FAILED: {e:#}");
                eprintln!("{msg}");
                errors.push(msg);
            }
        }
    }
    println!(
        "port-fixture: {} rules, {} cases total",
        rule_ids.len(),
        total
    );
    if !errors.is_empty() {
        Err(anyhow!(
            "{} rule(s) failed:\n{}",
            errors.len(),
            errors.join("\n")
        ))
    } else {
        Ok(())
    }
}

fn workspace_root() -> Result<PathBuf> {
    // `xtask` lives at `<workspace>/xtask`; walk up from CARGO_MANIFEST_DIR.
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    Ok(manifest_dir
        .parent()
        .ok_or_else(|| anyhow!("cannot locate workspace root"))?
        .to_path_buf())
}

fn list_rule_dirs(rules_dir: &Path) -> Result<Vec<String>> {
    let mut ids = Vec::new();
    for entry in fs::read_dir(rules_dir)? {
        let entry = entry?;
        let p = entry.path();
        if !p.is_dir() {
            continue;
        }
        // A rule dir must contain `index.test.ts`. Skip non-rule dirs.
        if !p.join("index.test.ts").is_file() {
            continue;
        }
        if let Some(name) = p.file_name().and_then(|n| n.to_str()) {
            ids.push(name.to_string());
        }
    }
    ids.sort();
    Ok(ids)
}

// ─────────────────────── per-rule port ───────────────────────

fn port_one_rule(
    rule_id: &str,
    rules_dir: &Path,
    fixtures_dir: &Path,
    force: bool,
) -> Result<usize> {
    let src_path = rules_dir.join(rule_id).join("index.test.ts");
    let src =
        fs::read_to_string(&src_path).with_context(|| format!("reading {}", src_path.display()))?;
    let src_hash = xxh3_64_hex(src.as_bytes());

    let dest_root = fixtures_dir.join(rule_id);
    let manifest_path = dest_root.join("manifest.json");

    // Idempotency: if a manifest exists with matching source hash, skip —
    // unless `--force` was passed.
    if !force {
        if let Ok(existing) = fs::read_to_string(&manifest_path) {
            if let Ok(m) = serde_json::from_str::<Manifest>(&existing) {
                if m.source_hash == src_hash {
                    return Ok(m.valid_count + m.invalid_count);
                }
            }
        }
    }

    let parsed = ParsedTestFile::parse(&src, rule_id)
        .with_context(|| format!("parsing {}", src_path.display()))?;

    fs::create_dir_all(&dest_root)?;

    // Each case → its own directory `NN/` (matches the layout spec-014 reads).
    let valid_dir = dest_root.join("valid");
    let invalid_dir = dest_root.join("invalid");
    fs::create_dir_all(&valid_dir)?;
    fs::create_dir_all(&invalid_dir)?;

    let mut written_valid = 0usize;
    let mut written_invalid = 0usize;
    let mut cases_manifest = Vec::new();

    for (idx, case) in parsed.valid.iter().enumerate() {
        let nn = format!("{:02}", idx + 1);
        let dir = valid_dir.join(&nn);
        fs::create_dir_all(&dir)?;
        write_case(&dir, case, false, &parsed.rule_id)?;
        cases_manifest.push(ManifestCase {
            kind: "valid".into(),
            id: nn.clone(),
            hash: case.hash.clone(),
            line: case.line,
        });
        written_valid += 1;
    }

    for (idx, case) in parsed.invalid.iter().enumerate() {
        let nn = format!("{:02}", idx + 1);
        let dir = invalid_dir.join(&nn);
        fs::create_dir_all(&dir)?;
        write_case(&dir, case, true, &parsed.rule_id)?;
        cases_manifest.push(ManifestCase {
            kind: "invalid".into(),
            id: nn.clone(),
            hash: case.hash.clone(),
            line: case.line,
        });
        written_invalid += 1;
    }

    let manifest = Manifest {
        source: src_path
            .strip_prefix(rules_dir.parent().unwrap_or(Path::new(".")))
            .unwrap_or(&src_path)
            .display()
            .to_string(),
        source_hash: src_hash.clone(),
        rule: parsed.rule_id.clone(),
        valid_count: written_valid,
        invalid_count: written_invalid,
        cases: cases_manifest,
    };
    fs::write(
        &manifest_path,
        serde_json::to_string_pretty(&manifest)? + "\n",
    )?;

    // Best-effort defaults.json (PLAN §8 risk mitigation).
    if let Some(raw) = extract_config_options(&rules_dir.join(rule_id).join("index.ts"))? {
        fs::write(
            dest_root.join("defaults.json"),
            serde_json::json!({ "configOptions_raw": raw }).to_string() + "\n",
        )?;
    }

    Ok(written_valid + written_invalid)
}

fn write_case(dir: &Path, case: &RawCase, invalid: bool, rule_id: &str) -> Result<()> {
    fs::write(dir.join("graphql"), case.code.as_bytes())?;

    let mut cfg = toml::value::Table::new();
    if let Some(s) = &case.schema {
        cfg.insert("schema".to_string(), toml::Value::String(s.clone()));
    }
    if let Some(d) = &case.documents {
        cfg.insert("documents".to_string(), toml::Value::String(d.clone()));
    }
    if case.loose_message {
        cfg.insert("loose_message".to_string(), toml::Value::Boolean(true));
    }
    if !case.options.is_null() {
        match json_to_toml_value(&case.options) {
            Ok(t) => {
                cfg.insert("options".to_string(), t);
            }
            Err(_) => {
                // Fall back to an inline JSON string. Documented as
                // "manual cleanup expected" in spec-015.
                cfg.insert(
                    "options_json".to_string(),
                    toml::Value::String(case.options.to_string()),
                );
            }
        }
    }
    let cfg_text = toml::to_string(&toml::Value::Table(cfg))?;
    fs::write(dir.join("config.toml"), cfg_text)?;

    if invalid {
        let errors = case.errors.clone().unwrap_or_default();
        let entries: Vec<serde_json::Value> = errors
            .iter()
            .map(|e| {
                serde_json::json!({
                    "rule": rule_id,
                    "message": e.message,
                    "line": e.line.unwrap_or(1),
                    "column": e.column.unwrap_or(0),
                })
            })
            .collect();
        let out = serde_json::json!({ "errors": entries });
        fs::write(dir.join("expected.json"), out.to_string() + "\n")?;
    }
    Ok(())
}

// ─────────────────────── TS extractor: types ───────────────────────

/// One parsed test case (valid or invalid), raw form.
#[derive(Clone)]
struct RawCase {
    /// TS source line (1-based) where this case's object literal starts.
    line: usize,
    /// Source `code` field — what becomes `NN.graphql`.
    code: String,
    /// Inline SDL schema (from `parserOptions.graphQLConfig.schema` or `schema`).
    schema: Option<String>,
    /// Sibling-documents text (`parserOptions.graphQLConfig.documents`).
    documents: Option<String>,
    /// `[options]` for the rule, JSON.
    options: serde_json::Value,
    /// `errors` declared for invalid cases (empty for valid).
    errors: Option<Vec<RawError>>,
    /// `loose_message` flag (spec-053). Always false here; humans flip in cleanup.
    loose_message: bool,
    /// xxh3 of the case's TS snippet, for idempotent re-runs.
    hash: String,
}

#[derive(Clone)]
struct RawError {
    message: String,
    line: Option<usize>,
    column: Option<usize>,
}

/// Top-level parse output of an `index.test.ts` file.
struct ParsedTestFile {
    rule_id: String,
    valid: Vec<RawCase>,
    invalid: Vec<RawCase>,
}

/// Entry in the const table built from top-level `const NAME = <value>;` decls.
struct ConstEntry {
    /// The value's raw TS fragment (e.g. `''' type Query { x: Int } '''`).
    /// Kept for debugging; not used by the extractor.
    #[allow(dead_code)]
    raw: String,
    /// If the value is a `'...'` / `"..."` / `` `...` `` string, the unescaped
    /// string content.
    as_string: Option<String>,
    /// If the value is a `{...}` object literal, the byte range of its inner
    /// body (between `{` and `}`), so we can walk its props directly.
    obj_range: Option<(usize, usize)>,
}

/// Resolves identifiers used as values (e.g. `schema: TEST_SCHEMA`) and
/// spreads (`...WITH_SCHEMA`) by consulting the file's top-level `const`s.
#[derive(Default)]
struct ConstTable {
    entries: HashMap<String, ConstEntry>,
}

impl ConstTable {
    /// Treat the byte range `[start, end)` as a value expression and resolve it
    /// to a string if it's a string literal or a string-typed const identifier.
    fn resolve_string(&self, b: &[u8], start: usize, end: usize) -> Option<String> {
        let i = skip_ws_comments(b, start);
        if i >= end {
            return None;
        }
        match b[i] {
            b'\'' | b'"' => {
                let qend = skip_string(b, i);
                Some(unescape_js_string(&src_range(b, i, qend)))
            }
            b'`' => {
                let tend = skip_template(b, i);
                Some(extract_template_literal(&src_range(b, i, tend)))
            }
            _ => {
                let ident = src_range(b, i, end);
                let ident = ident.trim();
                self.entries.get(ident).and_then(|e| e.as_string.clone())
            }
        }
    }

    /// Resolve the byte range as an object literal — either inline `{...}` or a
    /// const that holds one. Returns the *outer* byte range (including braces),
    /// so callers that dispatch on `b[i] == b'{'` work consistently.
    fn resolve_object_range(&self, b: &[u8], start: usize, end: usize) -> Option<(usize, usize)> {
        let i = skip_ws_comments(b, start);
        if i >= end {
            return None;
        }
        match b[i] {
            b'{' => {
                let e = skip_braced(b, i, b'{', b'}');
                Some((i, e))
            }
            _ => {
                let ident = src_range(b, i, end);
                let ident = ident.trim();
                self.entries.get(ident).and_then(|e| e.obj_range)
            }
        }
    }

    fn obj_range_of(&self, ident: &str) -> Option<(usize, usize)> {
        self.entries.get(ident).and_then(|e| e.obj_range)
    }
}

// ─────────────────────── TS extractor: parse ───────────────────────

impl ParsedTestFile {
    fn parse(src: &str, fallback_id: &str) -> Result<Self> {
        let b = src.as_bytes();
        let consts = collect_top_consts(src);
        let rule_id = find_rule_id(b).unwrap_or_else(|| fallback_id.to_string());

        let block_range = find_test_block(b)
            .ok_or_else(|| anyhow!("could not find `{{ valid, invalid }}` test block"))?;

        // Walk the test-block object literal's top-level keys.
        let props = parse_object_props(b, block_range.0, block_range.1);

        let mut valid = Vec::new();
        let mut invalid = Vec::new();
        for prop in &props {
            match prop.key.as_str() {
                "valid" => {
                    valid = parse_case_array(src, &consts, prop.val_start, prop.val_end, false)?;
                }
                "invalid" => {
                    invalid = parse_case_array(src, &consts, prop.val_start, prop.val_end, true)?;
                }
                _ => {}
            }
        }

        Ok(ParsedTestFile {
            rule_id,
            valid,
            invalid,
        })
    }
}

// — primitive scanner helpers ———————————————

#[inline]
fn skip_ws_comments(b: &[u8], mut i: usize) -> usize {
    while i < b.len() {
        if b[i].is_ascii_whitespace() {
            i += 1;
            continue;
        }
        if i + 1 < b.len() && b[i] == b'/' && b[i + 1] == b'/' {
            i += 2;
            while i < b.len() && b[i] != b'\n' {
                i += 1;
            }
            continue;
        }
        if i + 1 < b.len() && b[i] == b'/' && b[i + 1] == b'*' {
            i += 2;
            while i + 1 < b.len() && !(b[i] == b'*' && b[i + 1] == b'/') {
                i += 1;
            }
            i = (i + 2).min(b.len());
            continue;
        }
        break;
    }
    i
}

/// Skip a `'...'` or `"..."` JS string starting at the opening quote `i`.
fn skip_string(b: &[u8], i: usize) -> usize {
    debug_assert!(i < b.len() && (b[i] == b'\'' || b[i] == b'"'));
    let quote = b[i];
    let mut j = i + 1;
    while j < b.len() {
        match b[j] {
            b'\\' => j += 2,
            c if c == quote => return j + 1,
            _ => j += 1,
        }
    }
    j
}

/// Skip a `` `...` `` template literal from opening backtick at `i`, honouring
/// `${ ... }` interpolations (their content is scanned past but never
/// interpolated — passed through verbatim).
fn skip_template(b: &[u8], i: usize) -> usize {
    debug_assert!(i < b.len() && b[i] == b'`');
    let mut j = i + 1;
    while j < b.len() {
        match b[j] {
            b'\\' => j += 2,
            b'`' => return j + 1,
            b'$' if j + 1 < b.len() && b[j + 1] == b'{' => {
                j = skip_braced(b, j + 1, b'{', b'}');
            }
            _ => j += 1,
        }
    }
    j
}

/// Skip a TS type-argument list `<...>` starting at the opening `<` at `i`.
/// Heuristic: a real type-arg list won't contain a `<` immediately followed by
/// `=` (which would be `<=`), a `>` followed by `=` (`>=`), or contain newlines
/// with `;`. Strings/templates are still skipped so we don't misread `<`/`>`
/// inside them. Returns the index just past the matching `>`.
fn skip_angle_brackets(b: &[u8], i: usize) -> usize {
    debug_assert!(i < b.len() && b[i] == b'<');
    let mut j = i + 1;
    let mut depth: i32 = 1;
    while j < b.len() {
        match b[j] {
            b'\'' | b'"' => j = skip_string(b, j),
            b'`' => j = skip_template(b, j),
            b'<' => {
                depth += 1;
                j += 1;
            }
            b'>' => {
                depth -= 1;
                j += 1;
                if depth == 0 {
                    return j;
                }
            }
            _ => j += 1,
        }
    }
    j
}

/// Skip a balanced `[...]`, `{...}`, or `(...)` starting at `i` (which points
/// at the open bracket). Strings and templates inside are correctly skipped.
fn skip_braced(b: &[u8], i: usize, open: u8, close: u8) -> usize {
    debug_assert!(i < b.len() && b[i] == open);
    let mut j = i + 1;
    let mut depth: i32 = 1;
    while j < b.len() {
        match b[j] {
            b'\'' | b'"' => j = skip_string(b, j),
            b'`' => j = skip_template(b, j),
            c if c == open => {
                depth += 1;
                j += 1;
            }
            c if c == close => {
                depth -= 1;
                j += 1;
                if depth == 0 {
                    return j;
                }
            }
            _ => j += 1,
        }
    }
    j
}

/// Skip one JS value (string / template / number / atom / bracketed form),
/// returning the index just past its end.
fn skip_value(b: &[u8], mut i: usize) -> usize {
    i = skip_ws_comments(b, i);
    if i >= b.len() {
        return i;
    }
    match b[i] {
        b'\'' | b'"' => skip_string(b, i),
        b'`' => skip_template(b, i),
        b'[' => skip_braced(b, i, b'[', b']'),
        b'{' => skip_braced(b, i, b'{', b'}'),
        b'(' => skip_braced(b, i, b'(', b')'),
        _ => {
            // Identifier / number / bareword context — read a "token".
            while i < b.len() {
                let c = b[i];
                if c == b','
                    || c == b']'
                    || c == b'}'
                    || c == b')'
                    || c == b';'
                    || c.is_ascii_whitespace()
                {
                    break;
                }
                i += 1;
            }
            i
        }
    }
}

/// Split a comma-separated list spanned by `[start, end)` (exclusive) into
/// individual element byte-ranges. Handles nested structures + strings.
fn split_top_commas(b: &[u8], start: usize, end: usize) -> Vec<(usize, usize)> {
    let mut out = Vec::new();
    let mut i = skip_ws_comments(b, start);
    let mut el_start = i;
    while i < end {
        match b[i] {
            b'\'' | b'"' => i = skip_string(b, i),
            b'`' => i = skip_template(b, i),
            b'[' | b'{' | b'(' => {
                let close = match b[i] {
                    b'[' => b']',
                    b'{' => b'}',
                    _ => b')',
                };
                i = skip_braced(b, i, b[i], close);
            }
            b',' => {
                push_trimmed(b, &mut out, el_start, i);
                i += 1;
                i = skip_ws_comments(b, i);
                el_start = i;
            }
            _ => i += 1,
        }
    }
    push_trimmed(b, &mut out, el_start, end);
    out
}

fn push_trimmed(b: &[u8], out: &mut Vec<(usize, usize)>, start: usize, end: usize) {
    let s = skip_ws_comments(b, start);
    let mut e = end;
    while e > s && (b[e - 1] as char).is_ascii_whitespace() {
        e -= 1;
    }
    if s < e {
        out.push((s, e));
    }
}

// — object property extraction ———————————————

struct Prop {
    key: String,
    val_start: usize,
    val_end: usize,
}

/// Parse the top-level properties of an object literal whose body is spanned by
/// `[start, end)` (both inside the braces; the `{` may or may not be included).
/// Supports quoted keys, bareword keys, computed `[...]` keys (best-effort
/// capture of the bracketed text), and spread `...ident` (key recorded as
/// `"...ident"`).
fn parse_object_props(b: &[u8], mut start: usize, end: usize) -> Vec<Prop> {
    if start < end && b[start] == b'{' {
        start += 1;
    }
    let mut out = Vec::new();
    let mut i = skip_ws_comments(b, start);

    while i < end {
        if i + 3 <= end && &b[i..i + 3] == b"..." {
            // Spread `...ident`.
            let id_start = skip_ws_comments(b, i + 3);
            let mut id_end = id_start;
            while id_end < end
                && (b[id_end].is_ascii_alphanumeric() || b[id_end] == b'_' || b[id_end] == b'$')
            {
                id_end += 1;
            }
            let name = String::from_utf8_lossy(&b[id_start..id_end]).to_string();
            out.push(Prop {
                key: format!("...{name}"),
                val_start: id_start,
                val_end: id_end,
            });
            i = skip_ws_comments(b, id_end);
            if i < end && b[i] == b',' {
                i += 1;
                i = skip_ws_comments(b, i);
                continue;
            } else {
                break;
            }
        }

        // Key.
        let key = if b[i] == b'\'' || b[i] == b'"' {
            let kend = skip_string(b, i);
            let inner = src_range(b, i + 1, kend.saturating_sub(1).max(i + 1));
            i = kend;
            inner
        } else if b[i] == b'[' {
            let kend = skip_braced(b, i, b'[', b']');
            let s = src_range(b, i, kend);
            i = kend;
            s
        } else {
            let kstart = i;
            while i < end && !(b[i] == b':' || b[i] == b'?' || b[i].is_ascii_whitespace()) {
                i += 1;
            }
            src_range(b, kstart, i)
        };

        i = skip_ws_comments(b, i);
        if i < end && b[i] == b'?' {
            i += 1;
            i = skip_ws_comments(b, i);
        }
        if i >= end || b[i] != b':' {
            i += 1;
            continue;
        }
        i += 1; // consume ':'
        let val_start = skip_ws_comments(b, i);
        let val_end = skip_value(b, val_start);
        out.push(Prop {
            key,
            val_start,
            val_end,
        });
        i = skip_ws_comments(b, val_end);
        if i < end && b[i] == b',' {
            i += 1;
            i = skip_ws_comments(b, i);
        }
    }
    out
}

// — top-level const collection ———————————————

fn collect_top_consts(src: &str) -> ConstTable {
    let b = src.as_bytes();
    let needle = b"const ";
    let mut entries: HashMap<String, ConstEntry> = HashMap::new();
    let mut i = 0;
    while i + 6 <= b.len() {
        if &b[i..i + 6] == needle && (i == 0 || is_ident_terminator(b[i - 1])) {
            let mut j = skip_ws_comments(b, i + 6);
            let id_start = j;
            while j < b.len() && (b[j].is_ascii_alphanumeric() || b[j] == b'_' || b[j] == b'$') {
                j += 1;
            }
            let ident = String::from_utf8_lossy(&b[id_start..j]).to_string();
            let k = skip_ws_comments(b, j);
            if k < b.len() && b[k] == b'=' && !ident.is_empty() {
                let vs = skip_ws_comments(b, k + 1);
                let ve = skip_value(b, vs);
                let raw = src[vs..ve].to_string();
                let as_string = match b.get(vs) {
                    Some(b'\'') | Some(b'"') => Some(unescape_js_string(&src[vs..ve])),
                    Some(b'`') => Some(extract_template_literal(&src[vs..ve])),
                    _ => None,
                };
                let obj_range = match b.get(vs) {
                    Some(b'{') => {
                        let e = skip_braced(b, vs, b'{', b'}');
                        // Store the *outer* range (including braces) so callers
                        // that dispatch on `b[i] == b'{'` work consistently.
                        Some((vs, e))
                    }
                    _ => None,
                };
                entries.insert(
                    ident,
                    ConstEntry {
                        raw,
                        as_string,
                        obj_range,
                    },
                );
                i = ve;
                continue;
            }
        }
        // Skip past strings/templates/comments so we don't match `const `
        // inside a string.
        match b[i] {
            b'\'' | b'"' => i = skip_string(b, i),
            b'`' => i = skip_template(b, i),
            b'/' if i + 1 < b.len() && b[i + 1] == b'/' => {
                i += 2;
                while i < b.len() && b[i] != b'\n' {
                    i += 1;
                }
            }
            b'/' if i + 1 < b.len() && b[i + 1] == b'*' => {
                i += 2;
                while i + 1 < b.len() && !(b[i] == b'*' && b[i + 1] == b'/') {
                    i += 1;
                }
                i = (i + 2).min(b.len());
            }
            _ => i += 1,
        }
    }
    ConstTable { entries }
}

fn is_ident_terminator(c: u8) -> bool {
    c.is_ascii_whitespace()
        || matches!(
            c,
            b'=' | b'(' | b'{' | b'[' | b';' | b',' | b':' | b'&' | b'|' | b'<'
        )
}

// — rule id & test-block discovery ———————————————

/// Find `ruleTester.run('<id>', ...)` and extract the rule id from the first
/// string-literal argument.
fn find_rule_id(b: &[u8]) -> Option<String> {
    let needle = b"ruleTester.run";
    let mut search = 0;
    while let Some(rel) = find_subslice(&b[search..], needle) {
        let i = search + rel;
        let mut j = skip_ws_comments(b, i + needle.len());
        // Skip a TS type argument `<RuleOptions>` if present.
        if j < b.len() && b[j] == b'<' {
            j = skip_angle_brackets(b, j);
            j = skip_ws_comments(b, j);
        }
        if j >= b.len() || b[j] != b'(' {
            search = i + 1;
            continue;
        }
        let args_end = skip_braced(b, j, b'(', b')');
        let k = skip_ws_comments(b, j + 1);
        // First form: `ruleTester.run('id', rule, { ... })`.
        if k < args_end && (b[k] == b'\'' || b[k] == b'"') {
            let qend = skip_string(b, k);
            return Some(src_range(b, k + 1, qend.saturating_sub(1).max(k + 1)));
        }
        // Second form: `ruleTester.run({ ... }, 'id')` — try the second arg.
        let a1_end = skip_value(b, k);
        let mut k2 = skip_ws_comments(b, a1_end);
        if k2 < args_end && b[k2] == b',' {
            k2 += 1;
            k2 = skip_ws_comments(b, k2);
            if k2 < args_end && (b[k2] == b'\'' || b[k2] == b'"') {
                let qend = skip_string(b, k2);
                return Some(src_range(b, k2 + 1, qend.saturating_sub(1).max(k2 + 1)));
            }
        }
        search = i + 1;
    }
    None
}

/// Find the byte range of the test-block object literal (the argument of
/// `ruleTester.run` whose body contains both `valid:` and `invalid:` keys).
/// Returns the range *inside* the braces.
fn find_test_block(b: &[u8]) -> Option<(usize, usize)> {
    let needle = b"ruleTester.run";
    let mut search = 0;
    while let Some(rel) = find_subslice(&b[search..], needle) {
        let i = search + rel;
        let mut j = skip_ws_comments(b, i + needle.len());
        // Skip a TS type argument `<RuleOptions>` if present.
        if j < b.len() && b[j] == b'<' {
            j = skip_angle_brackets(b, j);
            j = skip_ws_comments(b, j);
        }
        if j >= b.len() || b[j] != b'(' {
            search = i + 1;
            continue;
        }
        let args_end = skip_braced(b, j, b'(', b')');
        let mut k = skip_ws_comments(b, j + 1);
        while k < args_end {
            let a_end = skip_value(b, k);
            if b.get(k) == Some(&b'{') {
                let inner_end = skip_braced(b, k, b'{', b'}');
                let props = parse_object_props(b, k + 1, inner_end - 1);
                let has_valid = props.iter().any(|p| p.key == "valid");
                let has_invalid = props.iter().any(|p| p.key == "invalid");
                if has_valid && has_invalid {
                    return Some((k + 1, inner_end - 1));
                }
            }
            k = skip_ws_comments(b, a_end);
            if k < args_end && b[k] == b',' {
                k += 1;
                k = skip_ws_comments(b, k);
            }
        }
        search = i + 1;
    }
    None
}

// — test array → cases ———————————————

fn parse_case_array(
    src: &str,
    consts: &ConstTable,
    arr_start: usize,
    arr_end: usize,
    invalid: bool,
) -> Result<Vec<RawCase>> {
    let b = src.as_bytes();
    let (start, end) = if b.get(arr_start) == Some(&b'[') {
        (arr_start + 1, arr_end.saturating_sub(1))
    } else {
        (arr_start, arr_end)
    };
    let mut cases = Vec::new();
    for (el_start, el_end) in split_top_commas(b, start, end) {
        match parse_case(src, consts, el_start, el_end, invalid) {
            Ok(Some(c)) => cases.push(c),
            Ok(None) => {}
            Err(e) => eprintln!("port-fixture: skipping case at offset {el_start}: {e:#}"),
        }
    }
    Ok(cases)
}

fn parse_case(
    src: &str,
    consts: &ConstTable,
    start: usize,
    end: usize,
    invalid: bool,
) -> Result<Option<RawCase>> {
    let b = src.as_bytes();
    let i = skip_ws_comments(b, start);
    if i >= end {
        return Ok(None);
    }

    // A bare string element (e.g. valid: ['query myQuery { a }']) is shorthand
    // for `{ code: '...' }`. Recognise it and synthesize the props map.
    let mut props: HashMap<String, (usize, usize)> = HashMap::new();
    // Track a wrapping helper name like `withSchema({ ... })` so we can apply
    // its semantics after reading the inner object's props.
    let mut wrapper: Option<String> = None;

    if b[i] == b'\'' || b[i] == b'"' || b[i] == b'`' {
        props.insert("code".to_string(), (i, end));
    } else if b[i] == b'{' {
        accumulate_props(src, consts, i, end, &mut props, 0);
    } else {
        // Identifier — either a function call (e.g. `withSchema({ ... })`) or a
        // const-identifier shorthand.
        let id_start = i;
        let mut id_end = i;
        while id_end < end
            && (b[id_end].is_ascii_alphanumeric() || b[id_end] == b'_' || b[id_end] == b'$')
        {
            id_end += 1;
        }
        let ident = src_range(b, id_start, id_end).to_string();
        let after = skip_ws_comments(b, id_end);
        if after < end && b[after] == b'(' {
            // Function-call wrapper: take the first argument as the case object.
            wrapper = Some(ident.clone());
            let call_end = skip_braced(b, after, b'(', b')');
            let arg_start = skip_ws_comments(b, after + 1);
            if arg_start < call_end && b[arg_start] == b'{' {
                accumulate_props(
                    src,
                    consts,
                    arg_start,
                    call_end.saturating_sub(1),
                    &mut props,
                    0,
                );
            }
        } else {
            // Maybe a const-identifier shorthand — try resolving it as a string.
            props.insert("code".to_string(), (i, end));
        }
    }

    // `code` is required.
    let code = match props.get("code").copied() {
        Some(sp) => match consts.resolve_string(b, sp.0, sp.1) {
            Some(s) => s,
            None => return Err(anyhow!("`code` is not a string literal")),
        },
        None => return Ok(None), // spec: missing `code` → skip + log
    };
    if code.trim().is_empty() {
        return Ok(None);
    }

    let schema = find_stringdeep(src, consts, &props, "schema");
    let documents = find_stringdeep(src, consts, &props, "documents");

    // `withSchema(...)` (test-utils.ts) returns `{ code, parserOptions: {
    // graphQLConfig: { schema: code } }, ...rest }` — i.e. it injects `schema
    // = code`. Apply that behavior here so the emitted config.toml mirrors it.
    let schema = if wrapper.as_deref() == Some("withSchema") {
        Some(code.clone())
    } else {
        schema
    };

    // `options` — typically `[{...}]`; first array element is the options table.
    let options = props
        .get("options")
        .and_then(|sp| js_span_to_json(b, sp.0, sp.1, consts).ok())
        .map(|v| match v {
            serde_json::Value::Array(a) => a.into_iter().next().unwrap_or(serde_json::Value::Null),
            other => other,
        })
        .unwrap_or(serde_json::Value::Null);

    // `errors` — invalid only. May be a count or array of objects.
    let errors = if invalid {
        props
            .get("errors")
            .and_then(|sp| parse_errors(b, sp.0, sp.1).ok())
    } else {
        None
    };

    let hash = xxh3_64_hex(&src.as_bytes()[start..end]);
    let line = line_of(src, start);

    Ok(Some(RawCase {
        line,
        code,
        schema,
        documents,
        options,
        errors,
        loose_message: false,
        hash,
    }))
}

fn parse_errors(b: &[u8], start: usize, end: usize) -> Result<Vec<RawError>> {
    let i = skip_ws_comments(b, start);
    match b.get(i) {
        Some(b'[') => {
            let arr_end = skip_braced(b, i, b'[', b']');
            let mut out = Vec::new();
            for (es, _ee) in split_top_commas(b, i + 1, arr_end.saturating_sub(1)) {
                if b.get(es) != Some(&b'{') {
                    continue;
                }
                let obj_end = skip_braced(b, es, b'{', b'}');
                let props = parse_object_props(b, es, obj_end);
                let mut msg = String::new();
                let mut line = None;
                let mut column = None;
                for p in props {
                    match p.key.as_str() {
                        "message" => {
                            msg = read_stringlike(b, p.val_start, p.val_end);
                        }
                        "line" => {
                            line = src_range(b, p.val_start, p.val_end).trim().parse().ok();
                        }
                        "column" => {
                            column = src_range(b, p.val_start, p.val_end).trim().parse().ok();
                        }
                        _ => {}
                    }
                }
                out.push(RawError {
                    message: msg,
                    line,
                    column,
                });
            }
            Ok(out)
        }
        _ => {
            // `errors: <number>` — emit N placeholder entries.
            let n: usize = src_range(b, i, end).trim().parse().unwrap_or(0);
            let mut out = Vec::with_capacity(n);
            for _ in 0..n {
                out.push(RawError {
                    message: "<unknown>".into(),
                    line: None,
                    column: None,
                });
            }
            Ok(out)
        }
    }
}

/// Read a string/template value at the given range; falls back to the verbatim
/// trimmed text for identifier-form (e.g. a `message` constant).
fn read_stringlike(b: &[u8], start: usize, end: usize) -> String {
    let i = skip_ws_comments(b, start);
    if i >= end {
        return String::new();
    }
    match b[i] {
        b'\'' | b'"' => {
            let qend = skip_string(b, i);
            unescape_js_string(&src_range(b, i, qend))
        }
        b'`' => {
            let tend = skip_template(b, i);
            extract_template_literal(&src_range(b, i, tend))
        }
        _ => src_range(b, i, end).trim().to_string(),
    }
}

/// Recursively (a couple of levels deep for spreads) accumulate object
/// properties into `out` from the literal-or-identifier span `[start, end)`.
fn accumulate_props(
    src: &str,
    consts: &ConstTable,
    start: usize,
    end: usize,
    out: &mut HashMap<String, (usize, usize)>,
    depth: u32,
) {
    let b = src.as_bytes();
    let i = skip_ws_comments(b, start);
    if i >= end {
        return;
    }
    if b[i] == b'{' {
        let obj_end = skip_braced(b, i, b'{', b'}');
        for p in parse_object_props(b, i, obj_end) {
            if p.key.starts_with("...") {
                if depth < 2 {
                    let ident = p.key.trim_start_matches("...");
                    if let Some(span) = consts.obj_range_of(ident) {
                        accumulate_props(src, consts, span.0, span.1, out, depth + 1);
                    }
                }
            } else {
                out.entry(p.key).or_insert((p.val_start, p.val_end));
            }
        }
    } else {
        // Identifier — try resolving it to a const object literal.
        let ident = src[i..end].trim();
        if depth == 0 {
            if let Some(span) = consts.obj_range_of(ident) {
                accumulate_props(src, consts, span.0, span.1, out, depth + 1);
            }
        }
    }
}

/// Look through the accumulator for a string-typed key, also descending into
/// `parserOptions.graphQLConfig.<key>`.
fn find_stringdeep(
    src: &str,
    consts: &ConstTable,
    props: &HashMap<String, (usize, usize)>,
    key: &str,
) -> Option<String> {
    let b = src.as_bytes();
    if let Some(sp) = props.get(key) {
        if let Some(s) = consts.resolve_string(b, sp.0, sp.1) {
            return Some(s);
        }
    }
    if let Some(po) = props.get("parserOptions") {
        if let Some(range) = consts.resolve_object_range(b, po.0, po.1) {
            let po_props = object_props_view(b, range.0, range.1, consts);
            if let Some(gc) = po_props.get("graphQLConfig") {
                if let Some(range) = consts.resolve_object_range(b, gc.0, gc.1) {
                    let gc_props = object_props_view(b, range.0, range.1, consts);
                    if let Some(sp) = gc_props.get(key) {
                        if let Some(s) = consts.resolve_string(b, sp.0, sp.1) {
                            return Some(s);
                        }
                    }
                }
            }
        }
    }
    None
}

/// Parse object props but also resolve spreads inside the literal.
fn object_props_view(
    b: &[u8],
    start: usize,
    end: usize,
    consts: &ConstTable,
) -> HashMap<String, (usize, usize)> {
    let mut out = HashMap::new();
    let i = skip_ws_comments(b, start);
    if i >= end || b.get(i) != Some(&b'{') {
        return out;
    }
    let obj_end = skip_braced(b, i, b'{', b'}');
    for p in parse_object_props(b, i, obj_end) {
        if p.key.starts_with("...") {
            let ident = p.key.trim_start_matches("...");
            if let Some(span) = consts.obj_range_of(ident) {
                for (k, v) in object_props_view(b, span.0, span.1, consts) {
                    out.entry(k).or_insert(v);
                }
            }
        } else {
            out.entry(p.key).or_insert((p.val_start, p.val_end));
        }
    }
    out
}

// — JS string / template / value → JSON conversions ———————————————

fn unescape_js_string(s: &str) -> String {
    let b = s.as_bytes();
    if b.is_empty() {
        return String::new();
    }
    let quote = b[0];
    if quote != b'\'' && quote != b'"' {
        return s.trim().to_string();
    }
    let mut out = String::new();
    let mut i = 1;
    let end = if b.last() == Some(&quote) {
        b.len() - 1
    } else {
        b.len()
    };
    while i < end {
        if b[i] == b'\\' && i + 1 < end {
            match b[i + 1] {
                b'n' => out.push('\n'),
                b't' => out.push('\t'),
                b'r' => out.push('\r'),
                b'\\' => out.push('\\'),
                b'\'' => out.push('\''),
                b'"' => out.push('"'),
                b'`' => out.push('`'),
                _ => out.push(b[i + 1] as char),
            }
            i += 2;
        } else {
            out.push(b[i] as char);
            i += 1;
        }
    }
    out
}

fn extract_template_literal(s: &str) -> String {
    let b = s.as_bytes();
    if b.is_empty() || b[0] != b'`' {
        return s.trim().to_string();
    }
    let mut out = String::new();
    let mut i = 1;
    let end = if b.last() == Some(&b'`') {
        b.len() - 1
    } else {
        b.len()
    };
    // Trim a single leading newline (graphql-eslint fixtures start their
    // template literals with a newline for indentation purposes).
    let mut started = false;
    while i < end {
        if b[i] == b'\\' && i + 1 < end {
            match b[i + 1] {
                b'`' => out.push('`'),
                b'\\' => out.push('\\'),
                b'$' => out.push('$'),
                b'n' => out.push('\n'),
                b't' => out.push('\t'),
                _ => out.push(b[i + 1] as char),
            }
            i += 2;
            continue;
        }
        if b[i] == b'$' && i + 1 < end && b[i + 1] == b'{' {
            let inner = skip_braced(b, i + 1, b'{', b'}');
            out.push_str(&String::from_utf8_lossy(
                &b[i + 2..inner.saturating_sub(1).max(i + 2)],
            ));
            i = inner;
            continue;
        }
        let c = b[i];
        if !started && (c == b'\n' || c == b'\r') {
            started = true;
            i += 1;
            continue;
        }
        started = true;
        out.push(c as char);
        i += 1;
    }
    // Strip a single trailing newline (mirrors the leading-newline trim so the
    // fixture's `NN.graphql` doesn't end with an extra blank line).
    while out.ends_with('\n') || out.ends_with('\r') {
        out.pop();
    }
    out
}

/// Convert a JS span into a JSON value. Supports strings (single/double/backtick
/// template), numbers, booleans, null, arrays, and objects (with bareword or
/// quoted keys, computed keys best-effort: the `[Kind.X]` text becomes a JSON
/// string key). Identifiers become a JSON string of their text (so users see
/// `Kind.ENUM` rather than nothing — easier cleanup).
fn js_span_to_json(
    b: &[u8],
    start: usize,
    end: usize,
    consts: &ConstTable,
) -> Result<serde_json::Value> {
    let i = skip_ws_comments(b, start);
    if i >= end {
        return Ok(serde_json::Value::Null);
    }
    match b[i] {
        b'\'' | b'"' => {
            let qend = skip_string(b, i);
            Ok(serde_json::Value::String(unescape_js_string(&src_range(
                b, i, qend,
            ))))
        }
        b'`' => {
            let tend = skip_template(b, i);
            Ok(serde_json::Value::String(extract_template_literal(
                &src_range(b, i, tend),
            )))
        }
        b'[' => {
            let arr_end = skip_braced(b, i, b'[', b']');
            let mut out = Vec::new();
            for (es, ee) in split_top_commas(b, i + 1, arr_end.saturating_sub(1)) {
                out.push(js_span_to_json(b, es, ee, consts)?);
            }
            Ok(serde_json::Value::Array(out))
        }
        b'{' => {
            let obj_end = skip_braced(b, i, b'{', b'}');
            let props = parse_object_props(b, i, obj_end);
            let mut map = serde_json::Map::new();
            for p in props {
                if p.key.starts_with("...") {
                    let ident = p.key.trim_start_matches("...");
                    if let Some(span) = consts.obj_range_of(ident) {
                        if let serde_json::Value::Object(m) =
                            js_span_to_json(b, span.0, span.1, consts)?
                        {
                            for (k, v) in m {
                                map.insert(k, v);
                            }
                        }
                    }
                    continue;
                }
                let val = js_span_to_json(b, p.val_start, p.val_end, consts)?;
                map.insert(p.key, val);
            }
            Ok(serde_json::Value::Object(map))
        }
        b't' | b'f' => {
            let s = src_range(b, i, end).trim().to_string();
            Ok(serde_json::Value::Bool(s == "true"))
        }
        b'n' => Ok(serde_json::Value::Null),
        c if c == b'-' || c.is_ascii_digit() => {
            let s = src_range(b, i, end).trim().to_string();
            if let Ok(n) = s.parse::<i64>() {
                Ok(serde_json::json!(n))
            } else if let Ok(f) = s.parse::<f64>() {
                Ok(serde_json::json!(f))
            } else {
                Err(anyhow!("unparseable number `{s}`"))
            }
        }
        _ => {
            let s = src_range(b, i, end).trim().to_string();
            if let Some(str_val) = consts.entries.get(&s).and_then(|e| e.as_string.clone()) {
                Ok(serde_json::Value::String(str_val))
            } else {
                Ok(serde_json::Value::String(s))
            }
        }
    }
}

// — JSON → TOML value ———————————————

fn json_to_toml_value(v: &serde_json::Value) -> Result<toml::Value> {
    match v {
        serde_json::Value::Null => Err(anyhow!("null not expressible in TOML")),
        serde_json::Value::Bool(b) => Ok(toml::Value::Boolean(*b)),
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                Ok(toml::Value::Integer(i))
            } else if let Some(f) = n.as_f64() {
                Ok(toml::Value::Float(f))
            } else {
                Err(anyhow!("unsupported JSON number `{n}`"))
            }
        }
        serde_json::Value::String(s) => Ok(toml::Value::String(s.clone())),
        serde_json::Value::Array(a) => {
            // Arrays-of-tables require homogeneous element types in TOML; reject
            // arrays whose elements are tables (forcing the `options_json`
            // fallback) and numbers/etc. stay as TOML arrays.
            if a.iter().any(|e| matches!(e, serde_json::Value::Object(_))) {
                return Err(anyhow!(
                    "TOML cannot represent arrays of tables — using options_json fallback"
                ));
            }
            let mut out = Vec::with_capacity(a.len());
            for e in a {
                out.push(json_to_toml_value(e)?);
            }
            Ok(toml::Value::Array(out))
        }
        serde_json::Value::Object(o) => {
            let mut t = toml::value::Table::new();
            for (k, v) in o {
                t.insert(k.clone(), json_to_toml_value(v)?);
            }
            Ok(toml::Value::Table(t))
        }
    }
}

// — meta.docs.configOptions extractor ———————————————

fn extract_config_options(ts_path: &Path) -> Result<Option<String>> {
    let body = match fs::read_to_string(ts_path) {
        Ok(b) => b,
        Err(_) => return Ok(None),
    };
    let b = body.as_bytes();
    let needle = b"configOptions";
    let mut search = 0;
    while let Some(rel) = find_subslice(&b[search..], needle) {
        let i = search + rel;
        let mut j = skip_ws_comments(b, i + needle.len());
        if j >= b.len() || b[j] != b':' {
            search = i + 1;
            continue;
        }
        j += 1;
        let vs = skip_ws_comments(b, j);
        if b.get(vs) != Some(&b'[') {
            search = i + 1;
            continue;
        }
        let arr_end = skip_braced(b, vs, b'[', b']');
        let ek = skip_ws_comments(b, vs + 1);
        if ek >= arr_end || b[ek] != b'{' {
            search = i + 1;
            continue;
        }
        let obj_end = skip_braced(b, ek, b'{', b'}');
        return Ok(Some(body[ek..obj_end].to_string()));
    }
    Ok(None)
}

// — utilities ———————————————

fn find_subslice(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() {
        return Some(0);
    }
    haystack.windows(needle.len()).position(|w| w == needle)
}

fn xxh3_64_hex(data: &[u8]) -> String {
    let h = xxhash_rust::xxh3::xxh3_64(data);
    format!("{h:016x}")
}

fn src_range(b: &[u8], start: usize, end: usize) -> String {
    String::from_utf8_lossy(&b[start.min(b.len())..end.min(b.len())]).to_string()
}

fn line_of(src: &str, pos: usize) -> usize {
    src[..pos.min(src.len())].matches('\n').count() + 1
}

// ─────────────────────── manifest types ───────────────────────

#[derive(Serialize, Deserialize)]
struct Manifest {
    source: String,
    source_hash: String,
    rule: String,
    valid_count: usize,
    invalid_count: usize,
    cases: Vec<ManifestCase>,
}

#[derive(Serialize, Deserialize)]
struct ManifestCase {
    kind: String,
    id: String,
    hash: String,
    line: usize,
}

// ─────────────────────── tests ───────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;

    fn write_tmp_rule(name: &str, ts: &str) -> PathBuf {
        let root = std::env::temp_dir()
            .join("rglint-port-fixture-tests")
            .join(name);
        let rule_dir = root
            .join("packages")
            .join("plugin")
            .join("src")
            .join("rules")
            .join(name);
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&rule_dir).unwrap();
        fs::write(rule_dir.join("index.test.ts"), ts).unwrap();
        root
    }

    fn run_port(source_root: &Path, fixtures_dir: &Path, rule: &str) -> Result<usize> {
        let args = PortFixtureArgs {
            rule: Some(rule.to_string()),
            all: false,
            force: false,
            source_root: Some(source_root.to_path_buf()),
            fixtures_dir: Some(fixtures_dir.to_path_buf()),
        };
        run(args)?;
        let mut n = 0;
        for kind in ["valid", "invalid"] {
            let dir = fixtures_dir.join(rule).join(kind);
            if dir.is_dir() {
                n += fs::read_dir(&dir)
                    .unwrap()
                    .filter_map(|e| e.ok())
                    .filter(|e| e.path().is_dir())
                    .count();
            }
        }
        Ok(n)
    }

    #[test]
    fn ports_simple_string_cases() {
        let ts = r#"
import { ruleTester } from '../../../__tests__/test-utils.js';
import { rule } from './index.js';
ruleTester.run('no-anonymous-operations', rule, {
  valid: ['query myQuery { a }', 'mutation doSomething { a }'],
  invalid: [
    { code: 'query { a }', errors: 1 },
    { code: 'mutation { renamed: a }', errors: 1 },
  ],
});
"#;
        let source_root = write_tmp_rule("no-anonymous-operations", ts);
        let fixtures_dir = std::env::temp_dir().join("rglint-port-fixture-out-1");
        let _ = fs::remove_dir_all(&fixtures_dir);
        let n = run_port(&source_root, &fixtures_dir, "no-anonymous-operations").unwrap();
        assert_eq!(n, 4);
        let case01 = fixtures_dir
            .join("no-anonymous-operations")
            .join("invalid")
            .join("01");
        assert!(case01.join("graphql").is_file());
        assert!(case01.join("config.toml").is_file());
        assert!(case01.join("expected.json").is_file());
        let gql = fs::read_to_string(case01.join("graphql")).unwrap();
        assert_eq!(gql, "query { a }");
        let expected = fs::read_to_string(case01.join("expected.json")).unwrap();
        assert!(expected.contains("\"rule\":\"no-anonymous-operations\""));
    }

    #[test]
    fn idempotent_rerun_no_writes() {
        let ts = r#"
ruleTester.run('r', rule, {
  valid: ['query { a }'],
  invalid: [{ code: 'query { a }', errors: 1 }],
});
"#;
        let source_root = write_tmp_rule("r", ts);
        let fixtures_dir = std::env::temp_dir().join("rglint-port-fixture-out-2");
        let _ = fs::remove_dir_all(&fixtures_dir);
        run_port(&source_root, &fixtures_dir, "r").unwrap();
        let first_mtime = fs::metadata(fixtures_dir.join("r").join("manifest.json"))
            .unwrap()
            .modified()
            .unwrap();
        std::thread::sleep(std::time::Duration::from_millis(20));
        run_port(&source_root, &fixtures_dir, "r").unwrap();
        let second_mtime = fs::metadata(fixtures_dir.join("r").join("manifest.json"))
            .unwrap()
            .modified()
            .unwrap();
        assert_eq!(
            first_mtime, second_mtime,
            "manifest should not be rewritten on idempotent rerun"
        );
    }

    #[test]
    fn force_regenerates() {
        let ts = r#"
ruleTester.run('r2', rule, {
  valid: ['query { a }'],
  invalid: [],
});
"#;
        let source_root = write_tmp_rule("r2", ts);
        let fixtures_dir = std::env::temp_dir().join("rglint-port-fixture-out-3");
        let _ = fs::remove_dir_all(&fixtures_dir);
        run_port(&source_root, &fixtures_dir, "r2").unwrap();

        // Corrupt the manifest's source_hash; re-running without --force must
        // regenerate (because source_hash no longer matches the file's hash).
        let mpath = fixtures_dir.join("r2").join("manifest.json");
        let mut m: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&mpath).unwrap()).unwrap();
        m["source_hash"] = serde_json::json!("0000000000000000");
        fs::write(&mpath, m.to_string()).unwrap();

        run_port(&source_root, &fixtures_dir, "r2").unwrap();
        let m2: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&mpath).unwrap()).unwrap();
        assert_ne!(m2["source_hash"], "0000000000000000");
    }

    #[test]
    fn schema_and_options_emit_in_config_toml() {
        let ts = r#"
const TEST_SCHEMA = `type Query { x: Int }`;
ruleTester.run('depth', rule, {
  valid: [],
  invalid: [
    {
      options: [{ maxDepth: 2 }],
      parserOptions: { graphQLConfig: { schema: TEST_SCHEMA } },
      code: `query { x }`,
      errors: [{ message: 'too deep' }],
    },
  ],
});
"#;
        let source_root = write_tmp_rule("depth", ts);
        let fixtures_dir = std::env::temp_dir().join("rglint-port-fixture-out-4");
        let _ = fs::remove_dir_all(&fixtures_dir);
        run_port(&source_root, &fixtures_dir, "depth").unwrap();
        let cfg = fs::read_to_string(
            fixtures_dir
                .join("depth")
                .join("invalid")
                .join("01")
                .join("config.toml"),
        )
        .unwrap();
        assert!(cfg.contains("schema"), "schema missing: {cfg}");
        assert!(
            cfg.contains("type Query { x: Int }"),
            "schema value missing: {cfg}"
        );
        assert!(cfg.contains("[options]"), "options table missing: {cfg}");
        assert!(cfg.contains("maxDepth = 2"), "options value missing: {cfg}");
    }

    #[test]
    fn extract_template_literal_strips_leading_newline() {
        let s = "`\n  type Query {\n    x: Int\n  }\n`";
        let v = extract_template_literal(s);
        assert!(v.starts_with("  type Query"), "got: {v:?}");
        assert!(!v.ends_with('\n'));
    }

    #[test]
    fn unescape_js_string_basic() {
        assert_eq!(unescape_js_string(r#"'a\nb'"#), "a\nb");
        assert_eq!(unescape_js_string(r#""x'y""#), "x'y");
    }

    #[test]
    fn rule_id_extraction() {
        let rule =
            find_rule_id(b"ruleTester.run('foo-bar', rule, { valid:[], invalid:[] });").unwrap();
        assert_eq!(rule, "foo-bar");
    }
}
