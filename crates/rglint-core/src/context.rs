//! The per-rule, per-document handle that rules receive in `Rule::create` and
//! `Handler::finalize`.
//!
//! spec-008 lands a minimal placeholder so the `Rule` / `Handler` trait
//! signatures compile; spec-009 implements the real body — `report()`,
//! `require_schema`, `require_operations`, `option::<T>()`, and the owned
//! diagnostics buffer — by replacing this struct. Keeping the type in core
//! lets both `rglint-rules` and `rglint-graphql-spec` reference it without
//! depending on each other.

use std::marker::PhantomData;

/// Per-rule, per-document rule execution context.
///
/// spec-008 deliberately leaves the body empty (the lifetime is carried via a
/// [`PhantomData`]); spec-009 populates it with the real fields
/// (`file`/`schema`/`siblings`/`project`/`options`) and the `report`,
/// `require_*`, `option::<T>`, and `take_diagnostics` methods. It exists here
/// only so `Rule::create(&self, ctx: &mut RuleContext)` and
/// `Handler::finalize(&mut self, ctx: &mut RuleContext)` type-check.
pub struct RuleContext<'a> {
    _phantom: PhantomData<&'a ()>,
}
