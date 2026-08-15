//! Checked target data layout for inline classes.
//!
//! MIR retains nominal identities and semantic field projections. This module
//! is the backend's sole authority for converting that metadata into sizes,
//! alignments, and byte offsets.

use crate::{
    backend::{BackendError, Target},
    identity::{ArrayTypeId, ClassId, FieldId, OptionalBoxTypeId, OptionalTypeId},
    mir::{MirOptionalRepresentation, MirOptionalStorage, MirProgram, MirType},
};

use super::abi;

const MAX_ADDRESSABLE_SIZE: usize = i32::MAX as usize;
pub(super) const SHARED_HANDLE_SIZE: usize = 8;
pub(super) const SHARED_HANDLE_ALIGNMENT: usize = 8;
pub(super) const SHARED_DYNAMIC_METADATA_OFFSET: i32 = 8;
pub(super) const SHARED_HEADER_SIZE: usize = 16;
pub(super) const ARRAY_DESCRIPTOR_SIZE: usize = 8;
pub(super) const ARRAY_DESCRIPTOR_ALIGNMENT: usize = 8;
pub(super) const ARRAY_OWNER_COUNT_OFFSET: i32 = 0;
pub(super) const ARRAY_LENGTH_OFFSET: i32 = 8;
const ARRAY_HEADER_SIZE: usize = 16;
pub(super) const SHARED_ARRAY_LENGTH_OFFSET: i32 = 16;
const SHARED_ARRAY_HEADER_SIZE: usize = 24;
const MAX_ARRAY_LENGTH: u64 = i64::MAX as u64;
const OPTIONAL_STATE_SIZE: usize = 8;
const OPTIONAL_STATE_ALIGNMENT: usize = 8;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct TypeLayout {
    size: usize,
    alignment: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct OptionalLayout {
    ty: TypeLayout,
    payload_offset: usize,
    nullable_niche: bool,
}

/// Complete allocation layout for one shared payload. The owner handle always
/// points at byte zero; `payload_offset` is the address passed to finalizers.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct SharedAllocationLayout {
    byte_count: u64,
    payload_offset: usize,
}

impl SharedAllocationLayout {
    pub(super) const fn byte_count(self) -> u64 {
        self.byte_count
    }

    pub(super) const fn payload_offset(self) -> usize {
        self.payload_offset
    }
}

impl OptionalLayout {
    pub(super) const fn ty(self) -> TypeLayout {
        self.ty
    }

    pub(super) const fn state_offset(self) -> usize {
        0
    }

    pub(super) const fn payload_offset(self) -> usize {
        self.payload_offset
    }

    pub(super) const fn is_nullable_niche(self) -> bool {
        self.nullable_niche
    }
}

impl TypeLayout {
    const fn new(size: usize, alignment: usize) -> Self {
        Self { size, alignment }
    }

    pub(super) const fn size(self) -> usize {
        self.size
    }

