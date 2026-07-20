# rglint Specs

Bitesized implementation specs derived from [../PLAN.md](../PLAN.md). Together,
these specs implement the **full** plan. Each spec is scoped to a coherent unit
of work (a few hours to ~2 days) and declares its dependencies on prior specs.

## Conventions

- Each spec lives at `spec-NNN.md` (zero-padded to 3 digits).
- Specs are numbered in approximate implementation order; later specs depend on
  earlier ones where stated.
- Every spec cites the `PLAN.md` sections it implements.
- "Deliverables" lists concrete files/artifacts to produce.
- "Testing" states how to verify the spec is complete.

## Status legend

- `[ ]` not started
- `[~]` in progress
- `[x]` done

## Index

### Foundation & Core (Phase 0)

| Spec | Title | Status |
|------|-------|--------|
| [spec-001](spec-001.md) | Project skeleton, Cargo workspace & cargo-deny | `[ ]` |
| [spec-002](spec-002.md) | SourceFile & Location/Span types | `[ ]` |
| [spec-003](spec-003.md) | Diagnostics model | `[ ]` |
| [spec-004](spec-004.md) | Schema loader & cache | `[ ]` |
| [spec-005](spec-005.md) | Document loader & dedup | `[ ]` |
| [spec-006](spec-006.md) | Sibling operations index (FragmentTracker) | `[ ]` |
| [spec-007](spec-007.md) | Project resolution (graphql-config) | `[ ]` |
| [spec-008](spec-008.md) | Rule trait, RuleMeta & registry | `[ ]` |
| [spec-009](spec-009.md) | RuleContext | `[ ]` |
| [spec-010](spec-010.md) | Selector engine | `[ ]` |
| [spec-011](spec-011.md) | LintEngine orchestration | `[ ]` |
| [spec-012](spec-012.md) | Shared utils & node_name helpers | `[ ]` |
| [spec-013](spec-013.md) | Content-hash cache | `[ ]` |
| [spec-014](spec-014.md) | Test harness (fixtures, snapshots, property tests) | `[ ]` |
| [spec-015](spec-015.md) | xtask port-fixture | `[ ]` |

### Phase 1 — Leaf rules (no shared deps)

| Spec | Title | Status |
|------|-------|--------|
| [spec-016](implemented/spec-016.md) | no-anonymous-operations | `[x]` |
| [spec-017](implemented/spec-017.md) | unique-fragment-name | `[x]` |
| [spec-018](implemented/spec-018.md) | unique-operation-name | `[x]` |
| [spec-019](implemented/spec-019.md) | no-duplicate-fields | `[x]` |
| [spec-020](implemented/spec-020.md) | lone-executable-definition | `[x]` |
| [spec-021](implemented/spec-021.md) | alphabetize | `[x]` |

### Phase 2 — Schema-only rules

| Spec | Title | Status |
|------|-------|--------|
| [spec-022](implemented/spec-022.md) | shared/case.rs (case styles & convertCase) | `[x]` |
| [spec-023](implemented/spec-023.md) | description-style | `[x]` |
| [spec-024](implemented/spec-024.md) | no-hashtag-description (CST trivia spike) | `[x]` |
| [spec-025](implemented/spec-025.md) | require-description | `[x]` |
| [spec-026](implemented/spec-026.md) | require-deprecation-reason | `[x]` |
| [spec-027](implemented/spec-027.md) | require-deprecation-date | `[x]` |
| [spec-028](implemented/spec-028.md) | naming-convention | `[x]` |
| [spec-029](implemented/spec-029.md) | unique-enum-value-names | `[x]` |
| [spec-030](implemented/spec-030.md) | strict-id-in-types | `[x]` |
| [spec-031](implemented/spec-031.md) | no-typename-prefix | `[x]` |
| [spec-032](implemented/spec-032.md) | no-root-type | `[x]` |
| [spec-033](implemented/spec-033.md) | match-document-filename | `[x]` |

### Phase 3 — Schema-aware operations

| Spec | Title | Status |
|------|-------|--------|
| [spec-034](implemented/spec-034.md) | no-deprecated | `[x]` |
| [spec-035](implemented/spec-035.md) | no-unused-fields | `[x]` |
| [spec-036](implemented/spec-036.md) | no-unreachable-types | `[x]` |
| [spec-037](spec-037.md) | no-scalar-result-type-on-mutation | `[ ]` |
| [spec-038](spec-038.md) | require-nullable-result-in-root | `[ ]` |
| [spec-039](spec-039.md) | require-field-of-type-query-in-mutation-result | `[ ]` |

### Phase 4 — Siblings + cross-document

| Spec | Title | Status |
|------|-------|--------|
| [spec-040](spec-040.md) | selection-set-depth | `[ ]` |
| [spec-041](spec-041.md) | require-import-fragment | `[ ]` |
| [spec-042](spec-042.md) | require-selections | `[ ]` |
| [spec-043](spec-043.md) | no-one-place-fragments | `[ ]` |

