# rglint-test-harness

The reusable test harness that drives rule **parity** against `graphql-eslint`
fixtures, plus `insta` snapshot scaffolding and `proptest` property-test
helpers. Spec-014 (PLAN §3, §6 Testing Strategy).

## What it does

- `fixture` — parse a `rules-fixtures/<rule-id>/{valid,invalid}/NN/` case
  directory (`*.graphql` + optional `*.config.toml` + optional
  `*.expected.json`) into an in-memory `FixtureCase`.
- `expected` — the `ExpectedError` parity record and `Comparator` that checks
  actual diagnostics against expected with the relaxed byte-offset rule
  (PLAN §6.3: compare line + column only, never raw byte offsets).
- `runner` — `run_fixture(case, engine)` lints the case and asserts parity,
  producing a `pretty_assertions`-rendered diff on mismatch. The
  `rglint_test_suite!("rule-id")` macro walks a rule's fixture tree and runs
  every case.
- `snapshot` — `assert_diagnostic_snapshot(diagnostics, source)` renders the
  source with `^^^` carets + messages into a stable `insta` `.snap` (the
  format the `pretty` reporter uses, spec-057).
- `property` — `prop_parse_roundtrip(src)` and `assert_no_panic(src, engine)`
  (PLAN §6.4 / §6.5).

## `loose_message` — message-verbatim parity opt-out

By default the `Comparator` checks each expected error's **message verbatim**
against the actual diagnostic's message — the strictest assertion, and the one
that makes porting a rule's wording faithful to `graphql-eslint`.

Some `graphql-js` spec rules (spec-053) phrase their messages differently from
`graphql-eslint`, so demanding verbatim parity would make porting those rules
impossible. A fixture opts out of the message check by setting
`loose_message = true` in its `config.toml`:

```toml
loose_message = true
```

Under `loose_message`, the comparator still checks **rule id + line + column**
for every expected error — only the `message` field is ignored. This keeps
location parity enforced (the property that catches real regressions like an
off-by-one span or a diagnostic attributed to the wrong node) while letting the
message wording drift where upstream phrasing differs.

`loose_message` is per-**case** (one fixture's `config.toml`), not per-rule, so
a rule's fixture suite can mix strict and loose cases: port the message
faithfully where `graphql-eslint` controls the wording, and flag the cases
where a `graphql-js` spec rule emits the message.

## Fixture layout

```text
rules-fixtures/<rule-id>/
  valid/
    01/
      case.graphql          # source under lint
      case.config.toml      # optional: schema / options / kind / loose_message
  invalid/
    01/
      case.graphql
      case.expected.json    # { "errors": [{ rule, message, line, column }] }
```

A case directory without `*.expected.json` is a **valid** case (the runner
asserts zero diagnostics). See `fixture` module docs for the full `config.toml`
shape and the suffix-based file discovery.