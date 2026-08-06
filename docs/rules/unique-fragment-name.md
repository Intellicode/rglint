# `unique-fragment-name`

| Property | Value |
| --- | --- |
| Category | `operations` |
| Default severity | `warn` |
| Requires schema | `false` |
| Requires siblings | `true` |
| Has suggestions | `false` |

## Description

_No description is provided for this rule._

## Options

This rule has no options.

## Examples

### `rules-fixtures/unique-fragment-name/valid/01/01.graphql`

```graphql
fragment Test on U {
  a
  b
  c
}
```

### `rules-fixtures/unique-fragment-name/valid/01/01.sibling.graphql`

```graphql
fragment HasIdFields on HasId {
  id
}
```
