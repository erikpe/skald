//! Independent structural checks for supplied semantic planning facts.

use std::collections::BTreeSet;

use crate::identity::{ArrayTypeId, ClassId, InterfaceId, OptionalTypeId};

use super::*;

pub(super) fn check(facts: &PlanFacts) -> Result<(), PlanError> {
    if facts.semantic == SemanticFacts::default() {
        return Ok(());
    }
    check_types(facts)?;
    check_shared_header(facts)?;
    check_classes(facts)?;
    check_optionals(facts)?;
    check_arrays(facts)?;
    check_optional_boxes(facts)?;
    check_dispatch(facts)
}

fn layout(facts: &PlanFacts, id: LayoutId) -> Result<LayoutFact, PlanError> {
    facts
        .layouts
        .get(id.index())
        .copied()
        .ok_or(PlanError::UnknownDeclaration)
}

fn addressable_layout(facts: &PlanFacts, id: LayoutId) -> Result<LayoutFact, PlanError> {
    let layout = layout(facts, id)?;
    (layout.disposition == LayoutDisposition::Addressable)
        .then_some(layout)
        .ok_or(PlanError::InvalidLayout)
}

fn check_types(facts: &PlanFacts) -> Result<(), PlanError> {
    let mut types = BTreeSet::new();
    for binding in &facts.semantic.types {
        if !types.insert(binding.ty) {
            return Err(PlanError::DuplicateDeclaration);
        }
        let value = layout(facts, binding.layout)?;
        let expected = match binding.ty {
            SemanticType::Unit => LayoutDisposition::ElidedUnit,
            SemanticType::Obj | SemanticType::Interface(_) => LayoutDisposition::ElidedMetadata,
            _ => LayoutDisposition::Addressable,
        };
        if value.disposition != expected || !known_type(facts, binding.ty) {
            return Err(PlanError::InvalidLayout);
        }
    }
    Ok(())
}

fn known_type(facts: &PlanFacts, ty: SemanticType) -> bool {
    match ty {
        SemanticType::Class(id) => id.index() < facts.semantic.classes.len(),
        SemanticType::Interface(id) => id.index() < facts.semantic.interfaces.len(),
        SemanticType::Array(id) => id.index() < facts.semantic.arrays.len(),
        SemanticType::Optional(id) => id.index() < facts.semantic.optionals.len(),
        SemanticType::Shared(target) => known_shared_target(facts, target),
        _ => true,
    }
}

fn known_shared_target(facts: &PlanFacts, target: SharedTarget) -> bool {
    match target {
        SharedTarget::Obj => true,
        SharedTarget::Class(id) => id.index() < facts.semantic.classes.len(),
        SharedTarget::Interface(id) => id.index() < facts.semantic.interfaces.len(),
        SharedTarget::Array(id) => id.index() < facts.semantic.arrays.len(),
        SharedTarget::OptionalBox(id) => id.index() < facts.semantic.optional_boxes.len(),
    }
}

fn declared_callable(facts: &PlanFacts, source: crate::identity::CallableId) -> bool {
    facts
        .callables
        .iter()
        .any(|declaration| declaration.key == LirCallableId::Source(source))
}

fn callable_signature(
    facts: &PlanFacts,
    source: crate::identity::CallableId,
) -> Option<SignatureId> {
    facts.callables.iter().find_map(|declaration| {
        (declaration.key == LirCallableId::Source(source)).then_some(declaration.signature)
    })
}

fn type_layout(facts: &PlanFacts, ty: SemanticType) -> Result<LayoutId, PlanError> {
    facts
        .semantic
        .types
        .iter()
        .find_map(|binding| (binding.ty == ty).then_some(binding.layout))
        .ok_or(PlanError::UnknownDeclaration)
}

