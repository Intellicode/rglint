# `input-name`

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

### `rules-fixtures/input-name/valid/01/01.graphql`

```graphql
type Mutation { SetMessage(input: SetMessageInput): String }
```

### `rules-fixtures/input-name/valid/02/02.graphql`

```graphql
type Mutation { CreateMessage(input: CreateMessageInput): String DeleteMessage(input: DeleteMessageInput): Boolean }
```

### `rules-fixtures/input-name/valid/03/03.graphql`

```graphql
type Mutation { CreateMessage(input: CreateMessageInput!): String }
```
