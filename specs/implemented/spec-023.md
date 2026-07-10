# Spec-023: description-style

> Plan reference: §5 Phase 2, §3 (`crates/rglint-rules/src/schema/description_style.rs`)

## Goal

Port `description-style`: enforce a single description quotation style across
the schema — either block (`"""..."""`) or single-quote (`"..."`) — via the
`style` option. Schema-only rule.

## Source

`packages/plugin/src/rules/description-style/index.ts`

## Scope

**In scope:**

- Rule id `description-style`, category `Schema`.
- Options: `{ style: "block" | "single" }` (default `"block"`).
- Subscribe to description-bearing nodes (`ObjectTypeDefinition`,
  `FieldDefinition`, `EnumValueDefinition`, `DirectiveDefinition`, etc.) +
  their `description` child.
- For each description whose quote style ≠ configured style, report with
  message `Use ${style} descriptions` (verify exact wording) and a suggestion
  to re-quote.
- `hasSuggestions: true`.

**Out of scope:**

- `no-hashtag-description` (spec-024 — different concern: `#` comments).

## Dependencies

- spec-008, spec-009, spec-011, spec-014, spec-015.

## Deliverables

- `crates/rglint-rules/src/schema/description_style.rs`.
- `rules-fixtures/description-style/`.
- `tests/rule_description_style.rs`.

## Interface / API

```rust
#[derive(Default, Deserialize)]
struct Opts { style: DescStyle }
#[derive(Default, Deserialize, Copy, Clone)]
enum DescStyle { #[default] Block, Single }
#[derive(Rule)]
#[rule(id = "description-style", category = "schema", has_suggestions)]
pub struct DescriptionStyle;
```

## Behavior

- Detects quote style by inspecting the description node's leading source
  characters (`"""` vs `"`).
- Suggestion `Fix::Replace` rewrites the description preserving inner content
  (escaping `"` → `\"` when going block→single; un-escaping single→block).
- Nodes without a description are ignored.

## Testing

- `rglint_test_suite!("description-style")`.
- Snapshot of a block→single suggestion applied.

## Risks / Notes

- Re-quoting requires reading the raw source (not the AST string value) to
  preserve trivia; use `SourceFile::slice(span)`.
