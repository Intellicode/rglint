# `no-unreachable-types`

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

### `rules-fixtures/no-unreachable-types/valid/01/01.graphql`

```graphql
        scalar A
        scalar B

        # UnionTypeDefinition
        union Response = A | B

        type Query {
          foo: Response
        }
```

### `rules-fixtures/no-unreachable-types/valid/02/02.graphql`

```graphql
        type Query {
          me: User
        }

        # ObjectTypeDefinition
        type User {
          id: ID
          name: String
        }
```

### `rules-fixtures/no-unreachable-types/valid/03/03.graphql`

```graphql
        type Query {
          me: User
        }

        # InterfaceTypeDefinition
        interface Address {
          city: String
        }

        type User implements Address {
          city: String
        }
```
