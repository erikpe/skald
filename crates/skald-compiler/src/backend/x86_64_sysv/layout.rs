//! Checked target data layout for inline classes.
//!
//! MIR retains nominal identities and semantic field projections. This module
//! is the backend's sole authority for converting that metadata into sizes,
//! alignments, and byte offsets.

use crate::{
    backend::{BackendError, Target},
    identity::{ClassId, FieldId},
    mir::{MirProgram, MirType},
};

use super::abi;

const MAX_ADDRESSABLE_SIZE: usize = i32::MAX as usize;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct TypeLayout {
    size: usize,
    alignment: usize,
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

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct ClassLayout {
    ty: TypeLayout,
    fields: Vec<FieldLayout>,
}

impl ClassLayout {
    pub(super) const fn ty(&self) -> TypeLayout {
        self.ty
    }

    pub(super) fn field(&self, field: FieldId) -> Option<FieldLayout> {
        self.fields.get(field.index()).copied()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct DataLayout {
    classes: Vec<ClassLayout>,
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
            MirType::Unit => Err(layout_error(
                "payload-free type `unit` has no storage layout",
            )),
            primitive => {
                Ok(primitive_layout(primitive)
                    .expect("every payload primitive has a target layout"))
            }
        }
    }

    pub(super) fn class(&self, class: ClassId) -> Option<&ClassLayout> {
        self.classes.get(class.index())
    }

    pub(super) fn field(&self, field: FieldId) -> Option<FieldLayout> {
        self.class(field.class())?.field(field)
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
}

impl<'mir> LayoutBuilder<'mir> {
    fn new(program: &'mir MirProgram) -> Self {
        let class_count = program.classes.len();
        Self {
            program,
            states: vec![VisitState::Unvisited; class_count],
            layouts: vec![None; class_count],
        }
    }

    fn compute(mut self) -> Result<DataLayout, BackendError> {
        for class in self.program.classes.iter() {
            self.compute_class(class.id)?;
        }
        Ok(DataLayout {
            classes: self
                .layouts
                .into_iter()
                .map(|layout| layout.expect("every declared class was laid out"))
                .collect(),
        })
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
        let fields: Vec<_> = declaration.fields.iter().map(|field| field.ty).collect();
        self.states[class.index()] = VisitState::Visiting;

        let mut laid_out_fields = Vec::with_capacity(fields.len());
        for ty in fields {
            let ty = match ty {
                MirType::Class(dependency) => self.compute_class(dependency)?,
                primitive => self.primitive(primitive)?,
            };
            laid_out_fields.push(ty);
        }
        let layout = layout_class(&laid_out_fields).ok_or_else(|| {
            layout_error(format!(
                "layout of class {class} exceeds target size limits"
            ))
        })?;
        let ty = layout.ty();
        self.layouts[class.index()] = Some(layout);
        self.states[class.index()] = VisitState::Complete;
        Ok(ty)
    }

    fn primitive(&self, ty: MirType) -> Result<TypeLayout, BackendError> {
        primitive_layout(ty).ok_or_else(|| match ty {
            MirType::Class(_) => unreachable!("class dependencies are handled recursively"),
            MirType::Unit => layout_error("field type `unit` has no target layout"),
            _ => unreachable!("every payload primitive has a target layout"),
        })
    }
}

fn primitive_layout(ty: MirType) -> Option<TypeLayout> {
    match ty {
        MirType::I64 | MirType::U64 | MirType::F64 => Some(TypeLayout::new(8, 8)),
        MirType::U8 | MirType::Bool => Some(TypeLayout::new(1, 1)),
        MirType::Class(_) | MirType::Unit => None,
    }
}

fn layout_class(fields: &[TypeLayout]) -> Option<ClassLayout> {
    let mut size = 0usize;
    let mut alignment = 1usize;
    let mut field_layouts = Vec::with_capacity(fields.len());
    for field in fields {
        size = abi::align_up(size, field.alignment())?;
        field_layouts.push(FieldLayout { offset: size });
        size = size.checked_add(field.size())?;
        alignment = alignment.max(field.alignment());
    }
    size = abi::align_up(size, alignment)?;
    if fields.is_empty() {
        size = 1;
    }
    if size > MAX_ADDRESSABLE_SIZE {
        return None;
    }
    Some(ClassLayout {
        ty: TypeLayout::new(size, alignment),
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
        let empty = layout_class(&[]).unwrap();
        assert_eq!(empty.ty(), TypeLayout::new(1, 1));

        let mixed = layout_class(&[
            TypeLayout::new(1, 1),
            TypeLayout::new(8, 8),
            TypeLayout::new(1, 1),
        ])
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

        let reordered = layout_class(&[
            TypeLayout::new(8, 8),
            TypeLayout::new(1, 1),
            TypeLayout::new(1, 1),
        ])
        .unwrap();
        assert_eq!(reordered.ty(), TypeLayout::new(16, 8));
        assert_eq!(reordered.fields[1].offset, 8);
        assert_eq!(reordered.fields[2].offset, 9);
    }

    #[test]
    fn defines_the_primitive_target_layout_contract() {
        let data = DataLayout { classes: vec![] };
        for ty in [MirType::I64, MirType::U64, MirType::F64] {
            assert_eq!(data.ty(ty).unwrap(), TypeLayout::new(8, 8));
        }
        for ty in [MirType::U8, MirType::Bool] {
            assert_eq!(data.ty(ty).unwrap(), TypeLayout::new(1, 1));
        }
        assert!(data.ty(MirType::Unit).is_err());
    }

    #[test]
    fn rejects_checked_size_and_alignment_overflow() {
        assert!(layout_class(&[TypeLayout::new(usize::MAX, 1), TypeLayout::new(1, 1),]).is_none());
        assert!(layout_class(&[TypeLayout::new(usize::MAX - 3, 8)]).is_none());
        assert!(layout_class(&[TypeLayout::new(MAX_ADDRESSABLE_SIZE + 1, 1)]).is_none());
    }
}
