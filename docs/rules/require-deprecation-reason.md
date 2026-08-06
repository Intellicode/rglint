# `require-deprecation-reason`

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

### `rules-fixtures/require-deprecation-reason/valid/01/01.graphql`

```graphql
      query getUser {
        f
        a
        b
      }
```

### `rules-fixtures/require-deprecation-reason/valid/02/01.graphql`

```graphql
      type test {
        field1: String @authorized
        field2: Number
        field4: String @deprecated(reason: "Reason")
      }

      enum testEnum {
        item1 @authorized
        item2 @deprecated(reason: 0)
        item3
      }

      interface testInterface {
        field1: String @authorized
        field2: Number
        field3: String @deprecated(reason: 1.5)
      }
```
