//! `no-duplicate-fields` (spec-019).
//!
//! Ports [`graphql-eslint`'s rule of the same id] to rglint. Within a single
//! object/interface/input type definition (or a selection set), reports every
//! field name occurrence after the first as a duplicate.
//!
//! [`graphql-eslint`'s rule of the same id`]: https://the-guild.dev/graphql/eslint/rules/no-duplicate-fields

use std::collections::HashMap;

use rglint_core::{DiagnosticBuilder, Handler, Node, RuleContext, Span, SyntaxKind};
use rglint_derive::Rule;

/// The `no-duplicate-fields` rule.
///
/// Subscribes to three CST kinds:
/// - `FIELD_DEFINITION` — fields inside object / interface type definitions.
/// - `INPUT_VALUE_DEFINITION` — fields inside input object type definitions.
/// - `FIELD` — field selections inside operation selection sets.
///
/// The handler walks the parent chain from each visited node to identify the
/// containing definition / selection-set. Duplicate field names within the same
/// container are reported; the first occurrence is canonical (not reported).
#[derive(Rule)]
#[rule(
    id = "no-duplicate-fields",
    category = "schema",
    kinds = "FIELD_DEFINITION|INPUT_VALUE_DEFINITION|FIELD"
)]
pub struct NoDuplicateFields;

impl NoDuplicateFields {
    fn handler(&self, _ctx: &mut RuleContext) -> Box<dyn Handler> {
        Box::new(NoDuplicateFieldsHandler {
            seen: HashMap::new(),
            duplicates: Vec::new(),
        })
    }
}

/// Per-document handler state.
///
/// `seen` maps each container's byte-offset to the set of field names (and
/// their first occurrence's span) already visited. `duplicates` buffers the
/// (field_name, is_schema, span) of each duplicate found during the walk for
/// emission in `finalize`, where `RuleContext` is available.
struct NoDuplicateFieldsHandler {
    /// container_offset → field_name → first_span
    seen: HashMap<usize, HashMap<String, Span>>,
    /// (field_name, is_schema, span_at_duplicate)
    duplicates: Vec<(String, bool, Span)>,
}

/// CST kinds that constitute a field-containing container for this rule.
fn is_container_kind(kind: SyntaxKind) -> bool {
    matches!(
        kind,
        SyntaxKind::OBJECT_TYPE_DEFINITION
            | SyntaxKind::OBJECT_TYPE_EXTENSION
            | SyntaxKind::INTERFACE_TYPE_DEFINITION
            | SyntaxKind::INTERFACE_TYPE_EXTENSION
            | SyntaxKind::INPUT_OBJECT_TYPE_DEFINITION
            | SyntaxKind::INPUT_OBJECT_TYPE_EXTENSION
            | SyntaxKind::SELECTION_SET
    )
}

impl NoDuplicateFieldsHandler {
    /// Walk the parent chain from `parent` upward until we find a node whose
    /// kind is a container kind. Returns `None` when the walk reaches a
    /// node with no parent (the CST root) without finding a container — this
    /// should never happen for well-formed GraphQL fields but is handled
    /// defensively.
    fn find_container<'a>(parent: &'a Node<'a>) -> Option<&'a Node<'a>> {
        let mut current = parent;
        loop {
            if is_container_kind(current.kind) {
                return Some(current);
            }
            current = current.parent?;
        }
    }
}

impl Handler for NoDuplicateFieldsHandler {
    fn on_node(&mut self, node: &Node<'_>, _parent: Option<&Node<'_>>) {
        // Skip nameless nodes (anonymous fields shouldn't exist in valid
        // GraphQL, but be defensive).
        let field_name = match &node.name {
            Some(n) => n.clone(),
            None => return,
        };

        // The engine always sets `_parent` for visited nodes, but the
        // parameter is `Option` — guard defensively.
        let parent = match _parent {
            Some(p) => p,
            None => return,
        };

        // Find the containing definition or selection-set.
        let container = match Self::find_container(parent) {
            Some(c) => c,
            None => return,
        };

        // Use the container's byte-offset as a unique key within this file.
        let container_key = container.span.unwrap_or(Span::new(0, 0)).offset;

        let span = node.span.unwrap_or(Span::new(0, 0));

        // Check if we've already seen this field name in this container.
        // Clippy suggests Entry::Vacant, but we need field_name in the
        // duplicate path below, and Entry consumes the key.
        let fields = self.seen.entry(container_key).or_default();
        #[allow(clippy::map_entry)]
        if !fields.contains_key(&field_name) {
            // First occurrence: record it.
            fields.insert(field_name, Span::new(span.offset, 0));
            return;
        }

        // Duplicate: buffer for emission in finalize.
        let is_schema = matches!(
            node.kind,
            SyntaxKind::FIELD_DEFINITION | SyntaxKind::INPUT_VALUE_DEFINITION
        );
        self.duplicates
            .push((field_name, is_schema, Span::new(span.offset, 0)));
    }

