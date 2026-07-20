use apollo_parser::cst::CstNode;
use apollo_parser::SyntaxKind;
use rglint_core::{DiagnosticBuilder, Handler, RuleContext, Span};
use rglint_derive::Rule;
use serde::Deserialize;

use crate::shared::case::convert_case;
use crate::shared::case_styles::CaseStyle;

#[derive(Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PerTypeOpts {
    #[serde(default)]
    style: Option<String>,
    #[serde(default)]
    suffix: Option<String>,
    #[serde(default)]
    prefix: Option<String>,
}

#[derive(Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Opts {
    #[serde(default)]
    file_extension: Option<String>,
    #[serde(default)]
    query: Option<TypeOption>,
    #[serde(default)]
    mutation: Option<TypeOption>,
    #[serde(default)]
    subscription: Option<TypeOption>,
    #[serde(default)]
    fragment: Option<TypeOption>,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum TypeOption {
    String(String),
    Object(PerTypeOpts),
}

fn parse_style(s: &str) -> Option<CaseStyle> {
    match s {
        "camelCase" => Some(CaseStyle::Camel),
        "PascalCase" => Some(CaseStyle::Pascal),
        "snake_case" => Some(CaseStyle::Snake),
        "UPPER_CASE" => Some(CaseStyle::ScreamingSnake),
        "kebab-case" => Some(CaseStyle::Kebab),
        _ => None,
    }
}

fn get_style(opt: &PerTypeOpts) -> Option<&str> {
    opt.style.as_deref()
}

fn operation_kind_from_cst(syn: &apollo_parser::SyntaxNode) -> &'static str {
    for child in syn.children() {
        if child.kind() == SyntaxKind::OPERATION_TYPE {
            for token in child.children_with_tokens() {
                use apollo_parser::SyntaxElement;
                if let SyntaxElement::Token(t) = token {
                    match t.text() {
                        "mutation" => return "mutation",
                        "subscription" => return "subscription",
                        _ => return "query",
                    }
                }
            }
        }
    }
    "query"
}

#[derive(Rule)]
#[rule(id = "match-document-filename", category = "operations")]
pub struct MatchDocumentFilename;

impl MatchDocumentFilename {
    fn handler(&self, _ctx: &mut RuleContext) -> Box<dyn Handler> {
        Box::new(MatchDocumentFilenameHandler)
    }
}

struct MatchDocumentFilenameHandler;

impl Handler for MatchDocumentFilenameHandler {
    fn finalize(&mut self, ctx: &mut RuleContext) {
        let opts: Opts = ctx.option().unwrap_or_default();
        let source_path = ctx.source_code().path();
        let source = ctx.source_code().source();

        let file_ext = source_path
            .extension()
            .map(|e| format!(".{}", e.to_string_lossy()))
            .unwrap_or_default();
        let file_stem = source_path
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_default();

        let expected_ext = opts
            .file_extension
            .as_deref()
            .unwrap_or(&file_ext);

        if let Some(ref opt_ext) = opts.file_extension {
            if opt_ext.as_str() != file_ext {
                ctx.report(DiagnosticBuilder::new(
                    ctx.rule_id(),
                    source_path.to_path_buf(),
                    Span::new(0, 0),
                    format!(
                        "File extension \"{}\" don't match extension \"{}\"",
                        file_ext, opt_ext
                    ),
                ));
                return;
            }
        }

        let tree = apollo_parser::Parser::new(source).parse();
        let doc = tree.document();

        let mut first_op_name: Option<String> = None;
        let mut first_op_kind: Option<SyntaxKind> = None;
        let mut first_op_cst: Option<apollo_parser::SyntaxNode> = None;

        for child in doc.syntax().children() {
            let kind = child.kind();
            if kind == SyntaxKind::OPERATION_DEFINITION || kind == SyntaxKind::FRAGMENT_DEFINITION {
                let name = extract_name_from_cst(&child);
                if name.is_some() {
                    first_op_name = name;
                    first_op_kind = Some(kind);
                    first_op_cst = Some(child.clone());
                    break;
                }
            }
        }

        let Some(doc_name) = first_op_name else {
            return;
        };
        let Some(doc_kind) = first_op_kind else {
            return;
        };
        let Some(child_node) = first_op_cst else {
            return;
        };

        let type_key = if doc_kind == SyntaxKind::OPERATION_DEFINITION {
            operation_kind_from_cst(&child_node)
        } else {
            "fragment"
        };

        let type_opt = match type_key {
            "query" => opts.query.as_ref(),
            "mutation" => opts.mutation.as_ref(),
            "subscription" => opts.subscription.as_ref(),
            "fragment" => opts.fragment.as_ref(),
            _ => None,
        };

        let Some(option) = type_opt else {
            return;
        };

        let per_type = match option {
            TypeOption::String(style) => PerTypeOpts {
                style: Some(style.clone()),
                ..Default::default()
            },
            TypeOption::Object(obj) => PerTypeOpts {
                style: obj.style.clone(),
                suffix: obj.suffix.clone(),
                prefix: obj.prefix.clone(),
            },
        };

        let mut expected_filename = per_type.prefix.as_deref().unwrap_or("").to_owned();

        if let Some(style) = get_style(&per_type) {
            if style == "matchDocumentStyle" {
                expected_filename.push_str(&doc_name);
            } else if let Some(case_style) = parse_style(style) {
                expected_filename.push_str(&convert_case(&doc_name, case_style, &[]));
            }
        } else {
            expected_filename.push_str(&file_stem);
        }

        expected_filename.push_str(per_type.suffix.as_deref().unwrap_or(""));
        expected_filename.push_str(expected_ext);

        let actual_filename = format!("{}{}", file_stem, file_ext);

        if expected_filename != actual_filename {
            ctx.report(DiagnosticBuilder::new(
                ctx.rule_id(),
                source_path.to_path_buf(),
                Span::new(0, 0),
                format!(
                    "Unexpected filename \"{}\". Rename it to \"{}\"",
                    actual_filename, expected_filename
                ),
            ));
        }
    }
}

fn extract_name_from_cst(syn: &apollo_parser::SyntaxNode) -> Option<String> {
    for child in syn.children() {
        if matches!(child.kind(), SyntaxKind::NAME | SyntaxKind::FRAGMENT_NAME) {
            let text = child.to_string().trim().to_owned();
            if !text.is_empty() {
                return Some(text);
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use rglint_core::{Category, Rule, Severity};

    #[test]
    fn rule_meta_matches_spec_033() {
        let rule = MatchDocumentFilename;
        let meta = rule.meta();
        assert_eq!(meta.id, "match-document-filename");
        assert_eq!(meta.category, Category::Operations);
        assert_eq!(meta.severity, Severity::Warn);
        assert!(!meta.requires_schema);
        assert!(!meta.requires_siblings);
        assert!(!meta.has_suggestions);
    }
}
