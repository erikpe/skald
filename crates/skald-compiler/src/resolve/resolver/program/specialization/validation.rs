//! Validation and atomic publication of closed specializations.

use super::*;
use crate::identity::{ArrayTypeId, FunctionTypeId, OptionalBoxTypeId, OptionalTypeId};

pub(crate) fn validate_specialization_requirements(
    program: &mut ResolvedProgram,
    diagnostics: &mut Diagnostics,
    ordinary_class_count: usize,
    ordinary_hierarchy: ResolvedClassHierarchy,
    ordinary_classes: ResolvedClassDeclarationTable,
) {
    let bound_failures = failed_exact_bounds(program);
    let duplicate_bound_failures = duplicate_closed_bounds(program);
    let requirement_failures = crate::typeck::failed_specialization_requirements(program);
    if bound_failures.is_empty()
        && duplicate_bound_failures.is_empty()
        && requirement_failures.is_empty()
    {
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
        let interface_id = specialization.closed_interface_bounds[*bound_index]
            .expect("complete specialization closes every interface bound");
        let interface = program
            .interface(interface_id)
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
            bound_failure_label(program, argument, interface_id, &interface.name),
        )
        .with_secondary_label(bound.span, "bound declared here")
        .with_secondary_label(interface.name_span, "interface declared here")
        .with_secondary_label(template.name_span, "template declared here")
        .with_note(bound_satisfaction_note());
        diagnostic = add_repeated_application_origins(diagnostic, specialization);
        diagnostics.push(diagnostic);
    }

    for (class, first_index, duplicate_index) in &duplicate_bound_failures {
        let specialization = program
            .generic_specializations
            .for_class(*class)
            .expect("duplicate bounds reference a specialization class");
        let semantics = program
            .template_semantics
            .get(specialization.key.template)
            .expect("specialization key references template semantics");
        let template = program
            .class_templates
            .get(specialization.key.template)
            .expect("specialization key references a template declaration");
        let interface = specialization.closed_interface_bounds[*duplicate_index]
            .expect("complete specialization closes every interface bound");
        let interface = program
            .interface(interface)
            .expect("closed bound references an interface");
        let origin = specialization
            .provenance
            .origins
            .first()
            .expect("requested specialization retains an origin");
        let mut diagnostic = Diagnostic::error(
            super::super::super::DUPLICATE_GENERIC_BOUND,
            format!(
                "type arguments for `{}` produce duplicate bound `{}`",
                application_name(program, specialization),
                interface.name
            ),
        )
        .with_primary_label(origin.span, "this closed application duplicates a bound")
        .with_secondary_label(
            semantics.bounds[*duplicate_index].span,
            "duplicate closed bound declared here",
        )
        .with_secondary_label(
            semantics.bounds[*first_index].span,
            "first equivalent closed bound declared here",
        )
        .with_secondary_label(template.name_span, "template declared here");
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

fn application_name(program: &ResolvedProgram, specialization: &GenericSpecialization) -> String {
    let template = program
        .class_templates
        .get(specialization.key.template)
        .expect("specialization keys reference collected templates");
    let context = ProgramTypeNameContext(program);
    let arguments =
        ResolvedTypeNameRenderer::new(&context).render_list(&specialization.key.arguments);
    format!(
        "{}<{arguments}>",
        qualified_name(program, template.module, &template.name)
    )
}

struct ProgramTypeNameContext<'program>(&'program ResolvedProgram);

impl ResolvedTypeNameContext for ProgramTypeNameContext<'_> {
    fn array(&self, id: ArrayTypeId) -> Option<&ResolvedArrayType> {
        self.0.array_types.get(id)
    }

    fn function(&self, id: FunctionTypeId) -> Option<&ResolvedFunctionType> {
        self.0.function_types.get(id)
    }

    fn optional(&self, id: OptionalTypeId) -> Option<&ResolvedOptionalType> {
        self.0.optional_types.get(id)
    }

    fn optional_box(&self, id: OptionalBoxTypeId) -> Option<&ResolvedOptionalBoxType> {
        self.0.optional_box_types.get(id)
    }

    fn direct_class_name(&self, id: ClassId) -> Option<String> {
        self.0
            .class(id)
            .map(|class| qualified_name(self.0, class.module, &class.name))
    }

    fn class_specialization(&self, id: ClassId) -> Option<&GenericClassInstanceKey> {
        self.0
            .generic_specializations
            .for_class(id)
            .map(|specialization| &specialization.key)
    }

    fn template_name(&self, id: ClassTemplateId) -> Option<String> {
        self.0
            .class_templates
            .get(id)
            .map(|template| qualified_name(self.0, template.module, &template.name))
    }

    fn interface_name(&self, id: InterfaceId) -> Option<String> {
        self.0
            .interface(id)
            .map(|interface| qualified_name(self.0, interface.module, &interface.name))
    }

    fn missing_optional_box_leaf_name(&self, _id: OptionalBoxTypeId) -> String {
        "Obj".to_owned()
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

fn failed_exact_bounds(program: &ResolvedProgram) -> Vec<(ClassId, usize)> {
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
            let interface = specialization.closed_interface_bounds[bound_index]
                .expect("complete specialization closes every interface bound");
            if (0..bound_index).any(|previous| {
                semantics.bounds[previous].parameter == bound.parameter
                    && specialization.closed_interface_bounds[previous] == Some(interface)
            }) {
                continue;
            }
            let argument = specialization.key.arguments[bound.parameter.index()];
            let satisfied = exact_bound_is_satisfied(program, argument, interface);
            if !satisfied {
                failures.push((class, bound_index));
            }
        }
    }
    failures
}

