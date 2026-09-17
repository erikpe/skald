use super::super::{
    LoweredBlockId, LoweredObjectId, LoweredValueId, OwnedArena, SelectedBlockId, SelectedObjectId,
    SelectedValueId,
};
use super::*;
use crate::backend::plan::{
    test_fixtures::{facts, source},
    CheckedPlan,
};

#[test]
fn allocation_rejects_index_overflow_before_extending_storage() {
    assert_eq!(checked_index(usize::MAX), Err(PlanError::SizeOverflow));
    assert_eq!(checked_index(usize::MAX - 1), Ok(usize::MAX - 1));
}

#[test]
fn arenas_check_live_context_owner_and_range_before_indexing() {
    let first = CheckedPlan::check(facts()).unwrap();
    let second = CheckedPlan::check(facts()).unwrap();
    let owner = first.view().callable(source(0)).unwrap();
    let mut values = OwnedArena::<LoweredValueId, _>::new(owner);
    let value = values.push(42).unwrap();
    assert_eq!(value.id().index(), 0);
    assert_eq!(value.id().callable(), source(0));
    assert_eq!(values.get(value), Ok(&42));
    assert_eq!(values.get_id(value.id()), Ok(&42));
    *values.get_mut(value).unwrap() = 44;
    assert_eq!(values.get(value), Ok(&44));
    assert_eq!(
        values
            .handles()
            .map(|handle| handle.id())
            .collect::<Vec<_>>(),
        [value.id()]
    );
    let mut foreign =
        OwnedArena::<LoweredValueId, _>::new(second.view().callable(source(0)).unwrap());
    assert_eq!(
        values.get(foreign.push(99).unwrap()),
        Err(PlanError::WrongContext)
    );
    let mut other = OwnedArena::<LoweredValueId, _>::new(first.view().callable(source(1)).unwrap());
    assert_eq!(
        values.get(other.push(99).unwrap()),
        Err(PlanError::WrongOwner)
    );
    let mut malformed = value;
    malformed.id.index = usize::MAX;
    assert_eq!(values.get(malformed), Err(PlanError::OutOfBounds));
    assert_eq!(values.get_id(malformed.id), Err(PlanError::OutOfBounds));
    assert_eq!(values.get_mut(malformed), Err(PlanError::OutOfBounds));
    malformed.id.callable = source(1);
    assert_eq!(values.get(malformed), Err(PlanError::WrongOwner));
    assert_eq!(values.get_id(malformed.id), Err(PlanError::WrongOwner));
    values.push(43).unwrap();
    assert_eq!(
        values
            .iter()
            .map(|(id, value)| (id.index(), *value))
            .collect::<Vec<_>>(),
        [(0, 44), (1, 43)]
    );
}

#[test]
fn every_local_stage_and_storage_kind_has_a_distinct_type() {
    use std::{any::TypeId, collections::BTreeSet};
    let kinds = [
        TypeId::of::<LoweredBlockId>(),
        TypeId::of::<LoweredValueId>(),
        TypeId::of::<LoweredObjectId>(),
        TypeId::of::<SelectedBlockId>(),
        TypeId::of::<SelectedValueId>(),
        TypeId::of::<SelectedObjectId>(),
    ];
    assert_eq!(kinds.into_iter().collect::<BTreeSet<_>>().len(), 6);
}
