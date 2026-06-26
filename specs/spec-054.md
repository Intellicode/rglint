# Spec-054: Config loader (.rglintrc)

> Plan reference: §3 (`crates/rglint-config/`), §7 (Configuration Format)

## Goal

Implement the `.rglintrc` loader: discover, parse, and surface the config as
typed structs. Supports both JSON and TOML formats. Mirrors eslint config
shape per PLAN §7.

## Scope

**In scope:**

- `rglint-config::schema` — serde structs:
  - `Config { projects: Option<Map<String, ProjectConfigRaw>>, schema:
    Option<SchemaSpecRaw>, documents: Option<DocumentSpecRaw>, rules:
    Map<String, RuleConfig>, ignore: Vec<String>, format: Option<Format> }`.
  - `RuleConfig` enum: `"off" | "warn" | "error" | ["off"|"warn"|"error", {options}]` (mirrors eslint tuple form).
  - `ProjectConfigRaw` — `schema`/`documents`/`ignore` per project.
- Discovery: search upward from CWD for `.rglintrc`, `.rglintrc.json`,
  `.rglintrc.toml`, `rglint.config.json` (precedence; closest wins).
- `load(path) -> Result<Config>` with clear errors (file not found, parse
  error with span).
- Normalization: expand `projects` absent → single default project from
  top-level `schema`/`documents`; resolve `RuleConfig` tuple into
  `(Severity, serde_json::Value)`.
- `Config::rules_config() -> RulesConfig` consumed by the engine (spec-011).

**Out of scope:**

- `.graphqlrc` interop (spec-055).
- JSON-schema option validation (spec-056).
- CLI wiring (spec-062).

## Dependencies

- spec-007 (ProjectConfig — the loader produces configs this resolves).
- spec-011 (RulesConfig shape — coordinate).
- spec-008 (Severity — same enum).

## Deliverables

- `crates/rglint-config/src/{lib,schema}.rs`.
- Unit tests: load JSON + TOML forms; tuple-form rule config; default-project
  synthesis; discovery from a nested directory.

## Interface / API

```rust
pub struct Config {
    pub projects: Vec<ProjectConfigRaw>,
    pub rules: AHashMap<String, (Severity, serde_json::Value)>,
    pub ignore: Vec<String>,
    pub format: Format,
}
pub enum Format { Pretty, Json, Sarif, Github }

pub fn discover(start: &Path) -> Option<PathBuf>;
pub fn load(path: &Path) -> Result<Config>;
```

## Behavior

- Unknown rule ids in config → **not** an error at load (the engine warns when
  an unknown rule is referenced); keeps config forward-compatible.
- Unknown top-level keys → ignored with a `tracing` warn.
- `format` defaults to `Pretty`.
- `ignore` is prepended to per-project ignore.

## Testing

- Round-trip: serialize a `Config` to JSON and TOML, reload, equal.
- Tuple form `"error"` → `(Error, {})`; `["warn", {"maxDepth": 7}]` →
  `(Warn, {"maxDepth":7})`.
- Discovery: from `a/b/c/`, finds `a/.rglintrc.toml` not `root/.rglintrc`.

## Risks / Notes

- Coordinate `RulesConfig` shape with spec-011 to avoid a refactor later.
