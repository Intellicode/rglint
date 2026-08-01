//! `input-name` (spec-052).
//!
//! This follows the pinned graphql-eslint visitor: it checks argument names on
//! local `Mutation`/`Query` definitions and extensions, and optionally checks
//! the named argument type against the field-name convention. It deliberately
//! does not inspect the compiled schema or verify that the referenced type is
//! an input object.

use rglint_core::{DiagnosticBuilder, Fix, Handler, Node, RuleContext, Span, SyntaxKind};
use rglint_derive::Rule;
use serde::Deserialize;

fn default_case_sensitive() -> bool {
    true
}

fn default_check_mutations() -> bool {
    true
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Opts {
    #[serde(default)]
    check_input_type: bool,
    #[serde(default = "default_case_sensitive")]
    case_sensitive_input_type: bool,
    #[serde(default)]
    check_queries: bool,
    #[serde(default = "default_check_mutations")]
    check_mutations: bool,
}

impl Default for Opts {
    fn default() -> Self {
        Self {
            check_input_type: false,
            case_sensitive_input_type: default_case_sensitive(),
            check_queries: false,
            check_mutations: default_check_mutations(),
        }
    }
}

#[derive(Rule)]
#[rule(
    id = "input-name",
    category = "schema",
    has_suggestions = true,
    kinds = "NAME|NAMED_TYPE"
)]
pub struct InputName;

impl InputName {
    fn handler(&self, ctx: &mut RuleContext) -> Box<dyn Handler> {
        Box::new(InputNameHandler {
            opts: ctx.option().unwrap_or_default(),
            candidates: Vec::new(),
        })
    }
}

enum CandidateKind {
    ArgumentName,
    InputType,
}

struct Candidate {
    kind: CandidateKind,
    actual: String,
    root: String,
    field: String,
    span: Span,
}

struct InputNameHandler {
    opts: Opts,
    candidates: Vec<Candidate>,
}

impl Handler for InputNameHandler {
    fn on_node(&mut self, node: &Node<'_>, _parent: Option<&Node<'_>>) {
        match node.kind {
            SyntaxKind::NAME => self.collect_argument_name(node),
            SyntaxKind::NAMED_TYPE if self.opts.check_input_type => self.collect_input_type(node),
            _ => {}
        }
    }

    fn finalize(&mut self, ctx: &mut RuleContext) {
        let path = ctx.file.path().to_path_buf();
        let rule_id = ctx.rule_id();
        for candidate in &self.candidates {
            match candidate.kind {
                CandidateKind::ArgumentName => {
                    ctx.report(
                        DiagnosticBuilder::new(
                            rule_id,
                            path.clone(),
                            candidate.span,
                            format!(
                                "Input \"{}\" should be named \"input\" for \"{}.{}\"",
                                candidate.actual, candidate.root, candidate.field
                            ),
                        )
                        .suggestion(
                            "Rename to `input`",
                            Fix::Replace {
                                span: candidate.span,
                                text: "input".to_owned(),
                            },
                        ),
                    );
                }
                CandidateKind::InputType => {
                    let expected = format!("{}Input", candidate.field);
                    let matches = if self.opts.case_sensitive_input_type {
                        candidate.actual == expected
                    } else {
                        candidate.actual.eq_ignore_ascii_case(&expected)
                    };
                    if matches {
                        continue;
                    }
                    ctx.report(
                        DiagnosticBuilder::new(
                            rule_id,
                            path.clone(),
                            candidate.span,
                            format!(
                                "Input type `{}` name should be `{expected}`.",
                                candidate.actual
                            ),
                        )
                        .suggestion(
                            format!("Rename to `{expected}`"),
                            Fix::Replace {
                                span: candidate.span,
                                text: expected,
                            },
                        ),
                    );
                }
            }
        }
    }
}

