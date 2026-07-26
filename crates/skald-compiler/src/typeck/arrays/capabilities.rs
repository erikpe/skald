//! Deterministic lifecycle plans for canonical array element types.

use crate::{
    hir::{
        HirArrayAssignElement, HirArrayCopyElement, HirArrayDefaultElement, HirArrayDestroyElement,
        HirArrayLifecycle, HirArrayType, HirArrayTypeTable, HirSharedTarget,
    },
    identity::ClassId,
    resolve::{ResolvedOptionalPayload, ResolvedProgram, ResolvedSharedTarget, ResolvedTypeKind},
};

use super::super::{capabilities::CopyCapabilities, program::lower_type};

pub(in crate::typeck) fn lower_array_types(
    program: &ResolvedProgram,
    class_capabilities: &CopyCapabilities,
) -> HirArrayTypeTable {
    let mut entries = Vec::with_capacity(program.array_types.len());
    for array in program.array_types.iter() {
        let element = lower_type(&array.element);
        entries.push(HirArrayType {
            id: array.id,
            element,
            lifecycle: HirArrayLifecycle {
                default: default_element(program, array.element.kind),
                copy: copy_element(class_capabilities, &entries, array.element.kind),
                assignment: assignment_element(class_capabilities, &entries, array.element.kind),
                destruction: destruction_element(array.element.kind),
            },
        });
    }
    HirArrayTypeTable::new(entries)
}

fn default_element(
    program: &ResolvedProgram,
    element: ResolvedTypeKind,
) -> Option<HirArrayDefaultElement> {
    match element {
        ResolvedTypeKind::I64
        | ResolvedTypeKind::U64
        | ResolvedTypeKind::U8
        | ResolvedTypeKind::F64
        | ResolvedTypeKind::Bool => Some(HirArrayDefaultElement::Primitive),
        ResolvedTypeKind::Optional { .. } | ResolvedTypeKind::OptionalShared { .. } => {
            Some(HirArrayDefaultElement::OptionalAbsent)
        }
        ResolvedTypeKind::Class(class) => zero_argument_initializer(program, class)
            .map(|initializer| HirArrayDefaultElement::Class { class, initializer }),
        ResolvedTypeKind::Array(array) => Some(HirArrayDefaultElement::ArrayEmpty(array)),
        ResolvedTypeKind::Shared(ResolvedSharedTarget::Class(class)) => {
            zero_argument_initializer(program, class)
                .map(|initializer| HirArrayDefaultElement::SharedClass { class, initializer })
        }
        ResolvedTypeKind::Shared(ResolvedSharedTarget::Array(array)) => {
            Some(HirArrayDefaultElement::SharedArrayEmpty(array))
        }
        ResolvedTypeKind::Shared(
            ResolvedSharedTarget::Obj | ResolvedSharedTarget::Interface(_),
        )
        | ResolvedTypeKind::Unit
        | ResolvedTypeKind::Obj
        | ResolvedTypeKind::Interface(_) => None,
    }
}

fn zero_argument_initializer(
    program: &ResolvedProgram,
    class: ClassId,
) -> Option<crate::identity::InitializerId> {
    let mut candidates = program
        .class(class)?
        .initializers
        .iter()
        .filter(|initializer| initializer.parameters.is_empty());
    let selected = candidates.next()?.id;
    candidates.next().is_none().then_some(selected)
}

