# Parity harness

`cargo run --locked -p xtask -- check-parity` compares every checked-in fixture
with the Rust engine. The default oracle is each fixture's `expected.json`,
which is the captured graphql-eslint result and keeps the check reproducible
offline.

For live upstream verification, pass an adapter with
`--ts-command <program>` or set `RGLINT_PARITY_TS_COMMAND`. The adapter is
invoked once per case and receives:

- `RGLINT_PARITY_RULE` — the rule id, or comma-separated rule ids for a valid
  graphql-js validation case;
- `RGLINT_PARITY_CASE` — the absolute fixture case directory; and
- `RGLINT_PARITY_SOURCE` — the absolute primary GraphQL source.

It must print ESLint JSON (`[{"filePath":"...","messages":[...]}]`) to
stdout. ESLint's one-based `column` is normalized to the repository's
zero-based convention. The adapter owns the graphql-eslint checkout and must
pin its upstream revision.

Generated `ts-output/`, `rust-output/`, and `diff.md` files are local run
artifacts and are ignored by Git. `known-divergences.json` is reviewed source
data; update it only with `--update-known` and a deliberate explanation.
