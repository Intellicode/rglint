//! The shared Apollo validation runner and its registry entries.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use apollo_compiler::diagnostic::ToCliReport;
use apollo_compiler::executable::ExecutableDocument;
use apollo_compiler::validation::{DiagnosticList, Valid};
use linkme::distributed_slice;
use rglint_core::{
    Category, DiagnosticBuilder, Handler, Rule, RuleContext, RuleEntry, RuleMeta, Severity, Span,
};

use crate::names::rule_id_for;

/// A rule value shared by every graphql-eslint validation wrapper.
pub struct SpecRule {
    id: &'static str,
    meta: &'static RuleMeta,
    requires_siblings: bool,
}

impl Rule for SpecRule {
    fn meta(&self) -> &'static RuleMeta {
        self.meta
    }

    fn create(&self, _ctx: &mut RuleContext) -> Box<dyn Handler> {
        Box::new(SpecHandler {
            id: self.id,
            requires_siblings: self.requires_siblings,
        })
    }
}

struct SpecHandler {
    id: &'static str,
    requires_siblings: bool,
}

impl Handler for SpecHandler {
    fn finalize(&mut self, ctx: &mut RuleContext) {
        if is_schema_source(ctx) {
            let Some(schema) = ctx.schema else { return };
            let Err(errors) = schema.clone().validate() else {
                return;
            };
            report_errors(self.id, &errors.errors, ctx, ctx.file.source().len());
            return;
        }

        let Some(schema) = ctx.schema else { return };
        let valid_schema = Valid::assume_valid_ref(schema);
        let mut source = ctx.file.source().to_owned();
        let current_len = source.len();
        if self.requires_siblings {
            append_sibling_sources(&mut source, ctx);
        }

        let result = ExecutableDocument::parse_and_validate(valid_schema, source, ctx.file.path());
        if let Err(errors) = result {
            report_errors(self.id, &errors.errors, ctx, current_len);
        }
    }
}

/// Schema sources are not present in the executable sibling index. This lets
/// one handler type support both SDL and executable validation while keeping
/// source ownership explicit and avoiding duplicate reports on every file.
fn is_schema_source(ctx: &RuleContext<'_>) -> bool {
    ctx.siblings
        .map(|siblings| siblings.source_for_file(ctx.file.path()).is_none())
        .unwrap_or(true)
}

fn append_sibling_sources(source: &mut String, ctx: &RuleContext<'_>) {
    let Some(siblings) = ctx.siblings else { return };
    let current = ctx.file.path();
    let mut seen = HashSet::<PathBuf>::new();

    for operation in siblings.operations() {
        append_source_if_needed(source, current, &mut seen, &operation.source);
    }
    for fragment in siblings.fragments_all() {
        append_source_if_needed(source, current, &mut seen, &fragment.source);
    }
}

fn append_source_if_needed(
    source: &mut String,
    current: &Path,
    seen: &mut HashSet<PathBuf>,
    sibling: &rglint_core::SourceFile,
) {
    let path = sibling.path().to_path_buf();
    if path == current || !seen.insert(path) {
        return;
    }
    source.push('\n');
    source.push_str(sibling.source());
}

fn report_errors(
    rule_id: &'static str,
    errors: &DiagnosticList,
    ctx: &mut RuleContext,
    current_source_len: usize,
) {
    for diagnostic in errors.iter() {
        let Some(mapped_id) = rule_id_for(diagnostic.error) else {
            if diagnostic.error.unstable_error_name().is_some() {
                tracing::debug!(
                    rule_id,
                    error = %diagnostic.error,
                    "dropping unmapped Apollo validation diagnostic"
                );
            }
            continue;
        };
        if mapped_id != rule_id {
            continue;
        }

        let span = diagnostic
            .error
            .location()
            .filter(|location| {
                diagnostic
                    .sources
                    .get(&location.file_id())
                    .is_some_and(|source| source.path() == ctx.file.path())
                    && location.offset() <= current_source_len
            })
            .map(|location| Span::new(location.offset(), location.node_len()))
            .unwrap_or_else(|| Span::new(0, 0));

        // The context owns the configured rule id and source path. The builder
        // id is still supplied for readability and is overwritten by report().
        ctx.report(
            DiagnosticBuilder::new(
                mapped_id,
                ctx.file.path().to_path_buf(),
                span,
                diagnostic.error.to_string(),
            )
            .severity(Severity::Error),
        );
    }
}

