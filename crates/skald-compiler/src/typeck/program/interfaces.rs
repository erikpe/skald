//! Interface signature validation and deterministic conformance selection.

use std::collections::{HashMap, HashSet};

use crate::{
    diagnostics::{Diagnostic, Diagnostics},
    hir::{
        HirAccess, HirInterfaceConformance, HirInterfaceDeclaration, HirInterfaceParameter,
        HirInterfaceRequirement, HirRequirementImplementation,
    },
    identity::ClassId,
    resolve::{
        ResolvedClassMember, ResolvedInterfaceParameter, ResolvedMethodDeclaration,
        ResolvedProgram, ResolvedReceiverAccess,
    },
};

use super::{
    lower_parameter_mode, lower_type, INVALID_INTERFACE_CONFORMANCE, INVALID_INTERFACE_REQUIREMENT,
};

pub(super) struct InterfaceAnalysis {
    pub declarations: Vec<HirInterfaceDeclaration>,
    pub conformances: Vec<Vec<HirInterfaceConformance>>,
}

pub(super) fn analyze_interfaces(
    program: &ResolvedProgram,
    diagnostics: &mut Diagnostics,
) -> InterfaceAnalysis {
    let declarations = lower_interfaces(program, diagnostics);
    let mut conformances = vec![Vec::new(); program.classes.len()];
    for class in program.classes.iter() {
        let inherited_interfaces = program
            .hierarchy
            .base_chain(class.id)
            .into_iter()
            .flatten()
            .flat_map(|base| {
                program
                    .class(base)
                    .into_iter()
                    .flat_map(|class| class.implemented_interfaces.iter())
            })
            .map(|claim| claim.interface)
            .collect::<Vec<_>>();
        let mut selected = HashSet::new();
        for claim in &class.implemented_interfaces {
            let interface = program
                .interface(claim.interface)
                .expect("resolved interface claim must reference an interface");
            if inherited_interfaces.contains(&claim.interface) {
                diagnostics.push(
                    Diagnostic::error(
                        INVALID_INTERFACE_CONFORMANCE,
                        format!(
                            "class `{}` redundantly implements interface `{}`",
                            class.name, interface.name
                        ),
                    )
                    .with_primary_label(claim.span, "the direct base already conforms")
                    .with_note("inherited interface conformance is automatic"),
                );
            }
            if selected.insert(claim.interface) {
                if let Some(conformance) =
                    validate_conformance(program, class.id, interface, diagnostics)
                {
                    conformances[class.id.index()].push(conformance);
                }
            }
        }
        for interface_id in inherited_interfaces {
            if !selected.insert(interface_id) {
                continue;
            }
            let interface = program
                .interface(interface_id)
                .expect("inherited interface claim must reference an interface");
            if let Some(conformance) =
                validate_conformance(program, class.id, interface, diagnostics)
            {
                conformances[class.id.index()].push(conformance);
            }
        }
    }
    InterfaceAnalysis {
        declarations,
        conformances,
    }
}

fn lower_interfaces(
    program: &ResolvedProgram,
    diagnostics: &mut Diagnostics,
) -> Vec<HirInterfaceDeclaration> {
    program
        .interfaces
        .iter()
        .map(|interface| {
            let mut names = HashMap::new();
            let requirements = interface
                .requirements
                .iter()
                .map(|requirement| {
                    if let Some(previous) = names.insert(&requirement.name, requirement.name_span) {
                        diagnostics.push(
                            Diagnostic::error(
                                INVALID_INTERFACE_REQUIREMENT,
                                format!(
                                    "duplicate requirement `{}` in interface `{}`",
                                    requirement.name, interface.name
                                ),
                            )
                            .with_primary_label(requirement.name_span, "redeclared here")
                            .with_secondary_label(previous, "first declared here"),
                        );
                    }
                    validate_requirement_signature(program, requirement, diagnostics);
                    HirInterfaceRequirement {
                        id: requirement.id,
                        name: requirement.name.clone(),
                        name_span: requirement.name_span,
                        receiver_access: if requirement.mutable {
                            HirAccess::Mutable
                        } else {
                            HirAccess::ReadOnly
                        },
                        parameters: requirement
                            .parameters
                            .iter()
                            .map(|parameter| lower_interface_parameter(program, parameter))
                            .collect(),
                        return_type: lower_type(program, &requirement.return_type),
                        span: requirement.span,
                    }
                })
                .collect();
            HirInterfaceDeclaration {
                id: interface.id,
                module: interface.module,
                name: interface.name.clone(),
                name_span: interface.name_span,
                requirements,
                span: interface.span,
            }
        })
        .collect()
}

