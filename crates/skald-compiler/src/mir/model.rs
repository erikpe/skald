//! Data model for target-independent MIR.

use std::{collections::BTreeMap, fmt};

use crate::{
    function_table::{DenseFunctionTable, SparseFunctionTable},
    identity::{BindingId, CallableId, ClassId, FieldId, FunctionId, InitializerId, MethodId},
    source::Span,
};

macro_rules! owned_id {
    ($name:ident, $prefix:literal) => {
        #[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
        pub struct $name {
            callable: CallableId,
            index: usize,
        }

        impl $name {
            pub const fn callable(self) -> CallableId {
                self.callable
            }

            pub const fn index(self) -> usize {
                self.index
            }

            pub(crate) fn new(callable: impl Into<CallableId>, index: usize) -> Self {
                Self {
                    callable: callable.into(),
                    index,
                }
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(formatter, "{}:{}{}", self.callable(), $prefix, self.index())
            }
        }
    };
}

owned_id!(StorageId, "s");
owned_id!(ValueId, "v");
owned_id!(BlockId, "b");

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum MirType {
    I64,
    U64,
    U8,
    F64,
    Bool,
    Class(ClassId),
    Unit,
}

impl MirType {
    pub const fn is_scalar_value(self) -> bool {
        !matches!(self, Self::Class(_) | Self::Unit)
    }
}

impl fmt::Display for MirType {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::I64 => formatter.write_str("i64"),
            Self::U64 => formatter.write_str("u64"),
            Self::U8 => formatter.write_str("u8"),
            Self::F64 => formatter.write_str("f64"),
            Self::Bool => formatter.write_str("bool"),
            Self::Class(class) => write!(formatter, "class {class}"),
            Self::Unit => formatter.write_str("unit"),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MirProgram {
    pub classes: MirClassDeclarationTable,
    pub declarations: MirFunctionDeclarationTable,
    pub definitions: MirFunctionDefinitionTable,
    pub member_definitions: MirMemberDefinitionTable,
    pub entry_function: FunctionId,
    pub span: Span,
}

impl MirProgram {
    pub fn class(&self, id: ClassId) -> Option<&MirClassDeclaration> {
        self.classes.get(id)
    }

    pub fn field(&self, id: FieldId) -> Option<&MirFieldDeclaration> {
        self.class(id.class())?.field(id)
    }

    pub fn initializer(&self, id: InitializerId) -> Option<&MirInitializerDeclaration> {
        self.class(id.class())?.initializer(id)
    }

    pub fn method(&self, id: MethodId) -> Option<&MirMethodDeclaration> {
        self.class(id.class())?.method(id)
    }

    pub fn member_definition(&self, callable: CallableId) -> Option<&MirMemberDefinition> {
        self.member_definitions.get(callable)
    }

    pub fn executable_definitions(&self) -> impl Iterator<Item = MirDefinitionRef<'_>> {
        self.definitions
            .iter()
            .map(MirDefinitionRef::Function)
            .chain(self.member_definitions.iter().map(MirDefinitionRef::Member))
    }

