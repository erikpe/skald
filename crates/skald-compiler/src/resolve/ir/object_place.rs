//! Object paths selected during name resolution.

use crate::{
    identity::{BindingId, ClassId, FieldId, StaticFieldId},
    object_path::{ObjectPath, ObjectProjection},
    source::Span,
};

use super::{
    ResolvedArrayProjectionExpr, ResolvedDereferenceExpr, ResolvedExpression,
    ResolvedObjectCastExpr, ResolvedUnwrapExpr,
};

pub type ResolvedObjectPlace = ObjectPath;

/// One class-typed member receiver with explicit source provenance.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ResolvedObjectReceiver {
    BindingPath(ResolvedObjectPlace),
    StaticField {
        field: StaticFieldId,
        projections: Vec<ObjectProjection>,
        class: ClassId,
        span: Span,
    },
    CastRelative {
        cast: Box<ResolvedObjectCastExpr>,
        projections: Vec<ObjectProjection>,
        class: ClassId,
        span: Span,
    },
    Dereference {
        dereference: Box<ResolvedDereferenceExpr>,
        projections: Vec<ObjectProjection>,
        class: ClassId,
        span: Span,
    },
    OptionalPayload {
        unwrap: Box<ResolvedUnwrapExpr>,
        projections: Vec<ObjectProjection>,
        class: ClassId,
        span: Span,
    },
    ArrayElement {
        projection: Box<ResolvedArrayProjectionExpr>,
        projections: Vec<ObjectProjection>,
        class: ClassId,
        span: Span,
    },
    /// One exact inline class produced for read-only member selection.
    ///
    /// `exact_class` is the complete-object class while `class` follows base
    /// projections used for inherited member selection. The producer is kept
    /// once and is never represented as a synthetic source binding.
    Produced {
        producer: Box<ResolvedExpression>,
        exact_class: ClassId,
        projections: Vec<ObjectProjection>,
        class: ClassId,
        span: Span,
    },
}

impl ResolvedObjectReceiver {
    /// Converts one already-resolved exact-class expression into the common
    /// receiver carrier used by structural sugars and ordinary member calls.
    pub(crate) fn from_expression(
        expression: ResolvedExpression,
        class: ClassId,
    ) -> Result<Self, Box<ResolvedExpression>> {
        Ok(match expression {
            ResolvedExpression::Binding(binding) => Self::from_place(ResolvedObjectPlace::root(
                binding.binding,
                class,
                binding.span,
            )),
            ResolvedExpression::Grouped(grouped) => {
                return Self::from_expression(*grouped.expression, class)
                    .map(|receiver| receiver.with_span(grouped.span));
            }
            ResolvedExpression::Dereference(dereference) => {
                let span = dereference.span;
                Self::Dereference {
                    dereference: Box::new(dereference),
                    projections: Vec::new(),
                    class,
                    span,
                }
            }
            ResolvedExpression::Unwrap(unwrap) => Self::from_optional_payload(unwrap, class),
            ResolvedExpression::ObjectCast(cast) => Self::from_cast(cast, class),
            ResolvedExpression::ArrayProjection(projection) => {
                let span = projection.span;
                Self::ArrayElement {
                    projection,
                    projections: Vec::new(),
                    class,
                    span,
                }
            }
            ResolvedExpression::FieldAccess(access) => {
                access
                    .receiver
                    .project_field(access.field, class, access.span)
            }
            ResolvedExpression::StaticFieldAccess(access) => {
                Self::from_static_field(access.field, class, access.span)
            }
            producer @ (ResolvedExpression::StringLiteral(_)
            | ResolvedExpression::DirectCall(_)
            | ResolvedExpression::IndirectCall(_)
            | ResolvedExpression::StaticCall(_)
            | ResolvedExpression::MethodCall(_)
            | ResolvedExpression::InterfaceCall(_)
            | ResolvedExpression::Construct(_)
            | ResolvedExpression::Unary(_)
            | ResolvedExpression::Binary(_)) => Self::from_produced(producer, class),
            unsupported => return Err(Box::new(unsupported)),
        })
    }

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

    pub fn from_static_field(field: StaticFieldId, class: ClassId, span: Span) -> Self {
        Self::StaticField {
            field,
            projections: Vec::new(),
            class,
            span,
        }
    }

    pub fn from_optional_payload(unwrap: ResolvedUnwrapExpr, class: ClassId) -> Self {
        let span = unwrap.span;
        Self::OptionalPayload {
            unwrap: Box::new(unwrap),
            projections: Vec::new(),
            class,
            span,
        }
    }

    pub fn from_produced(producer: ResolvedExpression, class: ClassId) -> Self {
        let span = producer.span();
        Self::Produced {
            producer: Box::new(producer),
            exact_class: class,
            projections: Vec::new(),
            class,
            span,
        }
    }

    pub const fn class(&self) -> ClassId {
        match self {
            Self::BindingPath(path) => path.class,
            Self::StaticField { class, .. } => *class,
            Self::CastRelative { class, .. } => *class,
            Self::Dereference { class, .. } => *class,
            Self::OptionalPayload { class, .. } => *class,
            Self::ArrayElement { class, .. } => *class,
            Self::Produced { class, .. } => *class,
        }
    }

    pub const fn span(&self) -> Span {
        match self {
            Self::BindingPath(path) => path.span,
            Self::StaticField { span, .. } => *span,
            Self::CastRelative { span, .. } => *span,
            Self::Dereference { span, .. } => *span,
            Self::OptionalPayload { span, .. } => *span,
            Self::ArrayElement { span, .. } => *span,
            Self::Produced { span, .. } => *span,
        }
    }

