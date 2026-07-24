//! Runtime type-test lowering and shared object-view translations.

use super::*;
use crate::hir::{HirAccess, HirTypeTest, HirTypeTestKind, HirViewTarget};

impl BodyLowerer<'_> {
    pub(super) fn lower_type_test(
        &mut self,
        expression: &HirExpression,
        test: &HirTypeTest,
    ) -> ValueId {
        let kind = match test.kind {
            HirTypeTestKind::StaticSuccess => MirRvalueKind::ConstantBool(true),
            HirTypeTestKind::StaticFailure => MirRvalueKind::ConstantBool(false),
            HirTypeTestKind::Runtime => MirRvalueKind::TypeTest {
                source: self.lower_object_view(&test.source),
                target: lower_view_target(test.target),
            },
        };
        self.assign(kind, MirType::Bool, expression.span)
    }
}

pub(super) const fn lower_view_target(target: HirViewTarget) -> MirViewTarget {
    match target {
        HirViewTarget::Class(class) => MirViewTarget::Class(class),
        HirViewTarget::Interface(interface) => MirViewTarget::Interface(interface),
        HirViewTarget::Obj => MirViewTarget::Obj,
    }
}

pub(super) const fn lower_access(access: HirAccess) -> MirAliasAccess {
    match access {
        HirAccess::ReadOnly => MirAliasAccess::ReadOnly,
        HirAccess::Mutable => MirAliasAccess::Mutable,
    }
}
