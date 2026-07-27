//! Target-independent interface declarations and class conformance metadata.

use crate::{
    id_table::DenseIdTable,
    identity::{InterfaceId, InterfaceRequirementId, MethodId, ModuleId},
    source::Span,
};

use super::{MirParameter, MirReceiverAccess, MirType};

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct MirInterfaceDeclarationTable {
    entries: DenseIdTable<InterfaceId, MirInterfaceDeclaration>,
}

impl MirInterfaceDeclarationTable {
    pub(crate) fn new(entries: Vec<MirInterfaceDeclaration>) -> Self {
        Self {
            entries: DenseIdTable::new(entries, |declaration| declaration.id),
        }
    }

    pub fn get(&self, id: InterfaceId) -> Option<&MirInterfaceDeclaration> {
        self.entries.get(id, |declaration| declaration.id)
    }

    pub fn iter(&self) -> impl ExactSizeIterator<Item = &MirInterfaceDeclaration> {
        self.entries.iter()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    #[cfg(test)]
    pub(crate) fn entries_mut_for_test(&mut self) -> &mut [MirInterfaceDeclaration] {
        self.entries.entries_mut_for_test()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MirInterfaceDeclaration {
    pub id: InterfaceId,
    pub module: ModuleId,
    pub name: String,
    pub requirements: Vec<MirInterfaceRequirement>,
    pub span: Span,
}

impl MirInterfaceDeclaration {
    pub fn requirement(&self, id: InterfaceRequirementId) -> Option<&MirInterfaceRequirement> {
        (id.interface() == self.id)
            .then(|| self.requirements.get(id.index()))
            .flatten()
            .filter(|requirement| requirement.id == id)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MirInterfaceRequirement {
    pub id: InterfaceRequirementId,
    pub name: String,
    pub receiver_access: MirReceiverAccess,
    pub parameters: Vec<MirParameter>,
    pub return_type: MirType,
    pub span: Span,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MirRequirementImplementation {
    pub requirement: InterfaceRequirementId,
    pub method: MethodId,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MirInterfaceConformance {
    pub interface: InterfaceId,
    /// One implementation per requirement in declaration order.
    pub implementations: Vec<MirRequirementImplementation>,
}
