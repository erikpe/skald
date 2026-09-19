//! Narrow projection of existing checked target services; no machine products escape.

use super::{dispatch::DispatchMetadata, layout, runtime_trace};
use crate::{
    backend::{
        plan::*,
        planning::{TraceContext, TraceFacts, TraceLocation, TraceRequest},
        BackendError, BackendInput, RuntimeTracePolicy,
    },
    mir::*,
};
use std::collections::BTreeMap;

#[cfg_attr(not(test), allow(dead_code))]
pub(in crate::backend) struct SemanticProjection {
    layouts: layout::DataLayout,
    dispatch: DispatchMetadata,
}

pub(in crate::backend) fn begin_semantic_projection(
    input: BackendInput<'_>,
    types: &[MirType],
) -> Result<(Vec<LayoutFact>, SemanticProjection), BackendError> {
    let layouts = layout::DataLayout::compute(input.program())?;
    let projected = types
        .iter()
        .map(|ty| {
            let disposition = match ty {
                MirType::Unit => LayoutDisposition::ElidedUnit,
                MirType::Obj | MirType::Interface(_) => LayoutDisposition::ElidedMetadata,
                _ => LayoutDisposition::Addressable,
            };
            if disposition != LayoutDisposition::Addressable {
                return Ok(LayoutFact {
                    size: 0,
                    alignment: 1,
                    disposition,
                });
            }
            let layout = layouts.ty(*ty)?;
            Ok(LayoutFact {
                size: layout.size(),
                alignment: layout.alignment(),
                disposition,
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    let dispatch = DispatchMetadata::compute(input)?;
    Ok((projected, SemanticProjection { layouts, dispatch }))
}

pub(in crate::backend) fn finish_semantic_projection(
    projection: SemanticProjection,
    program: &MirProgram,
    type_layouts: &[(MirType, LayoutId)],
    requirement_signatures: &BTreeMap<crate::identity::InterfaceRequirementId, SignatureId>,
) -> Result<SemanticFacts, BackendError> {
    let layout_id = |ty: MirType| {
        type_layouts
            .iter()
            .find_map(|(candidate, id)| (*candidate == ty).then_some(*id))
            .expect("complete semantic type layout pool")
    };
    let types = type_layouts
        .iter()
        .map(|(ty, layout)| TypeLayoutBinding {
            ty: semantic_type(*ty),
            layout: *layout,
        })
        .collect();
    let shared_handle = type_layouts
        .iter()
        .find_map(|(ty, id)| matches!(ty, MirType::Shared(_)).then_some(*id))
        .expect("canonical shared-handle layout");

    let mut classes = Vec::with_capacity(program.classes.len());
    for declaration in program.classes.iter() {
        let target = projection
            .layouts
            .class(declaration.id)
            .expect("every class was laid out");
        let fields = declaration
            .fields
            .iter()
            .zip(target.fields())
            .map(|(field, physical)| FieldLayoutFact {
                field: field.id,
                ty: semantic_type(field.ty),
                layout: layout_id(field.ty),
                offset: physical.offset,
            })
            .collect();
        let allocation = projection.layouts.shared_allocation(declaration.id)?;
        let class_layout = layout_id(MirType::Class(declaration.id));
        classes.push(ClassLayoutFact {
            class: declaration.id,
            exact_layout: class_layout,
            complete_layout: class_layout,
            base: target.base().map(|base| BaseLayoutFact {
                class: base.class,
                offset: base.offset,
            }),
            fields,
            shared_allocation: SharedAllocationLayout {
                byte_count: allocation.byte_count(),
                payload_offset: allocation.payload_offset(),
            },
            destruction: declaration
                .destruction
                .steps
                .iter()
                .copied()
                .map(destruction_step)
                .collect(),
        });
    }

    let optionals = program
        .optional_types
        .iter()
        .map(|optional| {
            let target = projection.layouts.optional_type(optional.id)?;
            Ok(OptionalLayoutFact {
                optional: optional.id,
                payload: semantic_type(optional.payload),
                storage: optional_storage(optional.storage),
                layout: layout_id(MirType::Optional(optional.id)),
                payload_layout: layout_id(optional.payload),
                state_offset: (!target.is_nullable_niche()).then_some(target.state_offset()),
                payload_offset: target.payload_offset(),
                nullable_niche: target.is_nullable_niche(),
            })
        })
        .collect::<Result<Vec<_>, BackendError>>()?;

    let arrays = program
        .array_types
        .iter()
        .map(|array| {
            let target = projection
                .layouts
                .array(array.id)
                .expect("every array was laid out");
            ArrayLayoutFact {
                array: array.id,
                descriptor_layout: layout_id(MirType::Array(array.id)),
                element: semantic_type(array.element),
                element_layout: layout_id(array.element),
                element_offset: target.element_offset(),
                shared_element_offset: target.shared_element_offset(),
                stride: target.stride(),
                maximum_length: target.maximum_length(),
                shared_maximum_length: target.shared_maximum_length(),
                default: array.lifecycle.default.map(array_default),
                copy: array.lifecycle.copy.map(array_copy),
                assignment: array.lifecycle.assignment.map(array_assignment),
                destruction: array_destruction(array.lifecycle.destruction),
            }
        })
        .collect();

    let optional_boxes = program
        .optional_box_types
        .iter()
        .map(|item| {
            let allocation = item
                .exact_optional
                .map(|_| projection.layouts.exact_optional_box(item.id))
                .transpose()?;
            Ok(OptionalBoxLayoutFact {
                optional_box: item.id,
                exact_optional: item.exact_optional,
                exact_dynamic_class: item.exact_dynamic_class,
                object_view: item.object_view.map(object_view_target),
                layer_offsets: projection
                    .layouts
                    .optional_object_box_layer_offsets(item.id)
                    .unwrap_or_default()
                    .to_vec(),
                payload_offset: projection
                    .layouts
                    .optional_object_box_layer_offsets(item.id)
                    .map(|_| {
                        projection
                            .layouts
                            .optional_object_box_payload_offset(item.id)
                    })
                    .transpose()?,
                allocation: allocation.map(|allocation| SharedAllocationLayout {
                    byte_count: allocation.byte_count(),
                    payload_offset: allocation.payload_offset(),
                }),
            })
        })
        .collect::<Result<Vec<_>, BackendError>>()?;

    let virtual_families = program
        .virtual_families
        .iter()
        .map(|family| VirtualFamilyFact {
            family: family.id,
            slot: family.slot,
            root: family.root,
            members: family.members.clone(),
        })
        .collect();
    let interfaces = program
        .interfaces
        .iter()
        .map(|interface| InterfaceFact {
            interface: interface.id,
            requirements: interface
                .requirements
                .iter()
                .map(|requirement| InterfaceRequirementFact {
                    requirement: requirement.id,
                    signature: requirement_signatures[&requirement.id],
                })
                .collect(),
        })
        .collect();
    let conformances = program
        .classes
        .iter()
        .flat_map(|class| {
            class
                .conformances
                .iter()
                .map(move |conformance| ConformanceFact {
                    class: class.id,
                    interface: conformance.interface,
                    implementations: conformance
                        .implementations
                        .iter()
                        .map(|implementation| RequirementImplementationFact {
                            requirement: implementation.requirement,
                            method: implementation.method,
                        })
                        .collect(),
                })
        })
        .collect();
    let mut method_slots = program
        .virtual_families
        .iter()
        .map(|family| MethodSlot::Virtual(family.id))
        .chain(program.interfaces.iter().flat_map(|interface| {
            interface
                .requirements
                .iter()
                .map(|requirement| MethodSlot::Interface(requirement.id))
        }))
        .enumerate()
        .map(|(index, slot)| MethodSlotFact {
            slot,
            index,
            byte_offset: index * 8,
        })
        .collect::<Vec<_>>();
    let finalizer_index = projection.dispatch.finalizer_index();
    method_slots.push(MethodSlotFact {
        slot: MethodSlot::Finalizer,
        index: finalizer_index,
        byte_offset: finalizer_index * 8,
    });
    let dispatch_tables = program
        .classes
        .iter()
        .map(|class| ClassDispatchFact {
            class: class.id,
            targets: projection
                .dispatch
                .class_entries(class.id)
                .expect("class table")
                .iter()
                .map(|method| method.map(|method| LirCallableId::Source(method.into())))
                .collect(),
        })
        .collect();
    let object_views = program
        .classes
        .iter()
        .map(|class| MirViewTarget::Class(class.id))
        .chain(
            program
                .interfaces
                .iter()
                .map(|interface| MirViewTarget::Interface(interface.id)),
        )
        .chain([MirViewTarget::Obj])
        .map(|target| ObjectViewFact {
            target: object_view_target(target),
            components: vec![
                ObjectComponent::StaticAddress,
                ObjectComponent::CompleteAddress,
                ObjectComponent::DynamicMetadata,
            ],
            members: projection.dispatch.classes_providing_view(program, target),
        })
        .collect();

    Ok(SemanticFacts {
        types,
        shared_header: Some(SharedHeaderLayout {
            handle_layout: shared_handle,
            owner_count_offset: 0,
            dynamic_metadata_offset: layout::SHARED_DYNAMIC_METADATA_OFFSET as usize,
            header_size: layout::SHARED_HEADER_SIZE,
        }),
        classes,
        optionals,
        optional_boxes,
        arrays,
        object_views,
        virtual_families,
        interfaces,
        conformances,
        method_slots,
        dispatch_tables,
    })
}

fn semantic_type(ty: MirType) -> SemanticType {
    match ty {
        MirType::I64 => SemanticType::I64,
        MirType::U64 => SemanticType::U64,
        MirType::U8 => SemanticType::U8,
        MirType::F64 => SemanticType::F64,
        MirType::Bool => SemanticType::Bool,
        MirType::Function(id) => SemanticType::Function(id),
        MirType::Array(id) => SemanticType::Array(id),
        MirType::Class(id) => SemanticType::Class(id),
        MirType::Interface(id) => SemanticType::Interface(id),
        MirType::Obj => SemanticType::Obj,
        MirType::Shared(target) => SemanticType::Shared(shared_target(target)),
        MirType::Optional(id) => SemanticType::Optional(id),
        MirType::Unit => SemanticType::Unit,
    }
}

fn shared_target(target: MirSharedTarget) -> SharedTarget {
    match target {
        MirSharedTarget::Obj => SharedTarget::Obj,
        MirSharedTarget::Class(id) => SharedTarget::Class(id),
        MirSharedTarget::Interface(id) => SharedTarget::Interface(id),
        MirSharedTarget::Array(id) => SharedTarget::Array(id),
        MirSharedTarget::OptionalBox(id) => SharedTarget::OptionalBox(id),
    }
}

fn object_view_target(target: MirViewTarget) -> ObjectViewTarget {
    match target {
        MirViewTarget::Class(id) => ObjectViewTarget::Class(id),
        MirViewTarget::Interface(id) => ObjectViewTarget::Interface(id),
        MirViewTarget::Obj => ObjectViewTarget::Obj,
    }
}

fn optional_storage(storage: MirOptionalStorage) -> OptionalStorageFact {
    match storage {
        MirOptionalStorage::Scalar => OptionalStorageFact::Scalar,
        MirOptionalStorage::InlineClass(id) => OptionalStorageFact::InlineClass(id),
        MirOptionalStorage::InlineArray(id) => OptionalStorageFact::InlineArray(id),
        MirOptionalStorage::SharedOwner(target) => {
            OptionalStorageFact::SharedOwner(shared_target(target))
        }
        MirOptionalStorage::Nested(id) => OptionalStorageFact::Nested(id),
    }
}

fn selected_copy<I>(operation: MirSelectedCopyOperation<I>) -> SelectedCopy<I> {
    match operation {
        MirSelectedCopyOperation::User(id) => SelectedCopy::User(id),
        MirSelectedCopyOperation::Synthesized(class) => SelectedCopy::Synthesized(class),
    }
}

fn destruction_step(step: MirDestructionStep) -> DestructionStepFact {
    match step {
        MirDestructionStep::UserBody(id) => DestructionStepFact::UserBody(id),
        MirDestructionStep::Field(id) => DestructionStepFact::Field(id),
        MirDestructionStep::SharedField(id) => DestructionStepFact::SharedField(id),
        MirDestructionStep::OptionalSharedField(id) => DestructionStepFact::OptionalSharedField(id),
        MirDestructionStep::OptionalClassField(id) => DestructionStepFact::OptionalClassField(id),
        MirDestructionStep::OptionalField { field, optional } => {
            DestructionStepFact::OptionalField { field, optional }
        }
        MirDestructionStep::ArrayField(id) => DestructionStepFact::ArrayField(id),
        MirDestructionStep::Base(id) => DestructionStepFact::Base(id),
    }
}

fn array_default(value: MirArrayDefaultElement) -> ArrayDefaultElementFact {
    match value {
        MirArrayDefaultElement::Primitive => ArrayDefaultElementFact::Primitive,
        MirArrayDefaultElement::OptionalAbsent => ArrayDefaultElementFact::OptionalAbsent,
        MirArrayDefaultElement::Class { class, initializer } => {
            ArrayDefaultElementFact::Class { class, initializer }
        }
        MirArrayDefaultElement::ArrayEmpty(id) => ArrayDefaultElementFact::ArrayEmpty(id),
        MirArrayDefaultElement::SharedClass { class, initializer } => {
            ArrayDefaultElementFact::SharedClass { class, initializer }
        }
        MirArrayDefaultElement::SharedArrayEmpty(id) => {
            ArrayDefaultElementFact::SharedArrayEmpty(id)
        }
        MirArrayDefaultElement::SharedOptionalBoxAbsent(id) => {
            ArrayDefaultElementFact::SharedOptionalBoxAbsent(id)
        }
    }
}

fn array_copy(value: MirArrayCopyElement) -> ArrayCopyElementFact {
    match value {
        MirArrayCopyElement::Primitive => ArrayCopyElementFact::Primitive,
        MirArrayCopyElement::OptionalPrimitive => ArrayCopyElementFact::OptionalPrimitive,
        MirArrayCopyElement::Class { class, operation } => ArrayCopyElementFact::Class {
            class,
            operation: selected_copy(operation),
        },
        MirArrayCopyElement::OptionalClass { class, operation } => {
            ArrayCopyElementFact::OptionalClass {
                class,
                operation: selected_copy(operation),
            }
        }
        MirArrayCopyElement::Array(id) => ArrayCopyElementFact::Array(id),
        MirArrayCopyElement::Shared(target) => ArrayCopyElementFact::Shared(shared_target(target)),
        MirArrayCopyElement::OptionalShared(target) => {
            ArrayCopyElementFact::OptionalShared(shared_target(target))
        }
        MirArrayCopyElement::Optional(id) => ArrayCopyElementFact::Optional(id),
    }
}

fn array_assignment(value: MirArrayAssignElement) -> ArrayAssignElementFact {
    match value {
        MirArrayAssignElement::Primitive => ArrayAssignElementFact::Primitive,
        MirArrayAssignElement::OptionalPrimitive => ArrayAssignElementFact::OptionalPrimitive,
        MirArrayAssignElement::Class { class, operation } => ArrayAssignElementFact::Class {
            class,
            operation: selected_copy(operation),
        },
        MirArrayAssignElement::OptionalClass {
            class,
            copy_constructor,
            copy_assignment,
        } => ArrayAssignElementFact::OptionalClass {
            class,
            copy_constructor: selected_copy(copy_constructor),
            copy_assignment: selected_copy(copy_assignment),
        },
        MirArrayAssignElement::Array(id) => ArrayAssignElementFact::Array(id),
        MirArrayAssignElement::Shared(target) => {
            ArrayAssignElementFact::Shared(shared_target(target))
        }
        MirArrayAssignElement::OptionalShared(target) => {
            ArrayAssignElementFact::OptionalShared(shared_target(target))
        }
        MirArrayAssignElement::Optional(id) => ArrayAssignElementFact::Optional(id),
    }
}

fn array_destruction(value: MirArrayDestroyElement) -> ArrayDestroyElementFact {
    match value {
        MirArrayDestroyElement::Trivial => ArrayDestroyElementFact::Trivial,
        MirArrayDestroyElement::Class(id) => ArrayDestroyElementFact::Class(id),
        MirArrayDestroyElement::OptionalClass(id) => ArrayDestroyElementFact::OptionalClass(id),
        MirArrayDestroyElement::Array(id) => ArrayDestroyElementFact::Array(id),
        MirArrayDestroyElement::Shared(target) => {
            ArrayDestroyElementFact::Shared(shared_target(target))
        }
        MirArrayDestroyElement::OptionalShared(target) => {
            ArrayDestroyElementFact::OptionalShared(shared_target(target))
        }
        MirArrayDestroyElement::Optional(id) => ArrayDestroyElementFact::Optional(id),
    }
}

#[cfg_attr(not(test), allow(dead_code))]
pub(in crate::backend) fn project_trace(
    input: BackendInput<'_>,
) -> Result<TraceFacts, BackendError> {
    if input.runtime_trace() == RuntimeTracePolicy::Omitted {
        return Ok(TraceFacts::default());
    }
    let metadata = runtime_trace::Metadata::new(input);
    let mut requests = Vec::new();
    for definition in input.program().executable_definitions() {
        let callable = definition.callable();
        for span in std::iter::once(definition.span()).chain(
            definition.body().blocks.iter().flat_map(|block| {
                block
                    .instructions
                    .iter()
                    .map(|i| i.span())
                    .chain(block.terminator.iter().map(|t| t.span()))
            }),
        ) {
            let key = metadata
                .request_location(callable, span)?
                .expect("enabled metadata");
            requests.push((callable, span, key));
        }
    }
    let metadata = metadata.finish();
    let strings = metadata
        .strings
        .iter()
        .enumerate()
        .map(|(index, string)| (string.symbol.as_str(), index))
        .collect::<BTreeMap<_, _>>();
    let contexts = metadata
        .contexts
        .iter()
        .enumerate()
        .map(|(index, context)| (context.symbol.as_str(), index))
        .collect::<BTreeMap<_, _>>();
    let locations = metadata
        .locations
        .iter()
        .enumerate()
        .map(|(index, location)| (location.symbol.as_str(), index))
        .collect::<BTreeMap<_, _>>();
    Ok(TraceFacts {
        record_layout: Some(LayoutFact {
            size: super::frame::TRACE_RECORD_SIZE,
            alignment: super::frame::TRACE_RECORD_ALIGNMENT,
            disposition: LayoutDisposition::Addressable,
        }),
        strings: metadata.strings.iter().map(|s| s.bytes.clone()).collect(),
        contexts: metadata
            .contexts
            .iter()
            .map(|context| TraceContext {
                name: DataKey::TraceBytes(strings[context.name_symbol.as_str()]),
                path: DataKey::TraceBytes(strings[context.path_symbol.as_str()]),
            })
            .collect(),
        locations: metadata
            .locations
            .iter()
            .map(|location| TraceLocation {
                context: DataKey::TraceContext(contexts[location.context_symbol.as_str()]),
                line: location.line,
                column: location.column,
            })
            .collect(),
        requests: requests
            .into_iter()
            .map(|(callable, span, key)| TraceRequest {
                callable,
                span,
                location: DataKey::TraceLocation(locations[key.as_str()]),
            })
            .collect(),
    })
}