fn validate_requirement_signature(
    program: &ResolvedProgram,
    requirement: &crate::resolve::ResolvedInterfaceRequirement,
    diagnostics: &mut Diagnostics,
) {
    let mut parameter_names = HashMap::new();
    for parameter in &requirement.parameters {
        if let Some(previous) = parameter_names.insert(&parameter.name, parameter.name_span) {
            diagnostics.push(
                Diagnostic::error(
                    INVALID_INTERFACE_REQUIREMENT,
                    format!(
                        "duplicate parameter `{}` in interface requirement `{}`",
                        parameter.name, requirement.name
                    ),
                )
                .with_primary_label(parameter.name_span, "redeclared here")
                .with_secondary_label(previous, "first declared here"),
            );
        }
        let ty = lower_type(program, &parameter.type_syntax);
        let valid = match parameter.binding_mode {
            crate::resolve::ResolvedParameterBindingMode::Value => !matches!(
                ty,
                crate::hir::Type::Unit | crate::hir::Type::Obj | crate::hir::Type::Interface(_)
            ),
            crate::resolve::ResolvedParameterBindingMode::ReadOnlyAlias { .. }
            | crate::resolve::ResolvedParameterBindingMode::MutableAlias { .. } => {
                matches!(
                    ty,
                    crate::hir::Type::Class(_)
                        | crate::hir::Type::Obj
                        | crate::hir::Type::Interface(_)
                        | crate::hir::Type::Optional(_)
                )
            }
        };
        if !valid {
            diagnostics.push(
                Diagnostic::error(
                    INVALID_INTERFACE_REQUIREMENT,
                    format!(
                        "interface parameter `{}` has an invalid storage or alias type",
                        parameter.name
                    ),
                )
                .with_primary_label(
                    parameter.type_syntax.span,
                    "invalid requirement parameter type",
                ),
            );
        }
    }
    if matches!(
        lower_type(program, &requirement.return_type),
        crate::hir::Type::Obj | crate::hir::Type::Interface(_)
    ) {
        diagnostics.push(
            Diagnostic::error(
                INVALID_INTERFACE_REQUIREMENT,
                format!(
                    "interface requirement `{}` cannot return a non-owning view",
                    requirement.name
                ),
            )
            .with_primary_label(
                requirement.return_type.span,
                "non-owning views cannot escape a call",
            ),
        );
    }
}

fn lower_interface_parameter(
    program: &ResolvedProgram,
    parameter: &ResolvedInterfaceParameter,
) -> HirInterfaceParameter {
    HirInterfaceParameter {
        mode: lower_parameter_mode(parameter.binding_mode),
        name: parameter.name.clone(),
        name_span: parameter.name_span,
        ty: lower_type(program, &parameter.type_syntax),
        span: parameter.span,
    }
}

