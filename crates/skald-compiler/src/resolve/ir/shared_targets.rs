//! Canonical resolved shared-owner target categories and capabilities.

use crate::identity::{ArrayTypeId, ClassId, InterfaceId, OptionalBoxTypeId};

/// The object view carried by an ordinary shared owner or an optional box.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum ResolvedObjectTarget {
    Obj,
    Class(ClassId),
    Interface(InterfaceId),
}

/// One semantic shared-owner target.
///
/// Consumers should branch through [`Self::category`] instead of treating
/// every non-array target as an object. Optional boxes are owners, but their
/// pointee is an optional place and may or may not have an object leaf.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum ResolvedSharedTarget {
    Obj,
    Class(ClassId),
    Interface(InterfaceId),
    Array(ArrayTypeId),
    OptionalBox(OptionalBoxTypeId),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ResolvedSharedTargetCategory {
    Object(ResolvedObjectTarget),
    Array(ArrayTypeId),
    OptionalBox(OptionalBoxTypeId),
}

impl ResolvedSharedTarget {
    pub const fn category(self) -> ResolvedSharedTargetCategory {
        match self {
            Self::Obj => ResolvedSharedTargetCategory::Object(ResolvedObjectTarget::Obj),
            Self::Class(class) => {
                ResolvedSharedTargetCategory::Object(ResolvedObjectTarget::Class(class))
            }
            Self::Interface(interface) => {
                ResolvedSharedTargetCategory::Object(ResolvedObjectTarget::Interface(interface))
            }
            Self::Array(array) => ResolvedSharedTargetCategory::Array(array),
            Self::OptionalBox(target) => ResolvedSharedTargetCategory::OptionalBox(target),
        }
    }

    pub const fn object(self) -> Option<ResolvedObjectTarget> {
        match self.category() {
            ResolvedSharedTargetCategory::Object(target) => Some(target),
            ResolvedSharedTargetCategory::Array(_)
            | ResolvedSharedTargetCategory::OptionalBox(_) => None,
        }
    }

    pub const fn array(self) -> Option<ArrayTypeId> {
        match self.category() {
            ResolvedSharedTargetCategory::Array(array) => Some(array),
            ResolvedSharedTargetCategory::Object(_)
            | ResolvedSharedTargetCategory::OptionalBox(_) => None,
        }
    }

    pub const fn optional_box(self) -> Option<OptionalBoxTypeId> {
        match self.category() {
            ResolvedSharedTargetCategory::OptionalBox(target) => Some(target),
            ResolvedSharedTargetCategory::Object(_) | ResolvedSharedTargetCategory::Array(_) => {
                None
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::identity::{ArrayTypeId, OptionalBoxTypeId};

    #[test]
    fn target_capabilities_are_disjoint_and_exhaustive() {
        let object = ResolvedSharedTarget::Obj;
        assert_eq!(object.object(), Some(ResolvedObjectTarget::Obj));
        assert_eq!(object.array(), None);
        assert_eq!(object.optional_box(), None);

        let array = ResolvedSharedTarget::Array(ArrayTypeId::new(3));
        assert_eq!(array.object(), None);
        assert_eq!(array.array(), Some(ArrayTypeId::new(3)));
        assert_eq!(array.optional_box(), None);

        let optional = ResolvedSharedTarget::OptionalBox(OptionalBoxTypeId::new(4));
        assert_eq!(optional.object(), None);
        assert_eq!(optional.array(), None);
        assert_eq!(optional.optional_box(), Some(OptionalBoxTypeId::new(4)));
    }
}
