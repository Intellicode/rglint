# `relay-arguments`

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

### `rules-fixtures/relay-arguments/valid/01/01.graphql`

```graphql
type User {
  posts(
    after: String!
    first: Int!
    before: Float
    last: Int
  ): PostConnection
}
type PostConnection { marker: String }
type Query { marker: String }
```

### `rules-fixtures/relay-arguments/valid/02/02.graphql`

```graphql
type User {
  posts(after: String!, first: Int!): PostConnection
  comments(before: Float, last: Int): PostConnection
}
type PostConnection { marker: String }
type Query { marker: String }
```

### `rules-fixtures/relay-arguments/valid/03/03.graphql`

```graphql
scalar Cursor
type User {
  posts(after: Cursor, first: Int, before: ID, last: Int): PostConnection
}
type PostConnection { marker: String }
type Query { marker: String }
```
