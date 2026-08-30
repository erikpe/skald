//! Executable callable definitions and their storage metadata.

use std::collections::BTreeMap;

use crate::{
    id_table::SparseFunctionTable,
    identity::{BindingId, CallableId, ClassId, FunctionId},
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

    /// Consumes definitions for an atomic executable-MIR rewrite while
    /// preserving every vacant function-ID slot.
    pub(crate) fn into_rewrite_slots(self) -> Vec<Option<MirFunctionDefinition>> {
        self.entries.into_slots()
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

    /// Consumes definitions in stable callable-identity order for an atomic
    /// executable-MIR rewrite.
    pub(crate) fn into_rewrite_entries(self) -> Vec<MirMemberDefinition> {
        self.entries.into_values().collect()
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
    pub class_owner: ClassId,
    pub return_storage: Option<StorageId>,
    pub receiver: Option<StorageId>,
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
    StaticInitializer(&'mir super::PreliminaryMirStaticInitializer),
}

impl<'mir> MirDefinitionRef<'mir> {
    pub const fn callable(self) -> CallableId {
        match self {
            Self::Function(definition) => definition.callable(),
            Self::Member(definition) => definition.callable,
            Self::StaticInitializer(definition) => definition.callable(),
        }
    }

    pub const fn receiver(self) -> Option<StorageId> {
        match self {
            Self::Function(_) => None,
            Self::Member(definition) => definition.receiver,
            Self::StaticInitializer(_) => None,
        }
    }

    pub const fn class_owner(self) -> Option<ClassId> {
        match self {
            Self::Function(_) => None,
            Self::Member(definition) => Some(definition.class_owner),
            Self::StaticInitializer(definition) => Some(definition.id.class()),
        }
    }

    pub const fn return_storage(self) -> Option<StorageId> {
        match self {
            Self::Function(definition) => definition.return_storage,
            Self::Member(definition) => definition.return_storage,
            Self::StaticInitializer(_) => None,
        }
    }

    pub fn parameters(self) -> &'mir [StorageId] {
        match self {
            Self::Function(definition) => &definition.parameters,
            Self::Member(definition) => &definition.parameters,
            Self::StaticInitializer(_) => &[],
        }
    }

    pub fn storage_entries(self) -> &'mir [MirStorage] {
        match self {
            Self::Function(definition) => &definition.storage,
            Self::Member(definition) => &definition.storage,
            Self::StaticInitializer(definition) => &definition.storage,
        }
    }

    pub fn values(self) -> &'mir [MirValue] {
        match self {
            Self::Function(definition) => &definition.values,
            Self::Member(definition) => &definition.values,
            Self::StaticInitializer(definition) => &definition.values,
        }
    }

    pub const fn body(self) -> &'mir MirBody {
        match self {
            Self::Function(definition) => &definition.body,
            Self::Member(definition) => &definition.body,
            Self::StaticInitializer(definition) => &definition.body,
        }
    }

    pub fn path_conditions(self) -> &'mir [super::path_condition::MirPathCondition] {
        &self.body().path_conditions
    }

    pub fn path_condition(
        self,
        id: super::ids::PathConditionId,
    ) -> Option<&'mir super::path_condition::MirPathCondition> {
        (id.callable() == self.callable())
            .then(|| self.path_conditions().get(id.index()))
            .flatten()
            .filter(|condition| condition.id == id)
    }

    pub fn logical_expressions(self) -> &'mir [super::logical::MirLogicalExpression] {
        &self.body().logical_expressions
    }

    pub const fn span(self) -> Span {
        match self {
            Self::Function(definition) => definition.span,
            Self::Member(definition) => definition.span,
            Self::StaticInitializer(definition) => definition.span,
        }
    }

    pub fn storage(self, id: StorageId) -> Option<&'mir MirStorage> {
        match self {
            Self::Function(definition) => definition.storage(id),
            Self::Member(definition) => definition.storage(id),
            Self::StaticInitializer(definition) => definition.storage(id),
        }
    }

    pub fn value(self, id: ValueId) -> Option<&'mir MirValue> {
        match self {
            Self::Function(definition) => definition.value(id),
            Self::Member(definition) => definition.value(id),
            Self::StaticInitializer(definition) => definition.value(id),
        }
    }

    pub fn block(self, id: BlockId) -> Option<&'mir MirBasicBlock> {
        match self {
            Self::Function(definition) => definition.block(id),
            Self::Member(definition) => definition.block(id),
            Self::StaticInitializer(definition) => definition.block(id),
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

impl<'mir> From<&'mir super::PreliminaryMirStaticInitializer> for MirDefinitionRef<'mir> {
    fn from(definition: &'mir super::PreliminaryMirStaticInitializer) -> Self {
        Self::StaticInitializer(definition)
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
    /// Repeatable lexical storage. Source locals carry a binding identity;
    /// compiler-owned loop state and result slots deliberately do not.
    Local,
    /// Caller-owned storage transferred to one callee value parameter.
    Argument,
    Temporary,
    /// Compiler-owned strong owner keeping a shared-backed call view alive.
    SharedAnchor,
    /// Compiler-owned scalar home used to preserve block-local MIR values
    /// across checked-cast control-flow edges.
    ScalarSpill,
    /// Caller-owned scalar storage initialized once for one produced
    /// read-only primitive alias argument.
    PrimitiveAlias,
    /// Compiler-owned canonical boolean selecting conditional MIR state.
    PathCondition,
    /// Compiler-owned scalar destination populated only by a successful
    /// checked primitive-optional unwrap edge.
    OptionalUnwrap,
    /// Compiler-owned unpublished storage for one heap allocation under
    /// construction. This is not a strong owner and cannot be source-named.
    SharedAllocation,
    /// Target-independent unpublished array storage under construction.
    ArrayBacking,
    /// A completed compiler-owned descriptor consumed exactly once.
    ArrayProduced,
    /// A completed copied-slice descriptor consumed or cleaned exactly once.
    ArraySlice,
    /// A checked normalized element or slice position (`u64`).
    ArrayPosition,
    /// A hidden dependency retaining an array backing or shared owner.
    ArrayAnchor(super::array::MirArrayAnchorKind),
    /// A compiler-owned address captured for one call-scoped array or array
    /// element alias argument.
    ArrayAlias(MirAliasAccess),
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
