//! The `Rule` derive (spec-008). See [`rule_derive`].

use proc_macro2::Span;
use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::{
    parse_macro_input, punctuated::Punctuated, DeriveInput, Expr, ExprLit, Ident, Lit,
    MetaNameValue, Token,
};

/// Entry point invoked by the thin `#[proc_macro_derive]` shim in `lib.rs`.
pub fn rule_derive_impl(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    match expand(&input) {
        Ok(ts) => ts.into(),
        Err(e) => e.to_compile_error().into(),
    }
}

struct Attrs {
    id: String,
    category: String,
    severity: Option<String>,
    docs: Option<String>,
    option_schema: Option<String>,
    default_options: Option<String>,
    requires_schema: Option<bool>,
    requires_siblings: Option<bool>,
    deprecated: Option<bool>,
    replaced_by: Option<String>,
    has_suggestions: Option<bool>,
}

impl Attrs {
    const ALLOWED: &'static [&'static str] = &[
        "id",
        "category",
        "severity",
        "docs",
        "option_schema",
        "default_options",
        "requires_schema",
        "requires_siblings",
        "deprecated",
        "replaced_by",
        "has_suggestions",
    ];
}

fn expand(input: &DeriveInput) -> syn::Result<TokenStream> {
    let struct_name = &input.ident;
    let attrs = parse_attrs(input)?;

    let id = &attrs.id;
    let category = category_variant(&attrs.category)?;
    let severity = severity_expr(attrs.severity.as_deref())?;
    let docs = attrs.docs.as_deref().unwrap_or("");
    let option_schema_src = opt_str_lit(attrs.option_schema.as_deref());
    let default_options_src = opt_str_lit(attrs.default_options.as_deref());
    let requires_schema = attrs.requires_schema.unwrap_or(false);
    let requires_siblings = attrs.requires_siblings.unwrap_or(false);
    let deprecated = attrs.deprecated.unwrap_or(false);
    let replaced_by = opt_str_lit(attrs.replaced_by.as_deref());
    let has_suggestions = attrs.has_suggestions.unwrap_or(false);

    let static_suffix = struct_name.to_string().to_uppercase();
    let meta_ident = format_ident!("__RG_RULE_META_{}", static_suffix);
    let entry_ident = format_ident!("__RG_RULE_ENTRY_{}", static_suffix);

    Ok(quote! {
        static #meta_ident: rglint_core::RuleMeta = rglint_core::RuleMeta::new(
            #id,
            rglint_core::Category::#category,
            #severity,
            #docs,
            #option_schema_src,
            #default_options_src,
            #requires_schema,
            #requires_siblings,
            #deprecated,
            #replaced_by,
            #has_suggestions,
        );

        impl rglint_core::Rule for #struct_name {
            fn meta(&self) -> &'static rglint_core::RuleMeta {
                &#meta_ident
            }
            fn create<'s>(
                &'s self,
                ctx: &'s mut rglint_core::RuleContext,
            ) -> std::boxed::Box<dyn rglint_core::Handler + 's> {
                #struct_name::handler(self, ctx)
            }
        }

        #[linkme::distributed_slice(rglint_core::ALL_RULES)]
        static #entry_ident: rglint_core::RuleEntry = rglint_core::RuleEntry {
            meta: &#meta_ident,
            factory: || -> std::boxed::Box<dyn rglint_core::Rule> {
                std::boxed::Box::new(#struct_name)
            },
            interested_kinds: &[],
        };
    })
}

