//! JSON-Schema validation for normalized rule options (spec-056).
//!
//! Configuration parsing deliberately keeps rule options as raw JSON. This
//! module is the boundary where a caller that has a rule registry can validate
//! those values without making the file-facing config model depend on the
//! built-in rule crate.

use ahash::AHashMap;
use rglint_core::{RuleEntry, RuleMeta, Severity};
use serde_json::Value;

use crate::schema::{Config, ConfigError};

/// One JSON-Schema validation failure for a configured rule.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuleOptionError {
    /// The configured rule id.
    pub rule_id: String,
    /// JSON Pointer to the offending option, such as `/maxDepth`.
    pub schema_path: String,
    /// Human-readable detail from the JSON-Schema validator.
    pub message: String,
}

/// Validate one rule's options, applying its shallow defaults first.
///
/// Rules without an option schema intentionally accept any JSON value. A
/// malformed or missing schema is treated the same way by `RuleMeta`'s lazy
/// accessor; rule metadata is static and is expected to be validated by the
/// rule crate's own tests.
pub fn validate_rule_options(meta: &RuleMeta, options: &Value) -> Result<(), Vec<RuleOptionError>> {
    let Some(schema) = meta.option_schema() else {
        return Ok(());
    };

    let mut effective = options.clone();
    apply_defaults(meta, &mut effective);

    let Err(errors) = schema.validate(&effective) else {
        return Ok(());
    };

    Err(errors
        .map(|error| RuleOptionError {
            rule_id: meta.id.to_owned(),
            schema_path: validation_path(&error),
            message: error.to_string(),
        })
        .collect())
}

/// Apply a rule's default option object using a shallow, user-wins merge.
///
/// Non-object option values are left unchanged so the schema validator can
/// report the useful type error. Defaults are only metadata for validation;
/// this helper does not mutate the normalized `Config` unless the caller
/// passes a mutable option value directly.
pub fn apply_defaults(meta: &RuleMeta, options: &mut Value) {
    let (Value::Object(options), Some(Value::Object(defaults))) = (options, meta.default_options())
    else {
        return;
    };

    for (key, value) in defaults {
        options.entry(key.clone()).or_insert_with(|| value.clone());
    }
}

/// Validate every non-disabled rule that is present in the supplied registry.
///
/// Unknown ids remain loadable for forward compatibility and are skipped here;
/// the engine's registry resolution remains responsible for reporting them.
/// Validation failures from all known rules are returned together in one
/// `ConfigError`.
impl Config {
    pub fn validate(&self, rules: &[&RuleEntry]) -> Result<(), ConfigError> {
        let mut errors = Vec::new();

        validate_rule_map(&self.rules, rules, &mut errors);
        for project in &self.projects {
            if let Some(project_rules) = &project.rules {
                validate_rule_map(project_rules, rules, &mut errors);
            }
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(ConfigError::InvalidRuleOptions { errors })
        }
    }
}

fn validate_rule_map(
    configured: &AHashMap<String, (Severity, Value)>,
    rules: &[&RuleEntry],
    errors: &mut Vec<RuleOptionError>,
) {
    for (rule_id, (severity, options)) in configured {
        if *severity == Severity::Off {
            continue;
        }

        let Some(entry) = rules.iter().find(|entry| entry.meta.id == rule_id) else {
            continue;
        };

        if let Err(mut rule_errors) = validate_rule_options(entry.meta, options) {
            errors.append(&mut rule_errors);
        }
    }
}

fn validation_path(error: &jsonschema::ValidationError<'_>) -> String {
    let instance_path = error.instance_path.to_string();
    if !instance_path.is_empty() {
        return instance_path;
    }

    if let jsonschema::error::ValidationErrorKind::Required { property } = &error.kind {
        if let Some(property) = property.as_str() {
            return format!("/{}", escape_json_pointer(property));
        }
    }

    match &error.kind {
        jsonschema::error::ValidationErrorKind::AdditionalProperties { unexpected }
        | jsonschema::error::ValidationErrorKind::UnevaluatedProperties { unexpected } => {
            if let Some(property) = unexpected.first() {
                return format!("/{}", escape_json_pointer(property));
            }
        }
        _ => {}
    }

    "/".to_owned()
}

fn escape_json_pointer(value: &str) -> String {
    value.replace('~', "~0").replace('/', "~1")
}

