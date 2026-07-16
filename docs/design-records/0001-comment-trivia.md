# ADR-0001: Comment/Trivia Access Strategy

## Context

Spec-024 (`no-hashtag-description`) requires detecting `#`-style comments
positioned immediately before a definition node (no blank line between).
`apollo-parser` 0.8 does not expose comments as addressable CST nodes — comments
are consumed as trivia that `apollo-parser`'s lexer discards before building the
rowan-based CST. There is no `COMMENT` SyntaxKind in the CST node tree, and
no API to retrieve trivia from a node.

## Options Considered

1. **apollo-parser CST Trivia tokens** — The rowan CST underlying `apollo-parser`
   stores trivia on green tokens, but `apollo-parser` 0.8 does not expose them
   through its public API. Upgrading to a hypothetical future version that does
   would couple the rule to parser internals.

2. **apollo-compiler AST token stream** — `apollo-compiler` retains tokens, but
   there is no stable API to query "tokens preceding node X filtered by kind
   COMMENT". Walking the token stream and correlating positions with the CST
   would be fragile and version-dependent.

3. **Hand-rolled `#`-scanner over the raw source** — Since `#` is the only
   comment marker in GraphQL (no `//` or `/* */`), and `#` is not used for any
   other purpose, a simple byte-scanner that walks backwards from a node's
   start position over lines of source text is correct and deterministic.

## Decision

**Adopt option 3**: a `preceding_comments` function in
`crates/rglint-rules/src/shared/comment_scanner.rs` that:

- Takes a `&SourceFile` and the node's `Span`
- Walks backwards line-by-line from the node's start position
- Skips blank lines (stop immediately — comments not attached)
- Collects contiguous `#` comment lines (optional whitespace + `#` + text)
- Stops at the first non-comment, non-blank line
- Returns the collected comments in source order (top to bottom)

This is ~30 LOC, has zero dependency on parser internals, and is guaranteed
correct because `#` has only one meaning in GraphQL (line comment start).

## Consequences

- No coupling to `apollo-parser` or `apollo-compiler` for trivia access.
- The scanner is reusable by other rules that need to inspect `#` comments
  (e.g., `#import` detection, `# eslint` suppression parsing).
- If a future `apollo-parser` exposes comments as addressable nodes, we can
  migrate the scanner to use that API without changing any rule code — the
  `preceding_comments` signature stays the same.
