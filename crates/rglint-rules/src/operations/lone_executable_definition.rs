use apollo_parser::SyntaxKind;
use rglint_core::{DiagnosticBuilder, Handler, RuleContext, Span};
use rglint_derive::Rule;
use serde::Deserialize;

#[derive(Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Opts {
    #[serde(default)]
    ignore: Vec<String>,
}

#[derive(Rule)]
#[rule(id = "lone-executable-definition", category = "operations")]
pub struct LoneExecutableDefinition;

impl LoneExecutableDefinition {
    fn handler(&self, _ctx: &mut RuleContext) -> Box<dyn Handler> {
        Box::new(LoneExecutableHandler)
    }
}

struct LoneExecutableHandler;

#[derive(Clone)]
struct Def {
    kind: &'static str,
    name: Option<String>,
}

fn collect_defs(source: &str) -> Vec<Def> {
    use apollo_parser::cst::CstNode;

    let tree = apollo_parser::Parser::new(source).parse();
    let root = tree.document();
    let mut defs = Vec::new();

    for child in root.syntax().children() {
        let kind = child.kind();
        let kind_str = match kind {
            SyntaxKind::OPERATION_DEFINITION => {
                let op_type = operation_type_from_cst(&child);
                Def {
                    kind: op_type,
                    name: extract_name_from_cst(&child),
                }
            }
            SyntaxKind::FRAGMENT_DEFINITION => Def {
                kind: "Fragment",
                name: extract_name_from_cst(&child),
            },
            _ => continue,
        };
        defs.push(kind_str);
    }

    defs
}

fn operation_type_from_cst(syn: &apollo_parser::SyntaxNode) -> &'static str {
    for child in syn.children() {
        if child.kind() == SyntaxKind::OPERATION_TYPE {
            for token in child.children_with_tokens() {
                use apollo_parser::SyntaxElement;
                if let SyntaxElement::Token(t) = token {
                    // apollo-parser's SyntaxKind Debug output shows lowercase
                    // variant names; check by raw source text instead.
                    match t.text() {
                        "mutation" => return "Mutation",
                        "subscription" => return "Subscription",
                        _ => return "Query",
                    }
                }
            }
        }
    }
    "Query"
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

fn is_ignored(kind: &str, ignore: &[String]) -> bool {
    for entry in ignore {
        match entry.as_str() {
            "fragment" if kind == "Fragment" => return true,
            "operation" if kind != "Fragment" => return true,
            "query" if kind == "Query" => return true,
            "mutation" if kind == "Mutation" => return true,
            "subscription" if kind == "Subscription" => return true,
            _ => {}
        }
    }
    false
}

impl Handler for LoneExecutableHandler {
    fn finalize(&mut self, ctx: &mut RuleContext) {
        let opts: Opts = ctx.option().unwrap_or_default();
        let source = ctx.source_code().source();

        let defs: Vec<Def> = collect_defs(source)
            .into_iter()
            .filter(|d| !is_ignored(d.kind, &opts.ignore))
            .collect();

        if defs.len() <= 1 {
            return;
        }

        for def in &defs[1..] {
            let msg = if let Some(ref name) = def.name {
                format!("{} \"{}\" should be in a separate file.", def.kind, name)
            } else {
                format!("{} should be in a separate file.", def.kind)
            };
            ctx.report(DiagnosticBuilder::new(
                ctx.rule_id(),
                ctx.source_code().path().to_path_buf(),
                Span::new(0, 0),
                msg,
            ));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rglint_core::{Category, Rule, Severity};

    #[test]
    fn rule_meta_matches_spec_020() {
        let rule = LoneExecutableDefinition;
        let meta = rule.meta();
        assert_eq!(meta.id, "lone-executable-definition");
        assert_eq!(meta.category, Category::Operations);
        assert_eq!(meta.severity, Severity::Warn);
        assert!(!meta.requires_schema);
        assert!(!meta.requires_siblings);
        assert!(!meta.has_suggestions);
    }

    #[test]
    fn operation_type_parses_known_keywords() {
        use apollo_parser::cst::CstNode;
        use apollo_parser::SyntaxKind;
        let src = "mutation { f }";
        let tree = apollo_parser::Parser::new(src).parse();
        let root = tree.document();
        let op = root.syntax().children().next().unwrap();
        eprintln!("op kind: {:?}", op.kind());
        for child in op.children() {
            eprintln!("  child kind: {:?} text='{}'", child.kind(), child.to_string());
            for t in child.children_with_tokens() {
                use apollo_parser::SyntaxElement;
                if let SyntaxElement::Token(tok) = t {
                    eprintln!("    token kind: {:?} text='{}'", tok.kind(), tok.text());
                }
            }
        }
        assert_eq!(op.kind(), SyntaxKind::OPERATION_DEFINITION);
        assert_eq!(
            operation_type_from_cst(&op),
            "Mutation"
        );
    }

    #[test]
    fn is_ignored_filters_correctly() {
        let empty: Vec<String> = vec![];
        assert!(!is_ignored("Query", &empty));
        assert!(is_ignored("Fragment", &["fragment".into()]));
        assert!(is_ignored("Query", &["operation".into()]));
        assert!(is_ignored("Mutation", &["mutation".into()]));
        assert!(!is_ignored("Query", &["mutation".into()]));
        assert!(is_ignored("Fragment", &["fragment".into()]));
    }

    #[test]
    fn collect_defs_counts_operations_and_fragments() {
        let defs = collect_defs("{ id }\nfragment X on Y { z }");
        assert_eq!(defs.len(), 2);
        assert_eq!(defs[0].kind, "Query");
        assert!(defs[0].name.is_none());
        assert_eq!(defs[1].kind, "Fragment");
        assert_eq!(defs[1].name.as_deref(), Some("X"));
    }

    #[test]
    fn collect_defs_excludes_type_definitions() {
        let defs = collect_defs("type Query { x: Int }");
        assert_eq!(defs.len(), 0);
    }

    #[test]
    fn collect_defs_identifies_operation_types() {
        let defs = collect_defs("mutation { f }\nsubscription { f }");
        assert!(!defs.is_empty(), "should find definitions");
        for d in &defs {
            eprintln!("def: kind={:?} name={:?}", d.kind, d.name);
        }
        assert_eq!(defs.len(), 2);
        assert_eq!(defs[0].kind, "Mutation");
        assert_eq!(defs[1].kind, "Subscription");
    }

    #[test]
    fn collect_defs_handles_mixed_document() {
        let src = "        query Valid {\n          id\n        }\n        {\n          id\n        }\n        fragment Bar on Bar {\n          id\n        }\n        mutation ($name: String!) {\n          createFoo {\n            name\n          }\n        }\n        mutation Baz($name: String!) {\n          createFoo {\n            name\n          }\n        }\n        subscription {\n          id\n        }\n        subscription Sub {\n          id\n        }\n";
        let defs = collect_defs(src);
        for d in &defs {
            eprintln!("def: kind={:?} name={:?}", d.kind, d.name);
        }
        assert_eq!(defs.len(), 7);
        assert_eq!(defs[0].kind, "Query");
        assert!(defs[0].name.as_deref() == Some("Valid"));
        assert_eq!(defs[2].kind, "Fragment");
        assert_eq!(defs[3].kind, "Mutation");
        assert!(defs[3].name.is_none());
        assert_eq!(defs[4].kind, "Mutation");
        assert_eq!(defs[4].name.as_deref(), Some("Baz"));
        assert_eq!(defs[5].kind, "Subscription");
        assert_eq!(defs[6].kind, "Subscription");
    }
}
