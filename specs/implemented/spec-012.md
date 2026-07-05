# Spec-012: Shared utils & node_name helpers

> Plan reference: §3 (`crates/rglint-core/src/node_name.rs`, `utils.rs`)

## Goal

Port the small shared helpers that many rules use: `getNodeName` (returns a
node's name string regardless of node kind) and the misc utils
(`ARRAY_DEFAULT_OPTIONS`, case-check helpers used across rules). Keeping these
in one place avoids duplication and gives rules a stable API.

## Scope

**In scope:**

- `node_name(node: &Node) -> Option<String>` — returns the `name.value` for any
  named definition node (ObjectTypeDefinition, FieldDefinition, EnumValue,
  OperationDefinition, FragmentDefinition, DirectiveDefinition, etc.), and
  `None` for anonymous operations / nameless nodes.
- `utils::ARRAY_DEFAULT_OPTIONS` helper: given an option that may be a scalar
  or an array, normalize to `Vec<T>` (mirrors graphql-eslint's
  `ARRAY_DEFAULT_OPTIONS`).
- `utils::is_type_field_definition(node) -> bool` and similar tiny predicates
  reused by ≥2 rules (add as discovered; start with the ones below).
- `utils::get_document_type(node) -> DocumentKind` (`Operation` | `Fragment`).
- `utils::strip_leading_slash`/path helpers for `match-document-filename`
  (spec-033 uses these).

**Out of scope:**

- Case-style helpers (spec-022 — those live in `rglint-rules/shared/case.rs`,
  not core).
- Relay/oneOf predicates (specs 044, 049 — rule-crate-specific).

## Dependencies

- spec-002 (Span — node_name returns text via SourceFile slice).
- spec-004/005 (Node types from apollo-compiler).

## Deliverables

- `crates/rglint-core/src/node_name.rs`.
- `crates/rglint-core/src/utils.rs`.
- Unit tests covering every named node kind + the array-normalizer.

## Interface / API

```rust
pub fn node_name(node: &Node<'_>) -> Option<String>;

pub enum DocumentKind { Operation, Fragment }
pub fn get_document_type(node: &Node<'_>) -> Option<DocumentKind>;

pub fn array_default_options<T: DeserializeOwned>(v: &serde_json::Value) -> Vec<T>;
// accepts T or [T, ...] or missing -> []

pub fn is_field_definition(node: &Node<'_>) -> bool;
pub fn is_object_type_definition(node: &Node<'_>) -> bool;
```

## Behavior

- `node_name` never panics; returns `None` for nodes without a name token.
- `array_default_options` accepts `7` → `vec![7]`, `[1,2]` → `vec![1,2]`,
  `null`/missing → `vec![]`, type mismatch → empty (log warn) to keep rules
  resilient.

## Testing

- Table test: one fixture per named node kind, assert `node_name` matches.
- Anonymous operation → `None`.
- `array_default_options::<i64>` over `7`, `[1,2]`, `null`, `"x"` (type
  mismatch → `[]`).

## Risks / Notes

- This spec is small but central; grow the predicate list incrementally as
  rules are ported. Keep `node_name.rs` stable — its signature is the most
  widely depended-on helper.
