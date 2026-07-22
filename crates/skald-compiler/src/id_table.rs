//! Narrow, typed storage for dense global-ID tables and sparse function slots.
//!
//! Phase modules retain their own public table and record types. These two
//! containers only centralize the dense and optional-slot invariants shared by
//! resolved IR, HIR, and MIR.

use std::marker::PhantomData;

use crate::identity::{ClassId, FunctionId};

pub(crate) trait DenseId: Copy + Eq {
    fn index(self) -> usize;
}

impl DenseId for FunctionId {
    fn index(self) -> usize {
        self.index()
    }
}

impl DenseId for ClassId {
    fn index(self) -> usize {
        self.index()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct DenseIdTable<I, T> {
    entries: Vec<T>,
    id: PhantomData<fn(I) -> I>,
}

impl<I: DenseId, T> DenseIdTable<I, T> {
    pub(crate) fn new(entries: Vec<T>, id_of: impl Fn(&T) -> I) -> Self {
        assert!(
            entries
                .iter()
                .enumerate()
                .all(|(index, entry)| id_of(entry).index() == index),
            "dense ID table entries must be ordered by ID"
        );
        Self {
            entries,
            id: PhantomData,
        }
    }

    pub(crate) fn get(&self, id: I, id_of: impl Fn(&T) -> I) -> Option<&T> {
        self.entries
            .get(id.index())
            .filter(|entry| id_of(entry) == id)
    }

    pub(crate) fn iter(&self) -> impl ExactSizeIterator<Item = &T> {
        self.entries.iter()
    }

    pub(crate) fn len(&self) -> usize {
        self.entries.len()
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    #[cfg(test)]
    pub(crate) fn entries_mut_for_test(&mut self) -> &mut [T] {
        &mut self.entries
    }
}

impl<I, T> Default for DenseIdTable<I, T> {
    fn default() -> Self {
        Self {
            entries: Vec::new(),
            id: PhantomData,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct SparseFunctionTable<T> {
    entries: Vec<Option<T>>,
    occupied: usize,
}

impl<T> SparseFunctionTable<T> {
    pub(crate) fn new(entries: Vec<Option<T>>, id_of: impl Fn(&T) -> FunctionId) -> Self {
        assert!(
            entries.iter().enumerate().all(|(index, entry)| entry
                .as_ref()
                .is_none_or(|entry| id_of(entry).index() == index)),
            "sparse function table entries must occupy their ID slot"
        );
        let occupied = entries.iter().flatten().count();
        Self { entries, occupied }
    }

    pub(crate) fn get(&self, id: FunctionId) -> Option<&T> {
        self.entries.get(id.index())?.as_ref()
    }

    pub(crate) fn iter(&self) -> impl Iterator<Item = &T> {
        self.entries.iter().flatten()
    }

    pub(crate) const fn len(&self) -> usize {
        self.occupied
    }

    pub(crate) const fn is_empty(&self) -> bool {
        self.occupied == 0
    }

    pub(crate) fn indexed_slots(&self) -> impl ExactSizeIterator<Item = (usize, Option<&T>)> {
        self.entries
            .iter()
            .enumerate()
            .map(|(index, entry)| (index, entry.as_ref()))
    }

    #[cfg(test)]
    pub(crate) fn get_mut_for_test(&mut self, id: FunctionId) -> Option<&mut T> {
        self.entries.get_mut(id.index())?.as_mut()
    }

    #[cfg(test)]
    pub(crate) fn remove_for_test(&mut self, id: FunctionId) {
        if self
            .entries
            .get_mut(id.index())
            .and_then(Option::take)
            .is_some()
        {
            self.occupied -= 1;
        }
    }
}

impl<T> Default for SparseFunctionTable<T> {
    fn default() -> Self {
        Self {
            entries: Vec::new(),
            occupied: 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Clone, Debug, Eq, PartialEq)]
    struct FunctionEntry {
        id: FunctionId,
        value: u8,
    }

    fn function_entry(id: usize, value: u8) -> FunctionEntry {
        FunctionEntry {
            id: FunctionId::new(id),
            value,
        }
    }

    #[derive(Clone, Debug, Eq, PartialEq)]
    struct ClassEntry {
        id: ClassId,
        value: u8,
    }

    fn class_entry(id: usize, value: u8) -> ClassEntry {
        ClassEntry {
            id: ClassId::new(id),
            value,
        }
    }

    #[test]
    fn dense_tables_validate_lookup_and_iterate_in_id_order() {
        let table = DenseIdTable::new(
            vec![function_entry(0, 10), function_entry(1, 20)],
            |entry| entry.id,
        );

        assert_eq!(table.len(), 2);
        assert!(!table.is_empty());
        assert_eq!(
            table
                .get(FunctionId::new(0), |entry| entry.id)
                .unwrap()
                .value,
            10
        );
        assert_eq!(
            table
                .get(FunctionId::new(1), |entry| entry.id)
                .unwrap()
                .value,
            20
        );
        assert!(table.get(FunctionId::new(99), |entry| entry.id).is_none());
        assert_eq!(
            table.iter().map(|entry| entry.value).collect::<Vec<_>>(),
            vec![10, 20]
        );
    }

    #[test]
    #[should_panic(expected = "dense ID table entries must be ordered by ID")]
    fn dense_function_tables_reject_non_dense_ids() {
        let _ = DenseIdTable::new(vec![function_entry(1, 10)], |entry| entry.id);
    }

    #[test]
    fn dense_tables_apply_the_same_contract_to_class_ids() {
        let table = DenseIdTable::new(vec![class_entry(0, 10), class_entry(1, 20)], |entry| {
            entry.id
        });

        assert_eq!(
            table.get(ClassId::new(1), |entry| entry.id).unwrap().value,
            20
        );
        assert_eq!(
            table.iter().map(|entry| entry.id).collect::<Vec<_>>(),
            [ClassId::new(0), ClassId::new(1)]
        );
    }

    #[test]
    #[should_panic(expected = "dense ID table entries must be ordered by ID")]
    fn dense_class_tables_reject_non_dense_ids() {
        let _ = DenseIdTable::new(vec![class_entry(1, 10)], |entry| entry.id);
    }

    #[test]
    fn dense_lookup_validates_the_entry_in_the_indexed_slot() {
        let mut table = DenseIdTable::new(vec![class_entry(0, 10), class_entry(1, 20)], |entry| {
            entry.id
        });
        table.entries_mut_for_test()[1].id = ClassId::new(0);

        assert!(table.get(ClassId::new(1), |entry| entry.id).is_none());
    }

    #[test]
    fn sparse_tables_distinguish_slots_from_occupied_entries() {
        let mut table = SparseFunctionTable::new(
            vec![
                Some(function_entry(0, 10)),
                None,
                Some(function_entry(2, 30)),
            ],
            |entry| entry.id,
        );

        assert_eq!(table.len(), 2);
        assert!(!table.is_empty());
        assert_eq!(table.get(FunctionId::new(0)).unwrap().value, 10);
        assert!(table.get(FunctionId::new(1)).is_none());
        assert!(table.get(FunctionId::new(99)).is_none());
        assert_eq!(
            table.iter().map(|entry| entry.value).collect::<Vec<_>>(),
            vec![10, 30]
        );
        assert_eq!(
            table
                .indexed_slots()
                .map(|(index, entry)| (index, entry.map(|entry| entry.value)))
                .collect::<Vec<_>>(),
            vec![(0, Some(10)), (1, None), (2, Some(30))]
        );

        table.get_mut_for_test(FunctionId::new(2)).unwrap().value = 31;
        table.remove_for_test(FunctionId::new(0));
        assert_eq!(table.len(), 1);
        assert_eq!(table.get(FunctionId::new(2)).unwrap().value, 31);
    }

    #[test]
    #[should_panic(expected = "sparse function table entries must occupy their ID slot")]
    fn sparse_tables_reject_entries_in_the_wrong_slot() {
        let _ = SparseFunctionTable::new(vec![Some(function_entry(1, 10))], |entry| entry.id);
    }

    #[test]
    fn empty_tables_have_consistent_defaults() {
        let dense = DenseIdTable::<FunctionId, FunctionEntry>::default();
        let sparse = SparseFunctionTable::<FunctionEntry>::default();

        assert!(dense.is_empty());
        assert_eq!(dense.iter().len(), 0);
        assert!(sparse.is_empty());
        assert_eq!(sparse.iter().count(), 0);
        assert_eq!(sparse.indexed_slots().len(), 0);
    }
}
