use super::*;
use crate::{
    identity::{ClassTemplateId, InterfaceId},
    resolve::{
        GenericAliasAccess, GenericCapability, GenericRequirementReason, ResolvedCopyOperation,
    },
    test_support::resolve_source,
};

fn resolved_capability_fixture() -> ResolvedProgram {
    let output = resolve_source(
        "interface View { fn inspect() -> unit; }\n\
         class Copyable { init() {} }\n\
         class NonCopy { init() {} }\n\
         class Samples {\n\
           primitive: i64;\n\
           exact: Copyable;\n\
           optional: Copyable?;\n\
           nested: Copyable??;\n\
           array: Copyable?[];\n\
           noncopy_optional: NonCopy?;\n\
           noncopy_nested: NonCopy??;\n\
           noncopy_array: NonCopy?[];\n\
           shared_exact: shared Copyable;\n\
           shared_noncopy: shared NonCopy;\n\
           shared_interface: shared View;\n\
           shared_obj: shared Obj;\n\
           optional_shared: (shared NonCopy)?;\n\
           optional_box: shared Copyable?;\n\
           init() {}\n\
         }\n\
         class Probe<T> { value: T; }\n\
         fn main() -> i64 { return 0; }\n",
    );
    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    let mut program = output.program;
    let noncopy = program
        .classes
        .iter()
        .find(|class| class.name == "NonCopy")
        .unwrap()
        .id;
    let class = &mut program.classes.entries_mut_for_test()[noncopy.index()];
    class.copy_constructor = ResolvedCopyOperation::Unavailable;
    class.copy_assignment = ResolvedCopyOperation::Unavailable;
    program
}

fn field_kind(program: &ResolvedProgram, name: &str) -> ResolvedTypeKind {
    program
        .classes
        .iter()
        .find(|class| class.name == "Samples")
        .unwrap()
        .fields
        .iter()
        .find(|field| field.name == name)
        .unwrap()
        .type_syntax
        .kind
}

fn requirement(
    program: &ResolvedProgram,
    capability: GenericCapability,
    reason: GenericRequirementReason,
) -> GenericRequirement {
    let mut requirement = program
        .template_semantics
        .get(ClassTemplateId::new(0))
        .unwrap()
        .requirements[0]
        .clone();
    requirement.capability = capability;
    requirement.reason = reason;
    requirement
}

fn supports(
    query: &GenericCapabilityQuery<'_>,
    program: &ResolvedProgram,
    capability: GenericCapability,
    kind: ResolvedTypeKind,
) -> bool {
    let requirement = requirement(
        program,
        capability,
        GenericRequirementReason::FieldDeclaration { member: 0 },
    );
    query.supports(&requirement, ClosedGenericRequirementSubject::Type(kind))
}

#[test]
fn declaration_roles_delegate_to_stored_alias_optional_array_and_shared_owners() {
    let program = resolved_capability_fixture();
    let query = GenericCapabilityQuery::new(&program);
    let exact = field_kind(&program, "exact");
    let optional = field_kind(&program, "optional");
    let nested = field_kind(&program, "nested");
    let array = field_kind(&program, "array");
    let shared = field_kind(&program, "shared_exact");
    let optional_shared = field_kind(&program, "optional_shared");

    for kind in [
        ResolvedTypeKind::I64,
        exact,
        optional,
        nested,
        array,
        shared,
    ] {
        assert!(supports(
            &query,
            &program,
            GenericCapability::FieldStorage,
            kind
        ));
        assert!(supports(
            &query,
            &program,
            GenericCapability::ArrayElement,
            kind
        ));
    }
    for kind in [
        ResolvedTypeKind::Unit,
        ResolvedTypeKind::Obj,
        ResolvedTypeKind::Interface(InterfaceId::new(0)),
    ] {
        assert!(!supports(
            &query,
            &program,
            GenericCapability::FieldStorage,
            kind
        ));
        assert!(!supports(
            &query,
            &program,
            GenericCapability::OptionalPayload,
            kind
        ));
    }

    let readonly = GenericCapability::AliasTarget(GenericAliasAccess::ReadOnly);
    for kind in [
        ResolvedTypeKind::I64,
        exact,
        optional,
        nested,
        array,
        ResolvedTypeKind::Obj,
        ResolvedTypeKind::Interface(InterfaceId::new(0)),
    ] {
        assert!(supports(&query, &program, readonly, kind));
    }
    assert!(!supports(&query, &program, readonly, shared));
    assert!(!supports(&query, &program, readonly, optional_shared));
    assert!(!supports(
        &query,
        &program,
        GenericCapability::ValueParameter,
        ResolvedTypeKind::Unit
    ));
    assert!(supports(
        &query,
        &program,
        GenericCapability::ValueResult,
        ResolvedTypeKind::Unit
    ));

    let shared_target = requirement(
        &program,
        GenericCapability::SharedTarget,
        GenericRequirementReason::SharedType,
    );
    for kind in [
        exact,
        array,
        ResolvedTypeKind::Obj,
        ResolvedTypeKind::Interface(InterfaceId::new(0)),
    ] {
        assert!(query.supports(&shared_target, ClosedGenericRequirementSubject::Type(kind)));
    }
    assert!(!query.supports(
        &shared_target,
        ClosedGenericRequirementSubject::Type(ResolvedTypeKind::I64)
    ));
    let ResolvedTypeKind::Shared(optional_box) = field_kind(&program, "optional_box") else {
        panic!("expected a shared optional-box field")
    };
    assert!(query.supports(
        &shared_target,
        ClosedGenericRequirementSubject::SharedTarget(optional_box)
    ));
}

