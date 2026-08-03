//! Built-in rule presets (spec-063).
//!
//! The rule ids and option objects in this module are copied from the pinned
//! graphql-eslint config snapshot recorded in `specs/spec-063.md`. Presets are
//! deliberately defined in the config crate without depending on a rule crate;
//! the CLI supplies the registry when it validates and runs them.

use ahash::AHashMap;
use rglint_core::Severity;
use serde_json::{json, Value};

use crate::schema::{Config, Format};

/// The immutable upstream revision used for this preset table.
pub const UPSTREAM_REVISION: &str = "f0f200ef0b030cb8a905bbcb32fe346b87cc2e24";

/// Return the schema-recommended preset.
pub fn schema_recommended() -> Config {
    resolve_public("schema-recommended")
}

/// Return the operations-recommended preset.
pub fn operations_recommended() -> Config {
    resolve_public("operations-recommended")
}

/// Return the Relay schema preset.
pub fn schema_relay() -> Config {
    resolve_public("schema-relay")
}

/// Return all schema rules, including schema-recommended.
pub fn schema_all() -> Config {
    resolve_public("schema-all")
}

/// Return all operations rules, including operations-recommended.
pub fn operations_all() -> Config {
    resolve_public("operations-all")
}

/// Return the union of schema-recommended and operations-recommended.
pub fn recommended() -> Config {
    resolve_public("recommended")
}

/// Return the union of schema-all and operations-all.
pub fn all() -> Config {
    resolve_public("all")
}

/// Resolve a name from a config file. This is crate-visible so the file loader
/// can turn unknown names and inheritance cycles into `ConfigError`s.
pub(crate) fn resolve(name: &str, stack: &mut Vec<String>) -> Result<Config, String> {
    if let Some(index) = stack.iter().position(|entry| entry == name) {
        let mut cycle = stack[index..].to_vec();
        cycle.push(name.to_owned());
        return Err(format!("preset inheritance cycle: {}", cycle.join(" -> ")));
    }

    let parents = match name {
        "schema-all" => &["schema-recommended"][..],
        "operations-all" => &["operations-recommended"][..],
        "recommended" => &["schema-recommended", "operations-recommended"][..],
        "all" => &["schema-all", "operations-all"][..],
        "schema-recommended" | "operations-recommended" | "schema-relay" => &[][..],
        _ => return Err(format!("unknown preset `{name}`")),
    };

    stack.push(name.to_owned());
    let mut result = empty_config();
    for parent in parents {
        let parent_config = resolve(parent, stack)?;
        result.extend_with(parent_config);
    }
    result.extend_with(direct(name));
    stack.pop();
    Ok(result)
}

fn resolve_public(name: &str) -> Config {
    resolve(name, &mut Vec::new()).expect("built-in preset graph must be valid")
}

fn empty_config() -> Config {
    Config {
        projects: Vec::new(),
        rules: AHashMap::new(),
        ignore: Vec::new(),
        format: Format::default(),
    }
}