#[cfg(test)]
mod tests {
    use super::*;
    use rglint_core::{Category, Handler, Rule, RuleContext, RuleEntry, RuleMeta};

    struct TestRule;

    impl Rule for TestRule {
        fn meta(&self) -> &'static RuleMeta {
            &TEST_META
        }

        fn create(&self, _ctx: &mut RuleContext) -> Box<dyn Handler> {
            Box::new(TestHandler)
        }
    }

    struct TestHandler;
    impl Handler for TestHandler {}

    static TEST_META: RuleMeta = RuleMeta::new(
        "test-options",
        Category::Other,
        Severity::Warn,
        "",
        Some(
            r#"{
                "type": "object",
                "properties": {"maxDepth": {"type": "integer"}},
                "required": ["maxDepth"],
                "additionalProperties": false
            }"#,
        ),
        Some(r#"{"maxDepth": 3}"#),
        false,
        false,
        false,
        None,
        false,
    );

    static TEST_ENTRY: RuleEntry = RuleEntry {
        meta: &TEST_META,
        factory: || Box::new(TestRule),
        interested_kinds: &[],
    };

    #[test]
    fn valid_options_pass() {
        assert!(validate_rule_options(&TEST_META, &serde_json::json!({"maxDepth": 7})).is_ok());
    }

    #[test]
    fn invalid_options_report_rule_and_path() {
        let errors = validate_rule_options(
            &TEST_META,
            &serde_json::json!({"maxDepth": "x", "other": true}),
        )
        .expect_err("invalid options must fail");

        assert_eq!(errors.len(), 2);
        assert!(errors.iter().all(|error| error.rule_id == "test-options"));
        assert!(errors.iter().any(|error| error.schema_path == "/maxDepth"));
        assert!(errors.iter().any(|error| error.schema_path == "/other"));
    }

    #[test]
    fn defaults_are_applied_before_validation() {
        let mut options = serde_json::json!({});
        apply_defaults(&TEST_META, &mut options);
        assert_eq!(options, serde_json::json!({"maxDepth": 3}));
        assert!(validate_rule_options(&TEST_META, &serde_json::json!({})).is_ok());
    }

    #[test]
    fn no_schema_accepts_any_options() {
        let meta = RuleMeta::new(
            "freeform",
            Category::Other,
            Severity::Warn,
            "",
            None,
            None,
            false,
            false,
            false,
            None,
            false,
        );
        assert!(validate_rule_options(&meta, &serde_json::json!("anything")).is_ok());
    }

    #[test]
    fn config_validation_batches_known_rule_failures_and_skips_off_rules() {
        let config = Config {
            projects: Vec::new(),
            rules: [
                (
                    "test-options".to_owned(),
                    (Severity::Error, serde_json::json!({"maxDepth": "x"})),
                ),
                (
                    "test-options-off".to_owned(),
                    (Severity::Off, serde_json::json!({"maxDepth": "x"})),
                ),
                (
                    "future-rule".to_owned(),
                    (Severity::Error, serde_json::json!({"anything": true})),
                ),
            ]
            .into_iter()
            .collect(),
            ignore: Vec::new(),
            format: crate::Format::Pretty,
        };

        let error = config
            .validate(&[&TEST_ENTRY])
            .expect_err("invalid options must be returned");
        match error {
            ConfigError::InvalidRuleOptions { errors } => {
                assert_eq!(errors.len(), 1);
                assert_eq!(errors[0].rule_id, "test-options");
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[test]
    fn config_validation_checks_project_local_options() {
        let config = Config {
            projects: vec![crate::ProjectConfigRaw {
                name: "project".to_owned(),
                schema: None,
                documents: None,
                ignore: Vec::new(),
                rules: Some(
                    [(
                        "test-options".to_owned(),
                        (Severity::Error, serde_json::json!({"maxDepth": "x"})),
                    )]
                    .into_iter()
                    .collect(),
                ),
            }],
            rules: Default::default(),
            ignore: Vec::new(),
            format: crate::Format::Pretty,
        };

        let error = config
            .validate(&[&TEST_ENTRY])
            .expect_err("project-local invalid options must be returned");
        assert!(matches!(error, ConfigError::InvalidRuleOptions { .. }));
    }
}
