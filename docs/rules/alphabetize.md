# `alphabetize`

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

### `rules-fixtures/alphabetize/valid/01/01.graphql`

```graphql
        type User {
          age: Int
          firstName: String!
          lastName: String!
          password: String
        }
```

### `rules-fixtures/alphabetize/valid/02/01.graphql`

```graphql
        input UserInput {
          age: Int
          firstName: String!
          lastName: String!
          password: String
          zip: String
        }
```

### `rules-fixtures/alphabetize/valid/03/01.graphql`

```graphql
        enum Role {
          ADMIN
          GOD
          SUPER_ADMIN
          USER
        }
```
