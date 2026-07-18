use regex::Regex;

use rglint_core::{DiagnosticBuilder, Fix, Handler, Node, RuleContext, Span, SyntaxKind};
use rglint_derive::Rule;

use crate::shared::case_styles::CaseStyle;
use crate::shared::convert_case;

#[derive(Rule)]
#[rule(
    id = "naming-convention",
    category = "schema",
    has_suggestions = true,
    kinds = "FIELD_DEFINITION|INPUT_VALUE_DEFINITION|ENUM_VALUE|ENUM_VALUE_DEFINITION|OBJECT_TYPE_DEFINITION|INTERFACE_TYPE_DEFINITION|UNION_TYPE_DEFINITION|ENUM_TYPE_DEFINITION|SCALAR_TYPE_DEFINITION|INPUT_OBJECT_TYPE_DEFINITION|DIRECTIVE_DEFINITION|OPERATION_DEFINITION|FRAGMENT_DEFINITION|VARIABLE_DEFINITION|FIELD|OBJECT_TYPE_EXTENSION|INTERFACE_TYPE_EXTENSION|UNION_TYPE_EXTENSION|ENUM_TYPE_EXTENSION|SCALAR_TYPE_EXTENSION|INPUT_OBJECT_TYPE_EXTENSION"
)]
pub struct NamingConvention;

impl NamingConvention {
    fn handler(&self, _ctx: &mut RuleContext) -> Box<dyn Handler> {
        let opts = parse_options(_ctx.options_raw());
        Box::new(NamingConventionHandler {
            opts,
            nodes: Vec::new(),
        })
    }
}

struct Opts {
    allow_leading_underscore: bool,
    allow_trailing_underscore: bool,
    kind_configs: Vec<KindSelector>,
}

struct KindSelector {
    kind_name: String,
    predicate: Option<SelectorPredicate>,
    config: KindConfig,
}

#[derive(Clone)]
enum SelectorPredicate {
    ParentNameEquals(String),
    ParentNameNotEquals(String),
    GqlTypeNameEquals(String),
    GqlTypeGqlTypeNameEquals(String),
}

#[derive(Clone, Default)]
struct KindConfig {
    style: Option<(CaseStyle, String)>,
    prefix: Option<String>,
    suffix: Option<String>,
    forbidden_prefixes: Vec<String>,
    forbidden_suffixes: Vec<String>,
    required_prefixes: Vec<String>,
    required_suffixes: Vec<String>,
    forbidden_patterns: Vec<Regex>,
    required_pattern: Option<Regex>,
    ignore_pattern: Option<Regex>,
}

struct NodeInfo {
    kind: SyntaxKind,
    name: String,
    span: Span,
    parent_name: Option<String>,
}

struct NamingConventionHandler {
    opts: Opts,
    nodes: Vec<NodeInfo>,
}

fn parse_options(raw: &serde_json::Value) -> Opts {
    let mut opts = Opts {
        allow_leading_underscore: false,
        allow_trailing_underscore: true,
        kind_configs: Vec::new(),
    };
    let obj = match raw.as_object() {
        Some(o) => o,
        None => return opts,
    };

    if let Some(v) = obj.get("allowLeadingUnderscore").and_then(|v| v.as_bool()) {
        opts.allow_leading_underscore = v;
    }

    if let Some(v) = obj.get("allowTrailingUnderscore").and_then(|v| v.as_bool()) {
        opts.allow_trailing_underscore = v;
    }

    for (key, val) in obj {
        if matches!(
            key.as_str(),
            "allowLeadingUnderscore" | "allowTrailingUnderscore"
        ) {
            continue;
        }
        let (kind_name, predicate) = parse_selector_key(key);
        let config = parse_kind_config(val);

        if kind_name == "types" {
            for tk in &[
                "ObjectTypeDefinition", "InterfaceTypeDefinition",
                "UnionTypeDefinition", "InputObjectTypeDefinition",
                "EnumTypeDefinition", "ScalarTypeDefinition",
            ] {
                opts.kind_configs.push(KindSelector {
                    kind_name: tk.to_string(),
                    predicate: predicate.clone(),
                    config: config.clone(),
                });
            }
        } else {
            opts.kind_configs.push(KindSelector {
                kind_name,
                predicate,
                config,
            });
        }
    }

    opts
}

