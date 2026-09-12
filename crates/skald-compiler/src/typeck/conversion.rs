//! Conversion from closed resolved types to typed-HIR types.

use crate::{
    hir::{HirParameterMode, Type},
    resolve::{ResolvedParameterBindingMode, ResolvedType, ResolvedTypeKind},
};

pub(super) fn lower_type(type_syntax: &ResolvedType) -> Type {
    lower_type_kind(type_syntax.kind)
}

/// Lowers an already closed resolved type identity without manufacturing
/// source syntax solely to call `lower_type`.
pub(super) fn lower_type_kind(kind: ResolvedTypeKind) -> Type {
    match kind {
        ResolvedTypeKind::I64 => Type::I64,
        ResolvedTypeKind::U64 => Type::U64,
        ResolvedTypeKind::U8 => Type::U8,
        ResolvedTypeKind::F64 => Type::F64,
        ResolvedTypeKind::Bool => Type::Bool,
        ResolvedTypeKind::Unit => Type::Unit,
        ResolvedTypeKind::Obj => Type::Obj,
        ResolvedTypeKind::Class(class) => Type::Class(class),
        ResolvedTypeKind::Interface(interface) => Type::Interface(interface),
        ResolvedTypeKind::Function(function) => Type::Function(function),
        ResolvedTypeKind::Array(array) => Type::Array(array),
        ResolvedTypeKind::Shared(target) => {
            Type::Shared(super::shared::lower_shared_target(target))
        }
        ResolvedTypeKind::Optional(optional) => Type::Optional(optional),
    }
}

pub(super) const fn lower_parameter_mode(mode: ResolvedParameterBindingMode) -> HirParameterMode {
    match mode {
        ResolvedParameterBindingMode::Value => HirParameterMode::Value,
        ResolvedParameterBindingMode::ReadOnlyAlias { .. } => HirParameterMode::ReadOnlyAlias,
        ResolvedParameterBindingMode::MutableAlias { .. } => HirParameterMode::MutableAlias,
    }
}

/// Compares resolved type identities while ignoring source-location metadata
/// carried by compound type syntax.
pub(super) fn same_resolved_type(left: &ResolvedType, right: &ResolvedType) -> bool {
    left.kind == right.kind
}
