# `require-field-of-type-query-in-mutation-result`

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

### `rules-fixtures/require-field-of-type-query-in-mutation-result/valid/01/01.graphql`

```graphql
type Query {
  user: User
}

type User {
  id: ID!
}
```

### `rules-fixtures/require-field-of-type-query-in-mutation-result/valid/02/02.graphql`

```graphql
# type Query is not defined and no error is reported
type Mutation {
  createUser: User!
}

type User {
  id: ID!
}
```

### `rules-fixtures/require-field-of-type-query-in-mutation-result/valid/03/03.graphql`

```graphql
type Query
type CreateUserPayload {
  user: User!
  query: Query!
}

type Mutation {
  createUser: CreateUserPayload!
}

type User {
  id: ID!
}
```
