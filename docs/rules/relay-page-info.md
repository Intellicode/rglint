# `relay-page-info`

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

| Option | Type | Default | Description |
| --- | --- | --- | --- |
| `pageInfoName` | `string` | `PageInfo` | — |

## Examples

### `rules-fixtures/relay-page-info/valid/01/01.graphql`

```graphql
        type PageInfo {
          hasPreviousPage: Boolean!
          hasNextPage: Boolean!
          startCursor: String
          endCursor: String
        }
```

### `rules-fixtures/relay-page-info/valid/02/02.graphql`

```graphql
        type PageInfo {
          hasPreviousPage: Boolean!
          hasNextPage: Boolean!
          startCursor: Int
          endCursor: Float
        }
```
