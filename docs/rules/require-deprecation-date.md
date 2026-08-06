# `require-deprecation-date`

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

### `rules-fixtures/require-deprecation-date/valid/01/01.graphql`

```graphql
type User { firstName: String }
```

### `rules-fixtures/require-deprecation-date/valid/02/01.graphql`

```graphql
scalar Old @deprecated(deletionDate: "01/01/2099")
```

### `rules-fixtures/require-deprecation-date/valid/03/01.graphql`

```graphql
scalar Old @deprecated(untilDate: "01/01/2099")
```