    pub(super) const fn alignment(self) -> usize {
        self.alignment
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct FieldLayout {
    pub offset: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct BaseLayout {
    pub class: ClassId,
    pub offset: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct ClassLayout {
    ty: TypeLayout,
    base: Option<BaseLayout>,
    fields: Vec<FieldLayout>,
}

impl ClassLayout {
    pub(super) const fn ty(&self) -> TypeLayout {
        self.ty
    }

    pub(super) const fn base(&self) -> Option<BaseLayout> {
        self.base
    }

    pub(super) fn field(&self, field: FieldId) -> Option<FieldLayout> {
        self.fields.get(field.index()).copied()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct ArrayLayout {
    element: TypeLayout,
    element_offset: usize,
    shared_element_offset: usize,
    stride: usize,
    maximum_length: u64,
    shared_maximum_length: u64,
}

impl ArrayLayout {
    pub(super) const fn descriptor(self) -> TypeLayout {
        TypeLayout::new(ARRAY_DESCRIPTOR_SIZE, ARRAY_DESCRIPTOR_ALIGNMENT)
    }

    pub(super) const fn element_offset(self) -> usize {
        self.element_offset
    }

    pub(super) const fn stride(self) -> usize {
        self.stride
    }

    pub(super) const fn shared_element_offset(self) -> usize {
        self.shared_element_offset
    }

    pub(super) const fn maximum_length(self) -> u64 {
        self.maximum_length
    }

    pub(super) const fn shared_maximum_length(self) -> u64 {
        self.shared_maximum_length
    }

    #[cfg(test)]
    fn allocation_size(self, length: u64) -> Option<Option<u64>> {
        if length > self.maximum_length {
            return None;
        }
        if length == 0 {
            return Some(None);
        }
        let bytes = u64::try_from(self.stride)
            .ok()?
            .checked_mul(length)?
            .checked_add(u64::try_from(self.element_offset).ok()?)?;
        Some(Some(bytes))
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct DataLayout {
    classes: Vec<ClassLayout>,
    arrays: Vec<ArrayLayout>,
    optionals: Vec<OptionalLayout>,
    exact_optional_boxes: Vec<Option<SharedAllocationLayout>>,
    optional_object_boxes: Vec<Option<OptionalObjectBoxLayout>>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct OptionalObjectBoxLayout {
    layer_offsets: Vec<usize>,
    payload_offset: usize,
}

impl DataLayout {
    pub(super) fn compute(program: &MirProgram) -> Result<Self, BackendError> {
        LayoutBuilder::new(program).compute()
    }

    pub(super) fn ty(&self, ty: MirType) -> Result<TypeLayout, BackendError> {
        match ty {
            MirType::Class(class) => self
                .class(class)
                .map(ClassLayout::ty)
                .ok_or_else(|| layout_error(format!("class {class} has no target layout"))),
            MirType::Obj => Err(layout_error("`Obj` views have no owning storage layout")),
            MirType::Interface(_) => Err(layout_error(
                "interface views have no owning storage layout",
            )),
            MirType::Shared(_) => Ok(TypeLayout::new(SHARED_HANDLE_SIZE, SHARED_HANDLE_ALIGNMENT)),
            MirType::Array(array) => self
                .array(array)
                .map(ArrayLayout::descriptor)
                .ok_or_else(|| layout_error(format!("array {array} has no target layout"))),
            MirType::Optional(optional) => self.optional_type(optional).map(OptionalLayout::ty),
            MirType::Unit => Err(layout_error(
                "payload-free type `unit` has no storage layout",
            )),
            primitive => {
                Ok(scalar_layout(primitive).expect("every stored scalar has a target layout"))
            }
        }
    }

    pub(super) fn optional_type(
        &self,
        optional: OptionalTypeId,
    ) -> Result<OptionalLayout, BackendError> {
        self.optionals
            .get(optional.index())
            .copied()
            .ok_or_else(|| layout_error(format!("optional {optional} has no target layout")))
    }

    pub(super) fn class(&self, class: ClassId) -> Option<&ClassLayout> {
        self.classes.get(class.index())
    }

    pub(super) fn array(&self, array: ArrayTypeId) -> Option<ArrayLayout> {
        self.arrays.get(array.index()).copied()
    }

    pub(super) fn field(&self, field: FieldId) -> Option<FieldLayout> {
        self.class(field.class())?.field(field)
    }

    pub(super) fn shared_allocation_size(&self, class: ClassId) -> Result<u64, BackendError> {
        let payload = self
            .class(class)
            .ok_or_else(|| layout_error(format!("class {class} has no target layout")))?
            .ty();
        shared_allocation_layout(payload, &format!("class {class}"))
            .map(|layout| layout.byte_count())
    }

    pub(super) fn exact_optional_box(
        &self,
        target: OptionalBoxTypeId,
    ) -> Result<SharedAllocationLayout, BackendError> {
        self.exact_optional_boxes
            .get(target.index())
            .copied()
            .flatten()
            .ok_or_else(|| {
                layout_error(format!("optional-box {target} has no exact target layout"))
            })
    }

    pub(super) fn optional_object_box_layer_offset(
        &self,
        target: OptionalBoxTypeId,
        layer: usize,
    ) -> Result<usize, BackendError> {
        self.optional_object_boxes
            .get(target.index())
            .and_then(Option::as_ref)
            .and_then(|layout| layout.layer_offsets.get(layer))
            .copied()
            .ok_or_else(|| {
                layout_error(format!("optional-box {target} has no object layer {layer}"))
            })
    }

    pub(super) fn optional_object_box_payload_offset(
        &self,
        target: OptionalBoxTypeId,
    ) -> Result<usize, BackendError> {
        self.optional_object_boxes
            .get(target.index())
            .and_then(Option::as_ref)
            .map(|layout| layout.payload_offset)
            .ok_or_else(|| {
                layout_error(format!(
                    "optional-box {target} has no object payload layout"
                ))
            })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum VisitState {
    Unvisited,
    Visiting,
    Complete,
}

struct LayoutBuilder<'mir> {
    program: &'mir MirProgram,
    states: Vec<VisitState>,
    layouts: Vec<Option<ClassLayout>>,
    optional_layouts: Vec<Option<OptionalLayout>>,
}

impl<'mir> LayoutBuilder<'mir> {
    fn new(program: &'mir MirProgram) -> Self {
        let class_count = program.classes.len();
        Self {
            program,
            states: vec![VisitState::Unvisited; class_count],
            layouts: vec![None; class_count],
            optional_layouts: vec![None; program.optional_types.iter().len()],
        }
    }

    fn compute(mut self) -> Result<DataLayout, BackendError> {
        for class in self.program.classes.iter() {
            self.compute_class(class.id)?;
        }
        for optional in self.program.optional_types.iter() {
            self.compute_optional(optional.id)?;
        }
        let exact_optional_boxes = self
            .program
            .optional_box_types
            .iter()
            .map(|box_type| {
                let Some(optional) = box_type.exact_optional else {
                    return Ok(None);
                };
                let payload = self
                    .optional_layouts
                    .get(optional.index())
                    .copied()
                    .flatten()
                    .expect("every declared optional was laid out")
                    .ty();
                shared_allocation_layout(payload, &format!("optional-box {}", box_type.id))
                    .map(Some)
            })
            .collect::<Result<Vec<_>, _>>()?;
        let optional_object_boxes = self.compute_optional_object_box_layouts()?;
        let arrays = self
            .program
            .array_types
            .iter()
            .map(|array| {
                self.element(array.element)
                    .and_then(|element| {
                        array_layout(element).ok_or_else(|| {
                            layout_error(format!("array {} exceeds x86-64 layout limits", array.id))
                        })
                    })
                    .map_err(|error| {
                        BackendError::new(
                            Target::X86_64SysV,
                            error.callable(),
                            format!("array {} has no x86-64 element layout: {}", array.id, error),
                        )
                    })
            })
            .collect::<Result<_, _>>()?;
        Ok(DataLayout {
            classes: self
                .layouts
                .into_iter()
                .map(|layout| layout.expect("every declared class was laid out"))
                .collect(),
            arrays,
            optionals: self
                .optional_layouts
                .into_iter()
                .map(|layout| layout.expect("every optional identity was laid out"))
                .collect(),
            exact_optional_boxes,
            optional_object_boxes,
        })
    }

    fn compute_optional_object_box_layouts(
        &self,
    ) -> Result<Vec<Option<OptionalObjectBoxLayout>>, BackendError> {
        let exact = self
            .program
            .optional_box_types
            .iter()
            .map(|box_type| {
                let (Some(optional), Some(_class)) =
                    (box_type.exact_optional, box_type.exact_dynamic_class)
                else {
                    return Ok(None);
                };
                self.object_box_layout(optional, box_type.optional_depth)
                    .map(Some)
            })
            .collect::<Result<Vec<_>, _>>()?;

        self.program
            .optional_box_types
            .iter()
            .map(|box_type| {
                if let Some(layout) = exact[box_type.id.index()].clone() {
                    return Ok(Some(layout));
                }
                let Some(target) = box_type.object_view else {
                    return Ok(None);
                };
                let mut candidates = self
                    .program
                    .optional_box_types
                    .iter()
                    .filter(|candidate| {
                        candidate.optional_depth == box_type.optional_depth
                            && candidate
                                .exact_dynamic_class
                                .is_some_and(|class| match target {
                                    crate::mir::MirViewTarget::Class(base) => {
                                        class == base || self.program.is_ancestor(base, class)
                                    }
                                    crate::mir::MirViewTarget::Interface(interface) => {
                                        self.program.conformance(class, interface).is_some()
                                    }
                                    crate::mir::MirViewTarget::Obj => true,
                                })
                    })
                    .filter_map(|candidate| exact[candidate.id.index()].as_ref());
                let Some(first) = candidates.next().cloned() else {
                    return Err(layout_error(format!(
                        "optional-box {} object view has no exact descriptor layout",
                        box_type.id
                    )));
                };
                if candidates.any(|candidate| candidate != &first) {
                    return Err(layout_error(format!(
                        "optional-box {} object view has incompatible exact layouts",
                        box_type.id
                    )));
                }
                Ok(Some(first))
            })
            .collect()
    }

    fn object_box_layout(
        &self,
        mut optional: OptionalTypeId,
        depth: usize,
    ) -> Result<OptionalObjectBoxLayout, BackendError> {
        let mut layer_offsets = Vec::with_capacity(depth);
        let mut offset = 0usize;
        for layer in 0..depth {
            layer_offsets.push(offset);
            let layout = self
                .optional_layouts
                .get(optional.index())
                .copied()
                .flatten()
                .ok_or_else(|| layout_error(format!("optional {optional} has no target layout")))?;
            offset = offset
                .checked_add(layout.payload_offset())
                .ok_or_else(|| layout_error("optional object-box payload offset overflow"))?;
            let metadata = self
                .program
                .optional_type(optional)
                .ok_or_else(|| layout_error(format!("optional {optional} is not declared")))?;
            if layer + 1 < depth {
                let crate::mir::MirOptionalStorage::Nested(nested) = metadata.storage else {
                    return Err(layout_error(format!(
                        "optional object-box {optional} has inconsistent nesting depth"
                    )));
                };
                optional = nested;
            }
        }
        Ok(OptionalObjectBoxLayout {
            layer_offsets,
            payload_offset: offset,
        })
    }

    fn compute_optional(
        &mut self,
        optional: OptionalTypeId,
    ) -> Result<OptionalLayout, BackendError> {
        if let Some(layout) = self
            .optional_layouts
            .get(optional.index())
            .copied()
            .flatten()
        {
            return Ok(layout);
        }
        let metadata = self
            .program
            .optional_type(optional)
            .ok_or_else(|| layout_error(format!("optional {optional} is not declared")))?;
        let layout = match metadata.representation {
            MirOptionalRepresentation::NullableSharedOwner => OptionalLayout {
                ty: TypeLayout::new(SHARED_HANDLE_SIZE, SHARED_HANDLE_ALIGNMENT),
                payload_offset: 0,
                nullable_niche: true,
            },
            MirOptionalRepresentation::TaggedPayload => {
                let payload = match metadata.storage {
                    MirOptionalStorage::Scalar => scalar_layout(metadata.payload)
                        .expect("scalar optional metadata must carry a primitive payload"),
                    MirOptionalStorage::InlineClass(class) => self.compute_class(class)?,
                    MirOptionalStorage::InlineArray(_) => {
                        TypeLayout::new(ARRAY_DESCRIPTOR_SIZE, ARRAY_DESCRIPTOR_ALIGNMENT)
                    }
                    MirOptionalStorage::Nested(nested) => self.compute_optional(nested)?.ty(),
                    MirOptionalStorage::SharedOwner(_) => unreachable!(
                        "shared-owner optional metadata must select the nullable representation"
                    ),
                };
                optional_layout_for(payload)?
            }
        };
        let slot = self
            .optional_layouts
            .get_mut(optional.index())
            .expect("declared optional identity must have a layout slot");
        *slot = Some(layout);
        Ok(layout)
    }

    fn compute_class(&mut self, class: ClassId) -> Result<TypeLayout, BackendError> {
        let Some(state) = self.states.get(class.index()).copied() else {
            return Err(layout_error(format!("class {class} is not declared")));
        };
        match state {
            VisitState::Complete => {
                return Ok(self.layouts[class.index()]
                    .as_ref()
                    .expect("complete class has a layout")
                    .ty());
            }
            VisitState::Visiting => {
                return Err(layout_error(format!(
                    "recursive inline layout involving class {class}"
                )));
            }
            VisitState::Unvisited => {}
        }

        let Some(declaration) = self.program.class(class) else {
            return Err(layout_error(format!("class {class} is not declared")));
        };
        let direct_base = declaration.direct_base.map(|base| base.class);
        let fields: Vec<_> = declaration.fields.iter().map(|field| field.ty).collect();
        self.states[class.index()] = VisitState::Visiting;

        let base = direct_base
            .map(|base| self.compute_class(base).map(|layout| (base, layout)))
            .transpose()?;
        let mut laid_out_fields = Vec::with_capacity(fields.len());
        for ty in fields {
            let ty = match ty {
                MirType::Class(dependency) => self.compute_class(dependency)?,
                MirType::Optional(optional) => self.compute_optional(optional)?.ty(),
                field => self.field(field)?,
            };
            laid_out_fields.push(ty);
        }
        let layout = layout_class(base, &laid_out_fields).ok_or_else(|| {
            layout_error(format!(
                "layout of class {class} exceeds target size limits"
            ))
        })?;
        let ty = layout.ty();
        self.layouts[class.index()] = Some(layout);
        self.states[class.index()] = VisitState::Complete;
        Ok(ty)
    }

    fn field(&mut self, ty: MirType) -> Result<TypeLayout, BackendError> {
        match ty {
            MirType::Shared(_) => Ok(TypeLayout::new(SHARED_HANDLE_SIZE, SHARED_HANDLE_ALIGNMENT)),
            MirType::Optional(optional) => self.compute_optional(optional).map(OptionalLayout::ty),
            MirType::Array(array) => self
                .program
                .array_type(array)
                .map(|_| TypeLayout::new(ARRAY_DESCRIPTOR_SIZE, ARRAY_DESCRIPTOR_ALIGNMENT))
                .ok_or_else(|| layout_error(format!("array {array} is not declared"))),
            _ => scalar_layout(ty).ok_or_else(|| match ty {
                MirType::Class(_) => unreachable!("class dependencies are handled recursively"),
                MirType::Unit => layout_error("field type `unit` has no target layout"),
                _ => unreachable!("every stored scalar has a target layout"),
            }),
        }
    }

    fn element(&mut self, ty: MirType) -> Result<TypeLayout, BackendError> {
        match ty {
            MirType::Class(class) => self
                .layouts
                .get(class.index())
                .and_then(Option::as_ref)
                .map(ClassLayout::ty)
                .ok_or_else(|| layout_error(format!("class {class} has no target layout"))),
            MirType::Optional(optional) => self.compute_optional(optional).map(OptionalLayout::ty),
            MirType::Array(array) => self
                .program
                .array_type(array)
                .map(|_| TypeLayout::new(ARRAY_DESCRIPTOR_SIZE, ARRAY_DESCRIPTOR_ALIGNMENT))
                .ok_or_else(|| layout_error(format!("array {array} is not declared"))),
            MirType::Shared(_) => Ok(TypeLayout::new(SHARED_HANDLE_SIZE, SHARED_HANDLE_ALIGNMENT)),
            scalar => scalar_layout(scalar)
                .ok_or_else(|| layout_error(format!("type {scalar:?} has no array layout"))),
        }
    }
}

fn array_layout(element: TypeLayout) -> Option<ArrayLayout> {
    let element_offset = abi::align_up(ARRAY_HEADER_SIZE, element.alignment())?;
    let shared_element_offset = abi::align_up(SHARED_ARRAY_HEADER_SIZE, element.alignment())?;
    let stride = abi::align_up(element.size(), element.alignment())?;
    let arithmetic_limit =
        u64::MAX.checked_sub(u64::try_from(element_offset).ok()?)? / u64::try_from(stride).ok()?;
    let shared_arithmetic_limit = u64::MAX
        .checked_sub(u64::try_from(shared_element_offset).ok()?)?
        / u64::try_from(stride).ok()?;
    Some(ArrayLayout {
        element,
        element_offset,
        shared_element_offset,
        stride,
        maximum_length: MAX_ARRAY_LENGTH.min(arithmetic_limit),
        shared_maximum_length: MAX_ARRAY_LENGTH.min(shared_arithmetic_limit),
    })
}

fn scalar_layout(ty: MirType) -> Option<TypeLayout> {
    match ty {
        MirType::I64 | MirType::U64 | MirType::F64 | MirType::Function(_) => {
            Some(TypeLayout::new(8, 8))
        }
        MirType::U8 | MirType::Bool => Some(TypeLayout::new(1, 1)),
        MirType::Class(_)
        | MirType::Array(_)
        | MirType::Interface(_)
        | MirType::Obj
        | MirType::Shared(_)
        | MirType::Optional(_)
        | MirType::Unit => None,
    }
}

#[cfg(test)]
fn optional_layout(payload: crate::mir::MirPrimitiveType) -> Result<OptionalLayout, BackendError> {
    let payload = scalar_layout(payload.payload_type())
        .expect("every primitive optional payload has a target layout");
    optional_layout_for(payload)
}

fn optional_layout_for(payload: TypeLayout) -> Result<OptionalLayout, BackendError> {
    let alignment = OPTIONAL_STATE_ALIGNMENT.max(payload.alignment());
    let payload_offset = abi::align_up(OPTIONAL_STATE_SIZE, payload.alignment())
        .ok_or_else(|| layout_error("optional payload offset exceeds target limits"))?;
    let size = payload_offset
        .checked_add(payload.size())
        .and_then(|size| abi::align_up(size, alignment))
        .filter(|size| *size <= MAX_ADDRESSABLE_SIZE)
        .ok_or_else(|| layout_error("optional layout exceeds target limits"))?;
    Ok(OptionalLayout {
        ty: TypeLayout::new(size, alignment),
        payload_offset,
        nullable_niche: false,
    })
}

fn shared_allocation_layout(
    payload: TypeLayout,
    description: &str,
) -> Result<SharedAllocationLayout, BackendError> {
    if payload.alignment() > SHARED_HANDLE_ALIGNMENT {
        return Err(layout_error(format!(
            "shared payload {description} requires unsupported alignment {}",
            payload.alignment()
        )));
    }
    let payload_offset = abi::align_up(SHARED_HEADER_SIZE, payload.alignment())
        .filter(|offset| *offset <= MAX_ADDRESSABLE_SIZE)
        .ok_or_else(|| {
            layout_error(format!(
                "shared allocation for {description} has an unaddressable payload offset"
            ))
        })?;
    let size = payload_offset
        .checked_add(payload.size())
        .filter(|size| *size <= MAX_ADDRESSABLE_SIZE)
        .ok_or_else(|| {
            layout_error(format!(
                "shared allocation for {description} exceeds target size limits"
            ))
        })?;
    let byte_count = u64::try_from(size)
        .map_err(|_| layout_error(format!("shared allocation for {description} is too large")))?;
    Ok(SharedAllocationLayout {
        byte_count,
        payload_offset,
    })
}

fn layout_class(base: Option<(ClassId, TypeLayout)>, fields: &[TypeLayout]) -> Option<ClassLayout> {
    let mut size = base.map_or(0, |(_, layout)| layout.size());
    let mut alignment = base.map_or(1, |(_, layout)| layout.alignment());
    let mut field_layouts = Vec::with_capacity(fields.len());
    for field in fields {
        size = abi::align_up(size, field.alignment())?;
        field_layouts.push(FieldLayout { offset: size });
        size = size.checked_add(field.size())?;
        alignment = alignment.max(field.alignment());
    }
    size = abi::align_up(size, alignment)?;
    if base.is_none() && fields.is_empty() {
        size = 1;
    }
    if size > MAX_ADDRESSABLE_SIZE {
        return None;
    }
    Some(ClassLayout {
        ty: TypeLayout::new(size, alignment),
        base: base.map(|(class, _)| BaseLayout { class, offset: 0 }),
        fields: field_layouts,
    })
}

fn layout_error(message: impl Into<String>) -> BackendError {
    BackendError::new(Target::X86_64SysV, None, message)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lays_out_empty_mixed_padded_and_reordered_fields() {
        let empty = layout_class(None, &[]).unwrap();
        assert_eq!(empty.ty(), TypeLayout::new(1, 1));

        let mixed = layout_class(
            None,
            &[
                TypeLayout::new(1, 1),
                TypeLayout::new(8, 8),
                TypeLayout::new(1, 1),
            ],
        )
        .unwrap();
        assert_eq!(mixed.ty(), TypeLayout::new(24, 8));
        assert_eq!(
            mixed.fields,
            [
                FieldLayout { offset: 0 },
                FieldLayout { offset: 8 },
                FieldLayout { offset: 16 },
            ]
        );

        let reordered = layout_class(
            None,
            &[
                TypeLayout::new(8, 8),
                TypeLayout::new(1, 1),
                TypeLayout::new(1, 1),
            ],
        )
        .unwrap();
        assert_eq!(reordered.ty(), TypeLayout::new(16, 8));
        assert_eq!(reordered.fields[1].offset, 8);
        assert_eq!(reordered.fields[2].offset, 9);
    }

    #[test]
    fn lays_out_the_direct_base_before_derived_fields() {
        let base = ClassId::new(0);
        let derived = layout_class(
            Some((base, TypeLayout::new(16, 8))),
            &[TypeLayout::new(1, 1), TypeLayout::new(8, 8)],
        )
        .unwrap();

        assert_eq!(
            derived.base(),
            Some(BaseLayout {
                class: base,
                offset: 0,
            })
        );
        assert_eq!(derived.fields[0].offset, 16);
        assert_eq!(derived.fields[1].offset, 24);
        assert_eq!(derived.ty(), TypeLayout::new(32, 8));

        let empty_base = layout_class(
            Some((base, TypeLayout::new(1, 1))),
            &[TypeLayout::new(8, 8)],
        )
        .unwrap();
        assert_eq!(empty_base.fields[0].offset, 8);
        assert_eq!(empty_base.ty(), TypeLayout::new(16, 8));
    }

    #[test]
    fn defines_the_scalar_target_layout_contract() {
        let data = DataLayout {
            classes: vec![],
            arrays: vec![],
            optionals: vec![],
            exact_optional_boxes: vec![],
            optional_object_boxes: vec![],
        };
        for ty in [
            MirType::I64,
            MirType::U64,
            MirType::F64,
            MirType::Function(crate::identity::FunctionTypeId::new(0)),
        ] {
            assert_eq!(data.ty(ty).unwrap(), TypeLayout::new(8, 8));
        }
        for ty in [MirType::U8, MirType::Bool] {
            assert_eq!(data.ty(ty).unwrap(), TypeLayout::new(1, 1));
        }
        assert_eq!(
            data.ty(MirType::Shared(crate::mir::MirSharedTarget::Obj))
                .unwrap(),
            TypeLayout::new(8, 8)
        );
        assert!(data.ty(MirType::Unit).is_err());
        assert!(data.ty(MirType::Obj).is_err());
    }

    #[test]
    fn lays_out_inline_primitive_arrays_and_checks_every_size_component() {
        for (element, expected_stride) in [(TypeLayout::new(8, 8), 8), (TypeLayout::new(1, 1), 1)] {
            let layout = array_layout(element).unwrap();
            assert_eq!(layout.descriptor(), TypeLayout::new(8, 8));
            assert_eq!(layout.element, element);
            assert_eq!(layout.element_offset(), 16);
            assert_eq!(layout.stride(), expected_stride);
            assert_eq!(layout.allocation_size(0), Some(None));
            assert_eq!(
                layout.allocation_size(3),
                Some(Some(16 + 3 * u64::try_from(expected_stride).unwrap()))
            );
            assert_eq!(layout.allocation_size(layout.maximum_length() + 1), None);
        }

        let wide = array_layout(TypeLayout::new(8, 8)).unwrap();
        assert_eq!(
            wide.maximum_length(),
            (u64::MAX - 16) / 8,
            "allocation arithmetic is stricter than the language length ceiling"
        );
        let byte = array_layout(TypeLayout::new(1, 1)).unwrap();
        assert_eq!(byte.maximum_length(), i64::MAX as u64);
    }

    #[test]
    fn lays_out_primitive_optionals_with_state_before_aligned_payload() {
        for (payload, expected) in [
            (crate::mir::MirPrimitiveType::I64, (16, 8, 8)),
            (crate::mir::MirPrimitiveType::U64, (16, 8, 8)),
            (crate::mir::MirPrimitiveType::F64, (16, 8, 8)),
            (crate::mir::MirPrimitiveType::U8, (16, 8, 8)),
            (crate::mir::MirPrimitiveType::Bool, (16, 8, 8)),
        ] {
            let layout = optional_layout(payload).unwrap();
            assert_eq!(
                (
                    layout.ty().size(),
                    layout.ty().alignment(),
                    layout.payload_offset()
                ),
                expected
            );
            assert_eq!(layout.state_offset(), 0);
        }
    }

    #[test]
    fn rejects_checked_size_and_alignment_overflow() {
        assert!(layout_class(
            None,
            &[TypeLayout::new(usize::MAX, 1), TypeLayout::new(1, 1),]
        )
        .is_none());
        assert!(layout_class(None, &[TypeLayout::new(usize::MAX - 3, 8)]).is_none());
        assert!(layout_class(None, &[TypeLayout::new(MAX_ADDRESSABLE_SIZE + 1, 1)]).is_none());
        assert!(layout_class(
            Some((ClassId::new(0), TypeLayout::new(usize::MAX, 8))),
            &[TypeLayout::new(1, 1)],
        )
        .is_none());

        let oversized_payload = TypeLayout::new(MAX_ADDRESSABLE_SIZE, 8);
        let error = optional_layout_for(oversized_payload)
            .unwrap_err()
            .to_string();
        assert!(
            error.contains("optional layout exceeds target limits"),
            "{error}"
        );

        let primitive_box = shared_allocation_layout(TypeLayout::new(16, 8), "test box")
            .expect("a primitive optional fits behind the shared header");
        assert_eq!(primitive_box.payload_offset(), SHARED_HEADER_SIZE);
        assert_eq!(primitive_box.byte_count(), 32);

        let oversized_box =
            shared_allocation_layout(TypeLayout::new(MAX_ADDRESSABLE_SIZE, 8), "test box")
                .unwrap_err()
                .to_string();
        assert!(oversized_box.contains("exceeds target size limits"));

        let over_aligned_box = shared_allocation_layout(TypeLayout::new(16, 16), "test box")
            .unwrap_err()
            .to_string();
        assert!(over_aligned_box.contains("requires unsupported alignment 16"));
    }
}
