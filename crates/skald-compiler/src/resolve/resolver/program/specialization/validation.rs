//! Validation and atomic publication of closed specializations.

use super::*;

pub(crate) fn validate_specialization_requirements(
    program: &mut ResolvedProgram,
    diagnostics: &mut Diagnostics,
    ordinary_class_count: usize,
    ordinary_hierarchy: ResolvedClassHierarchy,
) {
    let bound_failures = failed_nominal_bounds(program);
    let requirement_failures = crate::typeck::failed_specialization_requirements(program);
    if bound_failures.is_empty() && requirement_failures.is_empty() {
        return;
    }

    for (class, bound_index) in &bound_failures {
        let specialization = program
            .generic_specializations
            .for_class(*class)
            .expect("bound failures reference a specialization class");
        let semantics = program
            .template_semantics
            .get(specialization.key.template)
            .expect("specialization key references template semantics");
        let bound = &semantics.bounds[*bound_index];
        let template = program
            .class_templates
            .get(specialization.key.template)
            .expect("specialization key references a template declaration");
        let parameter = program
            .type_parameters
            .for_template(specialization.key.template)
            .and_then(|parameters| {
                parameters
                    .iter()
                    .find(|parameter| parameter.id == bound.parameter)
            })
            .expect("resolved bound parameter belongs to its template");
        let interface = program
            .interface(bound.interface)
            .expect("resolved bound references an interface");
        let argument = specialization.key.arguments[bound.parameter.index()];
        let origin = specialization
            .provenance
            .origins
            .first()
            .expect("requested specialization retains an origin");
        diagnostics.push(
            Diagnostic::error(
                super::super::super::UNSATISFIED_GENERIC_REQUIREMENT,
                format!(
                    "type argument for `{}.{}` does not satisfy interface bound `{}`",
                    template.name, parameter.name, interface.name
                ),
            )
            .with_primary_label(
                origin.span,
                format!(
                    "{} does not provide effective nominal conformance to `{}`",
                    argument_kind_name(argument), interface.name
                ),
            )
            .with_secondary_label(bound.span, "bound declared here")
            .with_secondary_label(interface.name_span, "interface declared here")
            .with_secondary_label(template.name_span, "template declared here")
            .with_note(
                "generic bounds accept only exact class arguments with direct or inherited declared conformance",
            ),
        );
    }

    for (class, requirement_index) in &requirement_failures {
        let specialization = program
            .generic_specializations
            .for_class(*class)
            .expect("requirement failures reference a specialization class");
        let semantics = program
            .template_semantics
            .get(specialization.key.template)
            .expect("specialization key references template semantics");
        let requirement = &semantics.requirements[*requirement_index];
        let template = program
            .class_templates
            .get(specialization.key.template)
            .expect("specialization key references a template declaration");
        let origin = specialization
            .provenance
            .origins
            .first()
            .expect("requested specialization retains an origin");
        diagnostics.push(
            Diagnostic::error(
                super::super::super::UNSATISFIED_GENERIC_REQUIREMENT,
                format!(
                    "type arguments for `{}` do not satisfy its requirements",
                    template.name
                ),
            )
            .with_primary_label(
                origin.span,
                format!(
                    "this application requires {}",
                    capability_name(requirement.capability)
                ),
            )
            .with_secondary_label(
                requirement.origin,
                format!(
                    "the requirement originates from {}",
                    reason_name(requirement.reason)
                ),
            )
            .with_secondary_label(template.name_span, "template declared here"),
        );
    }

    // Ordinary class tables are dense. If any candidate fails, retain no
    // generated declaration from this resolution attempt rather than expose a
    // hole or a declaration whose dependency graph includes a failed class.
    let failed_classes = bound_failures
        .iter()
        .map(|(class, _)| *class)
        .chain(requirement_failures.iter().map(|(class, _)| *class))
        .collect::<std::collections::BTreeSet<_>>();
    for class in failed_classes {
        program.generic_specializations.fail_class(class);
    }
    program.classes.truncate(ordinary_class_count);
    program.class_definitions.truncate(ordinary_class_count);
    program.hierarchy = ordinary_hierarchy;
}

