//! Schema-category rules: rules over SDL / type-system definitions (spec-019
//! onwards).
//!
//! Each rule lives in its own submodule so its registration (a `#[derive(Rule)]`
//! submission into `rglint_core::ALL_RULES` via `linkme`) and tests stay
//! self-contained.

pub mod alphabetize;
pub mod description_style;
pub mod naming_convention;
pub mod no_deprecated;
pub mod no_duplicate_fields;
pub mod no_hashtag_description;
pub mod no_scalar_result_type_on_mutation;
pub mod no_root_type;
pub mod no_unreachable_types;
pub mod no_unused_fields;
pub mod no_typename_prefix;
pub mod require_deprecation_date;
pub mod require_deprecation_reason;
pub mod require_description;
pub mod require_field_of_type_query_in_mutation_result;
pub mod require_nullable_result_in_root;
pub mod require_selections;
pub mod strict_id_in_types;
pub mod unique_enum_value_names;
