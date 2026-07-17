use std::collections::HashMap;

use rglint_core::{DiagnosticBuilder, Handler, Node, RuleContext, Span, SyntaxKind};
use rglint_derive::Rule;

#[derive(Rule)]
#[rule(
    id = "require-deprecation-date",
    category = "schema",
    requires_schema = true,
    kinds = "DIRECTIVE|ARGUMENT"
)]
pub struct RequireDeprecationDate;

impl RequireDeprecationDate {
    fn handler(&self, ctx: &mut RuleContext) -> Box<dyn Handler> {
        let opts: Opts = ctx.option().unwrap_or_default();
        Box::new(RequireDeprecationDateHandler {
            argument_name: opts.argument_name.unwrap_or_else(|| "deletionDate".to_string()),
            directives: HashMap::new(),
            arguments: HashMap::new(),
        })
    }
}

#[derive(Default, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct Opts {
    argument_name: Option<String>,
}

#[derive(Clone, Debug)]
struct ParentInfo {
    kind: SyntaxKind,
    name: Option<String>,
    span: Span,
    container_kind: Option<SyntaxKind>,
    container_name: Option<String>,
}

struct RequireDeprecationDateHandler {
    argument_name: String,
    directives: HashMap<usize, (Span, ParentInfo)>,
    arguments: HashMap<usize, (Span, String)>,
}

fn find_deprecated_directive<'a>(start: Option<&'a Node<'a>>) -> Option<&'a Node<'a>> {
    let mut current = start;
    loop {
        match current {
            Some(n) if n.kind == SyntaxKind::DIRECTIVE && n.name.as_deref() == Some("deprecated") => {
                return Some(n);
            }
            Some(n) => current = n.parent,
            None => return None,
        }
    }
}

fn is_definition_kind(kind: SyntaxKind) -> bool {
    matches!(
        kind,
        SyntaxKind::OBJECT_TYPE_DEFINITION
            | SyntaxKind::OBJECT_TYPE_EXTENSION
            | SyntaxKind::INTERFACE_TYPE_DEFINITION
            | SyntaxKind::INTERFACE_TYPE_EXTENSION
            | SyntaxKind::ENUM_TYPE_DEFINITION
            | SyntaxKind::ENUM_TYPE_EXTENSION
            | SyntaxKind::SCALAR_TYPE_DEFINITION
            | SyntaxKind::SCALAR_TYPE_EXTENSION
            | SyntaxKind::INPUT_OBJECT_TYPE_DEFINITION
            | SyntaxKind::INPUT_OBJECT_TYPE_EXTENSION
            | SyntaxKind::UNION_TYPE_DEFINITION
            | SyntaxKind::UNION_TYPE_EXTENSION
            | SyntaxKind::DIRECTIVE_DEFINITION
            | SyntaxKind::SCHEMA_DEFINITION
            | SyntaxKind::SCHEMA_EXTENSION
            | SyntaxKind::FIELD_DEFINITION
            | SyntaxKind::INPUT_VALUE_DEFINITION
            | SyntaxKind::ENUM_VALUE_DEFINITION
            | SyntaxKind::OPERATION_DEFINITION
            | SyntaxKind::FRAGMENT_DEFINITION
    )
}

fn is_container_kind(kind: SyntaxKind) -> bool {
    matches!(
        kind,
        SyntaxKind::OBJECT_TYPE_DEFINITION
            | SyntaxKind::INTERFACE_TYPE_DEFINITION
            | SyntaxKind::INPUT_OBJECT_TYPE_DEFINITION
            | SyntaxKind::ENUM_TYPE_DEFINITION
            | SyntaxKind::OBJECT_TYPE_EXTENSION
            | SyntaxKind::INTERFACE_TYPE_EXTENSION
            | SyntaxKind::INPUT_OBJECT_TYPE_EXTENSION
    )
}

/// Walk up from `start` to find the nearest definition node.
fn find_definition<'a>(start: &'a Node<'a>) -> Option<&'a Node<'a>> {
    let mut current = start;
    loop {
        if is_definition_kind(current.kind) {
            return Some(current);
        }
        current = current.parent?;
    }
}

/// Walk up from `start` to find the nearest container type node
/// (object/interface/input/enum), used for field "x" in type "T" messages.
fn find_container<'a>(start: &'a Node<'a>) -> Option<&'a Node<'a>> {
    let mut current = start;
    loop {
        if is_container_kind(current.kind) {
            return Some(current);
        }
        current = current.parent?;
    }
}

