# `no-duplicate-fields`

| Property | Value |
| --- | --- |
| Category | `schema` |
| Default severity | `warn` |
| Requires schema | `false` |
| Requires siblings | `false` |
| Has suggestions | `false` |

## Description

_No description is provided for this rule._

## Options

This rule has no options.

## Examples

### `rules-fixtures/no-duplicate-fields/valid/01/01.graphql`

```graphql
{ a }
```

### `rules-fixtures/no-duplicate-fields/valid/02/02.graphql`

```graphql
type Query { a: Int }
```