fn parse_selector_key(key: &str) -> (String, Option<SelectorPredicate>) {
    if let Some(start) = key.find('[') {
        if let Some(end) = key.rfind(']') {
            let kind = key[..start].to_string();
            let inner = &key[start + 1..end];
            return (kind, parse_predicate(inner));
        }
    }
    (key.to_string(), None)
}

fn parse_predicate(s: &str) -> Option<SelectorPredicate> {
    if let Some(pos) = s.find("!=") {
        let field = s[..pos].trim();
        let value = s[pos + 2..].trim().trim_matches('"');
        if field == "parent.name.value" {
            return Some(SelectorPredicate::ParentNameNotEquals(value.to_string()));
        }
        return None;
    }
    if let Some(pos) = s.find('=') {
        let field = s[..pos].trim();
        let value = s[pos + 1..].trim().trim_matches('"');
        match field {
            "parent.name.value" => return Some(SelectorPredicate::ParentNameEquals(value.to_string())),
            "gqlType.name.value" => return Some(SelectorPredicate::GqlTypeNameEquals(value.to_string())),
            "gqlType.gqlType.name.value" => return Some(SelectorPredicate::GqlTypeGqlTypeNameEquals(value.to_string())),
            _ => return None,
        }
    }
    None
}

fn parse_case_style(s: &str) -> Option<(CaseStyle, String)> {
    match s {
        "camelCase" => Some((CaseStyle::Camel, s.to_string())),
        "PascalCase" => Some((CaseStyle::Pascal, s.to_string())),
        "StrictPascalCase" => Some((CaseStyle::StrictPascal, s.to_string())),
        "snake_case" => Some((CaseStyle::Snake, s.to_string())),
        "UPPER_CASE" => Some((CaseStyle::ScreamingSnake, s.to_string())),
        "kebab-case" => Some((CaseStyle::Kebab, s.to_string())),
        "SCREAMING-KEBAB-CASE" => Some((CaseStyle::ScreamingKebab, s.to_string())),
        _ => None,
    }
}

fn parse_kind_config(val: &serde_json::Value) -> KindConfig {
    match val {
        serde_json::Value::String(s) => KindConfig {
            style: parse_case_style(s),
            ..Default::default()
        },
        serde_json::Value::Object(obj) => {
            let mut config = KindConfig::default();
            config.style = obj.get("style").and_then(|v| v.as_str()).and_then(parse_case_style);
            config.prefix = obj.get("prefix").and_then(|v| v.as_str()).map(String::from);
            config.suffix = obj.get("suffix").and_then(|v| v.as_str()).map(String::from);
            if let Some(arr) = obj.get("forbiddenPrefixes").and_then(|v| v.as_array()) {
                config.forbidden_prefixes = arr.iter().filter_map(|e| e.as_str().map(String::from)).collect();
            }
            if let Some(arr) = obj.get("forbiddenSuffixes").and_then(|v| v.as_array()) {
                config.forbidden_suffixes = arr.iter().filter_map(|e| e.as_str().map(String::from)).collect();
            }
            if let Some(arr) = obj.get("requiredPrefixes").and_then(|v| v.as_array()) {
                config.required_prefixes = arr.iter().filter_map(|e| e.as_str().map(String::from)).collect();
            }
            if let Some(arr) = obj.get("requiredSuffixes").and_then(|v| v.as_array()) {
                config.required_suffixes = arr.iter().filter_map(|e| e.as_str().map(String::from)).collect();
            }
            if let Some(arr) = obj.get("forbiddenPatterns").and_then(|v| v.as_array()) {
                config.forbidden_patterns = arr.iter().filter_map(|e| e.as_str()).filter_map(|s| {
                    let p = s.strip_prefix('/').and_then(|s| s.strip_suffix('/')).unwrap_or(s);
                    Regex::new(p).ok()
                }).collect();
            }
            config.required_pattern = obj.get("requiredPattern").and_then(|v| v.as_str()).and_then(|s| {
                let p = s.strip_prefix('/').and_then(|s| s.strip_suffix('/')).unwrap_or(s);
                Regex::new(p).ok()
            });
            config.ignore_pattern = obj.get("ignorePattern").and_then(|v| v.as_str()).and_then(|s| Regex::new(s).ok());
            config
        }
        _ => KindConfig::default(),
    }
}

