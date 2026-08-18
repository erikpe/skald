//! Contextual validation and atomic publication of generated interfaces.

use super::*;

pub(in crate::resolve::resolver::program) fn validate_interface_specializations(
    program: &mut ResolvedProgram,
    diagnostics: &mut Diagnostics,
    ordinary_interfaces: ResolvedInterfaceDeclarationTable,
) {
    let has_unpublished_dependency = program
        .generic_interface_specializations
        .iter()
        .flat_map(|specialization| specialization.closed_type_uses.iter().flatten())
        .any(|kind| !type_is_fully_published(program, *kind));
    if has_unpublished_dependency {
        program.generic_interface_specializations.fail_all();
        program.interfaces = ordinary_interfaces;
        return;
    }

    let failures = crate::typeck::failed_interface_specialization_requirements(program);
    if failures.is_empty() {
        return;
    }

    for failure in failures {
        let specialization = program
            .generic_interface_specializations
            .for_interface(failure.interface)
            .expect("contextual failures reference an interface specialization");
        let semantics = program
            .interface_template_semantics
            .get(specialization.key.template)
            .expect("specialization key references interface template semantics");
        let requirement = &semantics.contextual_requirements[failure.requirement_index];
        let template = program
            .interface_templates
            .get(specialization.key.template)
            .expect("specialization key references an interface template");
        let origin = specialization
            .provenance
            .origins
            .first()
            .expect("requested specialization retains an origin");
        let mut diagnostic = Diagnostic::error(
            super::super::super::INVALID_GENERIC_INTERFACE_REQUIREMENT,
            format!(
                "type arguments for `{}` do not produce a valid interface signature",
                template.name
            ),
        )
        .with_primary_label(origin.span, capability_label(requirement.capability))
        .with_secondary_label(requirement.origin, "requirement originates here")
        .with_secondary_label(template.name_span, "generic interface declared here");
        for repeated in specialization.provenance.origins.iter().skip(1) {
            diagnostic = diagnostic.with_secondary_label(
                repeated.span,
                format!(
                    "same closed application also requested in module {}",
                    repeated.module
                ),
            );
        }
        diagnostics.push(diagnostic);
    }

    // Interface identities are reserved before recursive closure, while the
    // ordinary declaration table is dense. If any contextual obligation
    // fails, publish none of this attempt's generated suffix and mark every
    // reserved entry failed. Successful compilations still retain the exact
    // dense source/dependency order chosen by the coordinator.
    program.generic_interface_specializations.fail_all();
    program.interfaces = ordinary_interfaces;
}

fn type_is_fully_published(program: &ResolvedProgram, kind: ResolvedTypeKind) -> bool {
    match kind {
        ResolvedTypeKind::Class(class) => program.class(class).is_some(),
        ResolvedTypeKind::Interface(interface) => program.interface(interface).is_some(),
        ResolvedTypeKind::Function(function) => {
            program
                .function_types
                .get(function)
                .is_some_and(|signature| {
                    signature.parameters.iter().all(|parameter| {
                        type_is_fully_published(program, parameter.type_syntax.kind)
                    }) && type_is_fully_published(program, signature.result.kind)
                })
        }
        ResolvedTypeKind::Array(array) => program
            .array_types
            .get(array)
            .is_some_and(|array| type_is_fully_published(program, array.element.kind)),
        ResolvedTypeKind::Optional(optional) => program
            .optional_types
            .get(optional)
            .is_some_and(|optional| type_is_fully_published(program, optional.payload.kind)),
        ResolvedTypeKind::Shared(target) => match target {
            ResolvedSharedTarget::Class(class) => program.class(class).is_some(),
            ResolvedSharedTarget::Interface(interface) => program.interface(interface).is_some(),
            ResolvedSharedTarget::Array(array) => program
                .array_types
                .get(array)
                .is_some_and(|array| type_is_fully_published(program, array.element.kind)),
            ResolvedSharedTarget::OptionalBox(box_type) => program
                .optional_box_types
                .get(box_type)
                .is_some_and(|metadata| {
                    metadata.optional.is_none_or(|optional| {
                        type_is_fully_published(program, ResolvedTypeKind::Optional(optional))
                    }) && metadata.object_leaf.is_none_or(|object| match object {
                        ResolvedObjectTarget::Obj => true,
                        ResolvedObjectTarget::Class(class) => program.class(class).is_some(),
                        ResolvedObjectTarget::Interface(interface) => {
                            program.interface(interface).is_some()
                        }
                    })
                }),
            ResolvedSharedTarget::Obj => true,
        },
        ResolvedTypeKind::I64
        | ResolvedTypeKind::U64
        | ResolvedTypeKind::U8
        | ResolvedTypeKind::F64
        | ResolvedTypeKind::Bool
        | ResolvedTypeKind::Unit
        | ResolvedTypeKind::Obj => true,
    }
}

fn capability_label(capability: GenericCapability) -> &'static str {
    match capability {
        GenericCapability::ValueParameter => {
            "this application creates a non-storable value parameter"
        }
        GenericCapability::ValueResult => "this application creates a non-owning result type",
        GenericCapability::AliasTarget(_) => "this application creates an unsupported alias target",
        GenericCapability::OptionalPayload => {
            "this application creates an invalid optional payload"
        }
        GenericCapability::ArrayElement => "this application creates an invalid array element",
        GenericCapability::SharedTarget => "this application creates an invalid shared target",
        GenericCapability::FieldStorage
        | GenericCapability::StaticStorage
        | GenericCapability::DefaultConstructible
        | GenericCapability::CopyConstructible
        | GenericCapability::Assignable
        | GenericCapability::Destroyable => "this application is contextually invalid",
    }
}
