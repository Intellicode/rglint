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

spec-010 extends `Node` with the minimum surface the selector matcher needs
(`name`, `description`, `value_raw`, and a `parent` back-pointer) without
claiming to be the final shape — spec-011 replaces it with the engine's
fully-typed CST view, and spec-012 adds `node_name` / typed utils on top.

### Selector engine (spec-010)

`rglint_core::selector` compiles an esquery-style selector string
(`ObjectTypeDefinition > FieldDefinition[name.value=/^_/]`) into a
`Matcher: Box<dyn Fn(&Node, Option<&Node>) -> bool + Send + Sync>` once per
rule; the engine (spec-011) applies it during the walk.

**Scope (v1).** Five features: kind matchers, attribute predicates
(`name` / `kind` / `description` / `value`), `>` (child) and whitespace
(descendant) combinators, and the `:matches` / `:not` pseudo-classes. PLAN §8
defers everything else (sibling / adjacent combinators `+` / `~`, other
pseudo-classes like `:has`, nested attribute access like
`[parent.name.value=…]`) until a rule needs them.

**Compound encoding.** The spec's `SelectorNode` enum has no `And` variant
— only `Matches` (OR) and `Not`. A compound like
`FieldDefinition[name.value=/^_/]` ("Kind **AND** Attribute") is encoded by
De Morgan as `Not([Not(Kind), Not(Attribute)])`; the matcher evaluates that
as "not (not-Kind OR not-Attribute)" == "Kind AND Attribute".

**`=` operator is overloaded by the RHS.** graphql-eslint spells regex match
as `[k=/pattern/]`, not `[k=~ /pattern/]`. The lexer emits a plain `Eq` token
for `=`; the parser inspects the RHS to pick the op — a regex literal means
`AttrOp::RegexMatch`, a string or bare identifier means `AttrOp::Eq`. The
explicit `=~` form is also accepted (and requires a regex RHS) for parity,
but no graphql-eslint rule uses it.

**Regex crate.** PLAN §8 flagged lookbehind as a risk. We default to the
`regex` crate (already a workspace dep); it does not support lookbehind, but
no graphql-eslint rule uses lookbehind in a selector regex. If a fixture
demands it, switch the single offending rule's regex to `fancy-regex`
rather than replacing the workspace-wide dependency.

### Content-hash cache (spec-013)

`rglint_core::cache::Cache` maps `(file_path, xxh3-64 content_hash) ->
CachedResult { diagnostics }` so unchanged files skip re-linting on
incremental runs. The store is either in-memory (no-op mode / tests) or a
single `<target-dir>/rglint-cache.bin` written atomically (`*.tmp` then
rename) with a `b"rglint"` magic header + version byte.

**v1 caches diagnostics only.** A cache hit still re-parses the file
(re-parsing is cheap relative to linting, and avoids serializing the parsed
`LoadedSchema`/`Siblings` graph). Revisit if spec-065 profiling shows parse
dominates.

**Encode format is JSON, not bincode.** `Diagnostic.data` is a
`serde_json::Value`, which drives the serde Deserializer via
`deserialize_any` — unsupported by bincode (and postcard). The cache file
therefore stores the entry map as a JSON array of `(key, value)` pairs after
the magic+version header (JSON object keys must be strings, so a struct
`CacheKey` cannot be a JSON map key — hence the array shape). This reuses the
existing `serde_json` dep with acceptable size overhead for the small, rare
cache file.

**Loading never fails.** A missing, corrupt, truncated, or version-mismatched
cache file is treated as an empty cache with a `tracing` warn — caching is a
perf optimization, never a correctness or hard-error path. Version mismatches
have no downgrade path in v1.

### Fixer source ownership and staged CLI wiring (spec-061)

`rglint_core::Fixer` is the machine-fix adapter over `LintEngine` and
`ProjectLintResult`. It validates suggestion ranges against the result's
retained `SourceFile`, groups edits by physical operation-document path, keeps
the lowest-offset non-overlapping edits, and applies accepted edits
rightmost-first. Fixes are reloaded into the project between passes so sibling
indexes and parsed executable documents reflect the new source. Schema source
files are deliberately excluded by requiring membership in
`Project.documents`; this is a source-ownership check rather than a rule
category check because some rules intentionally visit both schema and
executable CST kinds. A capped in-memory simulation provides dry-run diffs.

The CLI flags and reporter output remain owned by spec-062. This keeps the
core API usable by tests and future integrations without introducing a
second, provisional CLI/configuration path while the CLI spec is unimplemented.
