# `no-scalar-result-type-on-mutation`

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

### `rules-fixtures/no-scalar-result-type-on-mutation/valid/01/01.graphql`

```graphql
type Query {
  good: Boolean
}
```

### `rules-fixtures/no-scalar-result-type-on-mutation/valid/02/02.graphql`

```graphql
type User {
  id: ID!
}

type Mutation {
  createUser: User!
}
```

### `rules-fixtures/no-scalar-result-type-on-mutation/valid/03/03.graphql`

```graphql
type User {
  id: ID!
}

type RootMutation {
  createUser: User!
}

schema {
  mutation: RootMutation
}
```
