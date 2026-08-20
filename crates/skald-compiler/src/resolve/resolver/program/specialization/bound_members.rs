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
            let ResolvedTemplateSelection::BoundMember {
                bound, requirement, ..
            } = selection
            else {
                continue;
            };
            let interface = specialization
                .closed_interface_bounds
                .get(*bound)
                .copied()
                .flatten()
                .expect("complete class specialization closes every interface bound");
            let requirement = match requirement {
                ResolvedTemplateBoundRequirement::Ordinary(requirement) => {
                    assert_eq!(
                        requirement.interface(),
                        interface,
                        "ordinary bound requirement belongs to its closed interface"
                    );
                    *requirement
                }
                ResolvedTemplateBoundRequirement::Generic(template_requirement) => {
                    interface_specializations
                        .for_interface(interface)
                        .and_then(|specialization| {
                            specialization
                                .requirement_mappings
                                .iter()
                                .find(|mapping| mapping.template == *template_requirement)
                        })
                        .map(|mapping| mapping.closed)
                        .expect("materialized interface maps every template requirement")
                }
            };
            specialization.closed_bound_members[selection_index] = Some(ClosedGenericBoundMember {
                interface,
                requirement,
            });
        }
    }
}
