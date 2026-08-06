# `require-nullable-fields-with-oneof`

| Property | Value |
| --- | --- |
| Category | `schema` |
| Default severity | `warn` |
| Requires schema | `true` |
| Requires siblings | `false` |
| Has suggestions | `false` |

## Description

_No description is provided for this rule._

## Options

This rule has no options.

## Examples

### `rules-fixtures/require-nullable-fields-with-oneof/valid/01/01.graphql`

```graphql
directive @oneOf on INPUT_OBJECT | OBJECT

input Input @oneOf {
  foo: [String]
  bar: Int
}
```

### `rules-fixtures/require-nullable-fields-with-oneof/valid/02/02.graphql`

```graphql
directive @oneOf on INPUT_OBJECT | OBJECT

type User @oneOf {
  foo: String
  bar: [Int!]
}
```