fn direct(name: &str) -> Config {
    let mut rules = AHashMap::new();

    match name {
        "schema-recommended" => {
            add(&mut rules, "description-style", json!({}));
            add(&mut rules, "known-argument-names", json!({}));
            add(&mut rules, "known-directives", json!({}));
            add(&mut rules, "known-type-names", json!({}));
            add(&mut rules, "lone-schema-definition", json!({}));
            add(
                &mut rules,
                "naming-convention",
                json!({
                    "types": "PascalCase",
                    "FieldDefinition": "camelCase",
                    "InputValueDefinition": "camelCase",
                    "Argument": "camelCase",
                    "DirectiveDefinition": "camelCase",
                    "EnumValueDefinition": "UPPER_CASE",
                    "FieldDefinition[parent.name.value=Query]": {
                        "forbiddenPrefixes": ["query", "get"],
                        "forbiddenSuffixes": ["Query"]
                    },
                    "FieldDefinition[parent.name.value=Mutation]": {
                        "forbiddenPrefixes": ["mutation"],
                        "forbiddenSuffixes": ["Mutation"]
                    },
                    "FieldDefinition[parent.name.value=Subscription]": {
                        "forbiddenPrefixes": ["subscription"],
                        "forbiddenSuffixes": ["Subscription"]
                    },
                    "EnumTypeDefinition,EnumTypeExtension": {
                        "forbiddenPrefixes": ["Enum"],
                        "forbiddenSuffixes": ["Enum"]
                    },
                    "InterfaceTypeDefinition,InterfaceTypeExtension": {
                        "forbiddenPrefixes": ["Interface"],
                        "forbiddenSuffixes": ["Interface"]
                    },
                    "UnionTypeDefinition,UnionTypeExtension": {
                        "forbiddenPrefixes": ["Union"],
                        "forbiddenSuffixes": ["Union"]
                    },
                    "ObjectTypeDefinition,ObjectTypeExtension": {
                        "forbiddenPrefixes": ["Type"],
                        "forbiddenSuffixes": ["Type"]
                    }
                }),
            );
            add(&mut rules, "no-hashtag-description", json!({}));
            add(&mut rules, "no-typename-prefix", json!({}));
            add(&mut rules, "no-unreachable-types", json!({}));
            add(&mut rules, "possible-type-extension", json!({}));
            add(&mut rules, "provided-required-arguments", json!({}));
            add(&mut rules, "require-deprecation-reason", json!({}));
            add(
                &mut rules,
                "require-description",
                json!({"types": true, "DirectiveDefinition": true, "rootField": true}),
            );
            add(&mut rules, "strict-id-in-types", json!({}));
            add(&mut rules, "unique-directive-names", json!({}));
            add(&mut rules, "unique-directive-names-per-location", json!({}));
            add(&mut rules, "unique-enum-value-names", json!({}));
            add(&mut rules, "unique-field-definition-names", json!({}));
            add(&mut rules, "unique-operation-types", json!({}));
            add(&mut rules, "unique-type-names", json!({}));
        }
        "operations-recommended" => {
            add(&mut rules, "executable-definitions", json!({}));
            add(&mut rules, "fields-on-correct-type", json!({}));
            add(&mut rules, "fragments-on-composite-type", json!({}));
            add(&mut rules, "known-argument-names", json!({}));
            add(&mut rules, "known-directives", json!({}));
            add(&mut rules, "known-fragment-names", json!({}));
            add(&mut rules, "known-type-names", json!({}));
            add(&mut rules, "lone-anonymous-operation", json!({}));
            add(
                &mut rules,
                "naming-convention",
                json!({
                    "VariableDefinition": "camelCase",
                    "OperationDefinition": {
                        "style": "PascalCase",
                        "forbiddenPrefixes": ["Query", "Mutation", "Subscription", "Get"],
                        "forbiddenSuffixes": ["Query", "Mutation", "Subscription"]
                    },
                    "FragmentDefinition": {
                        "style": "PascalCase",
                        "forbiddenPrefixes": ["Fragment"],
                        "forbiddenSuffixes": ["Fragment"]
                    }
                }),
            );
            add(&mut rules, "no-anonymous-operations", json!({}));
            add(&mut rules, "no-deprecated", json!({}));
            add(&mut rules, "no-duplicate-fields", json!({}));
            add(&mut rules, "no-fragment-cycles", json!({}));
            add(&mut rules, "no-undefined-variables", json!({}));
            add(&mut rules, "no-unused-fragments", json!({}));
            add(&mut rules, "no-unused-variables", json!({}));
            add(&mut rules, "one-field-subscriptions", json!({}));
            add(&mut rules, "overlapping-fields-can-be-merged", json!({}));
            add(&mut rules, "possible-fragment-spread", json!({}));
            add(&mut rules, "provided-required-arguments", json!({}));
            add(&mut rules, "require-selections", json!({}));
            add(&mut rules, "scalar-leafs", json!({}));
            add(&mut rules, "selection-set-depth", json!({"maxDepth": 7}));
            add(&mut rules, "unique-argument-names", json!({}));
            add(&mut rules, "unique-directive-names-per-location", json!({}));
            add(&mut rules, "unique-fragment-name", json!({}));
            add(&mut rules, "unique-input-field-names", json!({}));
            add(&mut rules, "unique-operation-name", json!({}));
            add(&mut rules, "unique-variable-names", json!({}));
            add(&mut rules, "value-literals-of-correct-type", json!({}));
            add(&mut rules, "variables-are-input-types", json!({}));
            add(&mut rules, "variables-in-allowed-position", json!({}));
        }
        "schema-relay" => {
            add(&mut rules, "relay-arguments", json!({}));
            add(&mut rules, "relay-connection-types", json!({}));
            add(&mut rules, "relay-edge-types", json!({}));
            add(&mut rules, "relay-page-info", json!({}));
        }
        "schema-all" => {
            add(
                &mut rules,
                "alphabetize",
                json!({
                    "definitions": true,
                    "fields": ["ObjectTypeDefinition", "InterfaceTypeDefinition", "InputObjectTypeDefinition"],
                    "values": true,
                    "arguments": ["FieldDefinition", "Field", "DirectiveDefinition", "Directive"],
                    "groups": ["...", "id", "*", "createdAt", "updatedAt"]
                }),
            );
            add(&mut rules, "input-name", json!({}));
            add(
                &mut rules,
                "no-root-type",
                json!({"disallow": ["mutation", "subscription"]}),
            );
            add(&mut rules, "no-scalar-result-type-on-mutation", json!({}));
            add(&mut rules, "require-deprecation-date", json!({}));
            add(
                &mut rules,
                "require-field-of-type-query-in-mutation-result",
                json!({}),
            );
            add(&mut rules, "require-nullable-fields-with-oneof", json!({}));
            add(&mut rules, "require-nullable-result-in-root", json!({}));
            add(&mut rules, "require-type-pattern-with-oneof", json!({}));
        }
        "operations-all" => {
            add(
                &mut rules,
                "alphabetize",
                json!({
                    "definitions": true,
                    "selections": ["OperationDefinition", "FragmentDefinition"],
                    "variables": true,
                    "arguments": ["Field", "Directive"],
                    "groups": ["...", "id", "*", "{"]
                }),
            );
            add(&mut rules, "lone-executable-definition", json!({}));
            add(
                &mut rules,
                "match-document-filename",
                json!({
                    "query": "kebab-case",
                    "mutation": "kebab-case",
                    "subscription": "kebab-case",
                    "fragment": "kebab-case"
                }),
            );
            add(&mut rules, "no-one-place-fragments", json!({}));
            add(&mut rules, "require-import-fragment", json!({}));
        }
        "recommended" | "all" => {}
        _ => unreachable!("direct() called for unknown preset"),
    }

    Config {
        projects: Vec::new(),
        rules,
        ignore: Vec::new(),
        format: Format::default(),
    }
}

