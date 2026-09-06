//! Typed static declarations, declaration initializers, and receiver-free places.

use crate::{
    identity::{StaticFieldId, StaticInitializerId},
    source::Span,
};

use super::{HirLocal, HirStoredValueInitialization, Type};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirStaticFieldDeclaration {
    pub id: StaticFieldId,
    pub static_span: Span,
    pub final_span: Option<Span>,
    pub name: String,
    pub name_span: Span,
    pub ty: Type,
    pub initializer: Option<HirStaticFieldInitializer>,
    pub span: Span,
}

/// Typed direct initialization of one previously uninitialized static slot.
///
/// The stored-value plan retains source ownership and selected lifecycle
/// operations. MIR lowering remains responsible for making temporary cleanup
/// and publication boundaries explicit.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirStaticFieldInitializer {
    pub id: StaticInitializerId,
    pub equal_span: Span,
    pub locals: Vec<HirLocal>,
    pub value: HirStoredValueInitialization,
    pub span: Span,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HirStaticPlace {
    pub field: StaticFieldId,
    pub span: Span,
}