    fn finalize(&mut self, ctx: &mut RuleContext) {
        for (name, is_schema, span) in self.duplicates.drain(..) {
            let message = if is_schema {
                format!("Field \"{name}\" is defined multiple times")
            } else {
                format!("Field \"{name}\" is selected multiple times")
            };
            ctx.report(DiagnosticBuilder::new(
                ctx.rule_id(),
                ctx.source_code().path().to_path_buf(),
                span,
                message,
            ));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rglint_core::{Category, Rule, Severity};

    #[test]
    fn rule_meta_matches_spec_019() {
        let rule = NoDuplicateFields;
        let meta = rule.meta();
        assert_eq!(meta.id, "no-duplicate-fields");
        assert_eq!(meta.category, Category::Schema);
        assert_eq!(meta.severity, Severity::Warn);
        assert!(!meta.requires_schema);
        assert!(!meta.requires_siblings);
        assert!(!meta.has_suggestions);
    }

    #[test]
    fn rule_interested_in_field_definition_input_value_and_field() {
        let entry = rglint_core::ALL_RULES
            .iter()
            .find(|e| e.meta.id == "no-duplicate-fields")
            .expect("no-duplicate-fields must be registered via #[derive(Rule)]");
        assert_eq!(
            entry.interested_kinds,
            &[
                SyntaxKind::FIELD_DEFINITION,
                SyntaxKind::INPUT_VALUE_DEFINITION,
                SyntaxKind::FIELD,
            ],
            "expected the kinds attribute to populate interested_kinds"
        );
    }

    /// The handler correctly identifies FIELD_DEFINITION as a schema field and
    /// FIELD as an operation field, buffering duplicates with the right
    /// `is_schema` flag.
    #[test]
    fn handler_buffers_duplicates_with_correct_is_schema() {
        let mut handler = NoDuplicateFieldsHandler {
            seen: HashMap::new(),
            duplicates: Vec::new(),
        };

        // Build a tiny parent chain simulating `type T { a: Int a: Int }`.
        // The container is an OBJECT_TYPE_DEFINITION; its parent is None.
        let container = Node::new(SyntaxKind::OBJECT_TYPE_DEFINITION)
            .with_name("T")
            .with_span(Span::new(0, 20));

        // Simulate a FIELDS_DEFINITION wrapper (the CST intermediate node
        // between the type definition and its field definitions).
        let fields_def = Node::new(SyntaxKind::FIELDS_DEFINITION)
            .with_parent(&container)
            .with_span(Span::new(5, 10));

        // First field "a": should be tracked, not reported.
        let field1 = Node::new(SyntaxKind::FIELD_DEFINITION)
            .with_name("a")
            .with_parent(&fields_def)
            .with_span(Span::new(6, 3));
        handler.on_node(&field1, Some(&fields_def));
        assert!(
            handler.duplicates.is_empty(),
            "first occurrence is not a duplicate"
        );

        // Second field "a": should be buffered as a duplicate.
        let field2 = Node::new(SyntaxKind::FIELD_DEFINITION)
            .with_name("a")
            .with_parent(&fields_def)
            .with_span(Span::new(12, 3));
        handler.on_node(&field2, Some(&fields_def));
        assert_eq!(
            handler.duplicates.len(),
            1,
            "second occurrence is a duplicate"
        );
        assert_eq!(handler.duplicates[0].0, "a");
        assert!(handler.duplicates[0].1, "FIELD_DEFINITION is schema");
        assert_eq!(handler.duplicates[0].2, Span::new(12, 0));

        // Operation field "a" in a selection set at a different offset so
        // the container key doesn't collide with the schema container above.
        let sel_set = Node::new(SyntaxKind::SELECTION_SET).with_span(Span::new(100, 10));
        let selection = Node::new(SyntaxKind::SELECTION)
            .with_parent(&sel_set)
            .with_span(Span::new(101, 8));

        let op_field1 = Node::new(SyntaxKind::FIELD)
            .with_name("a")
            .with_parent(&selection)
            .with_span(Span::new(102, 1));
        handler.on_node(&op_field1, Some(&selection));
        assert_eq!(handler.duplicates.len(), 1, "no new duplicate yet");

        let op_field2 = Node::new(SyntaxKind::FIELD)
            .with_name("a")
            .with_parent(&selection)
            .with_span(Span::new(105, 1));
        handler.on_node(&op_field2, Some(&selection));
        assert_eq!(handler.duplicates.len(), 2, "operation duplicate buffered");
        assert_eq!(handler.duplicates[1].0, "a");
        assert!(!handler.duplicates[1].1, "FIELD is operation");
        assert_eq!(handler.duplicates[1].2, Span::new(105, 0));
    }
}
