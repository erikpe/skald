//! Deterministic lowering of typed optional-box identities into MIR metadata.

use crate::{
    hir::{HirOptionalBoxTypeTable, HirViewTarget},
    mir::{MirOptionalBoxType, MirOptionalBoxTypeTable, MirViewTarget},
};

pub(super) fn lower(types: &HirOptionalBoxTypeTable) -> MirOptionalBoxTypeTable {
    MirOptionalBoxTypeTable::new(
        types
            .iter()
            .map(|box_type| MirOptionalBoxType {
                id: box_type.id,
                exact_optional: box_type.exact_optional,
                optional_depth: box_type.optional_depth,
                object_view: box_type.object_view.map(lower_view),
                span: box_type.span,
            })
            .collect(),
    )
}

fn lower_view(target: HirViewTarget) -> MirViewTarget {
    match target {
        HirViewTarget::Class(class) => MirViewTarget::Class(class),
        HirViewTarget::Interface(interface) => MirViewTarget::Interface(interface),
        HirViewTarget::Obj => MirViewTarget::Obj,
    }
}
