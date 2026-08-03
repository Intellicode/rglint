//! Iterative application of machine-applicable operation-document fixes.
//!
//! `Fixer` is deliberately an engine adapter, not a second lint engine. It
//! consumes the engine's diagnostics and retained source index, validates
//! every byte range, applies deterministic non-overlapping edits, and asks
//! the engine to lint the reloaded project again.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::{Path, PathBuf};

use crate::diagnostics::Fix;
use crate::engine::{LintEngine, LintEngineError, ProjectLintResult};
use crate::project::Project;

/// The default number of fix passes. This bounds rules whose fixes recreate
/// their own diagnostics.
pub const DEFAULT_MAX_PASSES: usize = 10;

/// Errors raised while validating, writing, or reloading fixes.
#[derive(Debug, thiserror::Error)]
pub enum FixError {
    /// Linting failed while collecting a pass.
    #[error(transparent)]
    Lint(#[from] LintEngineError),
    /// Reloading the operation documents failed.
    #[error(transparent)]
    Documents(#[from] crate::documents::DocumentLoadError),
    /// A source path is not a writable filesystem file.
    #[error("cannot apply a fix to non-file source `{path}`")]
    NonFileSource { path: PathBuf },
    /// A diagnostic referred to a source that was not retained by the engine.
    #[error("cannot apply a fix: source `{path}` is not available in the lint result")]
    MissingSource { path: PathBuf },
    /// A fix's byte range is outside its source or splits UTF-8.
    #[error("invalid fix range {start}..{end} for `{path}` (source length {source_len})")]
    InvalidRange {
        path: PathBuf,
        start: usize,
        end: usize,
        source_len: usize,
    },
    /// A write failed after a pass was prepared.
    #[error("failed to write fixed source `{path}`: {source}")]
    Write {
        path: PathBuf,
        source: std::io::Error,
    },
}

/// Result of iterative fix mode.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct FixSummary {
    /// Number of lint/apply passes attempted.
    pub passes: usize,
    /// Number of distinct files whose contents changed.
    pub files_changed: usize,
    /// Diagnostics remaining after the final pass.
    pub remaining: usize,
}

/// A deterministic unified diff for one source file.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FileDiff {
    /// The source path represented by this diff.
    pub path: PathBuf,
    /// Unified diff text, including `---`, `+++`, and one hunk.
    pub unified_diff: String,
}

/// Applies suggestions produced by an existing [`LintEngine`].
#[derive(Debug)]
pub struct Fixer<'e> {
    engine: &'e LintEngine,
    max_passes: usize,
}

impl<'e> Fixer<'e> {
    /// Construct a fixer with [`DEFAULT_MAX_PASSES`].
    pub fn new(engine: &'e LintEngine) -> Self {
        Self {
            engine,
            max_passes: DEFAULT_MAX_PASSES,
        }
    }

    /// Set the maximum number of passes. A value of zero performs no writes
    /// and reports the current diagnostic count.
    pub fn with_max_passes(mut self, max_passes: usize) -> Self {
        self.max_passes = max_passes;
        self
    }

    /// Lint, apply one deterministic pass at a time, write changed operation
    /// files, reload them, and stop when no applicable fixes remain or the
    /// iteration cap is reached.
    pub fn fix(&self, project: &mut Project) -> Result<FixSummary, FixError> {
        let mut changed_files = HashSet::new();
        let mut passes = 0;

        if self.max_passes == 0 {
            return Ok(FixSummary {
                passes,
                files_changed: 0,
                remaining: self.engine.lint(project)?.all.len(),
            });
        }

        while passes < self.max_passes {
            let result = self.engine.lint(project)?;
            let edits = collect_edits(self.engine, project, &result)?;
            if edits.is_empty() {
                return Ok(FixSummary {
                    passes,
                    files_changed: changed_files.len(),
                    remaining: result.all.len(),
                });
            }
            passes += 1;

            let mut replacements = HashMap::new();
            for (path, file_edits) in edits {
                let old = result
                    .sources
                    .get(&path)
                    .ok_or_else(|| FixError::MissingSource { path: path.clone() })?;
                if !path.is_file() {
                    return Err(FixError::NonFileSource { path });
                }
                let updated = apply_edits(old.source(), &file_edits);
                if updated != old.source() {
                    std::fs::write(&path, &updated).map_err(|source| FixError::Write {
                        path: path.clone(),
                        source,
                    })?;
                    changed_files.insert(path.clone());
                }
                replacements.insert(path, updated);
            }

            // Reload even when a malicious fix is a no-op. Such a rule still
            // re-emits its diagnostic and is correctly bounded by max_passes.
            project.reload_documents(&replacements)?;
        }

        let result = self.engine.lint(project)?;
        warn_about_fix_loop(self.engine, &result);
        Ok(FixSummary {
            passes,
            files_changed: changed_files.len(),
            remaining: result.all.len(),
        })
    }

