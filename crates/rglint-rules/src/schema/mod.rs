//! Schema-category rules: rules over SDL / type-system definitions (spec-019
//! onwards).
//!
//! Each rule lives in its own submodule so its registration (a `#[derive(Rule)]`
//! submission into `rglint_core::ALL_RULES` via `linkme`) and tests stay
//! self-contained.

pub mod alphabetize;
pub mod description_style;
pub mod no_duplicate_fields;
pub mod no_hashtag_description;
pub mod require_deprecation_reason;
pub mod require_description;
