//! Closure of definition-site bound selections to ordinary interface IDs.

use super::*;
use crate::identity::TypeParameterId;

pub(in crate::resolve::resolver::program) fn close_bound_member_selections(
    semantics: &ResolvedClassTemplateSemanticTable,
    class_specializations: &mut GenericSpecializationTable,
    interface_specializations: &GenericInterfaceSpecializationTable,
    operator_language_item: Option<&ResolvedOperatorLanguageItem>,
    range_language_item: Option<&ResolvedRangeLanguageItem>,
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
                    parameter,
                    bound,
                    requirement,
                    ..
                } => {
                    let interface = closed_bound_interface(specialization, *bound);
                    let template_requirement = *requirement;
                    let requirement = close_requirement(
                        interface,
                        template_requirement,
                        interface_specializations,
                    );
                    let operation = primitive_bound_operation(
                        specialization,
                        *parameter,
                        interface,
                        template_requirement,
                        interface_specializations,
                        operator_language_item,
                        range_language_item,
                    );
                    let receiver = specialization.key.arguments[parameter.index()];
                    let closed = if is_primitive(receiver) {
                        operation.map(|operation| ClosedGenericBoundMember::PrimitiveIntrinsic {
                            operation,
                        })
                    } else {
                        Some(ClosedGenericBoundMember::Interface {
                            interface,
                            requirement,
                        })
                    };
                    specialization.closed_bound_members[selection_index] = closed;
                }
                ResolvedTemplateSelection::Operator(selection) => {
                    let interface = closed_bound_interface(specialization, selection.bound);
                    let requirement = close_requirement(
                        interface,
                        ResolvedTemplateBoundRequirement::Generic(selection.requirement),
                        interface_specializations,
                    );
                    let application = interface_specializations
                        .for_interface(interface)
                        .expect("generic operator bounds close to materialized interfaces");
                    let (rhs, output) =
                        closed_operator_arguments(selection.protocol, &application.key.arguments);
                    let receiver = specialization.key.arguments[selection.parameter.index()];
                    let operation =
                        primitive_operator_operation(receiver, selection.protocol, rhs, output);
                    specialization.closed_operator_selections[selection_index] =
                        if is_primitive(receiver) {
                            operation.map(|operation| {
                                ClosedGenericOperatorSelection::PrimitiveIntrinsic { operation }
                            })
                        } else {
                            Some(ClosedGenericOperatorSelection::ClassWitness {
                                interface,
                                requirement,
                                rhs,
                                output,
                                origin_span: selection.origin_span,
                            })
                        };
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

const fn is_primitive(ty: ResolvedTypeKind) -> bool {
    matches!(
        ty,
        ResolvedTypeKind::I64
            | ResolvedTypeKind::U64
            | ResolvedTypeKind::U8
            | ResolvedTypeKind::F64
            | ResolvedTypeKind::Bool
    )
}

fn primitive_bound_operation(
    specialization: &GenericSpecialization,
    parameter: TypeParameterId,
    interface: InterfaceId,
    requirement: ResolvedTemplateBoundRequirement,
    interface_specializations: &GenericInterfaceSpecializationTable,
    operator_language_item: Option<&ResolvedOperatorLanguageItem>,
    range_language_item: Option<&ResolvedRangeLanguageItem>,
) -> Option<ResolvedPrimitiveBoundOperation> {
    let ResolvedTemplateBoundRequirement::Generic(requirement) = requirement else {
        return None;
    };
    let receiver = specialization.key.arguments[parameter.index()];
    if let Some(protocol) = operator_language_item
        .into_iter()
        .flat_map(ResolvedOperatorLanguageItem::iter)
        .find(|protocol| protocol.requirement == requirement)
    {
        let application = interface_specializations.for_interface(interface)?;
        let (rhs, output) = closed_operator_arguments(protocol.kind, &application.key.arguments);
        return primitive_operator_operation(receiver, protocol.kind, rhs, output)
            .map(ResolvedPrimitiveBoundOperation::Operator);
    }
    let range = range_language_item?;
    (range.successor_requirement == requirement)
        .then(|| {
            primitive_successor_operation(
                receiver,
                interface,
                range.successor_template,
                interface_specializations,
            )
        })
        .flatten()
}

fn closed_operator_arguments(
    protocol: CanonicalOperatorProtocol,
    arguments: &[ResolvedTypeKind],
) -> (Option<ResolvedTypeKind>, ResolvedTypeKind) {
    match protocol.shape() {
        CanonicalOperatorProtocolShape::Unary => {
            let [output] = arguments else {
                unreachable!("validated unary operator applications have one argument")
            };
            (None, *output)
        }
        CanonicalOperatorProtocolShape::Binary => {
            let [rhs, output] = arguments else {
                unreachable!("validated binary operator applications have two arguments")
            };
            (Some(*rhs), *output)
        }
        CanonicalOperatorProtocolShape::Predicate => {
            let [rhs] = arguments else {
                unreachable!("validated predicate operator applications have one argument")
            };
            (Some(*rhs), ResolvedTypeKind::Bool)
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