fn copy_element(
    capabilities: &CopyCapabilities,
    arrays: &[HirArrayType],
    element: ResolvedTypeKind,
) -> Option<HirArrayCopyElement> {
    match element {
        ResolvedTypeKind::I64
        | ResolvedTypeKind::U64
        | ResolvedTypeKind::U8
        | ResolvedTypeKind::F64
        | ResolvedTypeKind::Bool => Some(HirArrayCopyElement::Primitive),
        ResolvedTypeKind::Optional {
            payload:
                ResolvedOptionalPayload::I64
                | ResolvedOptionalPayload::U64
                | ResolvedOptionalPayload::U8
                | ResolvedOptionalPayload::F64
                | ResolvedOptionalPayload::Bool,
            ..
        } => Some(HirArrayCopyElement::OptionalPrimitive),
        ResolvedTypeKind::Class(class) => capabilities
            .constructor(class)
            .selected()
            .map(|operation| HirArrayCopyElement::Class { class, operation }),
        ResolvedTypeKind::Optional {
            payload: ResolvedOptionalPayload::Class(class),
            ..
        } => capabilities
            .constructor(class)
            .selected()
            .map(|operation| HirArrayCopyElement::OptionalClass { class, operation }),
        ResolvedTypeKind::Array(array) => arrays
            .get(array.index())
            .expect("nested array identities must precede their containing identity")
            .lifecycle
            .copy
            .map(|_| HirArrayCopyElement::Array(array)),
        ResolvedTypeKind::Shared(target) => {
            Some(HirArrayCopyElement::Shared(lower_shared_target(target)))
        }
        ResolvedTypeKind::OptionalShared { target, .. } => Some(
            HirArrayCopyElement::OptionalShared(lower_shared_target(target)),
        ),
        ResolvedTypeKind::Unit | ResolvedTypeKind::Obj | ResolvedTypeKind::Interface(_) => None,
    }
}

fn assignment_element(
    capabilities: &CopyCapabilities,
    arrays: &[HirArrayType],
    element: ResolvedTypeKind,
) -> Option<HirArrayAssignElement> {
    match element {
        ResolvedTypeKind::I64
        | ResolvedTypeKind::U64
        | ResolvedTypeKind::U8
        | ResolvedTypeKind::F64
        | ResolvedTypeKind::Bool => Some(HirArrayAssignElement::Primitive),
        ResolvedTypeKind::Optional {
            payload:
                ResolvedOptionalPayload::I64
                | ResolvedOptionalPayload::U64
                | ResolvedOptionalPayload::U8
                | ResolvedOptionalPayload::F64
                | ResolvedOptionalPayload::Bool,
            ..
        } => Some(HirArrayAssignElement::OptionalPrimitive),
        ResolvedTypeKind::Class(class) => capabilities
            .assignment(class)
            .selected()
            .map(|operation| HirArrayAssignElement::Class { class, operation }),
        ResolvedTypeKind::Optional {
            payload: ResolvedOptionalPayload::Class(class),
            ..
        } => capabilities
            .constructor(class)
            .selected()
            .zip(capabilities.assignment(class).selected())
            .map(
                |(copy_constructor, copy_assignment)| HirArrayAssignElement::OptionalClass {
                    class,
                    copy_constructor,
                    copy_assignment,
                },
            ),
        ResolvedTypeKind::Array(array) => arrays
            .get(array.index())
            .expect("nested array identities must precede their containing identity")
            .lifecycle
            .assignment
            .map(|_| HirArrayAssignElement::Array(array)),
        ResolvedTypeKind::Shared(target) => {
            Some(HirArrayAssignElement::Shared(lower_shared_target(target)))
        }
        ResolvedTypeKind::OptionalShared { target, .. } => Some(
            HirArrayAssignElement::OptionalShared(lower_shared_target(target)),
        ),
        ResolvedTypeKind::Unit | ResolvedTypeKind::Obj | ResolvedTypeKind::Interface(_) => None,
    }
}

fn destruction_element(element: ResolvedTypeKind) -> HirArrayDestroyElement {
    match element {
        ResolvedTypeKind::Class(class) => HirArrayDestroyElement::Class(class),
        ResolvedTypeKind::Optional {
            payload: ResolvedOptionalPayload::Class(class),
            ..
        } => HirArrayDestroyElement::OptionalClass(class),
        ResolvedTypeKind::Array(array) => HirArrayDestroyElement::Array(array),
        ResolvedTypeKind::Shared(target) => {
            HirArrayDestroyElement::Shared(lower_shared_target(target))
        }
        ResolvedTypeKind::OptionalShared { target, .. } => {
            HirArrayDestroyElement::OptionalShared(lower_shared_target(target))
        }
        _ => HirArrayDestroyElement::Trivial,
    }
}

fn lower_shared_target(target: ResolvedSharedTarget) -> HirSharedTarget {
    match target {
        ResolvedSharedTarget::Obj => HirSharedTarget::Obj,
        ResolvedSharedTarget::Class(class) => HirSharedTarget::Class(class),
        ResolvedSharedTarget::Interface(interface) => HirSharedTarget::Interface(interface),
        ResolvedSharedTarget::Array(array) => HirSharedTarget::Array(array),
    }
}
