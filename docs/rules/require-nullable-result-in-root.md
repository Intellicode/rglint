# `require-nullable-result-in-root`

| Property | Value |
| --- | --- |
| Category | `schema` |
| Default severity | `warn` |
| Requires schema | `true` |
| Requires siblings | `false` |
| Has suggestions | `true` |

## Description

_No description is provided for this rule._

## Options

This rule has no options.

## Examples

### `rules-fixtures/require-nullable-result-in-root/valid/01/01.graphql`

```graphql
        type Query {
          foo: User
          baz: [User]!
          bar: [User!]!
        }
        type User {
          id: ID!
        }
```
