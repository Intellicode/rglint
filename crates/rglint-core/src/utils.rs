//! Shared misc helpers reused by two or more rules (spec-012). Mirrors the
//! grab-bag `utils.ts` from `graphql-eslint` — small, stateless, well-tested
//! utilities that don't belong to any one rule.
//!
//! Contents:
//! - [`DocumentKind`] / [`get_document_type`] — classify an executable node
//!   as `Operation` or `Fragment` (used by spec-020 `lone-executable-definition`
//!   and several siblings-aware rules).
//! - [`is_field_definition`] / [`is_object_type_definition`] — tiny node-kind
//!   predicates reused across the schema-only rule set. Grow this list
//!   incrementally as rules are ported (see spec-012 "Out of scope": case
//!   helpers live in `rglint-rules/shared/case.rs`, not here).
//! - [`array_default_options`] — the `ARRAY_DEFAULT_OPTIONS` normaliser that
//!   turns a rule option that may be a scalar *or* an array into a clean
//!   `Vec<T>`.
//! - [`strip_leading_slash`] — the path helper `match-document-filename`
//!   (spec-033) uses to normalize its `fileExtension` style options.

use serde::de::DeserializeOwned;
use serde_json::Value;

use crate::node::Node;
use apollo_parser::SyntaxKind;

/// Which kind of executable definition a node is.
///
/// Returned by [`get_document_type`] for `OperationDefinition` and
/// `FragmentDefinition` nodes; `None` for everything else (type-system
/// nodes, inner selections, …).
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum DocumentKind {
    /// `query` / `mutation` / `subscription` operation.
    Operation,
    /// A named fragment definition (`fragment X on T { ... }`).
    Fragment,
}

/// Classify an executable node as [`DocumentKind::Operation`] or
/// [`DocumentKind::Fragment`]. Returns `None` for any other node kind
/// (including anonymous operations — those are still `Operation`-typed nodes,
/// `node_name` is the helper that distinguishes anonymous from named).
pub fn get_document_type(node: &Node<'_>) -> Option<DocumentKind> {
    match node.kind {
        SyntaxKind::OPERATION_DEFINITION => Some(DocumentKind::Operation),
        SyntaxKind::FRAGMENT_DEFINITION => Some(DocumentKind::Fragment),
        _ => None,
    }
}

/// `true` iff `node` is a `FieldDefinition` (a field declared on an
/// object/interface/input type — *not* an executable `Field` selection).
pub fn is_field_definition(node: &Node<'_>) -> bool {
    matches!(node.kind, SyntaxKind::FIELD_DEFINITION)
}

/// `true` iff `node` is an `ObjectTypeDefinition` (or its extension).
pub fn is_object_type_definition(node: &Node<'_>) -> bool {
    matches!(
        node.kind,
        SyntaxKind::OBJECT_TYPE_DEFINITION | SyntaxKind::OBJECT_TYPE_EXTENSION
    )
}

/// Normalize a rule option that may be a scalar, an array, `null`, or absent
/// into a clean `Vec<T>`. Mirrors eslint's `ARRAY_DEFAULT_OPTIONS` helper.
///
/// Accepted shapes:
/// - `7` (scalar `T`) → `vec![7]`
/// - `[1, 2]` (array of `T`) → `vec![1, 2]`
/// - `null` / missing / `Value::Null` → `vec![]`
/// - type mismatch (e.g. `"x"` requested as `i64`) → `vec![]` (rules stay
///   resilient; the mismatch is dropped silently rather than panicking).
///
/// The element type `T` must be [`DeserializeOwned`]; the helper deserializes
/// each entry independently so one bad element fails the whole vec (matches
/// `graphql-eslint`, which validates the entire option before running the
/// rule). A config-validation step (spec-056) reports the actual mismatch to
/// the user; this helper's contract is "never panic".
pub fn array_default_options<T: DeserializeOwned>(v: &Value) -> Vec<T> {
    match v {
        Value::Null => Vec::new(),
        Value::Array(items) => {
            let mut out = Vec::with_capacity(items.len());
            for item in items {
                match serde_json::from_value::<T>(item.clone()) {
                    Ok(t) => out.push(t),
                    Err(_) => return Vec::new(),
                }
            }
            out
        }
        // A bare scalar: deserialize the value itself as `T`. Failure means a
        // type mismatch against the configured option — return empty.
        other => serde_json::from_value::<T>(other.clone())
            .map(|t| vec![t])
            .unwrap_or_default(),
    }
}

