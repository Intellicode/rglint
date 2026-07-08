//! Schema-category rules: rules over SDL / type-system definitions (spec-019
//! onwards).
//!
//! Each rule lives in its own submodule so its registration (a `#[derive(Rule)]`
//! submission into `rglint_core::ALL_RULES` via `linkme`) and tests stay
//! self-contained.

pub mod no_duplicate_fields;