fn validate_conformance(
    program: &ResolvedProgram,
    class: ClassId,
    interface: &crate::resolve::ResolvedInterfaceDeclaration,
    diagnostics: &mut Diagnostics,
) -> Option<HirInterfaceConformance> {
    let class_declaration = program.class(class).expect("conforming class must exist");
    let mut valid = true;
    let mut implementations = Vec::with_capacity(interface.requirements.len());
    let mut diagnosed_names = HashSet::new();
    for requirement in &interface.requirements {
        let Some(ResolvedClassMember::Method(method_id)) =
            program.hierarchy.member(class, &requirement.name)
        else {
            if diagnosed_names.insert(&requirement.name) {
                diagnostics.push(
                    Diagnostic::error(
                        INVALID_INTERFACE_CONFORMANCE,
                        format!(
                            "class `{}` does not implement requirement `{}.{}`",
                            class_declaration.name, interface.name, requirement.name
                        ),
                    )
                    .with_primary_label(
                        class_declaration.name_span,
                        "required method is missing from this class and its bases",
                    )
                    .with_secondary_label(requirement.name_span, "requirement declared here"),
                );
            }
            valid = false;
            continue;
        };
        let method = program
            .method(method_id)
            .expect("selected method must exist");
        if method.kind == crate::resolve::ResolvedMethodKind::Static {
            diagnostics.push(
                Diagnostic::error(
                    INVALID_INTERFACE_CONFORMANCE,
                    format!(
                        "static method `{}` cannot implement `{}.{}`",
                        method.name, interface.name, requirement.name
                    ),
                )
                .with_primary_label(
                    method.name_span,
                    "static methods have no interface receiver",
                )
                .with_secondary_label(requirement.name_span, "requirement declared here"),
            );
            valid = false;
            continue;
        }
        if let Some(private_span) = method.visibility.private_span() {
            diagnostics.push(
                Diagnostic::error(
                    INVALID_INTERFACE_CONFORMANCE,
                    format!(
                        "private method `{}` cannot implement `{}.{}`",
                        method.name, interface.name, requirement.name
                    ),
                )
                .with_primary_label(
                    private_span,
                    "private methods do not satisfy interface requirements",
                )
                .with_secondary_label(requirement.name_span, "requirement declared here"),
            );
            valid = false;
            continue;
        }
        if let Some(reason) = signature_difference(method, requirement) {
            diagnostics.push(
                Diagnostic::error(
                    INVALID_INTERFACE_CONFORMANCE,
                    format!(
                        "method `{}` does not exactly implement `{}.{}`",
                        method.name, interface.name, requirement.name
                    ),
                )
                .with_primary_label(method.name_span, reason)
                .with_secondary_label(requirement.name_span, "requirement declared here")
                .with_note("receiver access, binding modes, parameter types, and result type must match exactly"),
            );
            valid = false;
            continue;
        }
        implementations.push(HirRequirementImplementation {
            requirement: requirement.id,
            method: method_id,
        });
    }
    valid.then_some(HirInterfaceConformance {
        interface: interface.id,
        implementations,
    })
}

fn signature_difference(
    method: &ResolvedMethodDeclaration,
    requirement: &crate::resolve::ResolvedInterfaceRequirement,
) -> Option<&'static str> {
    let expected_access = if requirement.mutable {
        ResolvedReceiverAccess::Mutable
    } else {
        ResolvedReceiverAccess::ReadOnly
    };
    if method.kind.receiver_access() != Some(expected_access) {
        return Some("receiver access differs from the interface requirement");
    }
    if method.parameters.len() != requirement.parameters.len() {
        return Some("parameter count differs from the interface requirement");
    }
    for (actual, expected) in method.parameters.iter().zip(&requirement.parameters) {
        if parameter_mode_kind(actual.binding_mode) != parameter_mode_kind(expected.binding_mode) {
            return Some("a parameter binding mode differs from the interface requirement");
        }
        if !super::same_resolved_type(&actual.type_syntax, &expected.type_syntax) {
            return Some("a parameter type differs from the interface requirement");
        }
    }
    (!super::same_resolved_type(&method.return_type, &requirement.return_type))
        .then_some("result type differs from the interface requirement")
}

const fn parameter_mode_kind(mode: crate::resolve::ResolvedParameterBindingMode) -> u8 {
    match mode {
        crate::resolve::ResolvedParameterBindingMode::Value => 0,
        crate::resolve::ResolvedParameterBindingMode::ReadOnlyAlias { .. } => 1,
        crate::resolve::ResolvedParameterBindingMode::MutableAlias { .. } => 2,
    }
}
