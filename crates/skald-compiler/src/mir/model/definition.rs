//! Executable callable definitions and their storage metadata.

use std::collections::BTreeMap;

use crate::{
    id_table::SparseFunctionTable,
    identity::{BindingId, CallableId, FunctionId},
    source::Span,
};

use super::{
    control_flow::{MirBasicBlock, MirBody},
    ids::{BlockId, StorageId, ValueId},
    value::{MirType, MirValue},
};

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct MirFunctionDefinitionTable {
    entries: SparseFunctionTable<MirFunctionDefinition>,
}

impl MirFunctionDefinitionTable {
    pub(crate) fn new(entries: Vec<Option<MirFunctionDefinition>>) -> Self {
        Self {
            entries: SparseFunctionTable::new(entries, |definition| definition.function),
        }
    }

    pub fn get(&self, function: FunctionId) -> Option<&MirFunctionDefinition> {
        self.entries.get(function)
    }

    pub fn iter(&self) -> impl Iterator<Item = &MirFunctionDefinition> {
        self.entries.iter()
    }

    pub const fn len(&self) -> usize {
        self.entries.len()
    }

    pub const fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub(crate) fn indexed_slots(
        &self,
    ) -> impl ExactSizeIterator<Item = (usize, Option<&MirFunctionDefinition>)> {
        self.entries.indexed_slots()
    }

    #[cfg(test)]
    pub(crate) fn get_mut_for_test(
        &mut self,
        function: FunctionId,
    ) -> Option<&mut MirFunctionDefinition> {
        self.entries.get_mut_for_test(function)
    }

    #[cfg(test)]
    pub(crate) fn remove_for_test(&mut self, function: FunctionId) {
        self.entries.remove_for_test(function);
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MirFunctionDefinition {
    pub function: FunctionId,
    pub return_storage: Option<StorageId>,
    pub parameters: Vec<StorageId>,
    pub storage: Vec<MirStorage>,
    pub values: Vec<MirValue>,
    pub body: MirBody,
    pub span: Span,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct MirMemberDefinitionTable {
    entries: BTreeMap<CallableId, MirMemberDefinition>,
}

impl MirMemberDefinitionTable {
    pub(crate) fn new(entries: Vec<MirMemberDefinition>) -> Self {
        let mut table = BTreeMap::new();
        for definition in entries {
            assert!(
                !matches!(definition.callable, CallableId::Function(_)),
                "member definitions cannot use function identities"
            );
            let callable = definition.callable;
            assert!(
                table.insert(callable, definition).is_none(),
                "duplicate member definition {callable}"
            );
        }
        Self { entries: table }
    }

    pub fn get(&self, callable: CallableId) -> Option<&MirMemberDefinition> {
        self.entries.get(&callable)
    }

    pub fn iter(&self) -> impl ExactSizeIterator<Item = &MirMemberDefinition> {
        self.entries.values()
    }

    pub(crate) fn indexed_entries(
        &self,
    ) -> impl ExactSizeIterator<Item = (CallableId, &MirMemberDefinition)> {
        self.entries
            .iter()
            .map(|(callable, definition)| (*callable, definition))
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    #[cfg(test)]
    pub(crate) fn get_mut_for_test(
        &mut self,
        callable: CallableId,
    ) -> Option<&mut MirMemberDefinition> {
        self.entries.get_mut(&callable)
    }

    #[cfg(test)]
    pub(crate) fn remove_for_test(&mut self, callable: CallableId) {
        self.entries.remove(&callable);
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MirMemberDefinition {
    pub callable: CallableId,
    pub return_storage: Option<StorageId>,
    pub receiver: StorageId,
    pub parameters: Vec<StorageId>,
    pub storage: Vec<MirStorage>,
    pub values: Vec<MirValue>,
    pub body: MirBody,
    pub span: Span,
}

impl MirMemberDefinition {
    pub fn storage(&self, id: StorageId) -> Option<&MirStorage> {
        (id.callable() == self.callable)
            .then(|| self.storage.get(id.index()))
            .flatten()
            .filter(|storage| storage.id == id)
    }

    pub fn value(&self, id: ValueId) -> Option<&MirValue> {
        (id.callable() == self.callable)
            .then(|| self.values.get(id.index()))
            .flatten()
            .filter(|value| value.id == id)
    }

    pub fn block(&self, id: BlockId) -> Option<&MirBasicBlock> {
        (id.callable() == self.callable)
            .then(|| self.body.blocks.get(id.index()))
            .flatten()
            .filter(|block| block.id == id)
    }
}

#[derive(Clone, Copy, Debug)]
pub enum MirDefinitionRef<'mir> {
    Function(&'mir MirFunctionDefinition),
    Member(&'mir MirMemberDefinition),
}

impl<'mir> MirDefinitionRef<'mir> {
    pub const fn callable(self) -> CallableId {
        match self {
            Self::Function(definition) => definition.callable(),
            Self::Member(definition) => definition.callable,
        }
    }

    pub const fn receiver(self) -> Option<StorageId> {
        match self {
            Self::Function(_) => None,
            Self::Member(definition) => Some(definition.receiver),
        }
    }

    pub const fn return_storage(self) -> Option<StorageId> {
        match self {
            Self::Function(definition) => definition.return_storage,
            Self::Member(definition) => definition.return_storage,
        }
    }

    pub fn parameters(self) -> &'mir [StorageId] {
        match self {
            Self::Function(definition) => &definition.parameters,
            Self::Member(definition) => &definition.parameters,
        }
    }

    pub fn storage_entries(self) -> &'mir [MirStorage] {
        match self {
            Self::Function(definition) => &definition.storage,
            Self::Member(definition) => &definition.storage,
        }
    }

    pub fn values(self) -> &'mir [MirValue] {
        match self {
            Self::Function(definition) => &definition.values,
            Self::Member(definition) => &definition.values,
        }
    }

    pub const fn body(self) -> &'mir MirBody {
        match self {
            Self::Function(definition) => &definition.body,
            Self::Member(definition) => &definition.body,
        }
    }

    pub const fn span(self) -> Span {
        match self {
            Self::Function(definition) => definition.span,
            Self::Member(definition) => definition.span,
        }
    }

    pub fn storage(self, id: StorageId) -> Option<&'mir MirStorage> {
        match self {
            Self::Function(definition) => definition.storage(id),
            Self::Member(definition) => definition.storage(id),
        }
    }

    pub fn value(self, id: ValueId) -> Option<&'mir MirValue> {
        match self {
            Self::Function(definition) => definition.value(id),
            Self::Member(definition) => definition.value(id),
        }
    }

    pub fn block(self, id: BlockId) -> Option<&'mir MirBasicBlock> {
        match self {
            Self::Function(definition) => definition.block(id),
            Self::Member(definition) => definition.block(id),
        }
    }
}