fn check_shared_header(facts: &PlanFacts) -> Result<(), PlanError> {
    let header = facts
        .semantic
        .shared_header
        .ok_or(PlanError::InvalidLayout)?;
    let handle = addressable_layout(facts, header.handle_layout)?;
    if handle.size != facts.profile.data_layout.pointer_bytes
        || header.owner_count_offset >= header.header_size
        || header.dynamic_metadata_offset >= header.header_size
        || header.owner_count_offset == header.dynamic_metadata_offset
    {
        return Err(PlanError::InvalidLayout);
    }
    Ok(())
}

fn check_classes(facts: &PlanFacts) -> Result<(), PlanError> {
    for (index, class) in facts.semantic.classes.iter().enumerate() {
        if class.class != ClassId::new(index) {
            return Err(PlanError::InvalidDomain);
        }
        let exact = addressable_layout(facts, class.exact_layout)?;
        let complete = addressable_layout(facts, class.complete_layout)?;
        if exact != complete
            || type_layout(facts, SemanticType::Class(class.class))? != class.exact_layout
            || class.shared_allocation.payload_offset > class.shared_allocation.byte_count as usize
        {
            return Err(PlanError::InvalidLayout);
        }
        if let Some(base) = class.base {
            if base.class.index() >= index {
                return Err(PlanError::InvalidDomain);
            }
            let base_layout = addressable_layout(
                facts,
                facts.semantic.classes[base.class.index()].complete_layout,
            )?;
            complete.checked_access(base.offset, base_layout.size)?;
        }
        for (field_index, field) in class.fields.iter().enumerate() {
            if field.field.class() != class.class
                || field.field.index() != field_index
                || !known_type(facts, field.ty)
                || type_layout(facts, field.ty)? != field.layout
            {
                return Err(PlanError::InvalidDomain);
            }
            complete.checked_access(field.offset, addressable_layout(facts, field.layout)?.size)?;
        }
        for step in &class.destruction {
            check_destruction_step(facts, class.class, *step)?;
        }
    }
    Ok(())
}

fn check_destruction_step(
    facts: &PlanFacts,
    owner: ClassId,
    step: DestructionStepFact,
) -> Result<(), PlanError> {
    let fields = facts.semantic.classes[owner.index()].fields.len();
    let valid = match step {
        DestructionStepFact::UserBody(id) => {
            id.class() == owner && declared_callable(facts, id.into())
        }
        DestructionStepFact::Field(id)
        | DestructionStepFact::SharedField(id)
        | DestructionStepFact::OptionalSharedField(id)
        | DestructionStepFact::OptionalClassField(id)
        | DestructionStepFact::ArrayField(id) => id.class() == owner && id.index() < fields,
        DestructionStepFact::OptionalField { field, optional } => {
            field.class() == owner
                && field.index() < fields
                && optional.index() < facts.semantic.optionals.len()
        }
        DestructionStepFact::Base(base) => facts.semantic.classes[owner.index()]
            .base
            .is_some_and(|declared| declared.class == base),
    };
    valid.then_some(()).ok_or(PlanError::InvalidDomain)
}

fn check_optionals(facts: &PlanFacts) -> Result<(), PlanError> {
    for (index, optional) in facts.semantic.optionals.iter().enumerate() {
        if optional.optional != OptionalTypeId::new(index)
            || !known_type(facts, optional.payload)
            || type_layout(facts, SemanticType::Optional(optional.optional))? != optional.layout
            || type_layout(facts, optional.payload)? != optional.payload_layout
        {
            return Err(PlanError::InvalidDomain);
        }
        let outer = addressable_layout(facts, optional.layout)?;
        let payload = addressable_layout(facts, optional.payload_layout)?;
        outer.checked_access(optional.payload_offset, payload.size)?;
        if optional.nullable_niche != optional.state_offset.is_none()
            || !known_optional_storage(facts, optional.storage)
        {
            return Err(PlanError::InvalidLayout);
        }
    }
    let mut complete = BTreeSet::new();
    for optional in &facts.semantic.optionals {
        check_optional_acyclic(
            facts,
            optional.optional,
            &mut BTreeSet::new(),
            &mut complete,
        )?;
    }
    for class in &facts.semantic.classes {
        check_layout_acyclic(
            facts,
            SemanticType::Class(class.class),
            &mut BTreeSet::new(),
            &mut BTreeSet::new(),
        )?;
    }
    Ok(())
}

