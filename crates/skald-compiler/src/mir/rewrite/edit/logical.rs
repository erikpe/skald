use std::collections::BTreeSet;

use super::super::error::MirRewriteError;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct LogicalRecordIndex(usize);

impl LogicalRecordIndex {
    pub(in crate::mir::rewrite) const fn new(index: usize) -> Self {
        Self(index)
    }

    pub(super) const fn index(self) -> usize {
        self.0
    }
}

/// Ordered logical records whose stable transaction indices survive deletion.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct LogicalRecords<T> {
    original_len: usize,
    entries: Vec<Option<T>>,
    order: Vec<LogicalRecordIndex>,
}

impl<T> LogicalRecords<T> {
    pub(super) fn from_dense(entries: Vec<T>) -> Self {
        let order = (0..entries.len()).map(LogicalRecordIndex::new).collect();
        Self {
            original_len: entries.len(),
            entries: entries.into_iter().map(Some).collect(),
            order,
        }
    }

    pub(super) fn allocate(&mut self, entry: T) -> LogicalRecordIndex {
        let index = LogicalRecordIndex::new(self.entries.len());
        self.entries.push(Some(entry));
        self.order.push(index);
        index
    }

    pub(super) fn get(&self, index: LogicalRecordIndex) -> Result<&T, MirRewriteError> {
        match self.entries.get(index.index()) {
            Some(Some(entry)) => Ok(entry),
            Some(None) => Err(MirRewriteError::DeletedLogicalRecord {
                index: index.index(),
            }),
            None => Err(MirRewriteError::UnknownLogicalRecord {
                index: index.index(),
            }),
        }
    }

    pub(super) fn get_mut(&mut self, index: LogicalRecordIndex) -> Result<&mut T, MirRewriteError> {
        match self.entries.get_mut(index.index()) {
            Some(Some(entry)) => Ok(entry),
            Some(None) => Err(MirRewriteError::DeletedLogicalRecord {
                index: index.index(),
            }),
            None => Err(MirRewriteError::UnknownLogicalRecord {
                index: index.index(),
            }),
        }
    }

    pub(super) fn remove(&mut self, index: LogicalRecordIndex) -> Result<T, MirRewriteError> {
        match self.entries.get(index.index()) {
            Some(Some(_)) => {}
            Some(None) => {
                return Err(MirRewriteError::DeletedLogicalRecord {
                    index: index.index(),
                });
            }
            None => {
                return Err(MirRewriteError::UnknownLogicalRecord {
                    index: index.index(),
                });
            }
        }
        let order_index = self.order.iter().position(|entry| *entry == index).ok_or(
            MirRewriteError::MissingLogicalOrder {
                index: index.index(),
            },
        )?;
        let removed = self.entries[index.index()]
            .take()
            .expect("live logical record was checked");
        self.order.remove(order_index);
        Ok(removed)
    }

    pub(super) fn order(&self) -> &[LogicalRecordIndex] {
        &self.order
    }

    pub(super) const fn original_len(&self) -> usize {
        self.original_len
    }

    pub(super) fn slot_liveness(&self) -> impl Iterator<Item = bool> + '_ {
        self.entries.iter().map(Option::is_some)
    }

    pub(super) fn validate_order(&self) -> Result<(), MirRewriteError> {
        let mut ordered = BTreeSet::new();
        for index in self.order.iter().copied() {
            if !ordered.insert(index) {
                return Err(MirRewriteError::DuplicateLogicalOrder {
                    index: index.index(),
                });
            }
            self.get(index)?;
        }
        for (index, entry) in self.entries.iter().enumerate() {
            if entry.is_some() && !ordered.contains(&LogicalRecordIndex::new(index)) {
                return Err(MirRewriteError::MissingLogicalOrder { index });
            }
        }
        Ok(())
    }

    pub(super) fn into_explicit_order(mut self) -> Result<Vec<T>, MirRewriteError> {
        self.validate_order()?;
        let mut ordered = Vec::with_capacity(self.order.len());
        for index in self.order {
            ordered.push(
                self.entries[index.index()]
                    .take()
                    .expect("logical live order was validated"),
            );
        }
        Ok(ordered)
    }

    #[cfg(test)]
    pub(super) fn replace_order_for_test(&mut self, order: Vec<LogicalRecordIndex>) {
        self.order = order;
    }
}
