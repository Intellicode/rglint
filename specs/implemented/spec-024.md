# Spec-024: no-hashtag-description (CST trivia spike)

> Plan reference: §5 Phase 2, §3 (`crates/rglint-rules/src/schema/no_hashtag_description.rs`), §8 (apollo-parser trivia risk)

## Goal

Port `no-hashtag-description`: forbid `#`-style comments used as descriptions
on type/field/directive/argument/etc. definitions (encourage `"""..."""`/`"..."`
descriptions instead). This is the **Phase 0/2 trivia spike** — apollo-parser's
comment/trivia exposure is the #1 implementation risk per PLAN §8, and this
rule forces resolving it.

## Source

`packages/plugin/src/rules/no-hashtag-description/index.ts`

## Scope

**In scope:**

- Rule id `no-hashtag-description`, category `Schema`.
- Options: `{ allowLeadingLineComments: bool }` (default false).
- Detect `#` comments positioned immediately before a definition node (no
  blank line between, per graphql-eslint's heuristic) and report with message
  `Use """description""" instead of #description` (verify exact wording).
- The trivia access mechanism: a `CommentScanner` helper in `rglint-core`
  (or `rglint-rules/shared`) that, given a `SourceFile` + a node's span,
  returns the `#`-comment lines preceding it with their spans.

**Out of scope:**

- General comment-removal fixers.
- `description-style` (spec-023).

## Dependencies

- spec-002 (SourceFile — raw source slice + line table).
- spec-008, spec-009, spec-011, spec-014, spec-015.

## Deliverables

- A `comment_scanner.rs` in `rglint-core` (or `rglint-rules/shared/`) exposing
  `preceding_comments(source, node_span) -> Vec<Comment>`.
- `crates/rglint-rules/src/schema/no_hashtag_description.rs`.
- `rules-fixtures/no-hashtag-description/`.
- `tests/rule_no_hashtag_description.rs`.
- An ADR at `docs/design-records/0001-comment-trivia.md` recording how
  apollo-parser exposes comments (or doesn't) and the chosen approach.

## Interface / API

```rust
// rglint-core or shared
pub struct Comment { pub span: Span, pub text: String }
pub fn preceding_comments(source: &SourceFile, node_span: Span) -> Vec<Comment>;

#[derive(Rule)]
#[rule(id = "no-hashtag-description", category = "schema")]
pub struct NoHashtagDescription;
```

## Behavior

- A `#` comment counts as a "description" if it is on the line(s) immediately
  preceding the definition with no blank line between (graphql-eslint's rule).
- `allowLeadingLineComments: true` permits `#` comments that are *not*
  adjacent to a definition (file-leading or separated by blank lines).
- Reporting span = the comment's span.

## Testing

- `rglint_test_suite!("no-hashtag-description")`.
- Negative-path: a file with only `#` comments and no definitions → zero
  diagnostics, no panic.

## Risks / Notes

- §8 risk: "apollo-parser CST doesn't expose comments as addressable nodes."
  Spike options, in order of preference:
  1. apollo-parser's `cst::CST` with `Trivia` tokens — verify version 0.8.
  2. Raw token iteration via `apollo_parser::Parser` kept alongside the AST.
  3. Hand-rolled `#`-scanner over `SourceFile::source()` byte buffer (last
     resort; simplest; correct since `#` comments are line-based and
     GraphQL has no other `#` usage).
  Recommend option 3 for v1 (deterministic, no parser-version coupling) and
  record the decision in the ADR. This unblocks the rule immediately and the
  scanner is ~30 LOC.