fn check_layout_acyclic(
    facts: &PlanFacts,
    ty: SemanticType,
    visiting: &mut BTreeSet<SemanticType>,
    complete: &mut BTreeSet<SemanticType>,
) -> Result<(), PlanError> {
    if complete.contains(&ty) {
        return Ok(());
    }
    if !visiting.insert(ty) {
        return Err(PlanError::InvalidLayout);
    }
    match ty {
        SemanticType::Class(id) => {
            let class = &facts.semantic.classes[id.index()];
            for dependency in class
                .base
                .map(|base| SemanticType::Class(base.class))
                .into_iter()
                .chain(class.fields.iter().map(|field| field.ty))
                .filter(|dependency| {
                    matches!(
                        dependency,
                        SemanticType::Class(_) | SemanticType::Optional(_)
                    )
                })
            {
                check_layout_acyclic(facts, dependency, visiting, complete)?;
            }
        }
        SemanticType::Optional(id) => {
            let dependency = match facts.semantic.optionals[id.index()].storage {
                OptionalStorageFact::InlineClass(class) => Some(SemanticType::Class(class)),
                OptionalStorageFact::Nested(optional) => Some(SemanticType::Optional(optional)),
                _ => None,
            };
            if let Some(dependency) = dependency {
                check_layout_acyclic(facts, dependency, visiting, complete)?;
            }
        }
        _ => {}
    }
    visiting.remove(&ty);
    complete.insert(ty);
    Ok(())
}

fn known_optional_storage(facts: &PlanFacts, storage: OptionalStorageFact) -> bool {
    match storage {
        OptionalStorageFact::Scalar => true,
        OptionalStorageFact::InlineClass(id) => id.index() < facts.semantic.classes.len(),
        OptionalStorageFact::InlineArray(id) => id.index() < facts.semantic.arrays.len(),
        OptionalStorageFact::SharedOwner(target) => known_shared_target(facts, target),
        OptionalStorageFact::Nested(id) => id.index() < facts.semantic.optionals.len(),
    }
}

fn check_optional_acyclic(
    facts: &PlanFacts,
    id: OptionalTypeId,
    visiting: &mut BTreeSet<OptionalTypeId>,
    complete: &mut BTreeSet<OptionalTypeId>,
) -> Result<(), PlanError> {
    if complete.contains(&id) {
        return Ok(());
    }
    if !visiting.insert(id) {
        return Err(PlanError::InvalidLayout);
    }
    if let OptionalStorageFact::Nested(nested) = facts.semantic.optionals[id.index()].storage {
        check_optional_acyclic(facts, nested, visiting, complete)?;
    }
    visiting.remove(&id);
    complete.insert(id);
    Ok(())
}