fn display_kind_label(kind: SyntaxKind) -> &'static str {
    match kind {
        SyntaxKind::OBJECT_TYPE_DEFINITION | SyntaxKind::OBJECT_TYPE_EXTENSION => "type",
        SyntaxKind::INTERFACE_TYPE_DEFINITION | SyntaxKind::INTERFACE_TYPE_EXTENSION => "interface",
        SyntaxKind::ENUM_TYPE_DEFINITION | SyntaxKind::ENUM_TYPE_EXTENSION => "enum",
        SyntaxKind::SCALAR_TYPE_DEFINITION | SyntaxKind::SCALAR_TYPE_EXTENSION => "scalar",
        SyntaxKind::INPUT_OBJECT_TYPE_DEFINITION | SyntaxKind::INPUT_OBJECT_TYPE_EXTENSION => "input",
        SyntaxKind::UNION_TYPE_DEFINITION | SyntaxKind::UNION_TYPE_EXTENSION => "union",
        SyntaxKind::DIRECTIVE_DEFINITION => "directive",
        SyntaxKind::SCHEMA_DEFINITION | SyntaxKind::SCHEMA_EXTENSION => "schema",
        SyntaxKind::FIELD_DEFINITION => "field",
        SyntaxKind::INPUT_VALUE_DEFINITION => "input value",
        SyntaxKind::ENUM_VALUE_DEFINITION => "enum value",
        _ => "",
    }
}

fn display_node_name(kind: SyntaxKind, name: &Option<String>) -> String {
    let label = display_kind_label(kind);
    match name {
        Some(n) => format!("{label} \"{n}\""),
        None => label.to_owned(),
    }
}

fn get_node_name(info: &ParentInfo) -> String {
    match info.kind {
        SyntaxKind::FIELD_DEFINITION
        | SyntaxKind::INPUT_VALUE_DEFINITION
        | SyntaxKind::ENUM_VALUE_DEFINITION => {
            let self_str = display_node_name(info.kind, &info.name);
            let container_kind = info.container_kind.unwrap_or(SyntaxKind::OBJECT_TYPE_DEFINITION);
            let container_str = display_node_name(container_kind, &info.container_name);
            format!("{self_str} in {container_str}")
        }
        _ => display_node_name(info.kind, &info.name),
    }
}

fn name_offset(source: &str, start_offset: usize, name: &str) -> usize {
    source[start_offset..]
        .find(name)
        .map(|pos| start_offset + pos)
        .unwrap_or(start_offset)
}

fn extract_arg_value(arg_text: &str) -> Option<String> {
    let colon_pos = arg_text.find(':')?;
    let value = arg_text[colon_pos + 1..].trim();
    let value = value
        .strip_prefix('"')
        .and_then(|v| v.strip_suffix('"'))
        .unwrap_or(value);
    Some(value.to_owned())
}

fn arg_value_span(arg_text: &str, arg_offset: usize) -> Option<Span> {
    let colon_pos = arg_text.find(':')?;
    let value_start = colon_pos + 1 + arg_text[colon_pos + 1..].len()
        - arg_text[colon_pos + 1..].trim_start().len();
    let value_text = arg_text[colon_pos + 1..].trim_start();
    if value_text.is_empty() {
        return None;
    }
    Some(Span::new(arg_offset + value_start, value_text.len()))
}

fn is_dd_mm_yyyy(s: &str) -> bool {
    let b = s.as_bytes();
    b.len() == 10
        && b[0].is_ascii_digit()
        && b[1].is_ascii_digit()
        && b[2] == b'/'
        && b[3].is_ascii_digit()
        && b[4].is_ascii_digit()
        && b[5] == b'/'
        && b[6].is_ascii_digit()
        && b[7].is_ascii_digit()
        && b[8].is_ascii_digit()
        && b[9].is_ascii_digit()
}

fn days_from_ce(year: i64, month: u32, day: u32) -> i64 {
    let (y, m) = if month <= 2 {
        (year - 1, month + 12)
    } else {
        (year, month)
    };
    let era = if y >= 0 { y } else { y - 3 } / 400;
    let yoe = y - era * 400;
    let doy = (153 * (m as i64) - 457) / 5 + day as i64;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146097 + doe - 719468 - 365
}

