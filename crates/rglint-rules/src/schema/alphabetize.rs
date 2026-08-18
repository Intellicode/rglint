use std::collections::HashMap;

use rglint_core::{DiagnosticBuilder, Fix, Handler, Node, RuleContext, Span, SyntaxKind};
use rglint_derive::Rule;
use serde::Deserialize;

#[derive(Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Opts {
    #[serde(default)]
    fields: Option<Vec<String>>,
    #[serde(default)]
    values: Option<bool>,
    #[serde(default)]
    selections: Option<Vec<String>>,
    #[serde(default)]
    variables: Option<bool>,
    #[serde(default)]
    arguments: Option<Vec<String>>,
    #[serde(default)]
    definitions: Option<bool>,
    #[serde(default)]
    groups: Option<Vec<String>>,
}

#[derive(Rule)]
#[rule(
    id = "alphabetize",
    category = "schema",
    has_suggestions = true,
    kinds = "FIELD_DEFINITION|INPUT_VALUE_DEFINITION|ENUM_VALUE|FIELD|FRAGMENT_NAME|INLINE_FRAGMENT|ARGUMENT|VARIABLE|OBJECT_TYPE_DEFINITION|INTERFACE_TYPE_DEFINITION|INPUT_OBJECT_TYPE_DEFINITION|ENUM_TYPE_DEFINITION|SCALAR_TYPE_DEFINITION|UNION_TYPE_DEFINITION|DIRECTIVE_DEFINITION|FRAGMENT_DEFINITION|OPERATION_DEFINITION|SCHEMA_DEFINITION|OBJECT_TYPE_EXTENSION|INTERFACE_TYPE_EXTENSION|INPUT_OBJECT_TYPE_EXTENSION|ENUM_TYPE_EXTENSION|SCALAR_TYPE_EXTENSION|UNION_TYPE_EXTENSION"
)]
pub struct Alphabetize;

impl Alphabetize {
    fn handler(&self, ctx: &mut RuleContext) -> Box<dyn Handler> {
        let opts: Opts = ctx.option().unwrap_or_default();
        Box::new(AlphabetizeHandler {
            entries: Vec::new(),
            fields_enabled: opts.fields.unwrap_or_default(),
            values_enabled: opts.values.unwrap_or(false),
            selections_enabled: opts.selections.unwrap_or_default(),
            variables_enabled: opts.variables.unwrap_or(false),
            arguments_enabled: opts.arguments.unwrap_or_default(),
            definitions_enabled: opts.definitions.unwrap_or(false),
            groups: opts.groups.unwrap_or_default(),
        })
    }
}

struct Entry {
    offset: usize,
    kind: SyntaxKind,
    name: Option<String>,
    span: Span,
    container_offset: usize,
    mode: Mode,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
enum Mode {
    Fields,
    Values,
    Selections,
    Variables,
    Arguments,
    Definitions,
}

struct AlphabetizeHandler {
    entries: Vec<Entry>,
    fields_enabled: Vec<String>,
    values_enabled: bool,
    selections_enabled: Vec<String>,
    variables_enabled: bool,
    arguments_enabled: Vec<String>,
    definitions_enabled: bool,
    groups: Vec<String>,
}

fn display_kind(kind: SyntaxKind) -> &'static str {
    match kind {
        SyntaxKind::FIELD_DEFINITION => "field",
        SyntaxKind::FIELD => "field",
        SyntaxKind::INPUT_VALUE_DEFINITION => "input value",
        SyntaxKind::ENUM_VALUE => "enum value",
        SyntaxKind::ARGUMENT => "argument",
        SyntaxKind::VARIABLE => "variable",
        SyntaxKind::FRAGMENT_NAME => "fragment spread",
        SyntaxKind::INLINE_FRAGMENT => "inline fragment",
        SyntaxKind::OBJECT_TYPE_DEFINITION | SyntaxKind::OBJECT_TYPE_EXTENSION => "type",
        SyntaxKind::INTERFACE_TYPE_DEFINITION | SyntaxKind::INTERFACE_TYPE_EXTENSION => "interface",
        SyntaxKind::INPUT_OBJECT_TYPE_DEFINITION | SyntaxKind::INPUT_OBJECT_TYPE_EXTENSION => {
            "input"
        }
        SyntaxKind::ENUM_TYPE_DEFINITION | SyntaxKind::ENUM_TYPE_EXTENSION => "enum",
        SyntaxKind::SCALAR_TYPE_DEFINITION | SyntaxKind::SCALAR_TYPE_EXTENSION => "scalar",
        SyntaxKind::UNION_TYPE_DEFINITION | SyntaxKind::UNION_TYPE_EXTENSION => "union",
        SyntaxKind::DIRECTIVE_DEFINITION => "directive",
        SyntaxKind::FRAGMENT_DEFINITION => "fragment",
        SyntaxKind::OPERATION_DEFINITION => "operation",
        SyntaxKind::SCHEMA_DEFINITION => "schema",
        _ => "",
    }
}

