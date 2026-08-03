//! Configuration loading and normalization for `.rglintrc` files.

pub mod graphql_config;
pub mod preset;
pub mod schema;
pub mod validate;

pub use graphql_config::{discover_graphql_config, load_graphql_config};
pub use schema::{
    discover, load, Config, ConfigError, DocumentSpecRaw, Format, ProjectConfigRaw, RuleConfig,
    SchemaSpecRaw,
};
pub use validate::{apply_defaults, validate_rule_options, RuleOptionError};
