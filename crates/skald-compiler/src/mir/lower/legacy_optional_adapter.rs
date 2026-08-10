//! Exhaustive compatibility boundary from canonical HIR optionals to legacy MIR types.
//!
//! CO3 deliberately keeps executable MIR closed over the three previously
//! supported representations. New recursive HIR plans must fail here until
//! MIR itself is generalized.

use crate::{
    hir::{
        HirOptionalStorageCategory, HirOptionalTypeTable, HirPrimitiveType, HirSharedTarget, Type,
    },
    identity::{ClassId, OptionalTypeId},
    mir::{MirSharedTarget, MirType},
};

#[derive(Clone, Copy)]
pub(super) struct LegacyOptionalAdapter<'hir> {
    types: &'hir HirOptionalTypeTable,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum LegacyOptionalKind {
    Primitive(HirPrimitiveType),
    Class(ClassId),
    Shared(HirSharedTarget),
}

impl<'hir> LegacyOptionalAdapter<'hir> {
    pub(super) const fn new(types: &'hir HirOptionalTypeTable) -> Self {
        Self { types }
    }

    pub(super) fn kind(self, id: OptionalTypeId) -> LegacyOptionalKind {
        let optional = self
            .types
            .get(id)
            .expect("HIR optional identity must name metadata");
        match optional.storage {
            HirOptionalStorageCategory::Scalar => {
                LegacyOptionalKind::Primitive(match optional.payload {
                    Type::I64 => HirPrimitiveType::I64,
                    Type::U64 => HirPrimitiveType::U64,
                    Type::U8 => HirPrimitiveType::U8,
                    Type::F64 => HirPrimitiveType::F64,
                    Type::Bool => HirPrimitiveType::Bool,
                    _ => unreachable!("scalar optional plan must carry a primitive payload"),
                })
            }
            HirOptionalStorageCategory::InlineClass(class) => LegacyOptionalKind::Class(class),
            HirOptionalStorageCategory::SharedOwner(target) => LegacyOptionalKind::Shared(target),
            HirOptionalStorageCategory::InlineArray(_) | HirOptionalStorageCategory::Nested(_) => {
                unreachable!("recursive optional HIR is gated until MIR is generalized")
            }
        }
    }

    pub(super) fn lower_type(self, ty: Type) -> MirType {
        match ty {
            Type::Optional(optional) => match self.kind(optional) {
                LegacyOptionalKind::Primitive(payload) => {
                    MirType::OptionalPrimitive(super::primitive::lower_primitive_type(payload))
                }
                LegacyOptionalKind::Class(class) => MirType::OptionalClass(class),
                LegacyOptionalKind::Shared(target) => MirType::OptionalShared(lower_shared(target)),
            },
            _ => super::lower_non_optional_type(ty),
        }
    }

    pub(super) fn operand_primitive(
        self,
        operand: &crate::hir::HirOptionalOperand,
    ) -> HirPrimitiveType {
        match operand {
            crate::hir::HirOptionalOperand::Place(place) => place.payload,
            crate::hir::HirOptionalOperand::Produced(expression) => match expression.ty {
                Type::Optional(optional) => match self.kind(optional) {
                    LegacyOptionalKind::Primitive(payload) => payload,
                    _ => unreachable!("primitive operand must name primitive optional metadata"),
                },
                _ => unreachable!("produced optional operand must have optional type"),
            },
            _ => unreachable!("expected primitive optional operand"),
        }
    }

    pub(super) fn operand_class(self, operand: &crate::hir::HirOptionalOperand) -> ClassId {
        match operand {
            crate::hir::HirOptionalOperand::ClassPlace(place) => place.class,
            crate::hir::HirOptionalOperand::ClassProduced(expression) => match expression.ty {
                Type::Optional(optional) => match self.kind(optional) {
                    LegacyOptionalKind::Class(class) => class,
                    _ => unreachable!("class operand must name class optional metadata"),
                },
                _ => unreachable!("produced optional operand must have optional type"),
            },
            _ => unreachable!("expected class optional operand"),
        }
    }

    pub(super) fn operand_shared(
        self,
        operand: &crate::hir::HirOptionalOperand,
    ) -> HirSharedTarget {
        match operand {
            crate::hir::HirOptionalOperand::SharedPlace(place) => place.target,
            crate::hir::HirOptionalOperand::SharedProduced(expression) => match expression.ty {
                Type::Optional(optional) => match self.kind(optional) {
                    LegacyOptionalKind::Shared(target) => target,
                    _ => unreachable!("shared operand must name shared optional metadata"),
                },
                _ => unreachable!("produced optional operand must have optional type"),
            },
            _ => unreachable!("expected shared optional operand"),
        }
    }
}

fn lower_shared(target: HirSharedTarget) -> MirSharedTarget {
    match target {
        HirSharedTarget::Obj => MirSharedTarget::Obj,
        HirSharedTarget::Class(class) => MirSharedTarget::Class(class),
        HirSharedTarget::Interface(interface) => MirSharedTarget::Interface(interface),
        HirSharedTarget::Array(array) => MirSharedTarget::Array(array),
    }
}
