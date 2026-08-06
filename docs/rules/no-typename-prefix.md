# `no-typename-prefix`

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

### `rules-fixtures/no-typename-prefix/valid/01/01.graphql`

```graphql
      type User {
        id: ID!
      }
```

### `rules-fixtures/no-typename-prefix/valid/02/01.graphql`

```graphql
      interface Node {
        id: ID!
      }
```
