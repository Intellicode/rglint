# `relay-edge-types`

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
| `listTypeCanWrapOnlyEdgeType` | `boolean` | `true` | — |
| `shouldImplementNode` | `boolean` | `true` | — |
| `withEdgeSuffix` | `boolean` | `true` | — |

## Examples

### `rules-fixtures/relay-edge-types/valid/01/01.graphql`

```graphql
        type AEdge {
          node: Int!
          cursor: String!
        }
        type AConnection {
          edges: [AEdge]
        }
```

### `rules-fixtures/relay-edge-types/valid/02/02.graphql`

```graphql
        scalar Email
        type AEdge {
          node: Email!
          cursor: Email!
        }
        type AConnection {
          edges: [AEdge]
        }
```

### `rules-fixtures/relay-edge-types/valid/03/03.graphql`

```graphql
        scalar Email
        type AEdge {
          node: Email!
          cursor: Email!
        }
        type AConnection {
          edges: [AEdge]
        }
```
