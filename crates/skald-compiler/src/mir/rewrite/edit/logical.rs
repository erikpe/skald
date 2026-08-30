use super::super::error::MirRewriteError;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(in crate::mir::rewrite) struct LogicalRecordIndex(usize);

impl LogicalRecordIndex {
    pub(super) const fn new(index: usize) -> Self {
        Self(index)
    }

    pub(super) const fn index(self) -> usize {
        self.0
    }
}

/// Ordered logical records whose stable transaction indices survive deletion.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct LogicalRecords<T> {
    entries: Vec<Option<T>>,
    order: Vec<LogicalRecordIndex>,
}

impl<T> LogicalRecords<T> {
    pub(super) fn from_dense(entries: Vec<T>) -> Self {
        let order = (0..entries.len()).map(LogicalRecordIndex::new).collect();
        Self {
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
}
