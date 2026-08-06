# `match-document-filename`

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

### `rules-fixtures/match-document-filename/valid/01/test.gql`

```graphql
{ me }
```

### `rules-fixtures/match-document-filename/valid/02/user-by-id.query.gql`

```graphql
query USER_BY_ID { user { id } }
```

### `rules-fixtures/match-document-filename/valid/03/createUserQuery.gql`

```graphql
mutation CREATE_USER { user { id } }
```