impl Handler for RequireDeprecationDateHandler {
    fn on_node(&mut self, node: &Node<'_>, parent: Option<&Node<'_>>) {
        match node.kind {
            SyntaxKind::DIRECTIVE => {
                if node.name.as_deref() == Some("deprecated") {
                    if let Some(span) = node.span {
                        let definition = parent
                            .and_then(|p| find_definition(p))
                            .or_else(|| find_definition(node));
                        let parent_info = match definition {
                            Some(def) => {
                                let (cont_kind, cont_name) = find_container(def)
                                    .map(|c| (c.kind, c.name.clone()))
                                    .unwrap_or((SyntaxKind::OBJECT_TYPE_DEFINITION, None));
                                ParentInfo {
                                    kind: def.kind,
                                    name: def.name.clone(),
                                    span: def.span.unwrap_or(Span::new(0, 0)),
                                    container_kind: Some(cont_kind),
                                    container_name: cont_name,
                                }
                            }
                            None => ParentInfo {
                                kind: SyntaxKind::OBJECT_TYPE_DEFINITION,
                                name: None,
                                span: Span::new(0, 0),
                                container_kind: None,
                                container_name: None,
                            },
                        };
                        self.directives.insert(span.offset, (span, parent_info));
                    }
                }
            }
            SyntaxKind::ARGUMENT
                if node.name.as_deref() == Some(&self.argument_name) =>
            {
                if let Some(arg_span) = node.span {
                    if let Some(dir) = find_deprecated_directive(parent) {
                        if let Some(dir_span) = dir.span {
                            self.arguments
                                .insert(dir_span.offset, (arg_span, String::new()));
                        }
                    }
                }
            }
            _ => {}
        }
    }

    fn finalize(&mut self, ctx: &mut RuleContext) {
        let source = ctx.source_code().source().to_owned();
        let path = ctx.source_code().path().to_path_buf();
        let rule_id = ctx.rule_id();

        let mut arg_values: HashMap<usize, (Span, String)> = HashMap::new();
        for (&dir_offset, &(arg_span, _)) in &self.arguments {
            let arg_text = &source[arg_span.offset..arg_span.end()];
            if let Some(value) = extract_arg_value(arg_text) {
                if let Some(val_span) = arg_value_span(arg_text, arg_span.offset) {
                    arg_values.insert(dir_offset, (val_span, value.to_owned()));
                }
            }
        }

        for (&dir_offset, &(dir_span, ref parent_info)) in &self.directives {
            match arg_values.get(&dir_offset) {
                Some(&(val_span, ref value)) => {
                    if !is_dd_mm_yyyy(value) {
                        ctx.report(DiagnosticBuilder::new(
                            rule_id,
                            path.clone(),
                            val_span,
                            format!(
                                "Deletion date must be in format \"DD/MM/YYYY\" for {}",
                                get_node_name(parent_info)
                            ),
                        ));
                        continue;
                    }

                    let parts: Vec<&str> = value.split('/').collect();
                    if parts.len() == 3 {
                        let day: u32 = parts[0].parse().unwrap_or(0);
                        let month: u32 = parts[1].parse().unwrap_or(0);
                        let year: u32 = parts[2].parse().unwrap_or(0);

                        let is_valid_date = (1..=31).contains(&day)
                            && (1..=12).contains(&month)
                            && year >= 1;

                        if !is_valid_date {
                            ctx.report(DiagnosticBuilder::new(
                                rule_id,
                                path.clone(),
                                val_span,
                                format!(
                                    "Invalid \"{value}\" deletion date for {}",
                                    get_node_name(parent_info)
                                ),
                            ));
                            continue;
                        }

                        let now = std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_secs() as i64;

                        let date_secs = {
                            let days = days_from_ce(year as i64, month, day);
                            days * 86400
                        };

                        if date_secs < now {
                            let name_start = parent_info
                                .name
                                .as_ref()
                                .map_or(parent_info.span.offset, |n| {
                                    name_offset(&source, parent_info.span.offset, n)
                                });
                            let name_len =
                                parent_info.name.as_ref().map_or(0, |n| n.len());
                            let parent_name_span = Span::new(name_start, name_len);

                            ctx.report(DiagnosticBuilder::new(
                                rule_id,
                                path.clone(),
                                parent_name_span,
                                format!(
                                    "{} сan be removed",
                                    get_node_name(parent_info)
                                ),
                            ));
                        }
                    }
                }
                None => {
                    ctx.report(DiagnosticBuilder::new(
                        rule_id,
                        path.clone(),
                        dir_span,
                        format!(
                            "Directive \"@deprecated\" must have a deletion date for {}",
                            get_node_name(parent_info)
                        ),
                    ));
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rglint_core::{Category, Rule, Severity};

    #[test]
    fn rule_meta_matches_spec_027() {
        let rule = RequireDeprecationDate;
        let meta = rule.meta();
        assert_eq!(meta.id, "require-deprecation-date");
        assert_eq!(meta.category, Category::Schema);
        assert_eq!(meta.severity, Severity::Warn);
        assert!(meta.requires_schema);
        assert!(!meta.requires_siblings);
    }
}
