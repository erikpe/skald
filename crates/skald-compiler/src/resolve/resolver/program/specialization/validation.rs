//! Validation and atomic publication of closed specializations.

use super::*;

pub(crate) fn validate_specialization_requirements(
    program: &mut ResolvedProgram,
    diagnostics: &mut Diagnostics,
    ordinary_class_count: usize,
    ordinary_hierarchy: ResolvedClassHierarchy,
) {
    let failures = crate::typeck::failed_specialization_requirements(program);
    if failures.is_empty() {
        return;
    }

    for (class, requirement_index) in &failures {
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
    let failed_classes = failures
        .iter()
        .map(|(class, _)| *class)
        .collect::<std::collections::BTreeSet<_>>();
    for class in failed_classes {
        program.generic_specializations.fail_class(class);
    }
    program.classes.truncate(ordinary_class_count);
    program.class_definitions.truncate(ordinary_class_count);
    program.hierarchy = ordinary_hierarchy;
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
