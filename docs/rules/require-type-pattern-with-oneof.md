# `require-type-pattern-with-oneof`

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

### `rules-fixtures/require-type-pattern-with-oneof/valid/01/01.graphql`

```graphql
directive @oneOf on OBJECT

type T @oneOf {
  ok: Ok
  error: Error
}

type Ok
type Error
```

### `rules-fixtures/require-type-pattern-with-oneof/valid/02/02.graphql`

```graphql
directive @oneOf on OBJECT

type T {
  notok: Ok
  err: Error
}

type Ok
type Error
```

### `rules-fixtures/require-type-pattern-with-oneof/valid/03/03.graphql`

```graphql
directive @oneOf on OBJECT

input I {
  notok: Ok
  err: Error
}

type Ok
type Error
```
