//! Typed static declarations and receiver-free static places.

use crate::{identity::StaticFieldId, source::Span};

use super::Type;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirStaticFieldDeclaration {
    pub id: StaticFieldId,
    pub static_span: Span,
    pub name: String,
    pub name_span: Span,
    pub ty: Type,
    pub span: Span,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HirStaticPlace {
    pub field: StaticFieldId,
    pub span: Span,
}