#[test]
fn recursive_lifecycle_queries_follow_optional_array_and_shared_plans() {
    let program = resolved_capability_fixture();
    let query = GenericCapabilityQuery::new(&program);
    let copyable = field_kind(&program, "exact");
    let noncopy = program
        .classes
        .iter()
        .find(|class| class.name == "NonCopy")
        .unwrap()
        .id;
    let noncopy = ResolvedTypeKind::Class(noncopy);
    let optional = field_kind(&program, "optional");
    let nested = field_kind(&program, "nested");
    let array = field_kind(&program, "array");
    let noncopy_optional = field_kind(&program, "noncopy_optional");
    let noncopy_nested = field_kind(&program, "noncopy_nested");
    let noncopy_array = field_kind(&program, "noncopy_array");
    let shared = field_kind(&program, "shared_exact");
    let shared_noncopy = field_kind(&program, "shared_noncopy");
    let optional_shared = field_kind(&program, "optional_shared");

    for capability in [
        GenericCapability::CopyConstructible,
        GenericCapability::Assignable,
    ] {
        for kind in [
            copyable,
            optional,
            nested,
            array,
            shared,
            shared_noncopy,
            optional_shared,
        ] {
            assert!(supports(&query, &program, capability, kind));
        }
        for kind in [noncopy, noncopy_optional, noncopy_nested, noncopy_array] {
            assert!(!supports(&query, &program, capability, kind));
        }
    }

    let default = requirement(
        &program,
        GenericCapability::DefaultConstructible,
        GenericRequirementReason::ArrayLengthConstruction { member: 0 },
    );
    assert!(query.supports(
        &default,
        ClosedGenericRequirementSubject::Type(noncopy_optional)
    ));
    assert!(query.supports(
        &default,
        ClosedGenericRequirementSubject::Type(noncopy_nested)
    ));
    assert!(query.supports(
        &default,
        ClosedGenericRequirementSubject::Type(noncopy_array)
    ));

    let static_default = requirement(
        &program,
        GenericCapability::DefaultConstructible,
        GenericRequirementReason::StaticZeroInitialization { member: 0 },
    );
    assert!(!query.supports(
        &static_default,
        ClosedGenericRequirementSubject::Type(copyable)
    ));
    assert!(query.supports(
        &static_default,
        ClosedGenericRequirementSubject::Type(optional)
    ));
    assert!(query.supports(
        &static_default,
        ClosedGenericRequirementSubject::Type(array)
    ));

    let storage = requirement(
        &program,
        GenericCapability::FieldStorage,
        GenericRequirementReason::FieldDeclaration { member: 0 },
    );
    let copying = requirement(
        &program,
        GenericCapability::CopyConstructible,
        GenericRequirementReason::ExplicitCopyConstruction { member: 0 },
    );
    assert!(query.supports(&storage, ClosedGenericRequirementSubject::Type(noncopy)));
    assert!(!query.supports(&copying, ClosedGenericRequirementSubject::Type(noncopy)));
}

#[test]
fn effective_contract_allows_alias_and_shared_interface_but_rejects_inline_optional_interface() {
    let output = resolve_source(
        "interface View { fn inspect() -> unit; }\n\
         class Observer<T> { fn inspect(ref value: T) -> unit {} }\n\
         class Owner<T> { value: shared T; }\n\
         class Vec<T> { storage: T?[]; }\n\
         class Closed {\n\
           owner: shared View;\n\
           proxy_optional: i64?;\n\
           proxy_array: i64?[];\n\
           init() {}\n\
         }\n\
         fn main() -> i64 { return 0; }\n",
    );
    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    let program = output.program;
    let query = GenericCapabilityQuery::new(&program);
    let closed = program
        .classes
        .iter()
        .find(|class| class.name == "Closed")
        .unwrap();
    let shared_interface = closed.fields[0].type_syntax.kind;
    let proxy_optional = closed.fields[1].type_syntax.kind;
    let proxy_array = closed.fields[2].type_syntax.kind;
    let interface = ResolvedTypeKind::Interface(InterfaceId::new(0));

    let observer = &program
        .template_semantics
        .get(ClassTemplateId::new(0))
        .unwrap()
        .requirements;
    assert!(query
        .evaluate(observer, |_| Some(ClosedGenericRequirementSubject::Type(
            interface
        )))
        .is_empty());

    let owner = &program
        .template_semantics
        .get(ClassTemplateId::new(1))
        .unwrap()
        .requirements;
    let owner_failures = query.evaluate(owner, |requirement| {
        Some(match requirement.capability {
            GenericCapability::SharedTarget => ClosedGenericRequirementSubject::Type(interface),
            _ => ClosedGenericRequirementSubject::Type(shared_interface),
        })
    });
    assert!(owner_failures.is_empty());

    let vector = &program
        .template_semantics
        .get(ClassTemplateId::new(2))
        .unwrap()
        .requirements;
    let failures = query.evaluate(vector, |requirement| {
        Some(match requirement.capability {
            GenericCapability::OptionalPayload => ClosedGenericRequirementSubject::Type(interface),
            GenericCapability::ArrayElement => {
                ClosedGenericRequirementSubject::Type(proxy_optional)
            }
            _ => ClosedGenericRequirementSubject::Type(proxy_array),
        })
    });
    assert_eq!(failures.len(), 1);
    assert_eq!(
        failures[0].requirement.capability,
        GenericCapability::OptionalPayload
    );
}
