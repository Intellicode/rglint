# `selection-set-depth`

| Property | Value |
| --- | --- |
| Category | `operations` |
| Default severity | `warn` |
| Requires schema | `true` |
| Requires siblings | `true` |
| Has suggestions | `false` |

## Description

_No description is provided for this rule._

## Options

This rule has no options.

## Examples

### `rules-fixtures/selection-set-depth/valid/01/01.graphql`

```graphql
        {
          viewer { # Level 0
            albums { # Level 1
              title # Level 2
            }
          }
        }
```

### `rules-fixtures/selection-set-depth/valid/02/02.graphql`

```graphql
        query deep2 {
          viewer {
            albums {
              ...AlbumFields
            }
          }
        }
        fragment AlbumFields on Album { id }
```

### `rules-fixtures/selection-set-depth/valid/03/03.graphql`

```graphql
        query deep2 {
          viewer {
            albums {
              ...AlbumFields
            }
          }
        }
        fragment AlbumFields on Album { id }
```
