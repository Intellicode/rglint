# Spec-022: shared/case.rs (case styles & convertCase)

> Plan reference: §5 Phase 2 ("naming-convention: port case.rs once here"), §3 (`crates/rglint-rules/src/shared/case.rs`, `case_styles.rs`)

## Goal

Port the case-conversion + case-detection helpers that `naming-convention`
(spec-028) and any other rule touching identifier styles depend on. Standalone
module under `rglint-rules/shared/`, ported **before** `naming-convention` so
the big rule drops in cleanly.

## Source

`packages/plugin/src/rules/naming-convention/case-packages.ts` and related
helpers in `naming-convention/index.ts`.

## Scope

**In scope:**

- `case_styles.rs`: enums for each supported style:
  `CamelCase`, `PascalCase`, `SnakeCase`, `ScreamingSnakeCase`, `KebabCase`,
  `ScreamingKebabCase`, `StrictPascalCase`, `UpperCamelCase` (alias).
- `case.rs`:
  - `convert_case(name: &str, style: CaseStyle) -> String`.
  - `detect_case(name: &str) -> Option<CaseStyle>` (the primary style of the
    name; `None` if it matches none / is empty).
  - `is_case(name: &str, style: CaseStyle) -> bool`.
  - `split_words(name: &str) -> Vec<String>` — handles camel, pascal, snake,
    kebab, screaming, and digit boundaries (e.g. `foo2Bar` → `["foo","2","Bar"]`).
- Acronym handling option: `acronyms: Vec<String>` (default empty) — when set,
  an acronym is treated as one word in `StrictPascalCase` (e.g. `URL` stays
  `URL`, not `Url`).

**Out of scope:**

- The `naming-convention` rule itself (spec-028).
- Regex group options (handled in spec-028).

## Dependencies

- spec-001 (workspace).

## Deliverables

- `crates/rglint-rules/src/shared/case.rs`.
- `crates/rglint-rules/src/shared/case_styles.rs`.
- `crates/rglint-rules/src/shared/mod.rs` (re-exports).
- Property tests (`proptest`): `convert_case(convert_case(s, A), A) ==
  convert_case(s, A)` idempotence for each style; round-trip
  `detect_case(convert_case(s, X)) == Some(X)` for non-empty `s`.

## Interface / API

```rust
pub enum CaseStyle { Camel, Pascal, StrictPascal, Snake, ScreamingSnake, Kebab, ScreamingKebab }
pub fn convert_case(name: &str, style: CaseStyle, acronyms: &[String]) -> String;
pub fn detect_case(name: &str) -> Option<CaseStyle>;
pub fn is_case(name: &str, style: CaseStyle) -> bool { detect_case(name) == Some(style) }
pub fn split_words(name: &str) -> Vec<String>;
```

## Behavior

- Empty string → `detect_case` returns `None`; `convert_case` returns `""`.
- A name like `foo` is valid camel, snake, kebab, *and* pascal per graphql-eslint
  (single lowercase word is ambiguous-allowed) — match the TS `detectCase`
  precedence exactly (read the source for tie-breaking).
- Unicode: graphql-eslint case logic is ASCII-only; we match (don't use
  `char::to_uppercase` unicode semantics).

## Testing

- Table test mirroring `naming-convention`'s `valid` cases: for each
  identifier in the fixtures, `detect_case` returns the style the fixture
  expects.
- `proptest` idempotence + round-trip.
- Acronym: `convert_case("userUrl", Pascal, &["URL"]) == "UserURL"`.

## Risks / Notes

- PLAN §8 risk: "`naming-convention` is 579 LOC with extensive option
  combinations — port helpers up front as `shared/case.rs`; port rule last in
  Phase 2." This spec is the helper port; do it before spec-028.
