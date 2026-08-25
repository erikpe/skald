//! Closure of definition-site bound selections to ordinary interface IDs.

use super::*;

pub(in crate::resolve::resolver::program) fn close_bound_member_selections(
    semantics: &ResolvedClassTemplateSemanticTable,
    class_specializations: &mut GenericSpecializationTable,
    interface_specializations: &GenericInterfaceSpecializationTable,
) {
    for specialization in class_specializations.iter_mut() {
        if !matches!(
            specialization.state,
            GenericSpecializationState::Complete(_)
        ) {
            continue;
        }
        let semantics = semantics
            .get(specialization.key.template)
            .expect("class specialization references template semantics");
        for (selection_index, selection) in semantics.selections.iter().enumerate() {
            match selection {
                ResolvedTemplateSelection::BoundMember {
                    bound, requirement, ..
                } => {
                    let interface = closed_bound_interface(specialization, *bound);
                    let requirement =
                        close_requirement(interface, *requirement, interface_specializations);
                    specialization.closed_bound_members[selection_index] =
                        Some(ClosedGenericBoundMember {
                            interface,
                            requirement,
                        });
                }
                ResolvedTemplateSelection::Iteration {
                    bound,
                    iter_state,
                    iter_next,
                    ..
                } => {
                    let interface = closed_bound_interface(specialization, *bound);
                    let application = interface_specializations
                        .for_interface(interface)
                        .expect("a generic iteration bound closes to a materialized interface");
                    let [item, state] = application.key.arguments.as_slice() else {
                        unreachable!("validated Iterable applications have two arguments")
                    };
                    let close = |requirement| {
                        application
                            .requirement_mappings
                            .iter()
                            .find(|mapping| mapping.template == requirement)
                            .map(|mapping| mapping.closed)
                            .expect("materialized Iterable maps every requirement")
                    };
                    specialization.closed_iteration_selections[selection_index] =
                        Some(ClosedGenericIterationSelection {
                            interface,
                            iter_state: close(*iter_state),
                            iter_next: close(*iter_next),
                            item: *item,
                            state: *state,
                            origin_span: semantics.bounds[*bound].interface_span,
                        });
                }
                ResolvedTemplateSelection::TopLevel { .. }
                | ResolvedTemplateSelection::TemplateMember { .. }
                | ResolvedTemplateSelection::DefinitionSite { .. }
                | ResolvedTemplateSelection::ArgumentDependent { .. } => {}
            }
        }
    }
}

fn closed_bound_interface(specialization: &GenericSpecialization, bound: usize) -> InterfaceId {
    specialization
        .closed_interface_bounds
        .get(bound)
        .copied()
        .flatten()
        .expect("complete class specialization closes every interface bound")
}

fn close_requirement(
    interface: InterfaceId,
    requirement: ResolvedTemplateBoundRequirement,
    interface_specializations: &GenericInterfaceSpecializationTable,
) -> InterfaceRequirementId {
    match requirement {
        ResolvedTemplateBoundRequirement::Ordinary(requirement) => {
            assert_eq!(
                requirement.interface(),
                interface,
                "ordinary bound requirement belongs to its closed interface"
            );
            requirement
        }
        ResolvedTemplateBoundRequirement::Generic(template_requirement) => {
            interface_specializations
                .for_interface(interface)
                .and_then(|specialization| {
                    specialization
                        .requirement_mappings
                        .iter()
                        .find(|mapping| mapping.template == template_requirement)
                })
                .map(|mapping| mapping.closed)
                .expect("materialized interface maps every template requirement")
        }
    }
}
