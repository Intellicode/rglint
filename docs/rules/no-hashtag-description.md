# `no-hashtag-description`

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

### `rules-fixtures/no-hashtag-description/valid/01/01.graphql`

```graphql
      " Good "
      type Query {
        foo: String
      }
```

### `rules-fixtures/no-hashtag-description/valid/02/01.graphql`

```graphql
      # Good

      type Query {
        foo: String
      }
      # Good
```

### `rules-fixtures/no-hashtag-description/valid/03/01.graphql`

```graphql
      #import t

      type Query {
        foo: String
      }
```
