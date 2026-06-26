# Spec-010: Selector engine

> Plan reference: §3 (`crates/rglint-core/src/selector/`), §4.3, §8 (selector complexity risk)

## Goal

Implement the esquery-like selector language graphql-eslint rules use to
declare which AST nodes they care about: `ObjectTypeDefinition >
FieldDefinition[name.value=/^_/]`, `:matches(...)`, `:not(...)`, descendant and
child combinators, attribute predicates. Compiled once per rule into a
predicate the engine applies during the walk.

## Scope

**In scope:**

- `selector/lexer.rs` — tokenize selector strings: identifiers (kind names),
  `[`, `]`, `=`, `=~`, `:matches`, `:not`, `>`, whitespace (descendant), string
  & regex literals.
- `selector/parser.rs` — build `SelectorNode` tree:
  `Kind(name)`, `Child(Box, Box)`, `Descendant(Box, Box)`, `Attribute { target,
  op, value }`, `Matches(vec)`, `Not(vec)`.
- `selector/ast.rs` — `SelectorNode` enum.
- `selector/matcher.rs` — `Matcher: Fn(&Node, Option<&Node>) -> bool` compiled
  from `SelectorNode`. Attribute kinds per §4.3: `NameValue`, `Kind`,
  `DescriptionValue`, `ValueRaw`.
- `selector/mod.rs` — `compile(&str) -> Result<Matcher>`.
- `selector/tests.rs` — unit tests.

**Out of scope:**

- Full esquery parity (PLAN §8: defer features no rule uses).
- Integrating selectors into `Handler` dispatch (that's the engine, spec-011;
  rules declare selectors in their `RuleMeta` and the engine compiles them).

## Dependencies

- spec-001 (workspace).
- spec-002 (Span — attribute regex matches against name text).

## Deliverables

- `crates/rglint-core/src/selector/{mod,lexer,parser,ast,matcher,tests}.rs`.
- Snapshot tests (insta) of the parsed AST for ~15 representative selectors
  pulled from `packages/plugin/src/rules/*/index.ts`.

## Interface / API

```rust
pub enum SelectorNode {
    Kind(String),
    Child(Box<SelectorNode>, Box<SelectorNode>),
    Descendant(Box<SelectorNode>, Box<SelectorNode>),
    Attribute { target: AttrKind, op: AttrOp, value: AttrValue },
    Matches(Vec<SelectorNode>),
    Not(Vec<SelectorNode>),
}

pub enum AttrKind { NameValue, Kind, DescriptionValue, ValueRaw }
pub enum AttrOp { Eq, RegexMatch }
pub enum AttrValue { Str(String), Regex(Regex) }

pub type Matcher = Box<dyn Fn(&Node<'_>, Option<&Node<'_>>) -> bool + Send + Sync>;

pub fn compile(src: &str) -> Result<Matcher, SelectorError>;
```

## Behavior

- `Child(A, B)` matches a node of kind B whose **direct** parent matches A.
- `Descendant(A, B)` matches a node of kind B with **some** ancestor matching A.
- `Attribute[target=NameValue, op=RegexMatch, value=/^_/]` matches a node whose
  name text matches the regex.
- `:matches(A, B, C)` matches if the node matches any.
- `:not(A)` matches if the node matches none of the inner selectors.
- Whitespace between selectors = descendant combinator; `>` = child.
- Compilation errors are returned with a span in the selector string (for
  config-error reporting).

## Testing

- Snapshot the parsed AST for:
  - `ObjectTypeDefinition > FieldDefinition`
  - `FieldDefinition[name.value=/^_/]`
  - `:matches(ObjectTypeDefinition, InterfaceTypeDefinition)`
  - `:not(FieldDefinition)`
  - `ObjectTypeDefinition > FieldDefinition[name.value=PageInfo]`
- Matcher tests against a real parsed schema fixture: assert exactly the
  expected node set is selected.
- Negative: malformed selector (`[name.value=`, unclosed `:not(`) →
  `SelectorError` with offset.

## Risks / Notes

- §8 risk: start with the 5 features above; do **not** implement sibling/adjacent
  combinators or pseudo-classes beyond `:matches`/`:not` until a rule needs
  them. Document unsupported features in `ARCHITECTURE.md`.
- Regex with lookbehind: graphql-eslint uses some — verify whether `regex`
  crate suffices or we need `fancy-regex`. Decision: default to `regex`; switch
  a single rule to `fancy-regex` only if a fixture demands lookbehind.