    /// Simulate successive fix passes without writing files. The returned
    /// diffs compare the original project sources with the final in-memory
    /// state after the same cap and conflict rules used by [`Self::fix`].
    pub fn dry_run(&self, project: &Project) -> Result<Vec<FileDiff>, FixError> {
        let mut working = project.reloaded_documents(&HashMap::new())?;
        let mut original = HashMap::new();
        for doc in &working.documents.docs {
            original.insert(
                doc.source.path().to_path_buf(),
                doc.source.source().to_owned(),
            );
        }

        for _ in 0..self.max_passes {
            let result = self.engine.lint(&working)?;
            let edits = collect_edits(self.engine, &working, &result)?;
            if edits.is_empty() {
                break;
            }
            let mut replacements = HashMap::new();
            for (path, file_edits) in edits {
                let source = result
                    .sources
                    .get(&path)
                    .ok_or_else(|| FixError::MissingSource { path: path.clone() })?;
                replacements.insert(path, apply_edits(source.source(), &file_edits));
            }
            working = working.reloaded_documents(&replacements)?;
        }

        let mut diffs = Vec::new();
        for doc in &working.documents.docs {
            let path = doc.source.path().to_path_buf();
            let Some(old) = original.get(&path) else {
                continue;
            };
            if old != doc.source.source() {
                diffs.push(FileDiff {
                    unified_diff: unified_diff(&path, old, doc.source.source()),
                    path,
                });
            }
        }
        diffs.sort_by(|left, right| left.path.cmp(&right.path));
        Ok(diffs)
    }
}

#[derive(Clone, Debug)]
struct Edit {
    start: usize,
    end: usize,
    text: String,
    rule_id: String,
}

fn collect_edits(
    engine: &LintEngine,
    project: &Project,
    result: &ProjectLintResult,
) -> Result<BTreeMap<PathBuf, Vec<Edit>>, FixError> {
    let mut candidates: BTreeMap<PathBuf, Vec<Edit>> = BTreeMap::new();
    for diagnostic in &result.all {
        let Some(rule) = engine
            .enabled_rules()
            .iter()
            .find(|rule| rule.entry.meta.id == diagnostic.rule_id)
        else {
            continue;
        };
        // The source, rather than the rule category, is the eligibility
        // boundary: mixed rules such as alphabetize can safely fix executable
        // documents while schema sources remain out of scope for v1.
        if !rule.entry.meta.has_suggestions
            || !project.documents.by_file.contains_key(&diagnostic.file)
        {
            continue;
        }
        let Some(source) = result.sources.get(&diagnostic.file) else {
            return Err(FixError::MissingSource {
                path: diagnostic.file.clone(),
            });
        };
        for suggestion in &diagnostic.suggestions {
            let (start, end, text) = fix_parts(&suggestion.fix, &diagnostic.file, source.source())?;
            candidates
                .entry(diagnostic.file.clone())
                .or_default()
                .push(Edit {
                    start,
                    end,
                    text,
                    rule_id: diagnostic.rule_id.clone(),
                });
        }
    }

    for edits in candidates.values_mut() {
        edits.sort_by_key(|edit| (edit.start, edit.end, edit.rule_id.clone()));
        let mut accepted = Vec::with_capacity(edits.len());
        for edit in edits.drain(..) {
            if accepted.iter().any(|other: &Edit| overlaps(other, &edit)) {
                continue;
            }
            accepted.push(edit);
        }
        *edits = accepted;
    }
    candidates.retain(|_, edits| !edits.is_empty());
    Ok(candidates)
}

fn fix_parts(fix: &Fix, path: &Path, source: &str) -> Result<(usize, usize, String), FixError> {
    let (start, end, text) = match fix {
        Fix::Replace { span, text } => {
            (span.offset, span.offset.checked_add(span.len), text.clone())
        }
        Fix::Remove { span } => (
            span.offset,
            span.offset.checked_add(span.len),
            String::new(),
        ),
        Fix::Insert { offset, text } => (*offset, Some(*offset), text.clone()),
    };
    let Some(end) = end else {
        return Err(FixError::InvalidRange {
            path: path.to_path_buf(),
            start,
            end: usize::MAX,
            source_len: source.len(),
        });
    };
    if start > end
        || end > source.len()
        || !source.is_char_boundary(start)
        || !source.is_char_boundary(end)
    {
        return Err(FixError::InvalidRange {
            path: path.to_path_buf(),
            start,
            end,
            source_len: source.len(),
        });
    }
    Ok((start, end, text))
}

