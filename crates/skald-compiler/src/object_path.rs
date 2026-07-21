//! Target-independent identity path for an inline object place.

use crate::{
    identity::{BindingId, ClassId, FieldId},
    source::Span,
};

/// A root binding followed by zero or more inline class-field projections.
///
/// The terminal class is cached so consumers can select a member without
/// repeating declaration lookup. Resolution establishes that every projection
/// is a class-typed field owned by the preceding class.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ObjectPath {
    pub root: BindingId,
    pub projections: Vec<FieldId>,
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

    pub(crate) fn project(mut self, field: FieldId, class: ClassId, span: Span) -> Self {
        assert_eq!(
            field.class(),
            self.class,
            "object-path projection must belong to the current terminal class"
        );
        self.projections.push(field);
        self.class = class;
        self.span = span;
        self
    }

    pub(crate) fn with_span(mut self, span: Span) -> Self {
        self.span = span;
        self
    }

    pub(crate) fn direct_field(&self) -> Option<FieldId> {
        self.projections.first().copied()
    }

    pub(crate) fn is_root(&self) -> bool {
        self.projections.is_empty()
    }

    pub(crate) fn render_identity(&self) -> String {
        let mut rendered = self.root.to_string();
        for field in &self.projections {
            rendered.push_str(" -> ");
            rendered.push_str(&field.to_string());
        }
        rendered
    }
}
