# `no-root-type`

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

### `rules-fixtures/no-root-type/valid/01/01.graphql`

```graphql
type User {
  id: ID!
}
```

### `rules-fixtures/no-root-type/valid/02/02.graphql`

```graphql
type Query {
  x: Int
}
```