    pub fn callable_signature(&self, callable: CallableId) -> Option<MirCallableSignature<'_>> {
        match callable {
            CallableId::Function(function) => {
                self.declarations
                    .get(function)
                    .map(|declaration| MirCallableSignature {
                        parameters: &declaration.parameters,
                        return_type: declaration.return_type,
                    })
            }
            CallableId::Initializer(initializer) => {
                self.initializer(initializer)
                    .map(|declaration| MirCallableSignature {
                        parameters: &declaration.parameters,
                        return_type: MirType::Unit,
                    })
            }
            CallableId::Destructor(_) => None,
            CallableId::Method(method) => {
                self.method(method).map(|declaration| MirCallableSignature {
                    parameters: &declaration.parameters,
                    return_type: declaration.return_type,
                })
            }
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct MirCallableSignature<'mir> {
    pub parameters: &'mir [MirParameter],
    pub return_type: MirType,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MirParameter {
    pub mode: MirParameterMode,
    pub ty: MirType,
}

impl MirParameter {
    pub const fn value(ty: MirType) -> Self {
        Self {
            mode: MirParameterMode::Value,
            ty,
        }
    }

    pub const fn read_only_alias(ty: MirType) -> Self {
        Self {
            mode: MirParameterMode::ReadOnlyAlias,
            ty,
        }
    }

    pub const fn mutable_alias(ty: MirType) -> Self {
        Self {
            mode: MirParameterMode::MutableAlias,
            ty,
        }
    }

    pub fn values(types: impl IntoIterator<Item = MirType>) -> Vec<Self> {
        types.into_iter().map(Self::value).collect()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MirParameterMode {
    Value,
    ReadOnlyAlias,
    MutableAlias,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct MirClassDeclarationTable {
    entries: Vec<MirClassDeclaration>,
}

impl MirClassDeclarationTable {
    pub(crate) fn new(entries: Vec<MirClassDeclaration>) -> Self {
        assert!(
            entries
                .iter()
                .enumerate()
                .all(|(index, class)| class.id.index() == index),
            "class declarations must be ordered by dense class ID"
        );
        Self { entries }
    }

    pub fn get(&self, id: ClassId) -> Option<&MirClassDeclaration> {
        self.entries
            .get(id.index())
            .filter(|declaration| declaration.id == id)
    }

    pub fn iter(&self) -> impl ExactSizeIterator<Item = &MirClassDeclaration> {
        self.entries.iter()
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    #[cfg(test)]
    pub(crate) fn entries_mut_for_test(&mut self) -> &mut [MirClassDeclaration] {
        &mut self.entries
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MirClassDeclaration {
    pub id: ClassId,
    pub name: String,
    pub fields: Vec<MirFieldDeclaration>,
    pub initializers: Vec<MirInitializerDeclaration>,
    pub methods: Vec<MirMethodDeclaration>,
    pub span: Span,
}

impl MirClassDeclaration {
    pub fn field(&self, id: FieldId) -> Option<&MirFieldDeclaration> {
        (id.class() == self.id)
            .then(|| self.fields.get(id.index()))
            .flatten()
            .filter(|field| field.id == id)
    }

    pub fn initializer(&self, id: InitializerId) -> Option<&MirInitializerDeclaration> {
        (id.class() == self.id)
            .then(|| self.initializers.get(id.index()))
            .flatten()
            .filter(|initializer| initializer.id == id)
    }

    pub fn method(&self, id: MethodId) -> Option<&MirMethodDeclaration> {
        (id.class() == self.id)
            .then(|| self.methods.get(id.index()))
            .flatten()
            .filter(|method| method.id == id)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MirFieldDeclaration {
    pub id: FieldId,
    pub name: String,
    pub ty: MirType,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MirInitializerDeclaration {
    pub id: InitializerId,
    pub parameters: Vec<MirParameter>,
    pub span: Span,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MirReceiverAccess {
    ReadOnly,
    Mutable,
}

impl fmt::Display for MirReceiverAccess {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ReadOnly => formatter.write_str("readonly"),
            Self::Mutable => formatter.write_str("mutable"),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MirMethodDeclaration {
    pub id: MethodId,
    pub name: String,
    pub receiver_access: MirReceiverAccess,
    pub parameters: Vec<MirParameter>,
    pub return_type: MirType,
    pub span: Span,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct MirFunctionDeclarationTable {
    entries: DenseFunctionTable<MirFunctionDeclaration>,
}

impl MirFunctionDeclarationTable {
    pub(crate) fn new(entries: Vec<MirFunctionDeclaration>) -> Self {
        Self {
            entries: DenseFunctionTable::new(entries, |declaration| declaration.id),
        }
    }

    pub fn get(&self, id: FunctionId) -> Option<&MirFunctionDeclaration> {
        self.entries.get(id, |declaration| declaration.id)
    }

    pub fn iter(&self) -> impl ExactSizeIterator<Item = &MirFunctionDeclaration> {
        self.entries.iter()
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    #[cfg(test)]
    pub(crate) fn entries_mut_for_test(&mut self) -> &mut [MirFunctionDeclaration] {
        self.entries.entries_mut_for_test()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MirFunctionDeclaration {
    pub id: FunctionId,
    pub name: String,
    pub parameters: Vec<MirParameter>,
    pub return_type: MirType,
    pub linkage: MirFunctionLinkage,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MirFunctionLinkage {
    Internal,
    External { symbol: String },
}

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
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MirMemberDefinition {
    pub callable: CallableId,
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
    Receiver,
    Parameter,
    AliasParameter(MirAliasAccess),
    Local,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MirAliasAccess {
    ReadOnly,
    Mutable,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MirStorage {
    pub id: StorageId,
    pub source: BindingId,
    pub name: String,
    pub kind: MirStorageKind,
    pub ty: MirType,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MirValue {
    pub id: ValueId,
    pub ty: MirType,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct MirPlace {
    pub base: MirPlaceBase,
    pub projections: Vec<MirPlaceProjection>,
}

impl MirPlace {
    pub fn base(base: StorageId) -> Self {
        Self {
            base: MirPlaceBase::Storage(base),
            projections: Vec::new(),
        }
    }

    pub fn alias_parameter(base: StorageId) -> Self {
        Self {
            base: MirPlaceBase::AliasParameter(base),
            projections: Vec::new(),
        }
    }

    pub fn project_field(mut self, field: FieldId) -> Self {
        self.projections.push(MirPlaceProjection::Field(field));
        self
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum MirPlaceBase {
    Storage(StorageId),
    AliasParameter(StorageId),
}

impl MirPlaceBase {
    pub const fn storage(self) -> StorageId {
        match self {
            Self::Storage(storage) | Self::AliasParameter(storage) => storage,
        }
    }
}

impl From<StorageId> for MirPlace {
    fn from(storage: StorageId) -> Self {
        Self::base(storage)
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum MirPlaceProjection {
    Field(FieldId),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MirBody {
    pub entry: BlockId,
    pub blocks: Vec<MirBasicBlock>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MirBasicBlock {
    pub id: BlockId,
    pub instructions: Vec<MirInstruction>,
    /// `None` is representable while constructing MIR so the verifier can
    /// diagnose unfinished blocks. Successful lowering always sets it.
    pub terminator: Option<MirTerminator>,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MirInstruction {
    Assign(MirAssignment),
    Call(MirCall),
    Initialize(MirInitialize),
    Store(MirStore),
}

impl MirInstruction {
    pub const fn span(&self) -> Span {
        match self {
            Self::Assign(instruction) => instruction.span,
            Self::Call(instruction) => instruction.span,
            Self::Initialize(instruction) => instruction.span,
            Self::Store(instruction) => instruction.span,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MirAssignment {
    pub result: ValueId,
    pub rvalue: MirRvalue,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MirStore {
    pub destination: MirPlace,
    pub value: ValueId,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MirInitialize {
    pub destination: MirPlace,
    pub target: InitializerId,
    pub arguments: Vec<MirArgument>,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MirCall {
    pub target: MirCallTarget,
    pub receiver: Option<MirPlace>,
    pub arguments: Vec<MirArgument>,
    pub result: Option<ValueId>,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MirArgument {
    Value(ValueId),
    Place(MirPlace),
}

impl From<ValueId> for MirArgument {
    fn from(value: ValueId) -> Self {
        Self::Value(value)
    }
}

impl From<MirPlace> for MirArgument {
    fn from(place: MirPlace) -> Self {
        Self::Place(place)
    }
}

impl MirArgument {
    pub fn values(values: impl IntoIterator<Item = ValueId>) -> Vec<Self> {
        values.into_iter().map(Self::Value).collect()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MirCallTarget {
    Direct(FunctionId),
    Method(MethodId),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MirRvalue {
    pub kind: MirRvalueKind,
    pub ty: MirType,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MirRvalueKind {
    ConstantI64(i64),
    ConstantU64(u64),
    ConstantU8(u8),
    /// IEEE-754 binary64 payload, stored as raw bits for deterministic IR.
    ConstantF64Bits(u64),
    ConstantBool(bool),
    Load(MirPlace),
    Unary {
        operation: MirUnaryOperation,
        operand: ValueId,
    },
    Binary {
        operation: MirBinaryOperation,
        left: ValueId,
        right: ValueId,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MirUnaryOperation {
    NegateI64,
    NegateF64,
}

impl MirUnaryOperation {
    pub const fn operand_type(self) -> MirType {
        match self {
            Self::NegateI64 => MirType::I64,
            Self::NegateF64 => MirType::F64,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MirBinaryOperation {
    AddI64,
    SubtractI64,
    MultiplyI64,
    AddU64,
    SubtractU64,
    MultiplyU64,
    AddU8,
    SubtractU8,
    MultiplyU8,
    AddF64,
    SubtractF64,
    MultiplyF64,
}

impl MirBinaryOperation {
    pub const fn operand_type(self) -> MirType {
        match self {
            Self::AddI64 | Self::SubtractI64 | Self::MultiplyI64 => MirType::I64,
            Self::AddU64 | Self::SubtractU64 | Self::MultiplyU64 => MirType::U64,
            Self::AddU8 | Self::SubtractU8 | Self::MultiplyU8 => MirType::U8,
            Self::AddF64 | Self::SubtractF64 | Self::MultiplyF64 => MirType::F64,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MirTerminator {
    Return {
        value: Option<ValueId>,
        span: Span,
    },
    Goto {
        target: BlockId,
        span: Span,
    },
    Branch {
        condition: ValueId,
        true_target: BlockId,
        false_target: BlockId,
        span: Span,
    },
}

impl MirTerminator {
    pub const fn span(&self) -> Span {
        match self {
            Self::Return { span, .. } | Self::Goto { span, .. } | Self::Branch { span, .. } => {
                *span
            }
        }
    }

    /// Returns outgoing control-flow targets in semantic order. For a branch,
    /// the true edge always precedes the false edge.
    pub fn successors(&self) -> impl Iterator<Item = BlockId> {
        let targets = match self {
            Self::Return { .. } => [None, None],
            Self::Goto { target, .. } => [Some(*target), None],
            Self::Branch {
                true_target,
                false_target,
                ..
            } => [Some(*true_target), Some(*false_target)],
        };
        targets.into_iter().flatten()
    }
}