fn lower_case_kind(kind: SyntaxKind) -> &'static str {
    match kind {
        SyntaxKind::INLINE_FRAGMENT => "inline fragment",
        SyntaxKind::OPERATION_DEFINITION => "operation definition",
        SyntaxKind::SCHEMA_DEFINITION => "schema definition",
        _ => "",
    }
}

fn walk_up_to<'a>(node: &'a Node<'a>, target_kinds: &[SyntaxKind]) -> Option<&'a Node<'a>> {
    let mut current = node.parent?;
    loop {
        if target_kinds.contains(&current.kind) {
            return Some(current);
        }
        current = current.parent?;
    }
}

fn is_definition_kind(kind: SyntaxKind) -> bool {
    matches!(
        kind,
        SyntaxKind::OBJECT_TYPE_DEFINITION
            | SyntaxKind::INTERFACE_TYPE_DEFINITION
            | SyntaxKind::INPUT_OBJECT_TYPE_DEFINITION
            | SyntaxKind::ENUM_TYPE_DEFINITION
            | SyntaxKind::SCALAR_TYPE_DEFINITION
            | SyntaxKind::UNION_TYPE_DEFINITION
            | SyntaxKind::DIRECTIVE_DEFINITION
            | SyntaxKind::FRAGMENT_DEFINITION
            | SyntaxKind::OPERATION_DEFINITION
            | SyntaxKind::SCHEMA_DEFINITION
            | SyntaxKind::OBJECT_TYPE_EXTENSION
            | SyntaxKind::INTERFACE_TYPE_EXTENSION
            | SyntaxKind::INPUT_OBJECT_TYPE_EXTENSION
            | SyntaxKind::ENUM_TYPE_EXTENSION
            | SyntaxKind::SCALAR_TYPE_EXTENSION
            | SyntaxKind::UNION_TYPE_EXTENSION
    )
}

