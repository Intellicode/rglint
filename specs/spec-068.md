# Spec-068: xtask gen-docs

> Plan reference: §3 (`xtask/src/gen_docs.rs`), §9 ("xtask gen-docs --check")

## Goal

Generate `docs/rules/*.md` per rule from `RuleMeta` (id, category, docs,
options, defaults, examples) and a root `docs/rules/README.md` rule table.
`--check` verifies the generated docs are up to date (CI gate per §9).

## Scope

**In scope:**

- `xtask gen-docs` — iterate `all_rules()` (spec-008) + `all_spec_rules()`
  (spec-053); for each, write `docs/rules/<id>.md`:
  - Title, category, severity, deprecated/replaced-by notice.
  - `## Description` from `RuleMeta::docs`.
  - `## Options` rendered from `option_schema` (JSON-schema → human table).
  - `## Examples` pulled from the rule's `rules-fixtures/<id>/` valid cases
    (a few representative `.graphql` snippets).
- `docs/rules/README.md` — a table of all rules (id, category, severity,
  requires_schema, requires_siblings, has_suggestions) sorted by category.
- `--check` mode: generate to a temp dir, diff against committed `docs/rules/`;
  exit non-zero if stale.

**Out of scope:**

- A full docs site generator (v1 is markdown; a static-site front is stretch).
- User-guide prose (only rule reference is generated).

## Dependencies

- spec-008 (RuleMeta + all_rules).
- spec-053 (spec rules).
- spec-015 (fixtures for examples).

## Deliverables

- `xtask/src/gen_docs.rs`.
- Generated `docs/rules/*.md` committed.
- CI integration via spec-067 (`docs-check` job).

## Interface / API

```
xtask gen-docs            # write docs/rules/
xtask gen-docs --check    # exit 0 if up to date, 1 if stale
```

## Behavior

- Generation is deterministic (sorted rules, stable formatting) so `--check`
  is stable across runs.
- `option_schema = None` → "This rule has no options.".
- Deprecated rules rendered with a deprecation banner pointing to `replaced_by`.

## Testing

- `--check` self-test: run `gen-docs`, then `--check` → exit 0.
- Mutate a `RuleMeta::docs` string → `--check` exits 1, diff shows the change.

## Risks / Notes

- JSON-schema → human table rendering: write a small renderer (depth-1
  properties → name/type/default/description table); don't pull a generic
  schema-docs crate.
