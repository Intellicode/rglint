# graphql-js validation message divergences

Parity is pinned to `graphql-hive/graphql-eslint` source
`packages/plugin/src/rules/graphql-js-validation.ts` at commit
`241936acfebef3e6201703e483776d3f952a6f0f`.

The Rust bridge uses Apollo Compiler 1.32 diagnostics. The fixture harness
therefore sets `loose_message = true` and compares rule id plus location. The
following cases intentionally use Apollo's wording rather than treating the
wording difference as a port failure:

| Fixture | Rule | Apollo wording | graphql-eslint wording |
| --- | --- | --- | --- |
| 01 | `fields-on-correct-type` | `type \`User\` does not have a field \`missing\`` | `Cannot query field "missing" on type "User".` |
| 02 | `known-argument-names` | `the argument \`unknown\` is not supported by \`Query.user\`` | `Unknown argument "unknown" on field "Query.user".` |
| 03 | `no-unused-variables` | `unused variable: \`$unused\`` | `Variable "$unused" is never used.` |
| 04 | `lone-anonymous-operation` | `anonymous operation cannot be selected when the document contains other operations` | `This anonymous operation must be the only defined operation.` |
| 05 | `no-fragment-cycles` | `` `A` fragment cannot reference itself `` | `Cannot spread fragment "A" within itself.` |
| 06 | `scalar-leafs` | `interface, union and object types must have a subselection set` | `Field "user" of type "User" must have a selection of subfields.` |
| 07 | `variables-in-allowed-position` | `variable \`$id\` of type \`String\` cannot be used for argument \`id\` of type \`ID!\`` | `Variable "$id" of type "String" used in position expecting type "ID!".` |
| 08 | `value-literals-of-correct-type` | `expected value of type Int!, found a string` | `Int cannot represent non-integer value: "wrong"` |
| 09 | `known-fragment-names` | `cannot find fragment \`Missing\` in this document` | `Unknown fragment "Missing".` |
| 10 | `possible-fragment-spread` | `fragment \`AdminFields\` with type condition \`Admin\` cannot be applied to \`User\`` | `Fragment "AdminFields" cannot be spread here as objects of type "User" can never be of type "Admin".` |
| 11 | `provided-required-arguments` | `the required argument \`Query.user(id:)\` is not provided` | `Field "user" argument "id" of type "ID!" is required, but it was not provided.` |
| 12 | `unique-variable-names` | `the variable \`$id\` is declared multiple times` | `There can be only one variable named "$id".` |

Apollo 1.32 does not expose stable diagnostic names for the SDL-builder
directions behind `lone-schema-definition`, `possible-type-extension`,
`unique-directive-names`, `unique-field-definition-names`,
`unique-operation-types`, and `unique-type-names`. Those rule ids are still
registered for configuration compatibility; they are documented as
deliberately inactive until Apollo exposes structured codes or the bridge
gains a non-message-based adapter.
