# `lone-executable-definition`

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

### `rules-fixtures/lone-executable-definition/valid/01/01.graphql`

```graphql
        {
          id
        }
```

### `rules-fixtures/lone-executable-definition/valid/02/02.graphql`

```graphql
        query {
          id
        }
```

### `rules-fixtures/lone-executable-definition/valid/03/03.graphql`

```graphql
        query Foo {
          id
        }
```
