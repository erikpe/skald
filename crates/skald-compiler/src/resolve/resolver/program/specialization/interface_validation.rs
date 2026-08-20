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
    let bound_failures = failed_nominal_interface_bounds(program);
    let duplicate_bound_failures = duplicate_closed_interface_bounds(program);
    if failures.is_empty() && bound_failures.is_empty() && duplicate_bound_failures.is_empty() {
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

    for (interface, first_index, duplicate_index) in duplicate_bound_failures {
        let specialization = program
            .generic_interface_specializations
            .for_interface(interface)
            .expect("duplicate bounds reference an interface specialization");
        let semantics = program
            .interface_template_semantics
            .get(specialization.key.template)
            .expect("specialization key references interface template semantics");
        let template = program
            .interface_templates
            .get(specialization.key.template)
            .expect("specialization key references an interface template");
        let required = specialization.closed_interface_bounds[duplicate_index]
            .expect("complete specialization closes every interface bound");
        let required = program
            .interface(required)
            .expect("closed bound references a materialized interface");
        let origin = specialization
            .provenance
            .origins
            .first()
            .expect("requested specialization retains an origin");
        let mut diagnostic = Diagnostic::error(
            super::super::super::DUPLICATE_GENERIC_BOUND,
            format!(
                "type arguments for `{}` produce duplicate bound `{}`",
                template.name, required.name
            ),
        )
        .with_primary_label(origin.span, "this closed application duplicates a bound")
        .with_secondary_label(
            semantics.bounds[duplicate_index].span,
            "duplicate closed bound declared here",
        )
        .with_secondary_label(
            semantics.bounds[first_index].span,
            "first equivalent closed bound declared here",
        )
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

    for (interface, bound_index) in bound_failures {
        let specialization = program
            .generic_interface_specializations
            .for_interface(interface)
            .expect("bound failures reference an interface specialization");
        let semantics = program
            .interface_template_semantics
            .get(specialization.key.template)
            .expect("specialization key references interface template semantics");
        let bound = &semantics.bounds[bound_index];
        let template = program
            .interface_templates
            .get(specialization.key.template)
            .expect("specialization key references an interface template");
        let parameter = program
            .type_parameters
            .for_interface_template(specialization.key.template)
            .and_then(|parameters| {
                parameters
                    .iter()
                    .find(|parameter| parameter.id == bound.parameter)
            })
            .expect("resolved bound parameter belongs to its interface template");
        let required_interface = specialization.closed_interface_bounds[bound_index]
            .expect("complete specialization closes every interface bound");
        let required = program
            .interface(required_interface)
            .expect("closed bound references a materialized interface");
        let argument = specialization.key.arguments[bound.parameter.index()];
        let origin = specialization
            .provenance
            .origins
            .first()
            .expect("requested specialization retains an origin");
        let mut diagnostic = Diagnostic::error(
            super::super::super::INVALID_GENERIC_INTERFACE_REQUIREMENT,
            format!(
                "type argument for `{}` does not satisfy `{}`'s bound",
                template.name, parameter.name
            ),
        )
        .with_primary_label(
            origin.span,
            format!(
                "{} does not provide effective nominal conformance to `{}`",
                super::validation::argument_kind_name(argument),
                required.name
            ),
        )
        .with_secondary_label(bound.span, "bound declared here")
        .with_secondary_label(required.name_span, "required interface declared here")
        .with_secondary_label(template.name_span, "generic interface declared here")
        .with_note(
            "generic bounds accept only exact class arguments with direct or inherited declared conformance",
        );
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

fn failed_nominal_interface_bounds(program: &ResolvedProgram) -> Vec<(InterfaceId, usize)> {
    let mut failures = Vec::new();
    for specialization in program.generic_interface_specializations.iter() {
        let GenericInterfaceSpecializationState::Complete(interface) = specialization.state else {
            continue;
        };
        let semantics = program
            .interface_template_semantics
            .get(specialization.key.template)
            .expect("specialization key references interface template semantics");
        for (bound_index, bound) in semantics.bounds.iter().enumerate() {
            let required = specialization.closed_interface_bounds[bound_index]
                .expect("complete specialization closes every interface bound");
            if (0..bound_index).any(|previous| {
                semantics.bounds[previous].parameter == bound.parameter
                    && specialization.closed_interface_bounds[previous] == Some(required)
            }) {
                continue;
            }
            let argument = specialization.key.arguments[bound.parameter.index()];
            let satisfied = match argument {
                ResolvedTypeKind::Class(class) => {
                    super::validation::effective_nominal_conformance(program, class, required)
                }
                ResolvedTypeKind::I64
                | ResolvedTypeKind::U64
                | ResolvedTypeKind::U8
                | ResolvedTypeKind::F64
                | ResolvedTypeKind::Bool
                | ResolvedTypeKind::Unit
                | ResolvedTypeKind::Obj
                | ResolvedTypeKind::Function(_)
                | ResolvedTypeKind::Interface(_)
                | ResolvedTypeKind::Shared(_)
                | ResolvedTypeKind::Optional(_)
                | ResolvedTypeKind::Array(_) => false,
            };
            if !satisfied {
                failures.push((interface, bound_index));
            }
        }
    }
    failures
}

fn duplicate_closed_interface_bounds(
    program: &ResolvedProgram,
) -> Vec<(InterfaceId, usize, usize)> {
    let mut failures = Vec::new();
    for specialization in program.generic_interface_specializations.iter() {
        let GenericInterfaceSpecializationState::Complete(interface) = specialization.state else {
            continue;
        };
        let semantics = program
            .interface_template_semantics
            .get(specialization.key.template)
            .expect("specialization key references interface template semantics");
        for duplicate in 0..semantics.bounds.len() {
            let bound = &semantics.bounds[duplicate];
            let required = specialization.closed_interface_bounds[duplicate];
            if let Some(first) = (0..duplicate).find(|first| {
                semantics.bounds[*first].parameter == bound.parameter
                    && specialization.closed_interface_bounds[*first] == required
            }) {
                failures.push((interface, first, duplicate));
            }
        }
    }
    failures
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