impl<'mir> From<&'mir MirFunctionDefinition> for MirDefinitionRef<'mir> {
    fn from(definition: &'mir MirFunctionDefinition) -> Self {
        Self::Function(definition)
    }
}

impl<'mir> From<&'mir MirMemberDefinition> for MirDefinitionRef<'mir> {
    fn from(definition: &'mir MirMemberDefinition) -> Self {
        Self::Member(definition)
    }
}

impl MirFunctionDefinition {
    pub const fn callable(&self) -> CallableId {
        CallableId::Function(self.function)
    }

    pub fn storage(&self, id: StorageId) -> Option<&MirStorage> {
        (id.callable() == self.callable())
            .then(|| self.storage.get(id.index()))
            .flatten()
            .filter(|storage| storage.id == id)
    }

    pub fn value(&self, id: ValueId) -> Option<&MirValue> {
        (id.callable() == self.callable())
            .then(|| self.values.get(id.index()))
            .flatten()
            .filter(|value| value.id == id)
    }

    pub fn block(&self, id: BlockId) -> Option<&MirBasicBlock> {
        (id.callable() == self.callable())
            .then(|| self.body.blocks.get(id.index()))
            .flatten()
            .filter(|block| block.id == id)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MirStorageKind {
    /// Uninitialized caller-provided storage for an object result.
    Return,
    Receiver,
    Parameter,
    AliasParameter(MirAliasAccess),
    /// Full-expression indirect storage established by a checked object cast.
    CheckedView(MirAliasAccess),
    Local,
    /// Caller-owned storage transferred to one callee value parameter.
    Argument,
    Temporary,
    /// Compiler-owned scalar home used to preserve block-local MIR values
    /// across checked-cast control-flow edges.
    ScalarSpill,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MirAliasAccess {
    ReadOnly,
    Mutable,
}

impl std::fmt::Display for MirAliasAccess {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ReadOnly => formatter.write_str("readonly"),
            Self::Mutable => formatter.write_str("mutable"),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MirStorage {
    pub id: StorageId,
    /// Source binding for language-owned storage; compiler-owned argument and
    /// temporary storage deliberately have no source binding.
    pub source: Option<BindingId>,
    pub name: String,
    pub kind: MirStorageKind,
    pub ty: MirType,
    pub span: Span,
}