fn parse_attrs(input: &DeriveInput) -> syn::Result<Attrs> {
    let mut attrs = Attrs {
        id: String::new(),
        category: String::new(),
        severity: None,
        docs: None,
        option_schema: None,
        default_options: None,
        requires_schema: None,
        requires_siblings: None,
        deprecated: None,
        replaced_by: None,
        has_suggestions: None,
    };

    let mut found_rule_attr = false;
    for attr in &input.attrs {
        if !attr.path().is_ident("rule") {
            continue;
        }
        found_rule_attr = true;
        let nested: Punctuated<MetaNameValue, Token![,]> =
            attr.parse_args_with(Punctuated::parse_terminated)?;
        for nv in nested {
            let key = nv
                .path
                .get_ident()
                .ok_or_else(|| syn::Error::new_spanned(&nv.path, "expected an identifier"))?;
            let key_str = key.to_string();
            if !Attrs::ALLOWED.contains(&key_str.as_str()) {
                return Err(syn::Error::new_spanned(
                    &nv.path,
                    format!("unknown `#[rule]` attribute `{key_str}`"),
                ));
            }
            match key_str.as_str() {
                "id" => attrs.id = expect_str(&nv.value, "id")?,
                "category" => attrs.category = expect_str(&nv.value, "category")?,
                "severity" => attrs.severity = Some(expect_str(&nv.value, "severity")?),
                "docs" => attrs.docs = Some(expect_str(&nv.value, "docs")?),
                "option_schema" => {
                    attrs.option_schema = Some(expect_str(&nv.value, "option_schema")?)
                }
                "default_options" => {
                    attrs.default_options = Some(expect_str(&nv.value, "default_options")?)
                }
                "requires_schema" => {
                    attrs.requires_schema = Some(expect_bool(&nv.value, "requires_schema")?)
                }
                "requires_siblings" => {
                    attrs.requires_siblings = Some(expect_bool(&nv.value, "requires_siblings")?)
                }
                "deprecated" => attrs.deprecated = Some(expect_bool(&nv.value, "deprecated")?),
                "replaced_by" => attrs.replaced_by = Some(expect_str(&nv.value, "replaced_by")?),
                "has_suggestions" => {
                    attrs.has_suggestions = Some(expect_bool(&nv.value, "has_suggestions")?)
                }
                _ => unreachable!("validated against ALLOWED"),
            }
        }
    }

    if !found_rule_attr {
        return Err(syn::Error::new_spanned(
            input,
            "`#[derive(Rule)]` requires a `#[rule(id = ..., category = ...)]` attribute",
        ));
    }
    if attrs.id.is_empty() {
        return Err(syn::Error::new_spanned(
            input,
            "`#[rule(...)]` is missing `id = \"...\"`",
        ));
    }
    if attrs.category.is_empty() {
        return Err(syn::Error::new_spanned(
            input,
            "`#[rule(...)]` is missing `category = \"schema\" | \"operations\" | \"other\"`",
        ));
    }
    Ok(attrs)
}

fn category_variant(s: &str) -> syn::Result<Ident> {
    match s {
        "schema" => Ok(Ident::new("Schema", Span::call_site())),
        "operations" => Ok(Ident::new("Operations", Span::call_site())),
        "other" => Ok(Ident::new("Other", Span::call_site())),
        other => Err(syn::Error::new(
            Span::call_site(),
            format!("unknown category `{other}`: expected `schema`, `operations`, or `other`"),
        )),
    }
}

fn severity_expr(s: Option<&str>) -> syn::Result<TokenStream> {
    match s {
        None => Ok(quote! { rglint_core::Severity::Warn }),
        Some("warn") => Ok(quote! { rglint_core::Severity::Warn }),
        Some("error") => Ok(quote! { rglint_core::Severity::Error }),
        Some("off") => Ok(quote! { rglint_core::Severity::Off }),
        Some(other) => Err(syn::Error::new(
            Span::call_site(),
            format!("unknown severity `{other}`: expected `off`, `warn`, or `error`"),
        )),
    }
}

/// Emit `Some("...")` or `None` for an optional `&'static str` field.
fn opt_str_lit(v: Option<&str>) -> TokenStream {
    match v {
        None => quote! { std::option::Option::None },
        Some(s) => quote! { std::option::Option::Some(#s) },
    }
}

fn expect_str(value: &Expr, field: &str) -> syn::Result<String> {
    if let Expr::Lit(ExprLit {
        lit: Lit::Str(s), ..
    }) = value
    {
        Ok(s.value())
    } else {
        Err(syn::Error::new_spanned(
            value,
            format!("`#{{rule}}` `{field}` must be a string literal"),
        ))
    }
}

fn expect_bool(value: &Expr, field: &str) -> syn::Result<bool> {
    if let Expr::Lit(ExprLit {
        lit: Lit::Bool(b), ..
    }) = value
    {
        Ok(b.value)
    } else {
        Err(syn::Error::new_spanned(
            value,
            format!("`#{{rule}}` `{field}` must be a boolean literal"),
        ))
    }
}