fn overlaps(left: &Edit, right: &Edit) -> bool {
    if left.start == left.end && right.start == right.end {
        left.start == right.start
    } else {
        left.start < right.end && right.start < left.end
    }
}

fn apply_edits(source: &str, edits: &[Edit]) -> String {
    let mut output = source.to_owned();
    let mut rightmost = edits.to_vec();
    rightmost.sort_by_key(|edit| std::cmp::Reverse((edit.start, edit.end)));
    for edit in rightmost {
        output.replace_range(edit.start..edit.end, &edit.text);
    }
    output
}

fn warn_about_fix_loop(engine: &LintEngine, result: &ProjectLintResult) {
    for diagnostic in &result.all {
        if diagnostic.suggestions.iter().any(|_| {
            engine.enabled_rules().iter().any(|rule| {
                rule.entry.meta.id == diagnostic.rule_id && rule.entry.meta.has_suggestions
            })
        }) {
            tracing::warn!(
                rule = %diagnostic.rule_id,
                file = %diagnostic.file.display(),
                "fix iteration cap reached while diagnostics remain"
            );
        }
    }
}

fn unified_diff(path: &Path, old: &str, new: &str) -> String {
    let old_lines = split_lines(old);
    let new_lines = split_lines(new);
    let first = old_lines
        .iter()
        .zip(&new_lines)
        .position(|(left, right)| left != right)
        .unwrap_or(old_lines.len().min(new_lines.len()));
    let common_suffix = old_lines[first..]
        .iter()
        .rev()
        .zip(new_lines[first..].iter().rev())
        .take_while(|(left, right)| left == right)
        .count();
    let old_end = old_lines.len().saturating_sub(common_suffix);
    let new_end = new_lines.len().saturating_sub(common_suffix);
    let context_start = first.saturating_sub(3);
    let old_context_end = (old_end + 3).min(old_lines.len());
    let new_context_end = (new_end + 3).min(new_lines.len());

    let mut out = format!("--- {}\n+++ {}\n", path.display(), path.display());
    out.push_str(&format!(
        "@@ -{},{} +{},{} @@\n",
        context_start + 1,
        old_context_end.saturating_sub(context_start),
        context_start + 1,
        new_context_end.saturating_sub(context_start),
    ));
    for line in &old_lines[context_start..first] {
        push_diff_line(&mut out, ' ', line);
    }
    for line in &old_lines[first..old_end] {
        push_diff_line(&mut out, '-', line);
    }
    for line in &new_lines[first..new_end] {
        push_diff_line(&mut out, '+', line);
    }
    for line in &new_lines[new_end..new_context_end] {
        push_diff_line(&mut out, ' ', line);
    }
    out
}

fn split_lines(source: &str) -> Vec<&str> {
    if source.is_empty() {
        Vec::new()
    } else {
        source.split_inclusive('\n').collect()
    }
}

