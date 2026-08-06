# `no-deprecated`

| Property | Value |
| --- | --- |
| Category | `operations` |
| Default severity | `warn` |
| Requires schema | `true` |
| Requires siblings | `false` |
| Has suggestions | `false` |

## Description

_No description is provided for this rule._

## Options

This rule has no options.

## Examples

### `rules-fixtures/no-deprecated/valid/01/graphql.graphql`

```graphql
{ newField }
```

### `rules-fixtures/no-deprecated/valid/02/graphql.graphql`

```graphql
mutation { something(t: NEW) }
```