macro_rules! spec_rule {
    (
        $entry:ident,
        $meta:ident,
        $factory:ident,
        $id:literal,
        $category:expr,
        $requires_schema:expr,
        $requires_siblings:expr,
        $has_suggestions:expr,
        $docs:literal
    ) => {
        pub(crate) static $meta: RuleMeta = RuleMeta::new(
            $id,
            $category,
            Severity::Warn,
            $docs,
            None,
            None,
            $requires_schema,
            $requires_siblings,
            false,
            None,
            $has_suggestions,
        );

        fn $factory() -> Box<dyn Rule> {
            Box::new(SpecRule {
                id: $id,
                meta: &$meta,
                requires_siblings: $requires_siblings,
            })
        }

        #[distributed_slice(rglint_core::ALL_RULES)]
        pub(crate) static $entry: RuleEntry = RuleEntry {
            meta: &$meta,
            factory: $factory,
            interested_kinds: &[],
        };
    };
}

// The ids and metadata are copied from the pinned graphql-eslint source. A
// single category is retained where graphql-eslint declares both categories;
// the runner itself detects SDL versus executable source at runtime.
spec_rule!(
    EXECUTABLE_DEFINITIONS_ENTRY,
    EXECUTABLE_DEFINITIONS_META,
    executable_definitions,
    "executable-definitions",
    Category::Operations,
    true,
    false,
    false,
    "A GraphQL document is only valid for execution if all definitions are operation or fragment definitions."
);
spec_rule!(
    FIELDS_ON_CORRECT_TYPE_ENTRY,
    FIELDS_ON_CORRECT_TYPE_META,
    fields_on_correct_type,
    "fields-on-correct-type",
    Category::Operations,
    true,
    false,
    true,
    "A GraphQL document is only valid if all selected fields are defined by their parent type."
);
spec_rule!(
    FRAGMENTS_ON_COMPOSITE_TYPE_ENTRY,
    FRAGMENTS_ON_COMPOSITE_TYPE_META,
    fragments_on_composite_type,
    "fragments-on-composite-type",
    Category::Operations,
    true,
    false,
    false,
    "Fragment type conditions must name composite types."
);
spec_rule!(
    KNOWN_ARGUMENT_NAMES_ENTRY,
    KNOWN_ARGUMENT_NAMES_META,
    known_argument_names,
    "known-argument-names",
    Category::Operations,
    true,
    false,
    true,
    "Supplied arguments must be defined by their field or directive."
);
spec_rule!(
    KNOWN_DIRECTIVES_ENTRY,
    KNOWN_DIRECTIVES_META,
    known_directives,
    "known-directives",
    Category::Operations,
    true,
    false,
    false,
    "Directives must be known by the schema and legally positioned."
);
spec_rule!(
    KNOWN_FRAGMENT_NAMES_ENTRY,
    KNOWN_FRAGMENT_NAMES_META,
    known_fragment_names,
    "known-fragment-names",
    Category::Operations,
    true,
    true,
    false,
    "Fragment spreads must refer to known fragments."
);
spec_rule!(
    KNOWN_TYPE_NAMES_ENTRY,
    KNOWN_TYPE_NAMES_META,
    known_type_names,
    "known-type-names",
    Category::Operations,
    true,
    false,
    true,
    "Referenced GraphQL types must be defined by the schema."
);
spec_rule!(
    LONE_ANONYMOUS_OPERATION_ENTRY,
    LONE_ANONYMOUS_OPERATION_META,
    lone_anonymous_operation,
    "lone-anonymous-operation",
    Category::Operations,
    true,
    false,
    false,
    "An anonymous operation must be the only operation in its document."
);
spec_rule!(
    LONE_SCHEMA_DEFINITION_ENTRY,
    LONE_SCHEMA_DEFINITION_META,
    lone_schema_definition,
    "lone-schema-definition",
    Category::Schema,
    false,
    false,
    false,
    "A GraphQL document may contain only one schema definition."
);
spec_rule!(
    NO_FRAGMENT_CYCLES_ENTRY,
    NO_FRAGMENT_CYCLES_META,
    no_fragment_cycles,
    "no-fragment-cycles",
    Category::Operations,
    true,
    false,
    false,
    "Fragments must not form cycles."
);
spec_rule!(
    NO_UNDEFINED_VARIABLES_ENTRY,
    NO_UNDEFINED_VARIABLES_META,
    no_undefined_variables,
    "no-undefined-variables",
    Category::Operations,
    true,
    true,
    false,
    "All variables encountered by an operation must be defined by that operation."
);
spec_rule!(
    NO_UNUSED_FRAGMENTS_ENTRY,
    NO_UNUSED_FRAGMENTS_META,
    no_unused_fragments,
    "no-unused-fragments",
    Category::Operations,
    true,
    true,
    false,
    "Fragments must be used by an operation."
);
spec_rule!(
    NO_UNUSED_VARIABLES_ENTRY,
    NO_UNUSED_VARIABLES_META,
    no_unused_variables,
    "no-unused-variables",
    Category::Operations,
    true,
    true,
    false,
    "All variables defined by an operation must be used."
);
spec_rule!(
    OVERLAPPING_FIELDS_ENTRY,
    OVERLAPPING_FIELDS_META,
    overlapping_fields,
    "overlapping-fields-can-be-merged",
    Category::Operations,
    true,
    false,
    false,
    "Fields with the same response name must be mergeable."
);
spec_rule!(
    POSSIBLE_FRAGMENT_SPREAD_ENTRY,
    POSSIBLE_FRAGMENT_SPREAD_META,
    possible_fragment_spread,
    "possible-fragment-spread",
    Category::Operations,
    true,
    false,
    false,
    "Fragment spreads must be possible for their parent type."
);
spec_rule!(
    POSSIBLE_TYPE_EXTENSION_ENTRY,
    POSSIBLE_TYPE_EXTENSION_META,
    possible_type_extension,
    "possible-type-extension",
    Category::Schema,
    true,
    false,
    true,
    "Type extensions must extend an existing type of the same kind."
);
spec_rule!(
    PROVIDED_REQUIRED_ARGUMENTS_ENTRY,
    PROVIDED_REQUIRED_ARGUMENTS_META,
    provided_required_arguments,
    "provided-required-arguments",
    Category::Operations,
    true,
    false,
    false,
    "All required field and directive arguments must be provided."
);
spec_rule!(
    SCALAR_LEAFS_ENTRY,
    SCALAR_LEAFS_META,
    scalar_leafs,
    "scalar-leafs",
    Category::Operations,
    true,
    false,
    true,
    "Leaf fields must be scalar or enum types and composite fields must have selections."
);
spec_rule!(
    ONE_FIELD_SUBSCRIPTIONS_ENTRY,
    ONE_FIELD_SUBSCRIPTIONS_META,
    one_field_subscriptions,
    "one-field-subscriptions",
    Category::Operations,
    true,
    false,
    false,
    "Subscriptions must select one root field."
);
spec_rule!(
    UNIQUE_ARGUMENT_NAMES_ENTRY,
    UNIQUE_ARGUMENT_NAMES_META,
    unique_argument_names,
    "unique-argument-names",
    Category::Operations,
    true,
    false,
    false,
    "Arguments must be uniquely named at each location."
);
spec_rule!(
    UNIQUE_DIRECTIVE_NAMES_ENTRY,
    UNIQUE_DIRECTIVE_NAMES_META,
    unique_directive_names,
    "unique-directive-names",
    Category::Schema,
    false,
    false,
    false,
    "Directive definitions must be uniquely named."
);
spec_rule!(
    UNIQUE_DIRECTIVES_PER_LOCATION_ENTRY,
    UNIQUE_DIRECTIVES_PER_LOCATION_META,
    unique_directives_per_location,
    "unique-directive-names-per-location",
    Category::Operations,
    true,
    false,
    false,
    "Non-repeatable directives may be used only once at a location."
);
spec_rule!(
    UNIQUE_FIELD_DEFINITION_NAMES_ENTRY,
    UNIQUE_FIELD_DEFINITION_NAMES_META,
    unique_field_definition_names,
    "unique-field-definition-names",
    Category::Schema,
    false,
    false,
    false,
    "Complex type fields must be uniquely named."
);
spec_rule!(
    UNIQUE_INPUT_FIELD_NAMES_ENTRY,
    UNIQUE_INPUT_FIELD_NAMES_META,
    unique_input_field_names,
    "unique-input-field-names",
    Category::Operations,
    true,
    false,
    false,
    "Input object fields must be uniquely named."
);
spec_rule!(
    UNIQUE_OPERATION_TYPES_ENTRY,
    UNIQUE_OPERATION_TYPES_META,
    unique_operation_types,
    "unique-operation-types",
    Category::Schema,
    false,
    false,
    false,
    "A schema may define only one type for each operation kind."
);
spec_rule!(
    UNIQUE_TYPE_NAMES_ENTRY,
    UNIQUE_TYPE_NAMES_META,
    unique_type_names,
    "unique-type-names",
    Category::Schema,
    false,
    false,
    false,
    "Schema types must be uniquely named."
);
spec_rule!(
    UNIQUE_VARIABLE_NAMES_ENTRY,
    UNIQUE_VARIABLE_NAMES_META,
    unique_variable_names,
    "unique-variable-names",
    Category::Operations,
    true,
    false,
    false,
    "Variables in an operation must be uniquely named."
);
spec_rule!(
    VALUE_LITERALS_ENTRY,
    VALUE_LITERALS_META,
    value_literals,
    "value-literals-of-correct-type",
    Category::Operations,
    true,
    false,
    true,
    "Value literals must match the type expected at their position."
);
spec_rule!(
    VARIABLES_ARE_INPUT_TYPES_ENTRY,
    VARIABLES_ARE_INPUT_TYPES_META,
    variables_are_input_types,
    "variables-are-input-types",
    Category::Operations,
    true,
    false,
    false,
    "Variables must use input types."
);
spec_rule!(
    VARIABLES_IN_ALLOWED_POSITION_ENTRY,
    VARIABLES_IN_ALLOWED_POSITION_META,
    variables_in_allowed_position,
    "variables-in-allowed-position",
    Category::Operations,
    true,
    false,
    false,
    "Variables must be valid for the positions where they are used."
);

