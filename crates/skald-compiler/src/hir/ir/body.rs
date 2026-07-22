//! Typed callable definitions, statements, and control-flow summaries.

use crate::{
    id_table::{DenseIdTable, SparseFunctionTable},
    identity::{CallableId, ClassId, FunctionId, LocalId},
    source::Span,
};

use super::{
    declarations::HirLocal,
    expression::HirExpression,
    object::{
        HirCopyAssignment, HirCopyConstruction, HirFieldAssignment, HirFieldConstruction,
        HirFieldCopyAssignment, HirFieldCopyConstruction, HirObjectInitialization, HirObjectReturn,
    },
};

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct HirClassDefinitionTable {
    entries: DenseIdTable<ClassId, HirClassDefinition>,
}

impl HirClassDefinitionTable {
    pub(crate) fn new(entries: Vec<HirClassDefinition>) -> Self {
        Self {
            entries: DenseIdTable::new(entries, |class| class.class),
        }
    }

    pub fn get(&self, id: ClassId) -> Option<&HirClassDefinition> {
        self.entries.get(id, |definition| definition.class)
    }

    pub fn iter(&self) -> impl ExactSizeIterator<Item = &HirClassDefinition> {
        self.entries.iter()
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirClassDefinition {
    pub class: ClassId,
    pub initializer: HirMemberDefinition,
    pub copy_constructor: Option<HirMemberDefinition>,
    pub copy_assignment: Option<HirMemberDefinition>,
    pub destructor: Option<HirMemberDefinition>,
    pub methods: Vec<HirMemberDefinition>,
    pub span: Span,
}

impl HirClassDefinition {
    pub fn member(&self, callable: CallableId) -> Option<&HirMemberDefinition> {
        match callable {
            CallableId::Function(_) => None,
            CallableId::Initializer(id) if id.class() == self.class => (self.initializer.callable
                == callable)
                .then_some(&self.initializer)
                .or_else(|| {
                    self.copy_constructor
                        .as_ref()
                        .filter(|definition| definition.callable == callable)
                }),
            CallableId::CopyAssignment(id) if id.class() == self.class => self
                .copy_assignment
                .as_ref()
                .filter(|definition| definition.callable == callable),
            CallableId::Method(id) if id.class() == self.class => self
                .methods
                .get(id.index())
                .filter(|definition| definition.callable == callable),
            CallableId::Destructor(id) if id.class() == self.class => self
                .destructor
                .as_ref()
                .filter(|definition| definition.callable == callable),
            CallableId::Initializer(_)
            | CallableId::CopyAssignment(_)
            | CallableId::Destructor(_)
            | CallableId::Method(_) => None,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirMemberDefinition {
    pub callable: CallableId,
    pub locals: Vec<HirLocal>,
    pub body: HirBlock,
    pub span: Span,
}

impl HirMemberDefinition {
    pub fn local(&self, id: LocalId) -> Option<&HirLocal> {
        if id.callable() != self.callable {
            return None;
        }
        self.locals.get(id.index()).filter(|local| local.id == id)
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct HirFunctionDefinitionTable {
    entries: SparseFunctionTable<HirFunctionDefinition>,
}

impl HirFunctionDefinitionTable {
    pub(crate) fn new(entries: Vec<Option<HirFunctionDefinition>>) -> Self {
        Self {
            entries: SparseFunctionTable::new(entries, |definition| definition.function),
        }
    }

    pub fn get(&self, function: FunctionId) -> Option<&HirFunctionDefinition> {
        self.entries.get(function)
    }

    pub fn iter(&self) -> impl Iterator<Item = &HirFunctionDefinition> {
        self.entries.iter()
    }

    pub const fn len(&self) -> usize {
        self.entries.len()
    }

    pub const fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirFunctionDefinition {
    pub function: FunctionId,
    pub locals: Vec<HirLocal>,
    pub body: HirBlock,
    pub span: Span,
}

impl HirFunctionDefinition {
    pub fn local(&self, id: LocalId) -> Option<&HirLocal> {
        (id.callable() == self.function.into())
            .then(|| self.locals.get(id.index()))
            .flatten()
            .filter(|local| local.id == id)
    }
}

/// Whether execution can reach the end of a checked block or conditional.
///
/// `Terminates` currently means every path executes a `return`. The type
/// checker is the authority for this summary; later phases consume it rather
/// than reconstructing source-level control flow.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BlockFlow {
    FallsThrough,
    Terminates,
}

impl BlockFlow {
    pub(crate) const fn then(self, next: Self) -> Self {
        match self {
            Self::FallsThrough => next,
            Self::Terminates => Self::Terminates,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirBlock {
    pub statements: Vec<HirStatement>,
    pub flow: BlockFlow,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HirStatement {
    Local(HirLocalDecl),
    Return(HirReturn),
    Call(HirCallStatement),
    Conditional(HirConditional),
    Block(HirBlock),
    FieldAssignment(HirFieldAssignment),
    FieldConstruction(HirFieldConstruction),
    FieldCopyConstruction(HirFieldCopyConstruction),
    FieldCopyAssignment(HirFieldCopyAssignment),
    CopyAssignment(HirCopyAssignment),
}

impl HirStatement {
    pub const fn span(&self) -> Span {
        match self {
            Self::Local(statement) => statement.span,
            Self::Return(statement) => statement.span,
            Self::Call(statement) => statement.span,
            Self::Conditional(statement) => statement.span,
            Self::Block(block) => block.span,
            Self::FieldAssignment(statement) => statement.span,
            Self::FieldConstruction(statement) => statement.span,
            Self::FieldCopyConstruction(statement) => statement.span,
            Self::FieldCopyAssignment(statement) => statement.span,
            Self::CopyAssignment(statement) => statement.span,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirLocalDecl {
    pub local: LocalId,
    pub initializer: HirLocalInitializer,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HirLocalInitializer {
    Value(HirExpression),
    Object(HirObjectInitialization),
    Copy(HirCopyConstruction),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirReturn {
    pub value: Option<HirReturnValue>,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HirReturnValue {
    Scalar(HirExpression),
    Object(HirObjectReturn),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirCallStatement {
    pub call: HirExpression,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirConditional {
    pub arms: Vec<HirConditionalArm>,
    pub else_block: Option<HirBlock>,
    pub flow: BlockFlow,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirConditionalArm {
    pub condition: HirExpression,
    pub body: HirBlock,
    pub span: Span,
}
