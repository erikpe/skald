use super::*;
use crate::{
    identity::{CallableId, ClassId, ClassTemplateId, FunctionId, FunctionTypeId, MethodId},
    resolve::{
        dump_resolved, GenericCapability, ResolvedFunctionTypeParameterMode, ResolvedTypeKind,
        UNSATISFIED_GENERIC_REQUIREMENT,
    },
    test_support::resolve_source,
};

#[test]
fn nested_template_function_terms_close_children_before_the_signature() {
    let output = resolve_source(
        "class Signatures<T> {\n\
           callback: fn(ref T[], mut ref T?) -> fn(T) -> T;\n\
         }\n\
         fn use(ref value: Signatures<i64>) -> unit {}\n\
         fn main() -> i64 { return 0; }\n",
    );

    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    let signature = output
        .program
        .function_types
        .get(FunctionTypeId::new(1))
        .unwrap();
    assert_eq!(signature.parameters.len(), 2);
    assert_eq!(
        signature.parameters[0].mode,
        ResolvedFunctionTypeParameterMode::ReadOnlyAlias
    );
    assert!(matches!(
        signature.parameters[0].type_syntax.kind,
        ResolvedTypeKind::Array(_)
    ));
    assert_eq!(
        signature.parameters[1].mode,
        ResolvedFunctionTypeParameterMode::MutableAlias
    );
    assert!(matches!(
        signature.parameters[1].type_syntax.kind,
        ResolvedTypeKind::Optional(_)
    ));
    assert_eq!(
        signature.result.kind,
        ResolvedTypeKind::Function(FunctionTypeId::new(0))
    );
    let inner = output
        .program
        .function_types
        .get(FunctionTypeId::new(0))
        .unwrap();
    assert_eq!(inner.parameters[0].type_syntax.kind, ResolvedTypeKind::I64);
    assert_eq!(inner.result.kind, ResolvedTypeKind::I64);

    let dump = dump_resolved(&output.program);
    assert!(dump.contains(
        "fn(ref array (template0:type0), mut optional (template0:type0)) -> fn(template0:type0) -> template0:type0"
    ), "{dump}");
}

#[test]
fn specialized_bodies_form_top_level_and_exact_static_method_references() {
    let output = resolve_source(
        "fn passthrough(value: i64) -> i64 { return value; }\n\
         class Identity<T> {\n\
           static fn apply(value: T) -> T { return value; }\n\
           fn collect() -> unit {\n\
             var top: fn(i64) -> i64 = passthrough;\n\
             var specialized: fn(T) -> T = Identity<T>::apply;\n\
           }\n\
         }\n\
         fn use(ref value: Identity<i64>) -> unit {}\n\
         fn main() -> i64 { return 0; }\n",
    );

    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    let semantics = output
        .program
        .template_semantics
        .get(ClassTemplateId::new(0))
        .unwrap();
    assert!(semantics.selections.iter().any(|selection| matches!(
        selection,
        ResolvedTemplateSelection::TopLevel {
            declaration: ResolvedTopLevelId::Function(function),
            ..
        } if *function == FunctionId::new(0)
    )));
    assert!(semantics.selections.iter().any(|selection| matches!(
        selection,
        ResolvedTemplateSelection::ArgumentDependent {
            kind: ResolvedTemplateDependentSelectionKind::StaticMember,
            member_name: Some(name),
            ..
        } if name == "apply"
    )));

    let references = output
        .program
        .address_taken_callables
        .iter()
        .collect::<Vec<_>>();
    assert_eq!(references.len(), 2);
    assert_eq!(
        references[0].target,
        CallableId::Function(FunctionId::new(0))
    );
    assert_eq!(
        references[1].target,
        CallableId::Method(MethodId::new(ClassId::new(0), 0))
    );
    assert_eq!(references[0].function_type, references[1].function_type);
}

#[test]
fn specializations_keep_distinct_targets_while_reusing_equal_signatures() {
    let output = resolve_source(
        "class Factory<T> {\n\
           static fn stable(value: i64) -> i64 { return value; }\n\
           static fn identity(value: T) -> T { return value; }\n\
         }\n\
         fn main() -> i64 {\n\
           var i64_stable: fn(i64) -> i64 = Factory<i64>::stable;\n\
           var bool_stable: fn(i64) -> i64 = Factory<bool>::stable;\n\
           var i64_identity: fn(i64) -> i64 = Factory<i64>::identity;\n\
           var bool_identity: fn(bool) -> bool = Factory<bool>::identity;\n\
           return 0;\n\
         }\n",
    );

    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    let references = &output.program.address_taken_callables;
    let i64_stable = references
        .get(CallableId::Method(MethodId::new(ClassId::new(0), 0)))
        .unwrap();
    let bool_stable = references
        .get(CallableId::Method(MethodId::new(ClassId::new(1), 0)))
        .unwrap();
    let i64_identity = references
        .get(CallableId::Method(MethodId::new(ClassId::new(0), 1)))
        .unwrap();
    let bool_identity = references
        .get(CallableId::Method(MethodId::new(ClassId::new(1), 1)))
        .unwrap();

    assert_ne!(i64_stable.target, bool_stable.target);
    assert_eq!(i64_stable.function_type, bool_stable.function_type);
    assert_eq!(i64_stable.function_type, i64_identity.function_type);
    assert_ne!(i64_identity.function_type, bool_identity.function_type);
}