fn kind_name_str(kind: SyntaxKind) -> &'static str {
    match kind {
        SyntaxKind::FIELD_DEFINITION => "FieldDefinition",
        SyntaxKind::INPUT_VALUE_DEFINITION => "InputValueDefinition",
        SyntaxKind::ENUM_VALUE_DEFINITION | SyntaxKind::ENUM_VALUE => "EnumValueDefinition",
        SyntaxKind::OBJECT_TYPE_DEFINITION | SyntaxKind::OBJECT_TYPE_EXTENSION => "ObjectTypeDefinition",
        SyntaxKind::INTERFACE_TYPE_DEFINITION | SyntaxKind::INTERFACE_TYPE_EXTENSION => "InterfaceTypeDefinition",
        SyntaxKind::UNION_TYPE_DEFINITION | SyntaxKind::UNION_TYPE_EXTENSION => "UnionTypeDefinition",
        SyntaxKind::ENUM_TYPE_DEFINITION | SyntaxKind::ENUM_TYPE_EXTENSION => "EnumTypeDefinition",
        SyntaxKind::SCALAR_TYPE_DEFINITION | SyntaxKind::SCALAR_TYPE_EXTENSION => "ScalarTypeDefinition",
        SyntaxKind::INPUT_OBJECT_TYPE_DEFINITION | SyntaxKind::INPUT_OBJECT_TYPE_EXTENSION => "InputObjectTypeDefinition",
        SyntaxKind::DIRECTIVE_DEFINITION => "DirectiveDefinition",
        SyntaxKind::OPERATION_DEFINITION => "OperationDefinition",
        SyntaxKind::FRAGMENT_DEFINITION => "FragmentDefinition",
        SyntaxKind::VARIABLE_DEFINITION => "VariableDefinition",
        SyntaxKind::FIELD => "Field",
        _ => "",
    }
}

fn display_name(kind: SyntaxKind) -> &'static str {
    match kind {
        SyntaxKind::OBJECT_TYPE_DEFINITION | SyntaxKind::OBJECT_TYPE_EXTENSION => "type",
        SyntaxKind::INTERFACE_TYPE_DEFINITION | SyntaxKind::INTERFACE_TYPE_EXTENSION => "interface",
        SyntaxKind::UNION_TYPE_DEFINITION | SyntaxKind::UNION_TYPE_EXTENSION => "union",
        SyntaxKind::ENUM_TYPE_DEFINITION | SyntaxKind::ENUM_TYPE_EXTENSION => "enum",
        SyntaxKind::SCALAR_TYPE_DEFINITION | SyntaxKind::SCALAR_TYPE_EXTENSION => "scalar",
        SyntaxKind::INPUT_OBJECT_TYPE_DEFINITION | SyntaxKind::INPUT_OBJECT_TYPE_EXTENSION => "input",
        SyntaxKind::DIRECTIVE_DEFINITION => "directive",
        SyntaxKind::FIELD_DEFINITION => "field",
        SyntaxKind::INPUT_VALUE_DEFINITION => "input value",
        SyntaxKind::ENUM_VALUE_DEFINITION | SyntaxKind::ENUM_VALUE => "enum value",
        SyntaxKind::OPERATION_DEFINITION => "operation",
        SyntaxKind::FRAGMENT_DEFINITION => "fragment",
        SyntaxKind::VARIABLE_DEFINITION => "variable",
        SyntaxKind::FIELD => "field",
        _ => "",
    }
}

fn is_type_def(kind: SyntaxKind) -> bool {
    matches!(
        kind,
        SyntaxKind::OBJECT_TYPE_DEFINITION
            | SyntaxKind::INTERFACE_TYPE_DEFINITION
            | SyntaxKind::UNION_TYPE_DEFINITION
            | SyntaxKind::ENUM_TYPE_DEFINITION
            | SyntaxKind::SCALAR_TYPE_DEFINITION
            | SyntaxKind::INPUT_OBJECT_TYPE_DEFINITION
            | SyntaxKind::OBJECT_TYPE_EXTENSION
            | SyntaxKind::INTERFACE_TYPE_EXTENSION
            | SyntaxKind::UNION_TYPE_EXTENSION
            | SyntaxKind::ENUM_TYPE_EXTENSION
            | SyntaxKind::SCALAR_TYPE_EXTENSION
            | SyntaxKind::INPUT_OBJECT_TYPE_EXTENSION
    )
}

