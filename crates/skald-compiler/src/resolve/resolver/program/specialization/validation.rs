//! Validation and atomic publication of closed specializations.

use super::*;

pub(crate) fn validate_specialization_requirements(
    program: &mut ResolvedProgram,
    diagnostics: &mut Diagnostics,
    ordinary_class_count: usize,
    ordinary_hierarchy: ResolvedClassHierarchy,
    ordinary_classes: ResolvedClassDeclarationTable,
) {
    let bound_failures = failed_nominal_bounds(program);
    let requirement_failures = crate::typeck::failed_specialization_requirements(program);
    if bound_failures.is_empty() && requirement_failures.is_empty() {
        return;
    }

    suppress_generic_execution_gates_for_failed_applications(
        program,
        diagnostics,
        &bound_failures,
        &requirement_failures,
    );

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
        let application_name = application_name(program, specialization);
        let mut diagnostic = Diagnostic::error(
                super::super::super::UNSATISFIED_GENERIC_REQUIREMENT,
                format!(
                    "type argument for `{application_name}` does not satisfy `{}`'s bound",
                    parameter.name
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
            );
        diagnostic = add_repeated_application_origins(diagnostic, specialization);
        diagnostics.push(diagnostic);
    }

    for failure in &requirement_failures {
        let class = failure.class;
        let specialization = program
            .generic_specializations
            .for_class(class)
            .expect("requirement failures reference a specialization class");
        let semantics = program
            .template_semantics
            .get(specialization.key.template)
            .expect("specialization key references template semantics");
        let requirement = &semantics.requirements[failure.requirement_index];
        let template = program
            .class_templates
            .get(specialization.key.template)
            .expect("specialization key references a template declaration");
        let origin = specialization
            .provenance
            .origins
            .first()
            .expect("requested specialization retains an origin");
        let application_name = application_name(program, specialization);
        let mut diagnostic = Diagnostic::error(
            super::super::super::UNSATISFIED_GENERIC_REQUIREMENT,
            format!("type arguments for `{application_name}` do not satisfy its requirements"),
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
        .with_secondary_label(template.name_span, "generic class declared here");
        diagnostic = add_lifecycle_path(diagnostic, program, &failure.lifecycle_path);
        diagnostic = add_repeated_application_origins(diagnostic, specialization);
        diagnostics.push(diagnostic);
    }

    // Ordinary class tables are dense. If any candidate fails, retain no
    // generated declaration from this resolution attempt rather than expose a
    // hole or a declaration whose dependency graph includes a failed class.
    // No generated declaration is published when any member of the closed
    // specialization graph is invalid. Mark every reserved generated identity
    // failed, then restore the pre-specialization class product. This also
    // removes virtual-dispatch and initializer mutations made while validating
    // candidates, rather than leaving diagnostic output with dangling class
    // references.
    let generated_classes = program
        .generic_specializations
        .iter()
        .filter_map(GenericSpecialization::class)
        .collect::<Vec<_>>();
    for class in generated_classes {
        program.generic_specializations.fail_class(class);
    }
    debug_assert_eq!(ordinary_classes.len(), ordinary_class_count);
    program.classes = ordinary_classes;
    program.class_definitions = ResolvedClassDefinitionTable::default();
    program.definitions = ResolvedFunctionDefinitionTable::default();
    program.virtual_families = ResolvedVirtualFamilyTable::default();
    program.hierarchy = ordinary_hierarchy;
}

fn suppress_generic_execution_gates_for_failed_applications(
    program: &ResolvedProgram,
    diagnostics: &mut Diagnostics,
    bound_failures: &[(ClassId, usize)],
    requirement_failures: &[crate::typeck::FailedSpecializationRequirement],
) {
    let failed_classes = bound_failures
        .iter()
        .map(|(class, _)| *class)
        .chain(requirement_failures.iter().map(|failure| failure.class))
        .collect::<std::collections::BTreeSet<_>>();
    let failed_origins = failed_classes
        .into_iter()
        .filter_map(|class| program.generic_specializations.for_class(class))
        .flat_map(|specialization| {
            specialization
                .provenance
                .origins
                .iter()
                .map(|origin| origin.span)
        })
        .collect::<std::collections::HashSet<_>>();
    diagnostics.retain(|diagnostic| {
        diagnostic.code != super::super::super::UNSUPPORTED_GENERIC_SYNTAX
            || !diagnostic
                .labels
                .iter()
                .any(|label| failed_origins.contains(&label.span))
    });
}

fn application_name(program: &ResolvedProgram, specialization: &GenericSpecialization) -> String {
    let template = program
        .class_templates
        .get(specialization.key.template)
        .expect("specialization keys reference collected templates");
    let arguments = specialization
        .key
        .arguments
        .iter()
        .map(|argument| semantic_type_name(program, *argument, &mut Vec::new()))
        .collect::<Vec<_>>()
        .join(", ");
    format!(
        "{}<{arguments}>",
        qualified_name(program, template.module, &template.name)
    )
}

fn semantic_type_name(
    program: &ResolvedProgram,
    kind: ResolvedTypeKind,
    visiting: &mut Vec<ClassId>,
) -> String {
    match kind {
        ResolvedTypeKind::I64 => "i64".to_owned(),
        ResolvedTypeKind::U64 => "u64".to_owned(),
        ResolvedTypeKind::U8 => "u8".to_owned(),
        ResolvedTypeKind::F64 => "f64".to_owned(),
        ResolvedTypeKind::Bool => "bool".to_owned(),
        ResolvedTypeKind::Unit => "unit".to_owned(),
        ResolvedTypeKind::Obj => "Obj".to_owned(),
        ResolvedTypeKind::Class(class) => {
            if visiting.contains(&class) {
                return class.to_string();
            }
            if let Some(class) = program.class(class) {
                return qualified_name(program, class.module, &class.name);
            }
            let Some(specialization) = program.generic_specializations.for_class(class) else {
                return class.to_string();
            };
            visiting.push(class);
            let name = application_name(program, specialization);
            visiting.pop();
            name
        }
        ResolvedTypeKind::Interface(interface) => program.interface(interface).map_or_else(
            || interface.to_string(),
            |interface| qualified_name(program, interface.module, &interface.name),
        ),
        ResolvedTypeKind::Array(array) => program.array_types.get(array).map_or_else(
            || array.to_string(),
            |array| {
                format!(
                    "{}[]",
                    semantic_type_name(program, array.element.kind, visiting)
                )
            },
        ),
        ResolvedTypeKind::Shared(target) => {
            format!(
                "shared {}",
                semantic_shared_target_name(program, target, visiting)
            )
        }
        ResolvedTypeKind::Optional(optional) => program.optional_types.get(optional).map_or_else(
            || optional.to_string(),
            |optional| {
                let payload = semantic_type_name(program, optional.payload.kind, visiting);
                if matches!(optional.payload.kind, ResolvedTypeKind::Shared(_)) {
                    format!("({payload})?")
                } else {
                    format!("{payload}?")
                }
            },
        ),
    }
}

fn semantic_shared_target_name(
    program: &ResolvedProgram,
    target: ResolvedSharedTarget,
    visiting: &mut Vec<ClassId>,
) -> String {
    match target {
        ResolvedSharedTarget::Obj => "Obj".to_owned(),
        ResolvedSharedTarget::Class(class) => {
            semantic_type_name(program, ResolvedTypeKind::Class(class), visiting)
        }
        ResolvedSharedTarget::Interface(interface) => {
            semantic_type_name(program, ResolvedTypeKind::Interface(interface), visiting)
        }
        ResolvedSharedTarget::Array(array) => {
            semantic_type_name(program, ResolvedTypeKind::Array(array), visiting)
        }
        ResolvedSharedTarget::OptionalBox(optional_box) => {
            let Some(optional_box) = program.optional_box_types.get(optional_box) else {
                return optional_box.to_string();
            };
            if let Some(optional) = optional_box.optional {
                return semantic_type_name(program, ResolvedTypeKind::Optional(optional), visiting);
            }
            let mut name = optional_box.object_leaf.map_or_else(
                || "Obj".to_owned(),
                |leaf| match leaf {
                    ResolvedObjectTarget::Obj => "Obj".to_owned(),
                    ResolvedObjectTarget::Class(class) => {
                        semantic_type_name(program, ResolvedTypeKind::Class(class), visiting)
                    }
                    ResolvedObjectTarget::Interface(interface) => semantic_type_name(
                        program,
                        ResolvedTypeKind::Interface(interface),
                        visiting,
                    ),
                },
            );
            name.extend(std::iter::repeat_n('?', optional_box.optional_depth));
            name
        }
    }
}

fn qualified_name(program: &ResolvedProgram, module: ModuleId, name: &str) -> String {
    if program.modules.len() == 1 || name.contains("::") {
        return name.to_owned();
    }
    program.modules.get(module).map_or_else(
        || name.to_owned(),
        |module| format!("{}::{name}", module.module_path()),
    )
}

fn add_repeated_application_origins(
    mut diagnostic: Diagnostic,
    specialization: &GenericSpecialization,
) -> Diagnostic {
    for origin in specialization.provenance.origins.iter().skip(1) {
        diagnostic = diagnostic.with_secondary_label(
            origin.span,
            "the same invalid generic application is also used here",
        );
    }
    diagnostic
}

fn add_lifecycle_path(
    mut diagnostic: Diagnostic,
    program: &ResolvedProgram,
    path: &[crate::typeck::CopyPathElement],
) -> Diagnostic {
    if path.is_empty() {
        return diagnostic;
    }
    let mut names = Vec::with_capacity(path.len());
    for element in path {
        match *element {
            crate::typeck::CopyPathElement::Base(base) => {
                if let Some(base) = program.class(base) {
                    names.push(format!("base `{}`", base.name));
                    diagnostic = diagnostic.with_secondary_label(
                        base.name_span,
                        "unavailable lifecycle path enters this base class",
                    );
                }
            }
            crate::typeck::CopyPathElement::Field(field) => {
                if let Some(field) = program.field(field) {
                    names.push(format!("field `{}`", field.name));
                    diagnostic = diagnostic.with_secondary_label(
                        field.name_span,
                        "unavailable lifecycle path enters this field",
                    );
                }
            }
        }
    }
    if !names.is_empty() {
        diagnostic = diagnostic.with_note(format!(
            "lifecycle capability is unavailable through {}",
            names.join(" -> ")
        ));
    }
    diagnostic
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
