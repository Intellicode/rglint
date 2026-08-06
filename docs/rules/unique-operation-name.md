# `unique-operation-name`

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

### `rules-fixtures/unique-operation-name/valid/01/01.graphql`

```graphql
query Foo {
  a
  b
  c
}
```

### `rules-fixtures/unique-operation-name/valid/01/01.sibling.graphql`

```graphql
query Bar {
  x
  y
  z
}
```