fn failed_nominal_bounds(program: &ResolvedProgram) -> Vec<(ClassId, usize)> {
    let mut failures = Vec::new();
    for specialization in program.generic_specializations.iter() {
        let GenericSpecializationState::Complete(class) = specialization.state else {
            continue;
        };
        let semantics = program
            .template_semantics
            .get(specialization.key.template)
            .expect("specialization key references template semantics");
        for (bound_index, bound) in semantics.bounds.iter().enumerate() {
            let argument = specialization.key.arguments[bound.parameter.index()];
            let satisfied = match argument {
                ResolvedTypeKind::Class(argument_class) => {
                    effective_nominal_conformance(program, argument_class, bound.interface)
                }
                ResolvedTypeKind::I64
                | ResolvedTypeKind::U64
                | ResolvedTypeKind::U8
                | ResolvedTypeKind::F64
                | ResolvedTypeKind::Bool
                | ResolvedTypeKind::Unit
                | ResolvedTypeKind::Obj
                | ResolvedTypeKind::Interface(_)
                | ResolvedTypeKind::Shared(_)
                | ResolvedTypeKind::Optional(_)
                | ResolvedTypeKind::Array(_) => false,
            };
            if !satisfied {
                failures.push((class, bound_index));
            }
        }
    }
    failures
}

fn effective_nominal_conformance(
    program: &ResolvedProgram,
    class: ClassId,
    interface: InterfaceId,
) -> bool {
    std::iter::once(class)
        .chain(program.hierarchy.base_chain(class).into_iter().flatten())
        .any(|candidate| {
            program.class(candidate).is_some_and(|declaration| {
                declaration
                    .implemented_interfaces
                    .iter()
                    .any(|claim| claim.interface == interface)
            })
        })
}

const fn argument_kind_name(argument: ResolvedTypeKind) -> &'static str {
    match argument {
        ResolvedTypeKind::Class(_) => "the exact class argument",
        ResolvedTypeKind::Interface(_) => "the non-owning interface argument",
        ResolvedTypeKind::Shared(_) => "the shared-owner argument",
        ResolvedTypeKind::Optional(_) => "the optional argument",
        ResolvedTypeKind::Array(_) => "the array argument",
        ResolvedTypeKind::Obj => "the universal object-view argument",
        ResolvedTypeKind::I64
        | ResolvedTypeKind::U64
        | ResolvedTypeKind::U8
        | ResolvedTypeKind::F64
        | ResolvedTypeKind::Bool
        | ResolvedTypeKind::Unit => "the primitive argument",
    }
}

const fn capability_name(capability: GenericCapability) -> &'static str {
    match capability {
        GenericCapability::FieldStorage => "a storable field type",
        GenericCapability::StaticStorage => "a storable static-field type",
        GenericCapability::ValueParameter => "a storable value parameter",
        GenericCapability::ValueResult => "a supported value result",
        GenericCapability::AliasTarget(_) => "a supported alias target",
        GenericCapability::OptionalPayload => "a valid optional payload",
        GenericCapability::ArrayElement => "a valid array element",
        GenericCapability::SharedTarget => "a valid shared-owner target",
        GenericCapability::DefaultConstructible => "default construction",
        GenericCapability::CopyConstructible => "copy construction",
        GenericCapability::Assignable => "assignment",
        GenericCapability::Destroyable => "deterministic destruction",
    }
}

fn reason_name(reason: GenericRequirementReason) -> String {
    match reason {
        GenericRequirementReason::FieldDeclaration { member } => {
            format!("field declaration at member {member}")
        }
        GenericRequirementReason::StaticFieldDeclaration { member } => {
            format!("static-field declaration at member {member}")
        }
        GenericRequirementReason::ParameterDeclaration { member, parameter } => {
            format!("parameter {parameter} of member {member}")
        }
        GenericRequirementReason::MethodResult { member } => {
            format!("the result of member {member}")
        }
        GenericRequirementReason::OptionalType => "an optional type".to_owned(),
        GenericRequirementReason::ArrayType => "an array type".to_owned(),
        GenericRequirementReason::SharedType => "a shared-owner type".to_owned(),
        GenericRequirementReason::StaticZeroInitialization { member } => {
            format!("zero initialization of static member {member}")
        }
        GenericRequirementReason::ArrayLengthConstruction { member } => {
            format!("array construction in member {member}")
        }
        GenericRequirementReason::ExplicitArrayCopy { member } => {
            format!("array copying in member {member}")
        }
        GenericRequirementReason::ExplicitCopyConstruction { member } => {
            format!("copy construction in member {member}")
        }
        GenericRequirementReason::StoredInitializationCopy { member } => {
            format!("stored-value initialization in member {member}")
        }
        GenericRequirementReason::Assignment { member } => {
            format!("assignment in member {member}")
        }
        GenericRequirementReason::SynthesizedDestruction { member } => {
            format!("destruction of member {member}")
        }
    }
}