fn find_parent_name(node: &Node) -> Option<String> {
    let mut cur = node.parent;
    while let Some(p) = cur {
        if is_type_def(p.kind) || p.kind == SyntaxKind::FIELD {
            if let Some(ref n) = p.name {
                return Some(n.clone());
            }
        }
        cur = p.parent;
    }
    None
}

/// Extract the alias name from a field in the source text.
/// For `alias: fieldName`, returns the text before `:`.
fn field_alias_name(source: &str, span: Span) -> Option<String> {
    let field_text = &source[span.offset..span.end()];
    if let Some(colon_pos) = field_text.find(':') {
        let before = field_text[..colon_pos].trim();
        if !before.is_empty() {
            return Some(before.to_string());
        }
    }
    None
}

impl Handler for NamingConventionHandler {
    fn on_node(&mut self, node: &Node<'_>, _parent: Option<&Node<'_>>) {
        let name = match &node.name {
            Some(n) => n.clone(),
            None => return,
        };
        let span = match node.span {
            Some(s) => s,
            None => return,
        };
        let kstr = kind_name_str(node.kind);
        if kstr.is_empty() {
            return;
        }
        let parent_name = find_parent_name(node);
        self.nodes.push(NodeInfo {
            kind: node.kind,
            name,
            span,
            parent_name,
        });
    }

    fn finalize(&mut self, ctx: &mut RuleContext) {
        let source = ctx.source_code().source().to_owned();
        let path = ctx.source_code().path().to_path_buf();
        let rule_id = ctx.rule_id();

        for ni in &self.nodes {
            let kind = ni.kind;
            let name = &ni.name;
            let span = ni.span;
            let kstr = kind_name_str(kind);

            // For FIELD nodes with aliases, use the alias name
            let effective_name: std::borrow::Cow<'_, str> = if kind == SyntaxKind::FIELD {
                if let Some(alias) = field_alias_name(&source, span) {
                    std::borrow::Cow::Owned(alias)
                } else {
                    std::borrow::Cow::Borrowed(name)
                }
            } else {
                std::borrow::Cow::Borrowed(name)
            };

            let name = &*effective_name;

            if name == "__typename" {
                continue;
            }

            // Strip allowed underscores for convention checking
            let check_name: std::borrow::Cow<'_, str> = if self.opts.allow_leading_underscore || self.opts.allow_trailing_underscore {
                let s = name.trim_start_matches('_');
                let s = if self.opts.allow_trailing_underscore {
                    s.trim_end_matches('_')
                } else {
                    s
                };
                if s.is_empty() {
                    std::borrow::Cow::Borrowed(name)
                } else {
                    std::borrow::Cow::Owned(s.to_string())
                }
            } else {
                std::borrow::Cow::Borrowed(name)
            };

            let has_kind_config = self.opts.kind_configs.iter().any(|ks| {
                
                if kstr == "Field" && ks.kind_name == "Field" {
                    true
                } else {
                    ks.kind_name == kstr
                }
            });

            // Check if any matching config's ignorePattern matches — skip node entirely
            let mut ignored = false;
            if has_kind_config {
                for ks in &self.opts.kind_configs {
                    let kind_matches = if kstr == "Field" && ks.kind_name == "Field" {
                        true
                    } else {
                        ks.kind_name == kstr
                    };
                    if !kind_matches {
                        continue;
                    }
                    if let Some(ref ignore) = ks.config.ignore_pattern {
                        if ignore.is_match(name) {
                            ignored = true;
                            break;
                        }
                    }
                }
            }
            if ignored {
                continue;
            }

