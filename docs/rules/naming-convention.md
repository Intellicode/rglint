# `naming-convention`

| Property | Value |
| --- | --- |
| Category | `schema` |
| Default severity | `warn` |
| Requires schema | `false` |
| Requires siblings | `false` |
| Has suggestions | `true` |

## Description

_No description is provided for this rule._

## Options

This rule has no options.

## Examples

### `rules-fixtures/naming-convention/valid/01/01.graphql`

```graphql
        query GetUser($userId: ID!) {
          user(id: $userId) {
            id
            name
            isViewerFriend
            profilePicture(size: 50) {
              ...PictureFragment
            }
          }
        }

        fragment PictureFragment on Picture {
          uri
          width
          height
        }
```

### `rules-fixtures/naming-convention/valid/02/01.graphql`

```graphql
type B { test: String }
```

### `rules-fixtures/naming-convention/valid/03/01.graphql`

```graphql
type my_test_6_t { test: String }
```