/// Strip a single leading `/` from `s`, if present. Used by
/// `match-document-filename` (spec-033) to normalize glob-style path options
/// the user may have written with a leading slash.
///
/// `"foo"` → `"foo"`; `"/foo"` → `"foo"`; `"foo/bar"` → `"foo/bar"`;
/// `"/"` → `""`.
pub fn strip_leading_slash(s: &str) -> &str {
    s.strip_prefix('/').unwrap_or(s)
}

#[cfg(test)]
mod tests {
    use super::*;
    use SyntaxKind::*;

    fn node(kind: SyntaxKind) -> Node<'static> {
        Node::new(kind)
    }

    #[test]
    fn get_document_type_classifies_operations_and_fragments() {
        assert_eq!(
            get_document_type(&node(OPERATION_DEFINITION)),
            Some(DocumentKind::Operation)
        );
        assert_eq!(
            get_document_type(&node(FRAGMENT_DEFINITION)),
            Some(DocumentKind::Fragment)
        );
    }

    #[test]
    fn get_document_type_returns_none_for_other_kinds() {
        for kind in [
            OBJECT_TYPE_DEFINITION,
            FIELD_DEFINITION,
            FIELD,
            SELECTION_SET,
            DOCUMENT,
        ] {
            assert_eq!(get_document_type(&node(kind)), None, "{kind:?} not a doc");
        }
    }

    #[test]
    fn is_field_definition_matches_only_field_definition() {
        assert!(is_field_definition(&node(FIELD_DEFINITION)));
        assert!(!is_field_definition(&node(FIELD)));
        assert!(!is_field_definition(&node(OBJECT_TYPE_DEFINITION)));
    }

    #[test]
    fn is_object_type_definition_matches_definition_and_extension() {
        assert!(is_object_type_definition(&node(OBJECT_TYPE_DEFINITION)));
        assert!(is_object_type_definition(&node(OBJECT_TYPE_EXTENSION)));
        assert!(!is_object_type_definition(&node(INTERFACE_TYPE_DEFINITION)));
        assert!(!is_object_type_definition(&node(FIELD_DEFINITION)));
    }

    #[test]
    fn array_default_options_scalar_becomes_single_element_vec() {
        let v = serde_json::json!(7);
        assert_eq!(array_default_options::<i64>(&v), vec![7]);
    }

    #[test]
    fn array_default_options_array_round_trips() {
        let v = serde_json::json!([1, 2, 3]);
        assert_eq!(array_default_options::<i64>(&v), vec![1, 2, 3]);
    }

    #[test]
    fn array_default_options_null_or_missing_is_empty() {
        assert!(array_default_options::<i64>(&serde_json::Value::Null).is_empty());
        // "missing" surfaces as `Null` once a rule reads its option via
        // serde_json's default; assert the same empty contract directly.
        assert!(array_default_options::<i64>(&serde_json::Value::Null).is_empty());
    }

    #[test]
    fn array_default_options_type_mismatch_is_empty() {
        // Requesting `i64` from a string scalar — type mismatch must not panic.
        let v = serde_json::json!("x");
        let got: Vec<i64> = array_default_options(&v);
        assert!(got.is_empty(), "type mismatch should yield []");
    }

    #[test]
    fn array_default_options_mismatched_element_empties_whole_vec() {
        // One bad element fails the whole array (rule stays resilient; the
        // user-facing mismatch report is spec-056's job).
        let v = serde_json::json!([1, "x", 3]);
        let got: Vec<i64> = array_default_options(&v);
        assert!(got.is_empty());
    }

    #[test]
    fn array_default_options_string_scalar() {
        let v = serde_json::json!("camelCase");
        assert_eq!(
            array_default_options::<String>(&v),
            vec!["camelCase".to_owned()]
        );
    }

    #[test]
    fn strip_leading_slash_removes_single_leading_slash() {
        assert_eq!(strip_leading_slash("foo"), "foo");
        assert_eq!(strip_leading_slash("/foo"), "foo");
        assert_eq!(strip_leading_slash("foo/bar"), "foo/bar");
        assert_eq!(strip_leading_slash("/foo/bar"), "foo/bar");
        assert_eq!(strip_leading_slash("/"), "");
        assert_eq!(strip_leading_slash(""), "");
    }

    #[test]
    fn document_kind_is_copy() {
        fn assert_copy<T: Copy>() {}
        assert_copy::<DocumentKind>();
    }
}
