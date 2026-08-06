# `description-style`

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

### `rules-fixtures/description-style/valid/01/01.graphql`

```graphql
  enum EnumUserLanguagesSkill {
    """
    basic
    """
    basic
    """
    fluent
    """
    fluent
    """
    native
    """
    native
  }
```

### `rules-fixtures/description-style/valid/02/02.graphql`

```graphql
  " Test "
  type CreateOneUserPayload {
    "Created document ID"
    recordId: MongoID

    "Created document"
    record: User
  }
```
