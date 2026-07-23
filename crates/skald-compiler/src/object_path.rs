//! Target-independent identity path for an inline object place.

use crate::{
    identity::{BindingId, ClassId, FieldId},
    source::Span,
};

/// One identity-selected step through an inline complete object.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ObjectProjection {
    /// The direct base subobject declared by the current terminal class.
    Base(ClassId),
    /// An inline class field declared by the current terminal class.
    Field(FieldId),
}

/// A root binding followed by zero or more semantic object projections.
///
/// The terminal class is cached so consumers can select a member without
/// repeating declaration lookup. Resolution establishes that every projection
/// is either the current class's direct base or a class-typed field owned by
/// the preceding class.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ObjectPath {
    pub root: BindingId,
    pub projections: Vec<ObjectProjection>,
    pub class: ClassId,
    pub span: Span,
}

impl ObjectPath {
    pub(crate) fn root(root: BindingId, class: ClassId, span: Span) -> Self {
        Self {
            root,
            projections: Vec::new(),
            class,
            span,
        }
    }

    pub(crate) fn project_field(mut self, field: FieldId, class: ClassId, span: Span) -> Self {
        assert_eq!(
            field.class(),
            self.class,
            "object-path projection must belong to the current terminal class"
        );
        self.projections.push(ObjectProjection::Field(field));
        self.class = class;
        self.span = span;
        self
    }

    pub(crate) fn project_base(mut self, base: ClassId, span: Span) -> Self {
        self.projections.push(ObjectProjection::Base(base));
        self.class = base;
        self.span = span;
        self
    }

    pub(crate) fn with_span(mut self, span: Span) -> Self {
        self.span = span;
        self
    }

    pub(crate) fn direct_field(&self) -> Option<FieldId> {
        match self.projections.first() {
            Some(ObjectProjection::Field(field)) => Some(*field),
            Some(ObjectProjection::Base(_)) | None => None,
        }
    }

    pub(crate) fn is_root(&self) -> bool {
        self.projections.is_empty()
    }

    pub(crate) fn render_identity(&self) -> String {
        let mut rendered = self.root.to_string();
        for projection in &self.projections {
            rendered.push_str(" -> ");
            match projection {
                ObjectProjection::Base(base) => {
                    rendered.push_str("base ");
                    rendered.push_str(&base.to_string());
                }
                ObjectProjection::Field(field) => rendered.push_str(&field.to_string()),
            }
        }
        rendered
    }
}
