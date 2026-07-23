//! Deterministic target-side virtual-table analysis.
//!
//! MIR owns virtual-family identities and semantic slots. This module derives
//! the concrete per-class selections and target data representation so machine
//! details never leak back into target-independent IR.

use crate::{
    backend::{BackendError, Target},
    identity::{ClassId, MethodId, VirtualSlotId},
    mir::{MirProgram, MirVirtualFamily},
};

use super::{
    machine::AssemblyVirtualTable,
    symbol::{self, callable},
};

const VIRTUAL_ENTRY_SIZE: usize = 8;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct DispatchMetadata {
    tables: Vec<Vec<Option<MethodId>>>,
}

impl DispatchMetadata {
    pub(super) fn compute(program: &MirProgram) -> Result<Self, BackendError> {
        let mut tables = Vec::with_capacity(program.classes.len());
        for class in program.classes.iter() {
            let mut entries = vec![None; program.virtual_families.len()];
            for family in program.virtual_families.iter() {
                let selected = select_for_class(program, family, class.id);
                if let Some(method) = selected {
                    if program.member_definition(method.into()).is_none() {
                        return Err(BackendError::new(
                            Target::X86_64SysV,
                            None,
                            format!(
                                "virtual table for class {} selects method {method} without a MIR definition",
                                class.id
                            ),
                        ));
                    }
                }
                entries[family.slot.index()] = selected;
            }
            tables.push(entries);
        }
        Ok(Self { tables })
    }

    pub(super) fn table_symbol(&self, class: ClassId) -> Option<String> {
        (!self.tables[class.index()].is_empty()).then(|| symbol::virtual_table(class))
    }

    pub(super) fn slot_displacement(slot: VirtualSlotId) -> Result<i32, BackendError> {
        slot.index()
            .checked_mul(VIRTUAL_ENTRY_SIZE)
            .and_then(|offset| i32::try_from(offset).ok())
            .ok_or_else(|| {
                BackendError::new(
                    Target::X86_64SysV,
                    None,
                    "virtual table exceeds x86-64 displacement limits",
                )
            })
    }

    pub(super) fn assembly_tables(&self, program: &MirProgram) -> Vec<AssemblyVirtualTable> {
        self.tables
            .iter()
            .enumerate()
            .filter(|(_, entries)| !entries.is_empty())
            .map(|(index, entries)| {
                let class = ClassId::new(index);
                AssemblyVirtualTable {
                    symbol: symbol::virtual_table(class),
                    entries: entries
                        .iter()
                        .map(|method| method.map(|method| callable(program, method.into())))
                        .collect(),
                }
            })
            .collect()
    }
}

fn select_for_class(
    program: &MirProgram,
    family: &MirVirtualFamily,
    class: ClassId,
) -> Option<MethodId> {
    family
        .members
        .iter()
        .copied()
        .fold(None, |selected, method| {
            let owner = method.class();
            if owner != class && !program.is_ancestor(owner, class) {
                return selected;
            }
            match selected {
                Some(current) if !program.is_ancestor(current.class(), owner) => Some(current),
                _ => Some(method),
            }
        })
}
