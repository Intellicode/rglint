# Spec-072: Project-local rule configuration

## Goal

Allow one `.rglintrc` to assign different rule presets and overrides to named
projects, so schema and operation projects can use separate policies while
retaining the existing top-level configuration behavior.

## Scope

- Add optional `extends` and `rules` keys to each `.rglintrc` project entry.
- Resolve project presets and rule tuples during config normalization.
- Layer project-local rules over top-level rules; a project without local rules
  inherits the top-level map unchanged.
- Construct one lint engine per resolved project in the CLI.
- Validate project-local options against the supplied rule registry.
- Preserve project-local rules through JSON/TOML serialization round trips.

GraphQL-config files remain interoperable for schema/document discovery, but do
not gain rule settings because that format does not define a rule policy.

## Configuration shape

```toml
[projects.schema]
schema = "server/**/*.graphqls"
extends = "schema-recommended"

[projects.operations]
documents = "client/**/*.graphql"
extends = "operations-recommended"

[projects.operations.rules]
"selection-set-depth" = ["warn", { maxDepth = 5 }]
```

Top-level `extends` and `rules` remain the base layer for backward
compatibility. A project-local rule entry replaces the base entry with the same
id; project-local presets are resolved before project-local explicit rules.

## Behavior

- The resolver continues to bind schema/documents independently per project.
- The CLI selects the effective rule map by project name and runs fixes and
  linting with that map.
- `--rule` remains a command-wide replacement and takes precedence over all
  configured project rules.
- Unknown rule ids remain loadable; known project rules with invalid options are
  reported during the normal config validation boundary.

## Testing

- Config tests cover project presets, project tuple overrides, inheritance of
  top-level rules, and JSON/TOML round trips.
- CLI integration covers two projects with distinct rule sets and verifies
  that each project's diagnostics come only from its effective rules.
- Run `cargo test -p rglint-config`, `cargo test -p rglint`, and the workspace
  formatting/build/clippy/test checks.
