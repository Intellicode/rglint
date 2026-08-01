//! Mapping between Apollo's diagnostic names and graphql-eslint rule ids.
//!
//! Apollo's `unstable_error_name` is the only structured discriminator exposed
//! by the 1.32 API. The mapping deliberately uses that name rather than
//! matching message text. Schema-definition diagnostics are not included when
//! Apollo does not expose a stable discriminator; the corresponding upstream
//! rule ids remain registered and are documented in the parity notes.

/// Return the graphql-eslint rule id represented by an Apollo diagnostic.
pub fn rule_id_for(error: &apollo_compiler::validation::DiagnosticData) -> Option<&'static str> {
    rule_id_for_name(error.unstable_error_name()?)
}

fn rule_id_for_name(name: &str) -> Option<&'static str> {
    match name {
        "TypeSystemDefinition" => Some("executable-definitions"),
        "UndefinedField" => Some("fields-on-correct-type"),
        "InvalidFragmentTarget" => Some("fragments-on-composite-type"),
        "UndefinedArgument" => Some("known-argument-names"),
        "UndefinedDirective" | "UnsupportedLocation" => Some("known-directives"),
        "UndefinedFragment" => Some("known-fragment-names"),
        "UndefinedDefinition"
        | "UndefinedTypeInNamedFragmentTypeCondition"
        | "UndefinedTypeInInlineFragmentTypeCondition" => Some("known-type-names"),
        "AmbiguousAnonymousOperation" => Some("lone-anonymous-operation"),
        "RecursiveFragmentDefinition" => Some("no-fragment-cycles"),
        "UndefinedVariable" => Some("no-undefined-variables"),
        "UnusedFragment" => Some("no-unused-fragments"),
        "UnusedVariable" => Some("no-unused-variables"),
        "ConflictingFieldType" | "ConflictingFieldName" | "ConflictingFieldArgument" => {
            Some("overlapping-fields-can-be-merged")
        }
        "InvalidFragmentSpread" => Some("possible-fragment-spread"),
        "RequiredArgument" => Some("provided-required-arguments"),
        "MissingSubselection" | "SubselectionOnScalarType" | "SubselectionOnEnumType" => {
            Some("scalar-leafs")
        }
        "SubscriptionUsesMultipleFields" => Some("one-field-subscriptions"),
        "UniqueArgument" => Some("unique-argument-names"),
        "UniqueDirective" => Some("unique-directives-per-location"),
        "UniqueInputValue" => Some("unique-input-field-names"),
        "UniqueVariable" => Some("unique-variable-names"),
        "UndefinedEnumValue"
        | "UndefinedInputValue"
        | "UnsupportedValueType"
        | "IntCoercionError"
        | "FloatCoercionError" => Some("value-literals-of-correct-type"),
        "VariableInputType" => Some("variables-are-input-types"),
        "DisallowedVariableUsage" => Some("variables-in-allowed-position"),
        // Apollo 1.32 does not expose stable names for these schema-builder
        // diagnostics: LoneSchemaDefinition, PossibleTypeExtensions,
        // UniqueDirectiveNames, UniqueFieldDefinitionNames,
        // UniqueOperationTypes, and UniqueTypeNames. They are intentionally
        // absent rather than inferred from localized message text.
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn maps_representative_operation_diagnostics() {
        let names = [
            ("UndefinedField", "fields-on-correct-type"),
            ("UniqueVariable", "unique-variable-names"),
            ("DisallowedVariableUsage", "variables-in-allowed-position"),
        ];

        for (name, expected) in names {
            assert_eq!(super::rule_id_for_name(name), Some(expected));
        }
    }
}
