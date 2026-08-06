# `strict-id-in-types`

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

### `rules-fixtures/strict-id-in-types/valid/01/01.graphql`

```graphql
type A { id: ID! }
```

### `rules-fixtures/strict-id-in-types/valid/02/02.graphql`

```graphql
type A { _id: String! }
```

### `rules-fixtures/strict-id-in-types/valid/03/03.graphql`

```graphql
type A { _id: String! } type A1 { id: ID! }
```
