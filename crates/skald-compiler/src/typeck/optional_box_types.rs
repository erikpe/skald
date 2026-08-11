//! Lowering of resolved optional-box identities into typed HIR metadata.

use crate::{
    hir::{HirOptionalBoxType, HirOptionalBoxTypeTable, HirViewTarget},
    resolve::{ResolvedObjectTarget, ResolvedProgram},
};

pub(super) fn lower_optional_box_types(program: &ResolvedProgram) -> HirOptionalBoxTypeTable {
    HirOptionalBoxTypeTable::new(
        program
            .optional_box_types
            .iter()
            .map(|target| HirOptionalBoxType {
                id: target.id,
                exact_optional: target.optional,
                exact_dynamic_class: target.optional.and(match target.object_leaf {
                    Some(ResolvedObjectTarget::Class(class)) => Some(class),
                    Some(ResolvedObjectTarget::Interface(_) | ResolvedObjectTarget::Obj) | None => {
                        None
                    }
                }),
                optional_depth: target.optional_depth,
                object_view: target.object_leaf.map(lower_object_view),
                span: target.span,
            })
            .collect(),
    )
}

const fn lower_object_view(target: ResolvedObjectTarget) -> HirViewTarget {
    match target {
        ResolvedObjectTarget::Obj => HirViewTarget::Obj,
        ResolvedObjectTarget::Class(class) => HirViewTarget::Class(class),
        ResolvedObjectTarget::Interface(interface) => HirViewTarget::Interface(interface),
    }
}