fn push_diff_line(out: &mut String, prefix: char, line: &str) {
    out.push(prefix);
    out.push_str(line);
    if !line.ends_with('\n') {
        out.push('\n');
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        Category, Handler, Node, ProjectConfig, ProjectResolver, Rule, RuleContext, RuleEntry,
        RuleMeta, Severity, Span, SyntaxKind,
    };

    static TEST_META: RuleMeta = RuleMeta::new(
        "test-sort",
        Category::Operations,
        Severity::Warn,
        "",
        None,
        None,
        false,
        false,
        false,
        None,
        true,
    );

    struct SortRule;
    impl Rule for SortRule {
        fn meta(&self) -> &'static RuleMeta {
            &TEST_META
        }
        fn create(&self, _ctx: &mut RuleContext) -> Box<dyn Handler> {
            Box::new(SortHandler { fields: Vec::new() })
        }
    }

    struct SortHandler {
        fields: Vec<(String, Span)>,
    }
    impl Handler for SortHandler {
        fn on_node(&mut self, node: &Node<'_>, _parent: Option<&Node<'_>>) {
            if let (Some(name), Some(span)) = (node.name.clone(), node.span) {
                self.fields.push((name, span));
            }
        }
        fn finalize(&mut self, ctx: &mut RuleContext) {
            let source = ctx.source_code().source().to_owned();
            for pair in self.fields.windows(2) {
                if pair[0].0 > pair[1].0 {
                    let first_span = Span::new(
                        pair[0].1.offset
                            + source[pair[0].1.offset..pair[0].1.end()]
                                .find(&pair[0].0)
                                .unwrap(),
                        pair[0].0.len(),
                    );
                    let second_span = Span::new(
                        pair[1].1.offset
                            + source[pair[1].1.offset..pair[1].1.end()]
                                .find(&pair[1].0)
                                .unwrap(),
                        pair[1].0.len(),
                    );
                    ctx.report(
                        crate::DiagnosticBuilder::new(
                            ctx.rule_id(),
                            ctx.source_code().path().to_path_buf(),
                            first_span,
                            "fields are not sorted",
                        )
                        .suggestion(
                            "sort field",
                            Fix::Replace {
                                span: first_span,
                                text: pair[1].0.clone(),
                            },
                        )
                        .suggestion(
                            "sort field",
                            Fix::Replace {
                                span: second_span,
                                text: pair[0].0.clone(),
                            },
                        ),
                    );
                }
            }
        }
    }

    static TEST_ENTRY: RuleEntry = RuleEntry {
        meta: &TEST_META,
        factory: || Box::new(SortRule),
        interested_kinds: &[SyntaxKind::FIELD],
    };

    fn test_project(dir: &Path) -> Project {
        let config = ProjectConfig {
            name: "fixer".to_owned(),
            schema: None,
            documents: Some(crate::DocumentSpec::Files(vec![PathBuf::from(
                "query.graphql",
            )])),
            ignore: Vec::new(),
        };
        ProjectResolver::new(dir.to_path_buf())
            .resolve(&[config])
            .unwrap()
            .pop()
            .unwrap()
    }

    fn test_engine() -> LintEngine {
        LintEngine::from_enabled_rules(vec![crate::EnabledRule {
            entry: &TEST_ENTRY,
            severity: Severity::Warn,
            options: serde_json::Value::Null,
        }])
    }

    #[test]
    fn applies_two_non_overlapping_fixes_and_relints() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("query.graphql");
        std::fs::write(&path, "query Example {\n  b\n  a\n}\n").unwrap();
        let mut project = test_project(dir.path());
        let summary = Fixer::new(&test_engine()).fix(&mut project).unwrap();

        assert_eq!(summary.passes, 1);
        assert_eq!(summary.files_changed, 1);
        assert_eq!(summary.remaining, 0);
        assert_eq!(
            std::fs::read_to_string(path).unwrap(),
            "query Example {\n  a\n  b\n}\n"
        );
    }

    #[test]
    fn dry_run_returns_stable_unified_diff_without_writing() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("query.graphql");
        let original = "query Example {\n  b\n  a\n}\n";
        std::fs::write(&path, original).unwrap();
        let project = test_project(dir.path());
        let diffs = Fixer::new(&test_engine()).dry_run(&project).unwrap();

        assert_eq!(std::fs::read_to_string(&path).unwrap(), original);
        assert_eq!(diffs.len(), 1);
        assert_eq!(
            diffs[0].unified_diff,
            format!(
                "--- {}\n+++ {}\n@@ -1,4 +1,4 @@\n query Example {{\n-  b\n-  a\n+  a\n+  b\n }}\n",
                path.display(),
                path.display()
            )
        );
    }

    #[test]
    fn max_passes_bounds_a_non_resolving_fix() {
        struct LoopHandler;
        impl Handler for LoopHandler {
            fn finalize(&mut self, ctx: &mut RuleContext) {
                ctx.report(
                    crate::DiagnosticBuilder::new(
                        ctx.rule_id(),
                        ctx.source_code().path().to_path_buf(),
                        Span::new(0, 0),
                        "still broken",
                    )
                    .suggestion(
                        "no-op",
                        Fix::Insert {
                            offset: 0,
                            text: String::new(),
                        },
                    ),
                );
            }
        }
        static LOOP_META: RuleMeta = RuleMeta::new(
            "loop",
            Category::Operations,
            Severity::Warn,
            "",
            None,
            None,
            false,
            false,
            false,
            None,
            true,
        );
        static LOOP_ENTRY: RuleEntry = RuleEntry {
            meta: &LOOP_META,
            factory: || Box::new(LoopRule),
            interested_kinds: &[],
        };
        struct LoopRule;
        impl Rule for LoopRule {
            fn meta(&self) -> &'static RuleMeta {
                &LOOP_META
            }
            fn create(&self, _ctx: &mut RuleContext) -> Box<dyn Handler> {
                Box::new(LoopHandler)
            }
        }
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("query.graphql"), "query Example { x }\n").unwrap();
        let mut project = test_project(dir.path());
        let engine = LintEngine::from_enabled_rules(vec![crate::EnabledRule {
            entry: &LOOP_ENTRY,
            severity: Severity::Warn,
            options: serde_json::Value::Null,
        }]);
        let summary = Fixer::new(&engine)
            .with_max_passes(3)
            .fix(&mut project)
            .unwrap();
        assert_eq!(summary.passes, 3);
        assert_eq!(summary.files_changed, 0);
        assert_eq!(summary.remaining, 1);
    }
}