fn duplicate_closed_bounds(program: &ResolvedProgram) -> Vec<(ClassId, usize, usize)> {
    let mut failures = Vec::new();
    for specialization in program.generic_specializations.iter() {
        let GenericSpecializationState::Complete(class) = specialization.state else {
            continue;
        };
        let semantics = program
            .template_semantics
            .get(specialization.key.template)
            .expect("specialization key references template semantics");
        for duplicate in 0..semantics.bounds.len() {
            let bound = &semantics.bounds[duplicate];
            let interface = specialization.closed_interface_bounds[duplicate];
            if let Some(first) = (0..duplicate).find(|first| {
                semantics.bounds[*first].parameter == bound.parameter
                    && specialization.closed_interface_bounds[*first] == interface
            }) {
                failures.push((class, first, duplicate));
            }
        }
    }
    failures
}

pub(super) fn effective_nominal_conformance(
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
                    .any(|claim| claim.interface.ordinary() == Some(interface))
            })
        })
}

pub(super) fn exact_bound_is_satisfied(
    program: &ResolvedProgram,
    argument: ResolvedTypeKind,
    interface: InterfaceId,
) -> bool {
    match argument {
        ResolvedTypeKind::Class(argument_class) => {
            effective_nominal_conformance(program, argument_class, interface)
        }
        ResolvedTypeKind::I64
        | ResolvedTypeKind::U64
        | ResolvedTypeKind::U8
        | ResolvedTypeKind::F64
        | ResolvedTypeKind::Bool => {
            primitive_operator_evidence(program, argument, interface).is_some()
        }
        ResolvedTypeKind::Unit
        | ResolvedTypeKind::Obj
        | ResolvedTypeKind::Function(_)
        | ResolvedTypeKind::Interface(_)
        | ResolvedTypeKind::Shared(_)
        | ResolvedTypeKind::Optional(_)
        | ResolvedTypeKind::Array(_) => false,
    }
}

pub(super) const fn bound_satisfaction_note() -> &'static str {
    "generic bounds accept exact classes with effective nominal conformance and primitives with compiler-provided evidence for an exact canonical operator application"
}

pub(super) fn bound_failure_label(
    program: &ResolvedProgram,
    argument: ResolvedTypeKind,
    interface_id: InterfaceId,
    interface: &str,
) -> String {
    match argument {
        ResolvedTypeKind::Class(_) => format!(
            "{} does not provide effective nominal conformance to `{interface}`",
            argument_kind_name(argument)
        ),
        ResolvedTypeKind::I64
        | ResolvedTypeKind::U64
        | ResolvedTypeKind::U8
        | ResolvedTypeKind::F64
        | ResolvedTypeKind::Bool
            if canonical_operator_application(program, interface_id) => format!(
            "{} has no compiler-provided evidence for the exact canonical application `{interface}`",
            argument_kind_name(argument)
        ),
        _ => format!(
            "{} cannot satisfy the exact interface bound `{interface}`",
            argument_kind_name(argument)
        ),
    }
}

pub(super) const fn argument_kind_name(argument: ResolvedTypeKind) -> &'static str {
    match argument {
        ResolvedTypeKind::Class(_) => "the exact class argument",
        ResolvedTypeKind::Interface(_) => "the non-owning interface argument",
        ResolvedTypeKind::Shared(_) => "the shared-owner argument",
        ResolvedTypeKind::Optional(_) => "the optional argument",
        ResolvedTypeKind::Array(_) => "the array argument",
        ResolvedTypeKind::Function(_) => "the function-type argument",
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
        GenericRequirementReason::InterfaceParameter {
            requirement,
            parameter,
        } => format!("parameter {parameter} of interface requirement {requirement}"),
        GenericRequirementReason::InterfaceResult { requirement } => {
            format!("the result of interface requirement {requirement}")
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
