# `no-unused-fields`

| Property | Value |
| --- | --- |
| Category | `schema` |
| Default severity | `warn` |
| Requires schema | `true` |
| Requires siblings | `true` |
| Has suggestions | `false` |

## Description

_No description is provided for this rule._

## Options

This rule has no options.

## Examples

### `rules-fixtures/no-unused-fields/valid/01/01.graphql`

```graphql
  type User {
    id: ID!
    firstName: String
    lastName: String
    age: Int
    address: Address
  }

  type Address {
    country: String!
    zip: String!
    events: [Event!]!
  }

  enum EventName {
    CREATE
    UPDATE
    DELETE
  }

  type Event {
    by: User
    name: EventName
    data: String
  }

  type Query {
    user(id: ID!): User
  }

  type Mutation {
    createUser(firstName: String!): User
    deleteUser(id: ID!): User
  }
```