#[test]
fn nested_generic_terms_inside_signatures_reuse_cache_entries() {
    let output = resolve_source(
        "class Inner<T> { value: T; }\n\
         class Outer<T> {\n\
           first: fn(Inner<T>) -> Inner<T>;\n\
           second: fn(Inner<T>) -> Inner<T>;\n\
         }\n\
         fn use(ref value: Outer<i64>) -> unit {}\n\
         fn main() -> i64 { return 0; }\n",
    );

    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    let specializations = output
        .program
        .generic_specializations
        .iter()
        .collect::<Vec<_>>();
    assert_eq!(
        specializations.len(),
        2,
        "{}",
        dump_resolved(&output.program)
    );
    assert_eq!(specializations[0].key.template, ClassTemplateId::new(1));
    assert_eq!(specializations[1].key.template, ClassTemplateId::new(0));
    assert_eq!(specializations[1].provenance.origins.len(), 4);
    assert_eq!(output.program.function_types.len(), 1);
}

#[test]
fn transformed_recursion_is_detected_through_function_signature_children() {
    let output = resolve_source(
        "class Loop<T> { callback: fn(Loop<T?>) -> unit; }\n\
         fn use(ref value: Loop<i64>) -> unit {}\n\
         fn main() -> i64 { return 0; }\n",
    );

    assert!(output
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == NON_TERMINATING_GENERIC_SPECIALIZATION));
    assert!(output.program.classes.is_empty());
}

#[test]
fn function_arguments_support_closed_storage_parameter_result_and_initialization_roles() {
    let output = resolve_source(
        "fn identity(value: i64) -> i64 { return value; }\n\
         class Roles<T> {\n\
           value: T;\n\
           static stored: T = identity;\n\
           fn forward(value: T) -> T { return value; }\n\
         }\n\
         fn use(ref value: Roles<fn(i64) -> i64>) -> unit {}\n\
         fn main() -> i64 { return 0; }\n",
    );

    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    let class = output.program.classes.get(ClassId::new(0)).unwrap();
    assert!(matches!(
        class.fields[0].type_syntax.kind,
        ResolvedTypeKind::Function(_)
    ));
    assert!(matches!(
        class.static_fields[0].type_syntax.kind,
        ResolvedTypeKind::Function(_)
    ));
    assert!(matches!(
        class.methods[0].parameters[0].type_syntax.kind,
        ResolvedTypeKind::Function(_)
    ));
    assert!(matches!(
        class.methods[0].return_type.kind,
        ResolvedTypeKind::Function(_)
    ));
    assert_eq!(
        output
            .program
            .address_taken_callables
            .iter()
            .next()
            .unwrap()
            .target,
        CallableId::Function(FunctionId::new(0))
    );
}

#[test]
fn function_arguments_use_the_existing_contextual_requirement_owner() {
    let cases = [
        (
            "class Invalid<T> { value: T?; }",
            GenericCapability::OptionalPayload,
        ),
        (
            "class Invalid<T> { value: T[]; }",
            GenericCapability::ArrayElement,
        ),
        (
            "class Invalid<T> { value: shared T; }",
            GenericCapability::SharedTarget,
        ),
        (
            "class Invalid<T> { fn inspect(ref value: T) -> unit {} }",
            GenericCapability::AliasTarget(GenericAliasAccess::ReadOnly),
        ),
    ];

    for (declaration, capability) in cases {
        let output = resolve_source(format!(
            "{declaration}\n\
             fn use(ref value: Invalid<fn(i64) -> i64>) -> unit {{}}\n\
             fn main() -> i64 {{ return 0; }}\n"
        ));
        let semantics = output
            .program
            .template_semantics
            .get(ClassTemplateId::new(0))
            .unwrap();
        assert!(semantics
            .requirements
            .iter()
            .any(|requirement| requirement.capability == capability));
        let diagnostic = output
            .diagnostics
            .iter()
            .find(|diagnostic| diagnostic.code == UNSATISFIED_GENERIC_REQUIREMENT)
            .unwrap_or_else(|| panic!("missing requirement diagnostic for {capability:?}"));
        assert_eq!(
            diagnostic.labels[1].span,
            semantics
                .requirements
                .iter()
                .find(|requirement| requirement.capability == capability)
                .unwrap()
                .origin
        );
    }
}
