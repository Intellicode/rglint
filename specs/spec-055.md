# Spec-055: GraphQL config (.graphqlrc) interop

> Plan reference: §3 (`crates/rglint-config/src/graphql_config.rs`), §1 (Config Loader), §5 Phase 8

## Goal

Interoperate with the existing GraphQL ecosystem config formats:
`.graphqlrc`, `.graphqlrc.{yml,yaml,json}`, `.graphqlconfig`, `.graphqlconfig.{yml,json}`. rglint reads `schema`/`documents`/`projects` from these when
no `.rglintrc` is present, so users with existing graphql-config can adopt
rglint without duplicating config.

## Scope

**In scope:**

- `rglint-config::graphql_config` — parser for the graphql-config schema:
  - `schema: string | { [project]: string }` (path/glob/URL — URL rejected with
    clear error).
  - `documents`: same shape.
  - `projects: { [name]: { schema, documents } }` (multi-project form).
  - `include`/`exclude` per project.
- Discovery: when no `.rglintrc` found, search for graphql-config files
  upward (spec-054 delegates here).
- Conversion: graphql-config → rglint `Config` (rules section empty — the user
  gets the default rule preset, spec-063).
- YAML support via `serde_yaml` (add to deps).

**Out of scope:**

- `.rglintrc` itself (spec-054).
- HTTP schema documents (PLAN §11 stretch — error out).

## Dependencies

- spec-054 (Config target type).

## Deliverables

- `crates/rglint-config/src/graphql_config.rs`.
- `crates/rglint-config/Cargo.toml` adds `serde_yaml`.
- Integration test mirroring
  `examples/multiple-projects-graphql-config`.

## Interface / API

```rust
pub fn discover_graphql_config(start: &Path) -> Option<PathBuf>;
pub fn load_graphql_config(path: &Path) -> Result<Config>;
// Produces a Config with empty rules; engine applies default preset.
```

## Behavior

- Single-project graphql-config (no `projects` key) → one rglint project
  named `default`.
- Multi-project (`projects: { web: ..., admin: ... }`) → N rglint projects.
- `schema` as `{ http: "...url..." }` → error "HTTP schema documents not
  supported yet" (PLAN §11).
- Comments / extension keys ignored.

## Testing

- Load `tests/fixtures/graphqlrc/multi.yaml` → 2 projects.
- Load `.graphqlconfig.json` legacy form → 1 project.
- HTTP schema → error variant asserted.

## Risks / Notes

- graphql-config has several schema revisions; support the common subset used
  by graphql-eslint examples and error clearly on unknown shapes.
