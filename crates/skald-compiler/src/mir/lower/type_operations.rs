//! Runtime type-test lowering and shared object-view translations.

use super::*;
use crate::hir::{HirAccess, HirTypeTest, HirTypeTestKind, HirViewTarget};

impl BodyLowerer<'_> {
    pub(super) fn lower_type_test(
        &mut self,
        expression: &HirExpression,
        test: &HirTypeTest,
    ) -> ValueId {
        let optional_mark = self.optional_view_mark();
        // Source evaluation is observable even when the type relation is
        // statically known; checked optional views begin here.
        let source = self.lower_object_view(&test.source);
        let kind = match test.kind {
            HirTypeTestKind::StaticSuccess => MirRvalueKind::ConstantBool(true),
            HirTypeTestKind::StaticFailure => MirRvalueKind::ConstantBool(false),
            HirTypeTestKind::Runtime => MirRvalueKind::TypeTest {
                source,
                target: lower_view_target(test.target),
            },
        };
        let result = self.assign(kind, MirType::Bool, expression.span);
        self.end_optional_views_from(optional_mark, expression.span);
        result
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