/// The complete set of graphql-eslint validation wrapper entries.
static SPEC_RULES: [RuleEntry; 30] = [
    EXECUTABLE_DEFINITIONS_ENTRY,
    FIELDS_ON_CORRECT_TYPE_ENTRY,
    FRAGMENTS_ON_COMPOSITE_TYPE_ENTRY,
    KNOWN_ARGUMENT_NAMES_ENTRY,
    KNOWN_DIRECTIVES_ENTRY,
    KNOWN_FRAGMENT_NAMES_ENTRY,
    KNOWN_TYPE_NAMES_ENTRY,
    LONE_ANONYMOUS_OPERATION_ENTRY,
    LONE_SCHEMA_DEFINITION_ENTRY,
    NO_FRAGMENT_CYCLES_ENTRY,
    NO_UNDEFINED_VARIABLES_ENTRY,
    NO_UNUSED_FRAGMENTS_ENTRY,
    NO_UNUSED_VARIABLES_ENTRY,
    OVERLAPPING_FIELDS_ENTRY,
    POSSIBLE_FRAGMENT_SPREAD_ENTRY,
    POSSIBLE_TYPE_EXTENSION_ENTRY,
    PROVIDED_REQUIRED_ARGUMENTS_ENTRY,
    SCALAR_LEAFS_ENTRY,
    ONE_FIELD_SUBSCRIPTIONS_ENTRY,
    UNIQUE_ARGUMENT_NAMES_ENTRY,
    UNIQUE_DIRECTIVE_NAMES_ENTRY,
    UNIQUE_DIRECTIVES_PER_LOCATION_ENTRY,
    UNIQUE_FIELD_DEFINITION_NAMES_ENTRY,
    UNIQUE_INPUT_FIELD_NAMES_ENTRY,
    UNIQUE_OPERATION_TYPES_ENTRY,
    UNIQUE_TYPE_NAMES_ENTRY,
    UNIQUE_VARIABLE_NAMES_ENTRY,
    VALUE_LITERALS_ENTRY,
    VARIABLES_ARE_INPUT_TYPES_ENTRY,
    VARIABLES_IN_ALLOWED_POSITION_ENTRY,
];

pub fn all_spec_rules() -> &'static [RuleEntry] {
    &SPEC_RULES
}