impl Handler for AlphabetizeHandler {
    fn on_node(&mut self, node: &Node<'_>, _parent: Option<&Node<'_>>) {
        let kind = node.kind;
        let span = node.span.unwrap_or(Span::new(0, 0));

        match kind {
            SyntaxKind::FIELD_DEFINITION => {
                if self.fields_enabled.is_empty() {
                    return;
                }
                let gp = match node.parent.and_then(|p| p.parent) {
                    Some(gp) => gp,
                    None => return,
                };
                let container_kind_ok = matches!(
                    gp.kind,
                    SyntaxKind::OBJECT_TYPE_DEFINITION
                        | SyntaxKind::OBJECT_TYPE_EXTENSION
                        | SyntaxKind::INTERFACE_TYPE_DEFINITION
                        | SyntaxKind::INTERFACE_TYPE_EXTENSION
                );
                if !container_kind_ok {
                    return;
                }
                let kind_name = format!("{:?}", gp.kind);
                let enabled_key = if kind_name == "OBJECT_TYPE_DEFINITION"
                    || kind_name == "OBJECT_TYPE_EXTENSION"
                {
                    "ObjectTypeDefinition"
                } else if kind_name == "INTERFACE_TYPE_DEFINITION"
                    || kind_name == "INTERFACE_TYPE_EXTENSION"
                {
                    "InterfaceTypeDefinition"
                } else {
                    return;
                };
                if !self.fields_enabled.iter().any(|k| k == enabled_key) {
                    return;
                }
                let name = match &node.name {
                    Some(n) => n.clone(),
                    None => return,
                };
                self.entries.push(Entry {
                    offset: span.offset,
                    kind,
                    name: Some(name),
                    span,
                    container_offset: gp.span.unwrap_or(Span::new(0, 0)).offset,
                    mode: Mode::Fields,
                });
            }

            SyntaxKind::INPUT_VALUE_DEFINITION => {
                let parent_kind = node.parent.map(|p| p.kind);
                let gp = match node.parent.and_then(|p| p.parent) {
                    Some(gp) => gp,
                    None => return,
                };

                if parent_kind == Some(SyntaxKind::INPUT_FIELDS_DEFINITION) {
                    if self.fields_enabled.is_empty() {
                        return;
                    }
                    let kind_name = format!("{:?}", gp.kind);
                    let enabled = kind_name == "INPUT_OBJECT_TYPE_DEFINITION"
                        || kind_name == "INPUT_OBJECT_TYPE_EXTENSION";
                    if !enabled
                        || !self
                            .fields_enabled
                            .iter()
                            .any(|k| k == "InputObjectTypeDefinition")
                    {
                        return;
                    }
                    let name = match &node.name {
                        Some(n) => n.clone(),
                        None => return,
                    };
                    self.entries.push(Entry {
                        offset: span.offset,
                        kind,
                        name: Some(name),
                        span,
                        container_offset: gp.span.unwrap_or(Span::new(0, 0)).offset,
                        mode: Mode::Fields,
                    });
                } else if parent_kind == Some(SyntaxKind::ARGUMENTS_DEFINITION) {
                    if self.arguments_enabled.is_empty() {
                        return;
                    }
                    let is_enabled = self.arguments_enabled.iter().any(|k| {
                        (k == "FieldDefinition" && gp.kind == SyntaxKind::FIELD_DEFINITION)
                            || (k == "DirectiveDefinition"
                                && gp.kind == SyntaxKind::DIRECTIVE_DEFINITION)
                    });
                    if !is_enabled {
                        return;
                    }
                    let name = match &node.name {
                        Some(n) => n.clone(),
                        None => return,
                    };
                    self.entries.push(Entry {
                        offset: span.offset,
                        kind,
                        name: Some(name),
                        span,
                        container_offset: gp.span.unwrap_or(Span::new(0, 0)).offset,
                        mode: Mode::Arguments,
                    });
                }
            }

            SyntaxKind::ENUM_VALUE => {
                if !self.values_enabled {
                    return;
                }
                let container = match walk_up_to(
                    node,
                    &[
                        SyntaxKind::ENUM_TYPE_DEFINITION,
                        SyntaxKind::ENUM_TYPE_EXTENSION,
                    ],
                ) {
                    Some(c) => c,
                    None => return,
                };
                let name = match &node.name {
                    Some(n) => n.clone(),
                    None => return,
                };
                self.entries.push(Entry {
                    offset: span.offset,
                    kind,
                    name: Some(name),
                    span,
                    container_offset: container.span.unwrap_or(Span::new(0, 0)).offset,
                    mode: Mode::Values,
                });
            }

            SyntaxKind::FIELD => {
                if self.selections_enabled.is_empty() {
                    return;
                }
                let sel_set = match node.parent {
                    Some(p) if p.kind == SyntaxKind::SELECTION_SET => p,
                    _ => return,
                };
                if !self.is_selections_enabled(sel_set) {
                    return;
                }
                let name = match &node.name {
                    Some(n) => n.clone(),
                    None => return,
                };
                self.entries.push(Entry {
                    offset: span.offset,
                    kind,
                    name: Some(name),
                    span,
                    container_offset: sel_set.span.unwrap_or(Span::new(0, 0)).offset,
                    mode: Mode::Selections,
                });
            }

            SyntaxKind::FRAGMENT_NAME => {
                if self.selections_enabled.is_empty() {
                    return;
                }
                let frag_spread = match node.parent {
                    Some(p) if p.kind == SyntaxKind::FRAGMENT_SPREAD => p,
                    _ => return,
                };
                let sel_set = match frag_spread.parent {
                    Some(p) if p.kind == SyntaxKind::SELECTION_SET => p,
                    _ => return,
                };
                if !self.is_selections_enabled(sel_set) {
                    return;
                }
                let name = match &node.name {
                    Some(n) => n.clone(),
                    None => return,
                };
                self.entries.push(Entry {
                    offset: span.offset,
                    kind: SyntaxKind::FRAGMENT_NAME,
                    name: Some(name),
                    span,
                    container_offset: sel_set.span.unwrap_or(Span::new(0, 0)).offset,
                    mode: Mode::Selections,
                });
            }

            SyntaxKind::INLINE_FRAGMENT => {
                if self.selections_enabled.is_empty() {
                    return;
                }
                let sel_set = match node.parent {
                    Some(p) if p.kind == SyntaxKind::SELECTION_SET => p,
                    _ => return,
                };
                if !self.is_selections_enabled(sel_set) {
                    return;
                }
                self.entries.push(Entry {
                    offset: span.offset,
                    kind,
                    name: None,
                    span,
                    container_offset: sel_set.span.unwrap_or(Span::new(0, 0)).offset,
                    mode: Mode::Selections,
                });
            }

            SyntaxKind::ARGUMENT => {
                if self.arguments_enabled.is_empty() {
                    return;
                }
                let container = match node.parent.and_then(|p| p.parent) {
                    Some(gp) => gp,
                    None => return,
                };
                let is_enabled = self.arguments_enabled.iter().any(|k| {
                    (k == "Field" && container.kind == SyntaxKind::FIELD)
                        || (k == "Directive" && container.kind == SyntaxKind::DIRECTIVE)
                });
                if !is_enabled {
                    return;
                }
                let name = match &node.name {
                    Some(n) => n.clone(),
                    None => return,
                };
                self.entries.push(Entry {
                    offset: span.offset,
                    kind,
                    name: Some(name),
                    span,
                    container_offset: container.span.unwrap_or(Span::new(0, 0)).offset,
                    mode: Mode::Arguments,
                });
            }

            SyntaxKind::VARIABLE => {
                if !self.variables_enabled {
                    return;
                }
                let in_var_def = node
                    .parent
                    .is_some_and(|p| p.kind == SyntaxKind::VARIABLE_DEFINITION);
                if !in_var_def {
                    return;
                }
                let container = match walk_up_to(node, &[SyntaxKind::OPERATION_DEFINITION]) {
                    Some(c) => c,
                    None => return,
                };
                let name = match &node.name {
                    Some(n) => n.clone(),
                    None => return,
                };
                self.entries.push(Entry {
                    offset: span.offset,
                    kind,
                    name: Some(name),
                    span,
                    container_offset: container.span.unwrap_or(Span::new(0, 0)).offset,
                    mode: Mode::Variables,
                });
            }

            _ => {
                if !self.definitions_enabled {
                    return;
                }
                if !is_definition_kind(kind) {
                    return;
                }
                let is_top_level = node.parent.is_some_and(|p| p.kind == SyntaxKind::DOCUMENT);
                if !is_top_level {
                    return;
                }
                let name = node.name.clone();
                self.entries.push(Entry {
                    offset: span.offset,
                    kind,
                    name,
                    span,
                    container_offset: 0,
                    mode: Mode::Definitions,
                });
            }
        }
    }

