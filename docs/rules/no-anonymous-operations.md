# `no-anonymous-operations`

| Property | Value |
| --- | --- |
| Category | `operations` |
| Default severity | `warn` |
| Requires schema | `false` |
| Requires siblings | `false` |
| Has suggestions | `false` |

## Description

_No description is provided for this rule._

## Options

This rule has no options.

## Examples

### `rules-fixtures/no-anonymous-operations/valid/01/01.graphql`

```graphql
query myQuery { a }
```

### `rules-fixtures/no-anonymous-operations/valid/02/02.graphql`

```graphql
mutation doSomething { a }
```

### `rules-fixtures/no-anonymous-operations/valid/03/03.graphql`

```graphql
subscription myData { a }
```
