//! Configuration loading and normalization for `.rglintrc` files.

pub mod schema;

pub use schema::{
    discover, load, Config, ConfigError, DocumentSpecRaw, Format, ProjectConfigRaw, RuleConfig,
    SchemaSpecRaw,
};
