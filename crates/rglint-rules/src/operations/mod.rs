//! Operations-category rules: rules over executable GraphQL documents
//! (queries / mutations / subscriptions / fragments).
//!
//! Each rule lives in its own submodule so its registration (a `#[derive(Rule)]`
//! submission into `rglint_core::ALL_RULES` via `linkme`) and tests stay
//! self-contained. The submodules are annotated `#![allow(dead_code)]` by the
//! top-level `lib.rs` allow so per-rule tests can keep their private handler
//! state under `cfg(test)` without lint noise.

pub mod no_anonymous_operations;
pub mod unique_fragment_name;
