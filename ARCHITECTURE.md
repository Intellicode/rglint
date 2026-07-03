# Architecture

See [PLAN.md](PLAN.md) for the full architecture and design documentation.

## Decision records

### Rule registry: `linkme` distributed slice (spec-008)

`rglint_core::ALL_RULES` is a `linkme` distributed slice; every rule struct
annotated with `#[derive(Rule)]` submits a `RuleEntry` into it, and
`rglint_rules::all_rules()` returns the linker-populated slice.

spec-008's Risks/Notes floated an alternative ("start with an explicit
`all_rules()` array and add the derive later if maintenance pain emerges"). We
chose `linkme` upfront because the spec's *Testing* section requires a
meaningful negative test — "a rule **without** `#[derive(Rule)]` does not
appear" — which is only meaningful under auto-registration (an explicit array
makes the negative case vacuously true). `linkme` over `inventory` avoids
runtime init-order hazards and is WASM-friendly (no global ctor ordering).

`linkme` uses a linker section; the CI matrix (spec-067) must verify Linux /
macOS / Windows. Fallback if a platform breaks: revert `ALL_RULES` to an
explicit array literal in `rglint-rules/src/lib.rs`.

### RuleMeta `option_schema` laziness

`jsonschema::JSONSchema` (jsonschema 0.18 — the type the spec calls
`Validator` was renamed to `JSONSchema`) construction is non-`const`, so
`RuleMeta` stores the raw JSON-Schema source string and compiles it through a
`std::sync::OnceLock` on first access via [`RuleMeta::option_schema`]. The same
pattern applies to `default_options`. This keeps `RuleMeta` `const`-constructible
so the `Rule` derive can emit `static RULE_META_…: RuleMeta = RuleMeta::new(…);`.

### Forward placeholders (`RuleContext`, `Node`)

spec-008 lands minimal `RuleContext` (core/src/context.rs) and `Node`
(core/src/node.rs) placeholders so the `Rule` / `Handler` trait signatures
compile. spec-009 implements the real `RuleContext` body (`report`,
`require_schema`, `option::<T>`, `take_diagnostics`); spec-012 / spec-011 flesh
out `Node` into a typed view over the `apollo_compiler` CST.