fn check_arrays(facts: &PlanFacts) -> Result<(), PlanError> {
    for (index, array) in facts.semantic.arrays.iter().enumerate() {
        if array.array != ArrayTypeId::new(index)
            || !known_type(facts, array.element)
            || type_layout(facts, SemanticType::Array(array.array))? != array.descriptor_layout
            || type_layout(facts, array.element)? != array.element_layout
            || array.stride == 0
        {
            return Err(PlanError::InvalidDomain);
        }
        addressable_layout(facts, array.descriptor_layout)?;
        let element = addressable_layout(facts, array.element_layout)?;
        if array.stride < element.size || array.stride % element.alignment != 0 {
            return Err(PlanError::InvalidLayout);
        }
        for (offset, maximum) in [
            (array.element_offset, array.maximum_length),
            (array.shared_element_offset, array.shared_maximum_length),
        ] {
            u64::try_from(array.stride)
                .ok()
                .and_then(|stride| stride.checked_mul(maximum))
                .and_then(|size| size.checked_add(offset as u64))
                .ok_or(PlanError::SizeOverflow)?;
        }
        let valid_destroy = match array.destruction {
            ArrayDestroyElementFact::Array(id) => id.index() < facts.semantic.arrays.len(),
            ArrayDestroyElementFact::Optional(id) => id.index() < facts.semantic.optionals.len(),
            ArrayDestroyElementFact::Shared(target)
            | ArrayDestroyElementFact::OptionalShared(target) => known_shared_target(facts, target),
            ArrayDestroyElementFact::Class(id) | ArrayDestroyElementFact::OptionalClass(id) => {
                id.index() < facts.semantic.classes.len()
            }
            ArrayDestroyElementFact::Trivial => true,
        };
        if !valid_destroy {
            return Err(PlanError::InvalidDomain);
        }
        if !array
            .default
            .is_none_or(|operation| check_array_default(facts, operation))
            || !array
                .copy
                .is_none_or(|operation| check_array_copy(facts, operation))
            || !array
                .assignment
                .is_none_or(|operation| check_array_assignment(facts, operation))
        {
            return Err(PlanError::InvalidDomain);
        }
    }
    Ok(())
}

fn check_array_default(facts: &PlanFacts, operation: ArrayDefaultElementFact) -> bool {
    match operation {
        ArrayDefaultElementFact::Primitive | ArrayDefaultElementFact::OptionalAbsent => true,
        ArrayDefaultElementFact::Class { class, initializer }
        | ArrayDefaultElementFact::SharedClass { class, initializer } => {
            class.index() < facts.semantic.classes.len()
                && initializer.class() == class
                && declared_callable(facts, initializer.into())
        }
        ArrayDefaultElementFact::ArrayEmpty(id) | ArrayDefaultElementFact::SharedArrayEmpty(id) => {
            id.index() < facts.semantic.arrays.len()
        }
        ArrayDefaultElementFact::SharedOptionalBoxAbsent(id) => {
            id.index() < facts.semantic.optional_boxes.len()
        }
    }
}

fn check_array_copy(facts: &PlanFacts, operation: ArrayCopyElementFact) -> bool {
    match operation {
        ArrayCopyElementFact::Primitive | ArrayCopyElementFact::OptionalPrimitive => true,
        ArrayCopyElementFact::Class { class, operation }
        | ArrayCopyElementFact::OptionalClass { class, operation } => {
            class.index() < facts.semantic.classes.len()
                && check_constructor_copy(facts, class, operation)
        }
        ArrayCopyElementFact::Array(id) => id.index() < facts.semantic.arrays.len(),
        ArrayCopyElementFact::Shared(target) | ArrayCopyElementFact::OptionalShared(target) => {
            known_shared_target(facts, target)
        }
        ArrayCopyElementFact::Optional(id) => id.index() < facts.semantic.optionals.len(),
    }
}

fn check_array_assignment(facts: &PlanFacts, operation: ArrayAssignElementFact) -> bool {
    match operation {
        ArrayAssignElementFact::Primitive | ArrayAssignElementFact::OptionalPrimitive => true,
        ArrayAssignElementFact::Class { class, operation } => {
            class.index() < facts.semantic.classes.len()
                && check_assignment_copy(facts, class, operation)
        }
        ArrayAssignElementFact::OptionalClass {
            class,
            copy_constructor,
            copy_assignment,
        } => {
            class.index() < facts.semantic.classes.len()
                && check_constructor_copy(facts, class, copy_constructor)
                && check_assignment_copy(facts, class, copy_assignment)
        }
        ArrayAssignElementFact::Array(id) => id.index() < facts.semantic.arrays.len(),
        ArrayAssignElementFact::Shared(target) | ArrayAssignElementFact::OptionalShared(target) => {
            known_shared_target(facts, target)
        }
        ArrayAssignElementFact::Optional(id) => id.index() < facts.semantic.optionals.len(),
    }
}