            if has_kind_config {
                // Convention checks — report at most one per matching config
                for ks in &self.opts.kind_configs {
                    let kind_matches = if kstr == "Field" && ks.kind_name == "Field" {
                        true
                    } else {
                        ks.kind_name == kstr
                    };
                    if !kind_matches {
                        continue;
                    }
                    let selector_matches = match &ks.predicate {
                        Some(SelectorPredicate::ParentNameEquals(val)) => {
                            ni.parent_name.as_deref() == Some(val.as_str())
                        }
                        Some(SelectorPredicate::ParentNameNotEquals(val)) => {
                            ni.parent_name.as_deref() != Some(val.as_str())
                        }
                        Some(SelectorPredicate::GqlTypeNameEquals(val)) => {
                            ctx.schema.and_then(|s| {
                                let parent = ni.parent_name.as_ref()?;
                                s.type_field(parent, name).ok().map(|td| {
                                    td.node.ty.inner_named_type().to_string() == *val
                                })
                            }).unwrap_or(false)
                        }
                        Some(SelectorPredicate::GqlTypeGqlTypeNameEquals(val)) => {
                            ctx.schema.and_then(|s| {
                                let parent = ni.parent_name.as_ref()?;
                                s.type_field(parent, name).ok().map(|td| {
                                    td.node.ty.inner_named_type().to_string() == *val
                                })
                            }).unwrap_or(false)
                        }
                        None => true,
                    };
                    if !selector_matches {
                        continue;
                    }

                    let config = &ks.config;

                    let label = node_label(kind, ni, &source);

                    if let Some(ref style) = config.style {
                        if !is_case_loose(&check_name, style.0) {
                            let converted = convert_case(name, style.0, &[]);
                            ctx.report(
                                DiagnosticBuilder::new(
                                    rule_id, path.clone(), span,
                                    format!("{label} \"{name}\" should be in {} format", style.1),
                                )
                                .suggestion(
                                    format!("Convert to {}", style.1),
                                    Fix::Replace { span, text: converted },
                                ),
                            );
                            break;
                        }
                    }

                    if let Some(ref prefix) = config.prefix {
                        if !check_name.starts_with(prefix.as_str()) {
                            ctx.report(DiagnosticBuilder::new(
                                rule_id, path.clone(), span,
                                format!("{label} \"{name}\" should have \"{prefix}\" prefix"),
                            ));
                            break;
                        }
                    }

                    if let Some(ref suffix) = config.suffix {
                        if !suffix.is_empty() && !check_name.ends_with(suffix.as_str()) {
                            ctx.report(DiagnosticBuilder::new(
                                rule_id, path.clone(), span,
                                format!("{label} \"{name}\" should have \"{suffix}\" suffix"),
                            ));
                            break;
                        }
                    }

                    for fp in &config.forbidden_prefixes {
                        if name.starts_with(fp.as_str()) {
                            ctx.report(DiagnosticBuilder::new(
                                rule_id, path.clone(), span,
                                format!("{label} \"{name}\" should not have \"{fp}\" prefix"),
                            ));
                            break;
                        }
                    }

                    for fs in &config.forbidden_suffixes {
                        if name.ends_with(fs.as_str()) {
                            ctx.report(DiagnosticBuilder::new(
                                rule_id, path.clone(), span,
                                format!("{label} \"{name}\" should not have \"{fs}\" suffix"),
                            ));
                            break;
                        }
                    }

                    if !config.required_prefixes.is_empty() {
                        let has = config.required_prefixes.iter().any(|rp| check_name.starts_with(rp.as_str()));
                        if !has {
                            ctx.report(DiagnosticBuilder::new(
                                rule_id, path.clone(), span,
                                format!("{label} \"{name}\" should have \"{}\" prefix", config.required_prefixes[0]),
                            ));
                            break;
                        }
                    }

                    if !config.required_suffixes.is_empty() {
                        let has = config.required_suffixes.iter().any(|rs| check_name.ends_with(rs.as_str()));
                        if !has {
                            ctx.report(DiagnosticBuilder::new(
                                rule_id, path.clone(), span,
                                format!("{label} \"{name}\" should have \"{}\" suffix", config.required_suffixes[0]),
                            ));
                            break;
                        }
                    }

                    for fp in &config.forbidden_patterns {
                        if fp.is_match(name) {
                            ctx.report(DiagnosticBuilder::new(
                                rule_id, path.clone(), span,
                                format!("{label} \"{name}\" should not match pattern \"{fp}\""),
                            ));
                            break;
                        }
                    }

                    if let Some(ref rp) = config.required_pattern {
                        if !rp.is_match(name) {
                            ctx.report(DiagnosticBuilder::new(
                                rule_id, path.clone(), span,
                                format!("{label} \"{name}\" should match pattern \"{rp}\""),
                            ));
                            break;
                        }
                    }
                }
            }

