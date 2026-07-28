//! Deterministic pooling and target data objects for verified literal backing.

use std::collections::BTreeMap;

use crate::{identity::LiteralDataId, mir::MirProgram};

use super::{machine::AssemblyLiteralBacking, symbol};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct LiteralPool {
    symbols_by_data: Vec<String>,
    backings: Vec<AssemblyLiteralBacking>,
}

impl LiteralPool {
    pub(super) fn build(program: &MirProgram) -> Self {
        let mut pooled = BTreeMap::<Vec<u8>, usize>::new();
        let mut symbols_by_data = Vec::with_capacity(program.literal_data.iter().len());
        let mut backings = Vec::new();

        for data in program.literal_data.iter() {
            let pool_index = match pooled.get(&data.bytes).copied() {
                Some(index) => index,
                None => {
                    let index = backings.len();
                    pooled.insert(data.bytes.clone(), index);
                    backings.push(AssemblyLiteralBacking {
                        symbol: symbol::literal_backing(index),
                        metadata_symbol: symbol::shared_array_metadata(data.array),
                        bytes: data.bytes.clone(),
                    });
                    index
                }
            };
            symbols_by_data.push(symbol::literal_backing(pool_index));
        }

        Self {
            symbols_by_data,
            backings,
        }
    }

    pub(super) fn symbol(&self, data: LiteralDataId) -> &str {
        &self.symbols_by_data[data.index()]
    }

    pub(super) fn into_backings(self) -> Vec<AssemblyLiteralBacking> {
        self.backings
    }
}
