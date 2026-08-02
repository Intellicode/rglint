//! Configuration loading and normalization for `.rglintrc` files.

pub mod graphql_config;
pub mod schema;

pub use graphql_config::{discover_graphql_config, load_graphql_config};
pub use schema::{
    discover, load, Config, ConfigError, DocumentSpecRaw, Format, ProjectConfigRaw, RuleConfig,
    SchemaSpecRaw,
};
