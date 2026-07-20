//! Data model for target-independent MIR.

use std::fmt;

use crate::{
    identity::{BindingId, FunctionId},
    source::Span,
};

macro_rules! owned_id {
    ($name:ident, $prefix:literal) => {
        #[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
        pub struct $name {
            function: FunctionId,
            index: usize,
        }

        impl $name {
            pub const fn function(self) -> FunctionId {
                self.function
            }

            pub const fn index(self) -> usize {
                self.index
            }

            pub(crate) const fn new(function: FunctionId, index: usize) -> Self {
                Self { function, index }
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(formatter, "{}:{}{}", self.function(), $prefix, self.index())
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
    Unit,
}

impl MirType {
    pub const fn name(self) -> &'static str {
        match self {
            Self::I64 => "i64",
            Self::U64 => "u64",
            Self::U8 => "u8",
            Self::F64 => "f64",
            Self::Bool => "bool",
            Self::Unit => "unit",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MirProgram {
    pub declarations: MirFunctionDeclarationTable,
    pub definitions: MirFunctionDefinitionTable,
    pub entry_function: FunctionId,
    pub span: Span,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct MirFunctionDeclarationTable {
    entries: Vec<MirFunctionDeclaration>,
}

impl MirFunctionDeclarationTable {
    pub(crate) fn new(entries: Vec<MirFunctionDeclaration>) -> Self {
        debug_assert!(entries
            .iter()
            .enumerate()
            .all(|(index, declaration)| declaration.id.index() == index));
        Self { entries }
    }

    pub fn get(&self, id: FunctionId) -> Option<&MirFunctionDeclaration> {
        self.entries
            .get(id.index())
            .filter(|declaration| declaration.id == id)
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
        &mut self.entries
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MirFunctionDeclaration {
    pub id: FunctionId,
    pub name: String,
    pub parameter_types: Vec<MirType>,
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
    entries: Vec<Option<MirFunctionDefinition>>,
    definition_count: usize,
}

impl MirFunctionDefinitionTable {
    pub(crate) fn new(entries: Vec<Option<MirFunctionDefinition>>) -> Self {
        debug_assert!(entries.iter().enumerate().all(|(index, definition)| {
            definition
                .as_ref()
                .is_none_or(|definition| definition.function.index() == index)
        }));
        let definition_count = entries.iter().flatten().count();
        Self {
            entries,
            definition_count,
        }
    }

    pub fn get(&self, function: FunctionId) -> Option<&MirFunctionDefinition> {
        self.entries.get(function.index())?.as_ref()
    }

    pub fn iter(&self) -> impl Iterator<Item = &MirFunctionDefinition> {
        self.entries.iter().flatten()
    }

    pub const fn len(&self) -> usize {
        self.definition_count
    }

    pub const fn is_empty(&self) -> bool {
        self.definition_count == 0
    }

    pub(crate) fn slots(&self) -> &[Option<MirFunctionDefinition>] {
        &self.entries
    }

    #[cfg(test)]
    pub(crate) fn get_mut_for_test(
        &mut self,
        function: FunctionId,
    ) -> Option<&mut MirFunctionDefinition> {
        self.entries.get_mut(function.index())?.as_mut()
    }

    #[cfg(test)]
    pub(crate) fn remove_for_test(&mut self, function: FunctionId) {
        if self
            .entries
            .get_mut(function.index())
            .and_then(Option::take)
            .is_some()
        {
            self.definition_count -= 1;
        }
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

impl MirFunctionDefinition {
    pub fn storage(&self, id: StorageId) -> Option<&MirStorage> {
        (id.function() == self.function)
            .then(|| self.storage.get(id.index()))
            .flatten()
            .filter(|storage| storage.id == id)
    }

    pub fn value(&self, id: ValueId) -> Option<&MirValue> {
        (id.function() == self.function)
            .then(|| self.values.get(id.index()))
            .flatten()
            .filter(|value| value.id == id)
    }

    pub fn block(&self, id: BlockId) -> Option<&MirBasicBlock> {
        (id.function() == self.function)
            .then(|| self.body.blocks.get(id.index()))
            .flatten()
            .filter(|block| block.id == id)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MirStorageKind {
    Parameter,
    Local,
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
    Store(MirStore),
}

impl MirInstruction {
    pub const fn span(&self) -> Span {
        match self {
            Self::Assign(instruction) => instruction.span,
            Self::Call(instruction) => instruction.span,
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
    pub storage: StorageId,
    pub value: ValueId,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MirCall {
    pub target: MirCallTarget,
    pub arguments: Vec<ValueId>,
    pub result: Option<ValueId>,
    pub span: Span,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MirCallTarget {
    Direct(FunctionId),
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
    Load(StorageId),
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