    pub const fn binding_path(&self) -> Option<&ResolvedObjectPlace> {
        match self {
            Self::BindingPath(path) => Some(path),
            Self::StaticField { .. }
            | Self::CastRelative { .. }
            | Self::Dereference { .. }
            | Self::OptionalPayload { .. }
            | Self::ArrayElement { .. }
            | Self::Produced { .. } => None,
        }
    }

    pub const fn root(&self) -> Option<BindingId> {
        match self {
            Self::BindingPath(path) => Some(path.root),
            Self::StaticField { .. }
            | Self::CastRelative { .. }
            | Self::Dereference { .. }
            | Self::OptionalPayload { .. }
            | Self::ArrayElement { .. }
            | Self::Produced { .. } => None,
        }
    }

    pub fn projections(&self) -> &[ObjectProjection] {
        match self {
            Self::BindingPath(path) => &path.projections,
            Self::StaticField { projections, .. } => projections,
            Self::CastRelative { projections, .. } => projections,
            Self::Dereference { projections, .. } => projections,
            Self::OptionalPayload { projections, .. } => projections,
            Self::ArrayElement { projections, .. } => projections,
            Self::Produced { projections, .. } => projections,
        }
    }

    pub const fn cast(&self) -> Option<&ResolvedObjectCastExpr> {
        match self {
            Self::BindingPath(_) | Self::StaticField { .. } => None,
            Self::CastRelative { cast, .. } => Some(cast),
            Self::Dereference { .. }
            | Self::OptionalPayload { .. }
            | Self::ArrayElement { .. }
            | Self::Produced { .. } => None,
        }
    }

    pub fn with_span(self, span: Span) -> Self {
        match self {
            Self::BindingPath(path) => Self::BindingPath(path.with_span(span)),
            Self::StaticField {
                field,
                projections,
                class,
                ..
            } => Self::StaticField {
                field,
                projections,
                class,
                span,
            },
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
            Self::Dereference {
                dereference,
                projections,
                class,
                ..
            } => Self::Dereference {
                dereference,
                projections,
                class,
                span,
            },
            Self::OptionalPayload {
                unwrap,
                projections,
                class,
                ..
            } => Self::OptionalPayload {
                unwrap,
                projections,
                class,
                span,
            },
            Self::ArrayElement {
                projection,
                projections,
                class,
                ..
            } => Self::ArrayElement {
                projection,
                projections,
                class,
                span,
            },
            Self::Produced {
                producer,
                exact_class,
                projections,
                class,
                ..
            } => Self::Produced {
                producer,
                exact_class,
                projections,
                class,
                span,
            },
        }
    }

    pub fn project_base(self, base: ClassId, span: Span) -> Self {
        match self {
            Self::BindingPath(path) => Self::BindingPath(path.project_base(base, span)),
            Self::StaticField {
                field,
                mut projections,
                ..
            } => {
                projections.push(ObjectProjection::Base(base));
                Self::StaticField {
                    field,
                    projections,
                    class: base,
                    span,
                }
            }
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
            Self::Dereference {
                dereference,
                mut projections,
                ..
            } => {
                projections.push(ObjectProjection::Base(base));
                Self::Dereference {
                    dereference,
                    projections,
                    class: base,
                    span,
                }
            }
            Self::OptionalPayload {
                unwrap,
                mut projections,
                ..
            } => {
                projections.push(ObjectProjection::Base(base));
                Self::OptionalPayload {
                    unwrap,
                    projections,
                    class: base,
                    span,
                }
            }
            Self::ArrayElement {
                projection,
                mut projections,
                ..
            } => {
                projections.push(ObjectProjection::Base(base));
                Self::ArrayElement {
                    projection,
                    projections,
                    class: base,
                    span,
                }
            }
            Self::Produced {
                producer,
                exact_class,
                mut projections,
                ..
            } => {
                projections.push(ObjectProjection::Base(base));
                Self::Produced {
                    producer,
                    exact_class,
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
            Self::StaticField {
                field: root,
                mut projections,
                class: receiver_class,
                ..
            } => {
                assert_eq!(field.class(), receiver_class);
                projections.push(ObjectProjection::Field(field));
                Self::StaticField {
                    field: root,
                    projections,
                    class,
                    span,
                }
            }
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
            Self::Dereference {
                dereference,
                mut projections,
                class: receiver_class,
                ..
            } => {
                assert_eq!(
                    field.class(),
                    receiver_class,
                    "dereference-relative projection must belong to the current terminal class"
                );
                projections.push(ObjectProjection::Field(field));
                Self::Dereference {
                    dereference,
                    projections,
                    class,
                    span,
                }
            }
            Self::OptionalPayload {
                unwrap,
                mut projections,
                class: receiver_class,
                ..
            } => {
                assert_eq!(
                    field.class(),
                    receiver_class,
                    "optional-payload projection must belong to the current terminal class"
                );
                projections.push(ObjectProjection::Field(field));
                Self::OptionalPayload {
                    unwrap,
                    projections,
                    class,
                    span,
                }
            }
            Self::ArrayElement {
                projection,
                mut projections,
                class: receiver_class,
                ..
            } => {
                assert_eq!(field.class(), receiver_class);
                projections.push(ObjectProjection::Field(field));
                Self::ArrayElement {
                    projection,
                    projections,
                    class,
                    span,
                }
            }
            Self::Produced {
                producer,
                exact_class,
                mut projections,
                class: receiver_class,
                ..
            } => {
                assert_eq!(
                    field.class(),
                    receiver_class,
                    "produced projection must belong to the current terminal class"
                );
                projections.push(ObjectProjection::Field(field));
                Self::Produced {
                    producer,
                    exact_class,
                    projections,
                    class,
                    span,
                }
            }
        }
    }
}
