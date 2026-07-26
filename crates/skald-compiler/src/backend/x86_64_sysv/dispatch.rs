//! Deterministic target-side dynamic-dispatch analysis.
//!
//! MIR owns virtual-family, interface, and requirement identities. This module
//! derives concrete per-class selections and target table representation so
//! machine details never leak back into target-independent IR.

use crate::{
    backend::{BackendError, Target},
    identity::{ClassId, InterfaceRequirementId, MethodId, VirtualSlotId},
    mir::{MirProgram, MirVirtualFamily},
};

use super::{
    machine::AssemblyDispatchTable,
    symbol::{self, callable},
};

const DISPATCH_ENTRY_SIZE: usize = 8;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct DispatchMetadata {
    /// Per-class entries: canonical virtual slots, then each interface's
    /// requirements in typed identity order.
    tables: Vec<Vec<Option<MethodId>>>,
    /// First table entry for each dense `InterfaceId`.
    interface_starts: Vec<usize>,
    finalizer_displacement: i32,
}

impl DispatchMetadata {
    pub(super) fn compute(program: &MirProgram) -> Result<Self, BackendError> {
        let (interface_starts, entry_count) = interface_layout(program)?;
        let mut tables = Vec::with_capacity(program.classes.len());
        for class in program.classes.iter() {
            let mut entries = vec![None; entry_count];
            for family in program.virtual_families.iter() {
                let selected = select_for_class(program, family, class.id);
                verify_executable_selection(program, class.id, "virtual table", selected)?;
                entries[family.slot.index()] = selected;
            }
            for conformance in &class.conformances {
                let start = interface_starts[conformance.interface.index()];
                for implementation in &conformance.implementations {
                    verify_executable_selection(
                        program,
                        class.id,
                        "interface witness",
                        Some(implementation.method),
                    )?;
                    entries[start + implementation.requirement.index()] =
                        Some(implementation.method);
                }
            }
            tables.push(entries);
        }
        let finalizer_displacement = entry_displacement(entry_count, "complete finalizer table")?;
        Ok(Self {
            tables,
            interface_starts,
            finalizer_displacement,
        })
    }

    pub(super) fn table_symbol(&self, class: ClassId) -> String {
        debug_assert!(class.index() < self.tables.len());
        symbol::dispatch_table(class)
    }

    pub(super) fn classes_providing_view(
        &self,
        program: &MirProgram,
        target: crate::mir::MirViewTarget,
    ) -> Vec<ClassId> {
        program
            .classes
            .iter()
            .filter(|class| match target {
                crate::mir::MirViewTarget::Class(target) => {
                    class.id == target || program.is_ancestor(target, class.id)
                }
                crate::mir::MirViewTarget::Interface(interface) => {
                    program.conformance(class.id, interface).is_some()
                }
                crate::mir::MirViewTarget::Obj => true,
            })
            .map(|class| class.id)
            .collect()
    }

    pub(super) fn slot_displacement(slot: VirtualSlotId) -> Result<i32, BackendError> {
        entry_displacement(slot.index(), "virtual table")
    }

    pub(super) fn requirement_displacement(
        &self,
        requirement: InterfaceRequirementId,
    ) -> Result<i32, BackendError> {
        let index = self.interface_starts[requirement.interface().index()]
            .checked_add(requirement.index())
            .ok_or_else(|| displacement_error("interface witness table"))?;
        entry_displacement(index, "interface witness table")
    }

    pub(super) const fn finalizer_displacement(&self) -> i32 {
        self.finalizer_displacement
    }

    pub(super) fn assembly_tables(&self, program: &MirProgram) -> Vec<AssemblyDispatchTable> {
        let class_tables = self.tables.iter().enumerate().map(|(index, entries)| {
            let class = ClassId::new(index);
            AssemblyDispatchTable {
                symbol: symbol::dispatch_table(class),
                entries: entries
                    .iter()
                    .map(|method| method.map(|method| callable(program, method.into())))
                    .chain(std::iter::once(Some(symbol::complete_finalizer(class))))
                    .collect(),
            }
        });
        let array_tables = program
            .array_types
            .iter()
            .map(|array| AssemblyDispatchTable {
                symbol: symbol::shared_array_metadata(array.id),
                entries: std::iter::repeat_n(None, self.finalizer_displacement as usize / 8)
                    .chain(std::iter::once(Some(symbol::shared_array_finalizer(
                        array.id,
                    ))))
                    .collect(),
            });
        class_tables.chain(array_tables).collect()
    }
}

fn interface_layout(program: &MirProgram) -> Result<(Vec<usize>, usize), BackendError> {
    let mut starts = Vec::with_capacity(program.interfaces.iter().len());
    let mut entry_count = program.virtual_families.len();
    for interface in program.interfaces.iter() {
        starts.push(entry_count);
        entry_count = entry_count
            .checked_add(interface.requirements.len())
            .ok_or_else(|| displacement_error("interface witness table"))?;
    }
    if entry_count != 0 {
        entry_displacement(entry_count - 1, "class dispatch table")?;
    }
    Ok((starts, entry_count))
}

fn verify_executable_selection(
    program: &MirProgram,
    class: ClassId,
    table: &str,
    selected: Option<MethodId>,
) -> Result<(), BackendError> {
    if let Some(method) = selected {
        if program.member_definition(method.into()).is_none() {
            return Err(BackendError::new(
                Target::X86_64SysV,
                None,
                format!(
                    "{table} for class {class} selects method {method} without a MIR definition"
                ),
            ));
        }
    }
    Ok(())
}

fn entry_displacement(index: usize, table: &str) -> Result<i32, BackendError> {
    index
        .checked_mul(DISPATCH_ENTRY_SIZE)
        .and_then(|offset| i32::try_from(offset).ok())
        .ok_or_else(|| displacement_error(table))
}

fn displacement_error(table: &str) -> BackendError {
    BackendError::new(
        Target::X86_64SysV,
        None,
        format!("{table} exceeds x86-64 displacement limits"),
    )
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::identity::InterfaceId;

    #[test]
    fn reports_interface_witness_displacement_overflow() {
        let metadata = DispatchMetadata {
            tables: vec![],
            interface_starts: vec![usize::MAX],
            finalizer_displacement: 0,
        };
        let runner = InterfaceId::new(0);
        let error = metadata
            .requirement_displacement(InterfaceRequirementId::new(runner, 0))
            .unwrap_err();

        assert!(error
            .message()
            .contains("interface witness table exceeds x86-64 displacement limits"));
    }
}
