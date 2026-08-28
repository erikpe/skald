//! Type checking and construction of typed HIR.

use crate::{hir::Type, resolve::ResolvedTypeKind, type_capabilities::TypeCategory};

mod arrays;
mod capabilities;
mod containment;
mod expression;
mod function;
mod generic_requirements;
mod literal;
mod optional;
mod optional_box_types;
mod optional_types;
mod optional_validation;
mod program;
mod shared;
mod shared_compatibility;
mod stored_value;

pub use arrays::{
    ARRAY_CAPABILITY_UNAVAILABLE, ARRAY_LENGTH_OUT_OF_RANGE, ARRAY_PROJECTION_REQUIRES_ARRAY,
    INVALID_ARRAY_ELEMENT,
};
pub(crate) use capabilities::CopyPathElement;
pub use containment::RECURSIVE_INLINE_CONTAINMENT;
pub(crate) use generic_requirements::{
    failed_interface_specialization_requirements, failed_specialization_requirements,
};
pub use optional_validation::INVALID_OPTIONAL_TYPE;
pub use program::{
    type_check, TypeCheckOutput, AMBIGUOUS_INITIALIZER, COPY_OPERATION_UNAVAILABLE,
    F64_LITERAL_OUT_OF_RANGE, FIELD_INITIALIZATION, FINAL_FIELD_REPLACEMENT,
    FINAL_STATIC_INITIALIZER_REQUIRED, FINAL_STATIC_REPLACEMENT, GENERAL_ITERATION_UNSUPPORTED,
    IMPLICIT_SHARED_DEREFERENCE, INCOMPATIBLE_OPERATOR_RHS, INSUFFICIENT_ALIAS_ACCESS,
    INTEGER_LITERAL_OUT_OF_RANGE, INVALID_ALIAS_ARGUMENT, INVALID_ALIAS_PARAMETER,
    INVALID_CALL_STATEMENT, INVALID_CONSTRUCTION, INVALID_COPY_CONSTRUCTION, INVALID_ENTRY_POINT,
    INVALID_EXTERNAL_DECLARATION, INVALID_INITIALIZER_BODY, INVALID_INTERFACE_CONFORMANCE,
    INVALID_INTERFACE_REQUIREMENT, INVALID_OBJECT_CONTEXT, INVALID_OBJECT_DECLARATION,
    INVALID_OPERATOR_SELECTION, INVALID_OVERRIDE_SIGNATURE, INVALID_RETURN,
    INVALID_SHARED_CONVERSION, INVALID_STATIC_FIELD_TYPE, INVALID_TYPE_TEST, MISSING_ENTRY_POINT,
    MISSING_RETURN, NO_MATCHING_INITIALIZER, PANIC_REQUIRES_CALL_STATEMENT,
    PRIVATE_INITIALIZER_ACCESS, RANGE_HIR_PENDING, READ_ONLY_RECEIVER, TYPE_MISMATCH,
    U64_LITERAL_OUT_OF_RANGE, U8_LITERAL_OUT_OF_RANGE, WRONG_ARGUMENT_COUNT,
};

const fn type_category(ty: Type) -> TypeCategory {
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

const fn resolved_type_category(kind: ResolvedTypeKind) -> TypeCategory {
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

#[cfg(test)]
mod tests;