impl InputNameHandler {
    fn collect_argument_name(&mut self, node: &Node<'_>) {
        let Some(argument) = node.parent else { return };
        if argument.kind != SyntaxKind::INPUT_VALUE_DEFINITION
            || argument.name.as_deref() == Some("input")
        {
            return;
        }
        let Some(field) = containing_field(argument) else {
            return;
        };
        let Some(root) = containing_root(field) else {
            return;
        };
        if !self.checks_root(root) {
            return;
        }
        let (Some(actual), Some(root_name), Some(field_name), Some(span)) = (
            argument.name.as_deref(),
            root.name.as_deref(),
            field.name.as_deref(),
            node.span,
        ) else {
            return;
        };
        self.candidates.push(Candidate {
            kind: CandidateKind::ArgumentName,
            actual: actual.to_owned(),
            root: root_name.to_owned(),
            field: field_name.to_owned(),
            span,
        });
    }

    fn collect_input_type(&mut self, node: &Node<'_>) {
        let Some(argument) = containing_argument(node) else {
            return;
        };
        let Some(field) = containing_field(argument) else {
            return;
        };
        let Some(root) = containing_root(field) else {
            return;
        };
        if !self.checks_root(root) {
            return;
        }
        let (Some(actual), Some(root_name), Some(field_name), Some(span)) = (
            node.name.as_deref(),
            root.name.as_deref(),
            field.name.as_deref(),
            node.span,
        ) else {
            return;
        };
        self.candidates.push(Candidate {
            kind: CandidateKind::InputType,
            actual: actual.to_owned(),
            root: root_name.to_owned(),
            field: field_name.to_owned(),
            span,
        });
    }

    fn checks_root(&self, root: &Node<'_>) -> bool {
        match root.name.as_deref() {
            Some("Mutation") => self.opts.check_mutations,
            Some("Query") => self.opts.check_queries,
            _ => false,
        }
    }
}

fn containing_argument<'a>(node: &'a Node<'a>) -> Option<&'a Node<'a>> {
    let mut current = node.parent?;
    loop {
        if current.kind == SyntaxKind::INPUT_VALUE_DEFINITION {
            return Some(current);
        }
        if matches!(
            current.kind,
            SyntaxKind::FIELD_DEFINITION
                | SyntaxKind::OBJECT_TYPE_DEFINITION
                | SyntaxKind::OBJECT_TYPE_EXTENSION
                | SyntaxKind::INPUT_OBJECT_TYPE_DEFINITION
                | SyntaxKind::INPUT_OBJECT_TYPE_EXTENSION
        ) {
            return None;
        }
        current = current.parent?;
    }
}

fn containing_field<'a>(argument: &'a Node<'a>) -> Option<&'a Node<'a>> {
    let mut current = argument.parent?;
    loop {
        if current.kind == SyntaxKind::FIELD_DEFINITION {
            return Some(current);
        }
        if matches!(
            current.kind,
            SyntaxKind::OBJECT_TYPE_DEFINITION
                | SyntaxKind::OBJECT_TYPE_EXTENSION
                | SyntaxKind::INPUT_OBJECT_TYPE_DEFINITION
                | SyntaxKind::INPUT_OBJECT_TYPE_EXTENSION
        ) {
            return None;
        }
        current = current.parent?;
    }
}

fn containing_root<'a>(field: &'a Node<'a>) -> Option<&'a Node<'a>> {
    let mut current = field.parent?;
    loop {
        if matches!(
            current.kind,
            SyntaxKind::OBJECT_TYPE_DEFINITION | SyntaxKind::OBJECT_TYPE_EXTENSION
        ) {
            return Some(current);
        }
        current = current.parent?;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rglint_core::{Category, Rule, Severity};

    #[test]
    fn rule_meta_matches_spec_052() {
        let rule = InputName;
        let meta = rule.meta();
        assert_eq!(meta.id, "input-name");
        assert_eq!(meta.category, Category::Schema);
        assert_eq!(meta.severity, Severity::Warn);
        assert!(!meta.requires_schema);
        assert!(!meta.requires_siblings);
        assert!(meta.has_suggestions);
    }

    #[test]
    fn defaults_match_upstream() {
        let opts = Opts::default();
        assert!(!opts.check_input_type);
        assert!(opts.case_sensitive_input_type);
        assert!(!opts.check_queries);
        assert!(opts.check_mutations);
    }
}
