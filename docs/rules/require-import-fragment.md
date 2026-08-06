# `require-import-fragment`

| Property | Value |
| --- | --- |
| Category | `operations` |
| Default severity | `warn` |
| Requires schema | `false` |
| Requires siblings | `true` |
| Has suggestions | `true` |

## Description

_No description is provided for this rule._

## Options

This rule has no options.

## Examples

### `rules-fixtures/require-import-fragment/valid/01/01.fragment.graphql`

```graphql
fragment FooFields on User {
  id
}
```

### `rules-fixtures/require-import-fragment/valid/01/01.graphql`

```graphql
# import FooFields from './01.fragment.graphql'
query GetUser {
  user {
    ...FooFields
  }
}
```

### `rules-fixtures/require-import-fragment/valid/02/02.fragment.graphql`

```graphql
fragment FooFields on User {
  id
}
```
