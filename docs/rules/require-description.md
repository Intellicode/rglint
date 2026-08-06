# `require-description`

| Property | Value |
| --- | --- |
| Category | `schema` |
| Default severity | `warn` |
| Requires schema | `false` |
| Requires siblings | `false` |
| Has suggestions | `false` |

## Description

_No description is provided for this rule._

## Options

This rule has no options.

## Examples

### `rules-fixtures/require-description/valid/01/01.graphql`

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

### `rules-fixtures/require-description/valid/02/01.graphql`

```graphql
        input SalaryDecimalOperatorsFilterUpdateOneUserInput {
          """
          gt
          """
          gt: BSONDecimal
          """
          in
          """
          in: [BSONDecimal]
          " nin "
          nin: [BSONDecimal]
        }
```

### `rules-fixtures/require-description/valid/03/01.graphql`

```graphql
        " Test "
        type CreateOneUserPayload {
          "Created document ID"
          recordId: MongoID

          "Created document"
          record: User
        }
```
