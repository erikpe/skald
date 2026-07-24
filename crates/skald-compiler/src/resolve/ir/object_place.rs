//! Object paths selected during name resolution.

use crate::{
    identity::{BindingId, ClassId, FieldId},
    object_path::{ObjectPath, ObjectProjection},
    source::Span,
};

use super::ResolvedObjectCastExpr;

pub type ResolvedObjectPlace = ObjectPath;

/// A class-typed receiver selected from either a stable binding path or a
/// projection path relative to a full-expression checked cast.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ResolvedObjectReceiver {
    BindingPath(ResolvedObjectPlace),
    CastRelative {
        cast: Box<ResolvedObjectCastExpr>,
        projections: Vec<ObjectProjection>,
        class: ClassId,
        span: Span,
    },
}

impl ResolvedObjectReceiver {
    pub fn from_place(place: ResolvedObjectPlace) -> Self {
        Self::BindingPath(place)
    }

    pub fn from_cast(cast: ResolvedObjectCastExpr, class: ClassId) -> Self {
        let span = cast.span;
        Self::CastRelative {
            cast: Box::new(cast),
            projections: Vec::new(),
            class,
            span,
        }
    }

    pub const fn class(&self) -> ClassId {
        match self {
            Self::BindingPath(path) => path.class,
            Self::CastRelative { class, .. } => *class,
        }
    }

    pub const fn span(&self) -> Span {
        match self {
            Self::BindingPath(path) => path.span,
            Self::CastRelative { span, .. } => *span,
        }
    }

    pub const fn binding_path(&self) -> Option<&ResolvedObjectPlace> {
        match self {
            Self::BindingPath(path) => Some(path),
            Self::CastRelative { .. } => None,
        }
    }

    pub const fn root(&self) -> Option<BindingId> {
        match self {
            Self::BindingPath(path) => Some(path.root),
            Self::CastRelative { .. } => None,
        }
    }

    pub fn projections(&self) -> &[ObjectProjection] {
        match self {
            Self::BindingPath(path) => &path.projections,
            Self::CastRelative { projections, .. } => projections,
        }
    }

    pub const fn cast(&self) -> Option<&ResolvedObjectCastExpr> {
        match self {
            Self::BindingPath(_) => None,
            Self::CastRelative { cast, .. } => Some(cast),
        }
    }

    pub fn with_span(self, span: Span) -> Self {
        match self {
            Self::BindingPath(path) => Self::BindingPath(path.with_span(span)),
            Self::CastRelative {
                cast,
                projections,
                class,
                ..
            } => Self::CastRelative {
                cast,
                projections,
                class,
                span,
            },
        }
    }

    pub fn project_base(self, base: ClassId, span: Span) -> Self {
        match self {
            Self::BindingPath(path) => Self::BindingPath(path.project_base(base, span)),
            Self::CastRelative {
                cast,
                mut projections,
                ..
            } => {
                projections.push(ObjectProjection::Base(base));
                Self::CastRelative {
                    cast,
                    projections,
                    class: base,
                    span,
                }
            }
        }
    }

    pub fn project_field(self, field: FieldId, class: ClassId, span: Span) -> Self {
        match self {
            Self::BindingPath(path) => Self::BindingPath(path.project_field(field, class, span)),
            Self::CastRelative {
                cast,
                mut projections,
                class: receiver_class,
                ..
            } => {
                assert_eq!(
                    field.class(),
                    receiver_class,
                    "cast-relative projection must belong to the current terminal class"
                );
                projections.push(ObjectProjection::Field(field));
                Self::CastRelative {
                    cast,
                    projections,
                    class,
                    span,
                }
            }
        }
    }
}
