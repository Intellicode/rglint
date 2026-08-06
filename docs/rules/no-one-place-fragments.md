# `no-one-place-fragments`

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

### `rules-fixtures/no-one-place-fragments/valid/01/01.graphql`

```graphql
fragment UserFields on User {
  id
}

query GetUser {
  user {
    ...UserFields
    friend {
      ...UserFields
    }
  }
}
```

### `rules-fixtures/no-one-place-fragments/valid/02/02.graphql`

```graphql
fragment UserFields on User {
  id
}
```

### `rules-fixtures/no-one-place-fragments/valid/02/02.query.graphql`

```graphql
query GetUser {
  user {
    ...UserFields
  }
}

query GetFriend {
  user {
    friend {
      ...UserFields
    }
  }
}
```