    fn finalize(&mut self, ctx: &mut RuleContext) {
        if !self.groups.is_empty() && !self.groups.contains(&"*".to_owned()) {
            return;
        }

        let mut by_container: HashMap<(usize, Mode), Vec<usize>> = HashMap::new();
        for (i, entry) in self.entries.iter().enumerate() {
            by_container
                .entry((entry.container_offset, entry.mode))
                .or_default()
                .push(i);
        }

        let has_groups = !self.groups.is_empty();
        let source = ctx.source_code().source().to_owned();
        let path = ctx.source_code().path().to_path_buf();
        let rule_id = ctx.rule_id();

        for indices in by_container.values_mut() {
            indices.sort_by_key(|&i| self.entries[i].offset);

            for j in 1..indices.len() {
                let curr_idx = indices[j];
                let prev_idx = indices[j - 1];

                let curr_name = match &self.entries[curr_idx].name {
                    Some(n) => n.clone(),
                    None => continue,
                };

                let prev_name = self.entries[prev_idx].name.clone();

                let compare_result = match &prev_name {
                    Some(pn) => pn.to_lowercase().cmp(&curr_name.to_lowercase()),
                    None => std::cmp::Ordering::Less,
                };

                let mut should_report = compare_result == std::cmp::Ordering::Greater;

                if has_groups {
                    let prev_idx_val = if let Some(ref pn) = prev_name {
                        self.get_group_index(pn, self.entries[prev_idx].kind)
                    } else {
                        self.get_group_index("", self.entries[prev_idx].kind)
                    };
                    let curr_idx_val =
                        self.get_group_index(&curr_name, self.entries[curr_idx].kind);

                    if prev_idx_val > curr_idx_val {
                        should_report = true;
                    } else if prev_idx_val < curr_idx_val {
                        should_report = false;
                    }
                }

                if !should_report {
                    continue;
                }

                let prev_display = match &prev_name {
                    Some(pn) => {
                        format!("{} \"{}\"", display_kind(self.entries[prev_idx].kind), pn)
                    }
                    None => lower_case_kind(self.entries[prev_idx].kind).to_owned(),
                };

                let curr_display = format!(
                    "{} \"{}\"",
                    display_kind(self.entries[curr_idx].kind),
                    curr_name
                );

                let message = format!("{curr_display} should be before {prev_display}");

                let previous = &self.entries[prev_idx];
                let current = &self.entries[curr_idx];
                let previous_text = source
                    .get(previous.span.offset..previous.span.end())
                    .unwrap_or_default()
                    .to_owned();
                let current_text = source
                    .get(current.span.offset..current.span.end())
                    .unwrap_or_default()
                    .to_owned();
                ctx.report(
                    DiagnosticBuilder::new(rule_id, path.clone(), current.span, message)
                        .suggestion(
                            format!("Move {} before {}", curr_display, prev_display),
                            Fix::Replace {
                                span: current.span,
                                text: previous_text,
                            },
                        )
                        .suggestion(
                            format!("Move {} after {}", prev_display, curr_display),
                            Fix::Replace {
                                span: previous.span,
                                text: current_text,
                            },
                        ),
                );
            }
        }
    }
}