fn add(rules: &mut AHashMap<String, (Severity, Value)>, id: &str, options: Value) {
    rules.insert(id.to_owned(), (Severity::Error, options));
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;

    fn ids(config: &Config) -> BTreeMap<String, Severity> {
        config
            .rules
            .iter()
            .map(|(id, (severity, _))| (id.clone(), *severity))
            .collect()
    }

    fn expected(items: &[&str]) -> BTreeMap<String, Severity> {
        items
            .iter()
            .map(|id| ((*id).to_owned(), Severity::Error))
            .collect()
    }

    #[test]
    fn preset_rule_sets_match_pinned_upstream_snapshot() {
        assert_eq!(
            ids(&schema_recommended()),
            expected(&[
                "description-style",
                "known-argument-names",
                "known-directives",
                "known-type-names",
                "lone-schema-definition",
                "naming-convention",
                "no-hashtag-description",
                "no-typename-prefix",
                "no-unreachable-types",
                "possible-type-extension",
                "provided-required-arguments",
                "require-deprecation-reason",
                "require-description",
                "strict-id-in-types",
                "unique-directive-names",
                "unique-directive-names-per-location",
                "unique-enum-value-names",
                "unique-field-definition-names",
                "unique-operation-types",
                "unique-type-names",
            ])
        );
        assert_eq!(
            ids(&operations_recommended()),
            expected(&[
                "executable-definitions",
                "fields-on-correct-type",
                "fragments-on-composite-type",
                "known-argument-names",
                "known-directives",
                "known-fragment-names",
                "known-type-names",
                "lone-anonymous-operation",
                "naming-convention",
                "no-anonymous-operations",
                "no-deprecated",
                "no-duplicate-fields",
                "no-fragment-cycles",
                "no-undefined-variables",
                "no-unused-fragments",
                "no-unused-variables",
                "one-field-subscriptions",
                "overlapping-fields-can-be-merged",
                "possible-fragment-spread",
                "provided-required-arguments",
                "require-selections",
                "scalar-leafs",
                "selection-set-depth",
                "unique-argument-names",
                "unique-directive-names-per-location",
                "unique-fragment-name",
                "unique-input-field-names",
                "unique-operation-name",
                "unique-variable-names",
                "value-literals-of-correct-type",
                "variables-are-input-types",
                "variables-in-allowed-position",
            ])
        );
        assert_eq!(
            ids(&schema_relay()),
            expected(&[
                "relay-arguments",
                "relay-connection-types",
                "relay-edge-types",
                "relay-page-info",
            ])
        );
        assert_eq!(ids(&schema_all()).len(), 29);
        assert_eq!(ids(&operations_all()).len(), 37);
        assert_eq!(ids(&recommended()).len(), 46);
        assert_eq!(ids(&all()).len(), 59);
    }

    #[test]
    fn option_objects_match_upstream() {
        assert_eq!(
            schema_all().rules["no-root-type"].1,
            json!({"disallow": ["mutation", "subscription"]})
        );
        assert_eq!(
            operations_recommended().rules["selection-set-depth"].1,
            json!({"maxDepth": 7})
        );
        assert_eq!(
            operations_all().rules["match-document-filename"].1,
            json!({
                "query": "kebab-case",
                "mutation": "kebab-case",
                "subscription": "kebab-case",
                "fragment": "kebab-case"
            })
        );
    }

    #[test]
    fn resolver_detects_a_cycle_before_recursing() {
        let error = resolve("schema-all", &mut vec!["schema-all".to_owned()]).unwrap_err();
        assert!(error.contains("preset inheritance cycle"));
    }
}
