//! Structural relationships between MIR places.

use super::super::model::MirPlace;

pub(super) fn is_ancestor(ancestor: &MirPlace, place: &MirPlace) -> bool {
    ancestor.base == place.base && place.projections.starts_with(&ancestor.projections)
}

pub(super) fn places_overlap(left: &MirPlace, right: &MirPlace) -> bool {
    is_ancestor(left, right) || is_ancestor(right, left)
}

#[cfg(test)]
mod tests {
    use crate::identity::{FieldId, FunctionId};

    use super::super::super::model::StorageId;
    use super::*;

    #[test]
    fn ancestry_requires_the_same_base_and_a_projection_prefix() {
        let function = FunctionId::new(0);
        let class = crate::identity::ClassId::new(0);
        let root = MirPlace::base(StorageId::new(function, 0));
        let child = root.clone().project_field(FieldId::new(class, 0));
        let grandchild = child.clone().project_field(FieldId::new(class, 1));
        let sibling = root.clone().project_field(FieldId::new(class, 2));
        let other_root = MirPlace::base(StorageId::new(function, 1));
        let alias_root = MirPlace::alias_parameter(StorageId::new(function, 0));

        assert!(is_ancestor(&root, &root));
        assert!(is_ancestor(&root, &grandchild));
        assert!(is_ancestor(&child, &grandchild));
        assert!(!is_ancestor(&grandchild, &child));
        assert!(!is_ancestor(&sibling, &grandchild));
        assert!(!is_ancestor(&root, &other_root));
        assert!(!is_ancestor(&root, &alias_root));
    }

    #[test]
    fn overlap_is_symmetric_and_limited_to_ancestor_relationships() {
        let function = FunctionId::new(0);
        let class = crate::identity::ClassId::new(0);
        let root = MirPlace::base(StorageId::new(function, 0));
        let child = root.clone().project_field(FieldId::new(class, 0));
        let sibling = root.clone().project_field(FieldId::new(class, 1));

        assert!(places_overlap(&root, &child));
        assert!(places_overlap(&child, &root));
        assert!(!places_overlap(&child, &sibling));
    }
}
