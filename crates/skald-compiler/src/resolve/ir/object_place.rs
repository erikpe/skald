//! Object paths selected during name resolution.

use crate::{
    identity::{BindingId, ClassId, FieldId},
    object_path::ObjectPath,
    source::Span,
};

use super::ResolvedObjectCastExpr;

pub type ResolvedObjectPlace = ObjectPath;

/// A class-typed receiver selected either from a stable inline place or from a
/// full-expression checked cast place.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedObjectReceiver {
    pub path: ResolvedObjectPlace,
    pub cast: Option<Box<ResolvedObjectCastExpr>>,
}

impl ResolvedObjectReceiver {
    pub fn from_place(place: ResolvedObjectPlace) -> Self {
        Self {
            path: place,
            cast: None,
        }
    }

    pub fn from_cast(cast: ResolvedObjectCastExpr, root: BindingId, class: ClassId) -> Self {
        Self {
            path: ObjectPath::root(root, class, cast.span),
            cast: Some(Box::new(cast)),
        }
    }

    pub const fn class(&self) -> ClassId {
        self.path.class
    }

    pub const fn span(&self) -> Span {
        self.path.span
    }

    pub fn with_span(mut self, span: Span) -> Self {
        self.path.span = span;
        self
    }

    pub fn project_base(mut self, base: ClassId, span: Span) -> Self {
        self.path = self.path.project_base(base, span);
        self
    }

    pub fn project_field(mut self, field: FieldId, class: ClassId, span: Span) -> Self {
        self.path = self.path.project_field(field, class, span);
        self
    }
}

impl std::ops::Deref for ResolvedObjectReceiver {
    type Target = ResolvedObjectPlace;

    fn deref(&self) -> &Self::Target {
        &self.path
    }
}
