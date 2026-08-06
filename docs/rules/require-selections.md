# `require-selections`

| Property | Value |
| --- | --- |
| Category | `operations` |
| Default severity | `warn` |
| Requires schema | `true` |
| Requires siblings | `true` |
| Has suggestions | `true` |

## Description

_No description is provided for this rule._

## Options

| Option | Type | Default | Description |
| --- | --- | --- | --- |
| `fieldName` | `string or array of string` | `id` | — |
| `requireAllFields` | `boolean` | `—` | — |

## Examples

### `rules-fixtures/require-selections/valid/01/01.graphql`

```graphql
{ id }
```

### `rules-fixtures/require-selections/valid/02/01.graphql`

```graphql
{ noId { name } }
```

### `rules-fixtures/require-selections/valid/03/01.graphql`

```graphql
{ hasId { id name } }
```
