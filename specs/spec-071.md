# Spec-071: napi-rs bridge (stretch)

> Plan reference: §5 Phase 10, §11 (Stretch), §12 (Decision: "Phase 10 stretch")

## Goal

Optional `napi-rs` wrapper exposing rglint's engine to Node.js so the eslint
ecosystem can consume Rust rules. Mirrors apollo-rs's
`examples/validation-wasm-demo`. **Not blocking 1.0**; pursue only after
Phases 1-9 are at parity.

## Scope

**In scope (when pursued):**

- A new crate `crates/rglint-napi` using `napi-rs` to expose:
  - `lint({ schema, documents, rules }) -> LintResult[]` (JSON in/out).
  - `loadConfig(path) -> Config`.
- A `@rglint/napi` npm package (tri-platform: `linux-x64-gnu`,
  `darwin-arm64`, `win32-x64`).
- A `napi` build feature in `rglint-core` that excludes non-WASM/non-napi
  deps (e.g. miette's terminal rendering).
- JS-side smoke test calling `lint` on a fixture and asserting JSON matches
  spec-058's contract.

**Out of scope (v1):**

- An eslint plugin adapter (separate effort; this spec only exposes the
  engine).
- Browser WASM build (related but separate — note as future).

## Dependencies

- spec-011 (engine), spec-054 (config), spec-058 (JSON contract).
- `napi-rs`, `napi-derive` deps.

## Deliverables

- `crates/rglint-napi/` crate.
- `npm/` package scaffold + `@rglint/napi` publish workflow.
- `examples/napi-smoke.ts` (Node script).

## Interface / API

```ts
// @rglint/napi
export function lint(input: { schema?: string; documents: string[]; rules?: Record<string, [string, any]> }): LintResult[]
export interface LintResult { ruleId: string; message: string; line: number; column: number; filePath: string }
```

## Behavior

- Synchronous API first (engine is fast); async variant later if needed.
- Errors thrown as JS `Error` with the rglint error message.
- Schema/documents passed as strings (not file paths) for portability.

## Testing

- `examples/napi-smoke.ts` run via `node` in CI (when the napi crate is
  built); asserts JSON parity with the Rust CLI on the same input.

## Risks / Notes

- §12 decision: "Phase 10 stretch; do not block Phase 1-8 on it." Do not
  schedule until Phases 1-9 are green and a consumer demand is identified.
- napi build complexity (per-platform toolchains) is the main cost; reuse
  the release matrix from spec-066.
