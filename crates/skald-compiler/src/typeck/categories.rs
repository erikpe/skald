//! Type-checker adapters for target-independent type capability categories.

use crate::{
    hir::Type,
    resolve::{ResolvedProgram, ResolvedTypeKind},
    type_capabilities::TypeCategory,
};

pub(super) const fn type_category(ty: Type) -> TypeCategory {
    match ty {
        Type::I64 | Type::U64 | Type::U8 | Type::F64 | Type::Bool => TypeCategory::Primitive,
        Type::Unit => TypeCategory::Unit,
        Type::Obj => TypeCategory::Obj,
        Type::Class(_) => TypeCategory::Class,
        Type::Interface(_) => TypeCategory::Interface,
        Type::Function(_) => TypeCategory::Function,
        Type::Shared(_) => TypeCategory::Shared,
        Type::Optional(_) => TypeCategory::Optional,
        Type::Array(_) => TypeCategory::Array,
    }
}

pub(super) const fn resolved_type_category(kind: ResolvedTypeKind) -> TypeCategory {
    match kind {
        ResolvedTypeKind::I64
        | ResolvedTypeKind::U64
        | ResolvedTypeKind::U8
        | ResolvedTypeKind::F64
        | ResolvedTypeKind::Bool => TypeCategory::Primitive,
        ResolvedTypeKind::Unit => TypeCategory::Unit,
        ResolvedTypeKind::Obj => TypeCategory::Obj,
        ResolvedTypeKind::Class(_) => TypeCategory::Class,
        ResolvedTypeKind::Interface(_) => TypeCategory::Interface,
        ResolvedTypeKind::Function(_) => TypeCategory::Function,
        ResolvedTypeKind::Shared(_) => TypeCategory::Shared,
        ResolvedTypeKind::Optional(_) => TypeCategory::Optional,
        ResolvedTypeKind::Array(_) => TypeCategory::Array,
    }
}

pub(super) fn supports_alias_type(program: &ResolvedProgram, ty: Type) -> bool {
    let optional_payload_supports_alias = match ty {
        Type::Optional(optional) => matches!(
            super::optional_types::classify_payload(program, optional),
            Some(
                super::optional_types::OptionalPayloadKind::Primitive(_)
                    | super::optional_types::OptionalPayloadKind::Class(_)
                    | super::optional_types::OptionalPayloadKind::Shared(_)
                    | super::optional_types::OptionalPayloadKind::Nested(_)
                    | super::optional_types::OptionalPayloadKind::Array(_)
            )
        ),
        _ => false,
    };
    crate::type_capabilities::supports_alias_target(
        type_category(ty),
        optional_payload_supports_alias,
    )
}