fn check_constructor_copy(
    facts: &PlanFacts,
    class: ClassId,
    operation: SelectedCopy<crate::identity::CopyConstructorId>,
) -> bool {
    match operation {
        SelectedCopy::User(id) => id.class() == class && declared_callable(facts, id.into()),
        SelectedCopy::Synthesized(id) => id == class,
    }
}

fn check_assignment_copy(
    facts: &PlanFacts,
    class: ClassId,
    operation: SelectedCopy<crate::identity::CopyAssignmentId>,
) -> bool {
    match operation {
        SelectedCopy::User(id) => id.class() == class && declared_callable(facts, id.into()),
        SelectedCopy::Synthesized(id) => id == class,
    }
}

fn check_optional_boxes(facts: &PlanFacts) -> Result<(), PlanError> {
    for (index, item) in facts.semantic.optional_boxes.iter().enumerate() {
        if item.optional_box.index() != index
            || item
                .exact_optional
                .is_some_and(|id| id.index() >= facts.semantic.optionals.len())
            || item
                .exact_dynamic_class
                .is_some_and(|id| id.index() >= facts.semantic.classes.len())
            || item.exact_optional.is_some() != item.allocation.is_some()
        {
            return Err(PlanError::InvalidDomain);
        }
        if let Some(allocation) = item.allocation {
            if allocation.payload_offset > allocation.byte_count as usize {
                return Err(PlanError::InvalidLayout);
            }
        }
        if item.object_view.is_some()
            && (item.layer_offsets.is_empty()
                || item.payload_offset.is_none()
                || item.exact_dynamic_class.is_none() && item.exact_optional.is_some())
        {
            return Err(PlanError::InvalidLayout);
        }
        if !item.layer_offsets.windows(2).all(|pair| pair[0] <= pair[1]) {
            return Err(PlanError::InvalidLayout);
        }
    }
    Ok(())
}