### Phase 5 — Relay suite

| Spec | Title | Status |
|------|-------|--------|
| [spec-044](spec-044.md) | shared/relay.rs predicates | `[ ]` |
| [spec-045](spec-045.md) | relay-arguments | `[ ]` |
| [spec-046](spec-046.md) | relay-connection-types | `[ ]` |
| [spec-047](spec-047.md) | relay-edge-types | `[ ]` |
| [spec-048](spec-048.md) | relay-page-info | `[ ]` |

### Phase 6 — oneOf + remaining

| Spec | Title | Status |
|------|-------|--------|
| [spec-049](spec-049.md) | shared/oneof.rs helpers | `[ ]` |
| [spec-050](spec-050.md) | require-nullable-fields-with-oneof | `[ ]` |
| [spec-051](spec-051.md) | require-type-pattern-with-oneof | `[ ]` |
| [spec-052](spec-052.md) | input-name | `[ ]` |

### Phase 7 — Spec rules

| Spec | Title | Status |
|------|-------|--------|
| [spec-053](spec-053.md) | rglint-graphql-spec bridge | `[ ]` |

### Phase 8 — Config + CLI

| Spec | Title | Status |
|------|-------|--------|
| [spec-054](spec-054.md) | Config loader (.rglintrc) | `[ ]` |
| [spec-055](spec-055.md) | GraphQL config (.graphqlrc) interop | `[ ]` |
| [spec-056](spec-056.md) | JSON-schema option validation | `[ ]` |
| [spec-057](spec-057.md) | Pretty reporter (miette) | `[ ]` |
| [spec-058](spec-058.md) | JSON reporter | `[ ]` |
| [spec-059](spec-059.md) | SARIF reporter | `[ ]` |
| [spec-060](spec-060.md) | GitHub annotations reporter | `[ ]` |
| [spec-061](spec-061.md) | --fix mode | `[ ]` |
| [spec-062](spec-062.md) | CLI (clap) entry point & exit codes | `[ ]` |
| [spec-063](spec-063.md) | Default recommended config preset | `[ ]` |

### Phase 9 — Performance + packaging

| Spec | Title | Status |
|------|-------|--------|
| [spec-064](spec-064.md) | Rayon parallelization | `[ ]` |
| [spec-065](spec-065.md) | Benchmarks (criterion) | `[ ]` |
| [spec-066](spec-066.md) | Release binary & cargo-binstall | `[ ]` |

### Cross-cutting — CI & tooling

| Spec | Title | Status |
|------|-------|--------|
| [spec-067](spec-067.md) | CI pipeline (GitHub Actions) | `[ ]` |
| [spec-068](spec-068.md) | xtask gen-docs | `[ ]` |
| [spec-069](spec-069.md) | xtask check-parity | `[ ]` |
| [spec-070](spec-070.md) | Coverage gate + cross-cutting invariant tests | `[ ]` |

### Phase 10 — Optional stretch

| Spec | Title | Status |
|------|-------|--------|
| [spec-071](spec-071.md) | napi-rs bridge (stretch) | `[ ]` |

## Plan coverage map

| PLAN.md section | Specs |
|-----------------|-------|
| §1 High-Level Architecture | 001, 004–011, 054, 062 |
| §2 Tooling & Crate Choices | 001 |
| §3 Directory Structure | 001 + each spec's Deliverables |
| §4.1 Rule trait | 008 |
| §4.2 RuleContext | 009 |
| §4.3 Selector engine | 010 |
| §4.4 Sibling operations index | 006 |
| §4.5 Diagnostics | 003 |
| §5 Phase 0 foundation | 001–015 |
| §5 Phase 1 leaf rules | 016–021 |
| §5 Phase 2 schema-only | 022–033 |
| §5 Phase 3 schema-aware ops | 034–039 |
| §5 Phase 4 siblings/cross-doc | 040–043 |
| §5 Phase 5 Relay | 044–048 |
| §5 Phase 6 oneOf | 049–052 |
| §5 Phase 7 spec rules | 053 |
| §5 Phase 8 config + CLI | 054–063 |
| §5 Phase 9 perf + packaging | 064–066 |
| §5 Phase 10 napi (optional) | 071 |
| §6 Testing strategy | 014, 015, 069, 070 + per-rule Testing sections |
| §7 Configuration Format | 054, 056, 063 |
| §8 Implementation Risks | addressed in relevant specs (024, 010, 028, 040, 002, 056, 053, 065) |
| §9 CI Pipeline | 067 |
| §10 Release & Distribution | 066 |
| §11 Stretch Goals | 071 (napi); others documented as future |
| §12 Decision summary | informational — no spec |