impl AlphabetizeHandler {
    fn get_group_index(&self, name: &str, kind: SyntaxKind) -> usize {
        if let Some(idx) = self.groups.iter().position(|g| g == name) {
            return idx;
        }
        if kind == SyntaxKind::FRAGMENT_NAME || kind == SyntaxKind::INLINE_FRAGMENT {
            if let Some(idx) = self.groups.iter().position(|g| g == "...") {
                return idx;
            }
        }
        if let Some(idx) = self.groups.iter().position(|g| g == "{") {
            return idx;
        }
        self.groups
            .iter()
            .position(|g| g == "*")
            .unwrap_or(usize::MAX)
    }

    fn is_selections_enabled(&self, sel_set: &Node<'_>) -> bool {
        let mut current = sel_set.parent;
        while let Some(p) = current {
            if p.kind == SyntaxKind::OPERATION_DEFINITION {
                return self
                    .selections_enabled
                    .iter()
                    .any(|k| k == "OperationDefinition");
            }
            if p.kind == SyntaxKind::FRAGMENT_DEFINITION {
                return self
                    .selections_enabled
                    .iter()
                    .any(|k| k == "FragmentDefinition");
            }
            current = p.parent;
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rglint_core::{Category, Rule, Severity};

    #[test]
    fn rule_meta_matches_spec_021() {
        let rule = Alphabetize;
        let meta = rule.meta();
        assert_eq!(meta.id, "alphabetize");
        assert_eq!(meta.category, Category::Schema);
        assert_eq!(meta.severity, Severity::Warn);
        assert!(!meta.requires_schema);
        assert!(!meta.requires_siblings);
        assert!(meta.has_suggestions);
    }
}
