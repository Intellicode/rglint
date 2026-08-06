# `relay-connection-types`

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

### `rules-fixtures/relay-connection-types/valid/01/01.graphql`

```graphql
type UserConnection {
  edges: [UserEdge]
  pageInfo: PageInfo!
}
```

### `rules-fixtures/relay-connection-types/valid/02/02.graphql`

```graphql
type UserConnection {
  edges: [UserEdge]
  pageInfo: PageInfo!
}
type PostConnection {
  edges: [PostEdge!]
  pageInfo: PageInfo!
}
type CommentConnection {
  edges: [CommentEdge]!
  pageInfo: PageInfo!
}
type AddressConnection {
  edges: [AddressEdge!]!
  pageInfo: PageInfo!
}
```

### `rules-fixtures/relay-connection-types/valid/03/03.graphql`

```graphql
type UserConnection {
  edges: [UserEdge]
  pageInfo: PageInfo!
}
```
