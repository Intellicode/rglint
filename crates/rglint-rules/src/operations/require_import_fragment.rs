//! `require-import-fragment` (spec-041).
//!
//! Require cross-file fragment spreads to have a graphql-tools import comment.
//! The upstream graphql-eslint rule accepts both named imports such as
//! `# import FooFields from './fragments.graphql'` and default imports such as
//! `# import './fragments.graphql'`. Import paths are intentionally checked
//! only by normalized path identity; resolving glob patterns or validating
//! import syntax beyond the fragment name is outside this rule's scope.

use std::path::{Component, Path, PathBuf};

use rglint_core::{DiagnosticBuilder, Handler, Node, RuleContext, Span, SyntaxKind};
use rglint_derive::Rule;

use crate::shared::comment_scanner;

#[derive(Rule)]
#[rule(
    id = "require-import-fragment",
    category = "operations",
    requires_siblings = true,
    kinds = "FRAGMENT_NAME",
    has_suggestions = true
)]
pub struct RequireImportFragment;

impl RequireImportFragment {
    fn handler(&self, _ctx: &mut RuleContext) -> Box<dyn Handler> {
        Box::new(RequireImportFragmentHandler {
            spreads: Vec::new(),
        })
    }
}

struct RequireImportFragmentHandler {
    spreads: Vec<(String, Span)>,
}

impl Handler for RequireImportFragmentHandler {
    fn on_node(&mut self, node: &Node<'_>, parent: Option<&Node<'_>>) {
        if parent.is_some_and(|p| p.kind == SyntaxKind::FRAGMENT_SPREAD) {
            if let (Some(name), Some(span)) = (node.name.clone(), node.span) {
                self.spreads.push((name, span));
            }
        }
    }

    fn finalize(&mut self, ctx: &mut RuleContext) {
        let Some(siblings) = ctx.siblings else {
            return;
        };
        let source = ctx.source_code();
        let source_path = source.path().to_path_buf();
        let comments = comment_scanner::all_comments(source);

        for (fragment_name, span) in &self.spreads {
            let definitions: Vec<_> = siblings
                .fragments_all()
                .iter()
                .filter(|fragment| fragment.name == *fragment_name)
                .collect();

            if definitions
                .iter()
                .any(|fragment| fragment.source.path() == source_path)
            {
                continue;
            }

            let imported = comments.iter().any(|comment| {
                imported_fragment_path(&comment.text, fragment_name)
                    .map(|path| {
                        let imported_path = normalize_path(&source_path, Path::new(&path));
                        definitions.iter().any(|fragment| {
                            normalize_path(Path::new("."), fragment.source.path()) == imported_path
                        })
                    })
                    .unwrap_or(false)
            });

            if !imported {
                ctx.report(DiagnosticBuilder::new(
                    ctx.rule_id(),
                    source_path.clone(),
                    *span,
                    format!("Expected \"{fragment_name}\" fragment to be imported."),
                ));
            }
        }
    }
}

/// Extract an import path from a graphql-tools comment. The returned path is
/// accepted for either `import 'file'` or `import Name from 'file'`, with
/// optional whitespace and either quote character.
fn imported_fragment_path(comment: &str, fragment_name: &str) -> Option<String> {
    let value = comment.strip_prefix('#')?.trim_start();
    let value = value.strip_prefix("import")?;
    if value
        .chars()
        .next()
        .map(|character| !character.is_whitespace())
        .unwrap_or(false)
    {
        return None;
    }
    let value = value.trim_start();
    let value = if let Some(rest) = value.strip_prefix(fragment_name) {
        if rest
            .chars()
            .next()
            .map(|character| !character.is_whitespace())
            .unwrap_or(false)
        {
            return None;
        }
        let rest = rest.trim_start().strip_prefix("from")?;
        if rest
            .chars()
            .next()
            .map(|character| !character.is_whitespace())
            .unwrap_or(false)
        {
            return None;
        }
        rest.trim_start()
    } else {
        value
    };

    let quote = value.chars().next()?;
    if quote != '\'' && quote != '"' {
        return None;
    }
    let end = value[quote.len_utf8()..].find(quote)? + quote.len_utf8();
    Some(value[quote.len_utf8()..end].to_owned())
}

fn normalize_path(base_file: &Path, path: &Path) -> PathBuf {
    let joined = if path.is_absolute() {
        path.to_path_buf()
    } else {
        base_file
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join(path)
    };
    let mut normalized = PathBuf::new();
    for component in joined.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                normalized.pop();
            }
            other => normalized.push(other.as_os_str()),
        }
    }
    normalized
}

#[cfg(test)]
mod tests {
    use super::*;
    use rglint_core::{Category, Rule, Severity, SourceFile};
    use std::sync::Arc;

    #[test]
    fn rule_meta_matches_spec_041() {
        let meta = RequireImportFragment.meta();
        assert_eq!(meta.id, "require-import-fragment");
        assert_eq!(meta.category, Category::Operations);
        assert_eq!(meta.severity, Severity::Warn);
        assert!(!meta.requires_schema);
        assert!(meta.requires_siblings);
        assert!(meta.has_suggestions);
    }

    #[test]
    fn parses_named_and_default_imports() {
        assert_eq!(
            imported_fragment_path("# import FooFields from './fragments.graphql'", "FooFields"),
            Some("./fragments.graphql".to_owned())
        );
        assert_eq!(
            imported_fragment_path("#import \"fragments.graphql\"", "FooFields"),
            Some("fragments.graphql".to_owned())
        );
        assert_eq!(
            imported_fragment_path("# import Other from 'x.graphql'", "FooFields"),
            None
        );
    }

    #[test]
    fn scans_all_comments_including_imports() {
        let source = Arc::new(SourceFile::new(
            PathBuf::from("query.graphql"),
            "# import 'fragment.graphql'\nquery { x }\n".to_owned(),
        ));
        assert_eq!(comment_scanner::all_comments(&source).len(), 1);
    }
}
