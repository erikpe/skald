//! Program-wide verification of canonical optional-box identities.

use crate::mir::{MirType, MirViewTarget};

use super::context::Verifier;

impl Verifier<'_> {
    pub(super) fn verify_optional_box_declarations(&mut self) {
        for (index, box_type) in self.program.optional_box_types.iter().enumerate() {
            if box_type.id.index() != index {
                self.program_error(format!(
                    "optional-box type table index {index} contains {}",
                    box_type.id
                ));
            }
            if box_type.optional_depth == 0 {
                self.program_error(format!(
                    "optional-box {} must contain at least one optional layer",
                    box_type.id
                ));
            }
            let valid = match box_type.exact_optional {
                Some(optional) => self.exact_optional_box_metadata_matches(
                    optional,
                    box_type.optional_depth,
                    box_type.object_view,
                ),
                None => {
                    matches!(
                        box_type.object_view,
                        Some(MirViewTarget::Interface(interface))
                            if self.program.interface(interface).is_some()
                    ) || box_type.object_view == Some(MirViewTarget::Obj)
                }
            };
            if !valid {
                self.program_error(format!(
                    "optional-box {} has inconsistent exact optional or object-view metadata",
                    box_type.id
                ));
            }
        }
    }

    fn exact_optional_box_metadata_matches(
        &self,
        outer: crate::identity::OptionalTypeId,
        depth: usize,
        object_view: Option<MirViewTarget>,
    ) -> bool {
        let mut ty = MirType::Optional(outer);
        for _ in 0..depth {
            let MirType::Optional(optional) = ty else {
                return false;
            };
            let Some(metadata) = self.program.optional_type(optional) else {
                return false;
            };
            ty = metadata.payload;
        }
        match (ty, object_view) {
            (MirType::Class(class), Some(MirViewTarget::Class(view))) => class == view,
            (MirType::Class(_), _) => false,
            (_, None) => true,
            _ => false,
        }
    }
}