            // Global underscore checks (always run, independent of convention checks)
            if !self.opts.allow_leading_underscore && name.starts_with('_') {
                ctx.report(DiagnosticBuilder::new(
                    rule_id, path.clone(), span,
                    "Leading underscores are not allowed".to_string(),
                ));
            }
            if !self.opts.allow_trailing_underscore && name.ends_with('_') {
                ctx.report(DiagnosticBuilder::new(
                    rule_id, path.clone(), span,
                    "Trailing underscores are not allowed".to_string(),
                ));
            }
        }
    }
}

fn is_case_loose(name: &str, style: CaseStyle) -> bool {
    if name.len() <= 1 {
        return match style {
            CaseStyle::Camel => name.chars().next().is_some_and(|c| c.is_ascii_lowercase()),
            CaseStyle::Pascal | CaseStyle::StrictPascal => {
                name.chars().next().is_some_and(|c| c.is_ascii_uppercase())
            }
            CaseStyle::Snake | CaseStyle::Kebab => true,
            CaseStyle::ScreamingSnake | CaseStyle::ScreamingKebab => {
                name.chars().next().is_some_and(|c| c.is_ascii_uppercase())
            }
        };
    }
    let detected = crate::shared::detect_case(name);
    match style {
        CaseStyle::Pascal => {
            detected == Some(CaseStyle::Pascal) || detected == Some(CaseStyle::StrictPascal)
        }
        CaseStyle::StrictPascal => detected == Some(CaseStyle::StrictPascal),
        CaseStyle::ScreamingSnake => {
            detected == Some(CaseStyle::ScreamingSnake)
                || (!name.contains('-')
                    && name.chars().filter(|c| c.is_ascii_alphabetic()).all(|c| c.is_ascii_uppercase())
                    && name.chars().any(|c| c.is_ascii_uppercase()))
        }
        CaseStyle::ScreamingKebab => {
            detected == Some(CaseStyle::ScreamingKebab)
                || (!name.contains('_')
                    && name.chars().filter(|c| c.is_ascii_alphabetic()).all(|c| c.is_ascii_uppercase())
                    && name.chars().any(|c| c.is_ascii_uppercase()))
        }
        _ => detected == Some(style),
    }
}

fn node_label(kind: SyntaxKind, ni: &NodeInfo, source: &str) -> String {
    match kind {
        SyntaxKind::OPERATION_DEFINITION => operation_type_label(source, ni.span),
        SyntaxKind::FIELD => {
            if let Some(ref parent) = ni.parent_name {
                format!("field \"{parent}\"")
            } else {
                "field".to_string()
            }
        }
        _ => {
            let d = display_name(kind);
            let first = d.chars().next().map(|c| c.to_ascii_uppercase()).unwrap_or('?');
            let rest = &d[1..];
            format!("{}{}", first, rest)
        }
    }
}

fn operation_type_label(source: &str, span: Span) -> String {
    let at_span = source[span.offset..].trim_start();
    let first_word = at_span.split_whitespace().next().unwrap_or("operation");
    let op_type = match first_word {
        "query" | "mutation" | "subscription" => first_word,
        _ => "operation",
    };
    let first = op_type.chars().next().map(|c| c.to_ascii_uppercase()).unwrap_or('O');
    let rest = &op_type[1..];
    format!("{}{}", first, rest)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rglint_core::{Category, Rule, Severity};

    #[test]
    fn rule_meta_matches_spec_028() {
        let rule = NamingConvention;
        let meta = rule.meta();
        assert_eq!(meta.id, "naming-convention");
        assert_eq!(meta.category, Category::Schema);
        assert_eq!(meta.severity, Severity::Warn);
    }
}