fn check_dispatch(facts: &PlanFacts) -> Result<(), PlanError> {
    for (index, family) in facts.semantic.virtual_families.iter().enumerate() {
        let root_signature = callable_signature(facts, family.root.into());
        if family.family.index() != index
            || family.slot.index() != index
            || family.root.class().index() >= facts.semantic.classes.len()
            || family.members.iter().any(|method| {
                method.class().index() >= facts.semantic.classes.len()
                    || !declared_callable(facts, (*method).into())
            })
            || !declared_callable(facts, family.root.into())
            || root_signature.is_none()
            || family.members.iter().any(|method| {
                let member = callable_signature(facts, (*method).into());
                match (root_signature, member) {
                    (Some(root), Some(member)) => {
                        facts.signatures[root.index()] != facts.signatures[member.index()]
                    }
                    _ => true,
                }
            })
        {
            return Err(PlanError::InvalidDispatch);
        }
    }
    for (index, interface) in facts.semantic.interfaces.iter().enumerate() {
        if interface.interface != InterfaceId::new(index)
            || interface
                .requirements
                .iter()
                .enumerate()
                .any(|(i, requirement)| {
                    requirement.requirement.interface() != interface.interface
                        || requirement.requirement.index() != i
                        || facts
                            .signatures
                            .get(requirement.signature.index())
                            .is_none()
                })
        {
            return Err(PlanError::InvalidDispatch);
        }
    }
    for conformance in &facts.semantic.conformances {
        let Some(interface) = facts.semantic.interfaces.get(conformance.interface.index()) else {
            return Err(PlanError::InvalidDispatch);
        };
        if conformance.class.index() >= facts.semantic.classes.len()
            || conformance.implementations.len() != interface.requirements.len()
            || conformance
                .implementations
                .iter()
                .zip(&interface.requirements)
                .any(|(implementation, requirement)| {
                    let implementation_signature =
                        callable_signature(facts, implementation.method.into());
                    implementation.requirement != requirement.requirement
                        || implementation.method.class().index() >= facts.semantic.classes.len()
                        || !declared_callable(facts, implementation.method.into())
                        || implementation_signature.is_none_or(|signature| {
                            facts.signatures[signature.index()]
                                != facts.signatures[requirement.signature.index()]
                        })
                })
        {
            return Err(PlanError::InvalidDispatch);
        }
    }
    let finalizer_index = facts
        .semantic
        .method_slots
        .iter()
        .position(|slot| slot.slot == MethodSlot::Finalizer)
        .ok_or(PlanError::InvalidDispatch)?;
    for (index, slot) in facts.semantic.method_slots.iter().enumerate() {
        if slot.index != index
            || slot.byte_offset
                != index
                    .checked_mul(facts.profile.data_layout.pointer_bytes)
                    .ok_or(PlanError::SizeOverflow)?
        {
            return Err(PlanError::InvalidDispatch);
        }
    }
    if finalizer_index + 1 != facts.semantic.method_slots.len() {
        return Err(PlanError::InvalidDispatch);
    }
    for (index, table) in facts.semantic.dispatch_tables.iter().enumerate() {
        if table.class != ClassId::new(index) || table.targets.len() != finalizer_index {
            return Err(PlanError::InvalidDispatch);
        }
        for target in table.targets.iter().flatten() {
            if !facts.callables.iter().any(|declaration| {
                declaration.key == *target && declaration.body == BodyDisposition::Required
            }) {
                return Err(PlanError::AbsentBody);
            }
        }
    }
    let expected_views = facts.semantic.classes.len() + facts.semantic.interfaces.len() + 1;
    if facts.semantic.object_views.len() != expected_views {
        return Err(PlanError::InvalidDispatch);
    }
    let expected_components = [
        ObjectComponent::StaticAddress,
        ObjectComponent::CompleteAddress,
        ObjectComponent::DynamicMetadata,
    ];
    let mut targets = BTreeSet::new();
    for view in &facts.semantic.object_views {
        if view.components != expected_components
            || !targets.insert(view.target)
            || expected_view_members(facts, view.target)? != view.members
        {
            return Err(PlanError::InvalidDispatch);
        }
        let mut members = view.members.clone();
        members.sort();
        members.dedup();
        if members != view.members
            || members
                .iter()
                .any(|class| class.index() >= facts.semantic.classes.len())
        {
            return Err(PlanError::InvalidDispatch);
        }
    }
    Ok(())
}

fn expected_view_members(
    facts: &PlanFacts,
    target: ObjectViewTarget,
) -> Result<Vec<ClassId>, PlanError> {
    let members = match target {
        ObjectViewTarget::Obj => facts
            .semantic
            .classes
            .iter()
            .map(|class| class.class)
            .collect(),
        ObjectViewTarget::Class(target) => {
            if target.index() >= facts.semantic.classes.len() {
                return Err(PlanError::InvalidDispatch);
            }
            facts
                .semantic
                .classes
                .iter()
                .filter(|class| {
                    let mut current = Some(class.class);
                    while let Some(candidate) = current {
                        if candidate == target {
                            return true;
                        }
                        current = facts.semantic.classes[candidate.index()]
                            .base
                            .map(|base| base.class);
                    }
                    false
                })
                .map(|class| class.class)
                .collect()
        }
        ObjectViewTarget::Interface(interface) => {
            if interface.index() >= facts.semantic.interfaces.len() {
                return Err(PlanError::InvalidDispatch);
            }
            facts
                .semantic
                .conformances
                .iter()
                .filter_map(|conformance| {
                    (conformance.interface == interface).then_some(conformance.class)
                })
                .collect()
        }
    };
    Ok(members)
}
