//! Resolved callable definitions, statement trees, and control flow.

use crate::{
    id_table::{DenseIdTable, SparseFunctionTable},
    identity::{BindingId, CallableId, ClassId, FieldId, FunctionId, LocalId, LoopId},
    source::Span,
};

use super::{
    declarations::ResolvedLocal, expression::ResolvedExpression, object_place::ResolvedObjectPlace,
};

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ResolvedClassDefinitionTable {
    entries: DenseIdTable<ClassId, ResolvedClassDefinition>,
}

impl ResolvedClassDefinitionTable {
    pub(crate) fn new(entries: Vec<ResolvedClassDefinition>) -> Self {
        Self {
            entries: DenseIdTable::new(entries, |class| class.class),
        }
    }

    pub fn get(&self, id: ClassId) -> Option<&ResolvedClassDefinition> {
        self.entries.get(id, |definition| definition.class)
    }

    pub fn iter(&self) -> impl ExactSizeIterator<Item = &ResolvedClassDefinition> {
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
pub struct ResolvedClassDefinition {
    pub class: ClassId,
    pub initializers: Vec<ResolvedMemberDefinition>,
    pub copy_constructor: Option<ResolvedMemberDefinition>,
    pub copy_assignment: Option<ResolvedMemberDefinition>,
    pub destructor: Option<ResolvedMemberDefinition>,
    pub methods: Vec<ResolvedMemberDefinition>,
    pub span: Span,
}

impl ResolvedClassDefinition {
    pub fn member(&self, callable: CallableId) -> Option<&ResolvedMemberDefinition> {
        match callable {
            CallableId::Function(_) => None,
            CallableId::Initializer(initializer) if initializer.class() == self.class => self
                .initializers
                .get(initializer.index())
                .filter(|definition| definition.callable == callable),
            CallableId::CopyConstructor(copy) if copy.class() == self.class => self
                .copy_constructor
                .as_ref()
                .filter(|definition| definition.callable == callable),
            CallableId::CopyAssignment(assignment) if assignment.class() == self.class => self
                .copy_assignment
                .as_ref()
                .filter(|definition| definition.callable == callable),
            CallableId::Destructor(destructor) if destructor.class() == self.class => self
                .destructor
                .as_ref()
                .filter(|definition| definition.callable == callable),
            CallableId::Method(method) if method.class() == self.class => self
                .methods
                .get(method.index())
                .filter(|definition| definition.callable == callable),
            CallableId::Initializer(_)
            | CallableId::CopyConstructor(_)
            | CallableId::CopyAssignment(_)
            | CallableId::Destructor(_)
            | CallableId::Method(_) => None,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedMemberDefinition {
    pub callable: CallableId,
    pub locals: Vec<ResolvedLocal>,
    pub body: ResolvedBlock,
    pub span: Span,
}

impl ResolvedMemberDefinition {
    pub fn local(&self, id: LocalId) -> Option<&ResolvedLocal> {
        if id.callable() != self.callable {
            return None;
        }
        self.locals.get(id.index()).filter(|local| local.id == id)
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ResolvedFunctionDefinitionTable {
    entries: SparseFunctionTable<ResolvedFunctionDefinition>,
}

impl ResolvedFunctionDefinitionTable {
    pub(crate) fn new(entries: Vec<Option<ResolvedFunctionDefinition>>) -> Self {
        Self {
            entries: SparseFunctionTable::new(entries, |definition| definition.function),
        }
    }

    pub fn get(&self, function: FunctionId) -> Option<&ResolvedFunctionDefinition> {
        self.entries.get(function)
    }

    pub fn iter(&self) -> impl Iterator<Item = &ResolvedFunctionDefinition> {
        self.entries.iter()
    }

    pub const fn len(&self) -> usize {
        self.entries.len()
    }

    pub const fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    #[cfg(test)]
    pub(crate) fn get_mut_for_test(
        &mut self,
        function: FunctionId,
    ) -> Option<&mut ResolvedFunctionDefinition> {
        self.entries.get_mut_for_test(function)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedFunctionDefinition {
    pub function: FunctionId,
    pub locals: Vec<ResolvedLocal>,
    pub body: ResolvedBlock,
    pub span: Span,
}

impl ResolvedFunctionDefinition {
    pub fn local(&self, id: LocalId) -> Option<&ResolvedLocal> {
        (id.callable() == self.function.into())
            .then(|| self.locals.get(id.index()))
            .flatten()
            .filter(|local| local.id == id)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedBlock {
    pub statements: Vec<ResolvedStatement>,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ResolvedStatement {
    BaseInitialization(ResolvedBaseInitialization),
    Local(ResolvedLocalDecl),
    Return(ResolvedReturn),
    Break(ResolvedBreak),
    Expression(ResolvedExpressionStatement),
    Conditional(ResolvedConditional),
    While(ResolvedWhile),
    Block(ResolvedBlock),
    PrimitiveBindingAssignment(ResolvedPrimitiveBindingAssignment),
    FieldAssignment(ResolvedFieldAssignment),
    ObjectAssignment(ResolvedObjectAssignment),
    SharedAssignment(ResolvedSharedAssignment),
    OptionalAssignment(ResolvedOptionalAssignment),
    ArrayAssignment(ResolvedArrayAssignment),
}

impl ResolvedStatement {
    pub const fn span(&self) -> Span {
        match self {
            Self::BaseInitialization(statement) => statement.span,
            Self::Local(statement) => statement.span,
            Self::Return(statement) => statement.span,
            Self::Break(statement) => statement.span,
            Self::Expression(statement) => statement.span,
            Self::Conditional(statement) => statement.span,
            Self::While(statement) => statement.span,
            Self::Block(block) => block.span,
            Self::PrimitiveBindingAssignment(statement) => statement.span,
            Self::FieldAssignment(statement) => statement.span,
            Self::ObjectAssignment(statement) => statement.span,
            Self::SharedAssignment(statement) => statement.span,
            Self::OptionalAssignment(statement) => statement.span,
            Self::ArrayAssignment(statement) => statement.span,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedPrimitiveBindingAssignment {
    pub destination: BindingId,
    pub equal_span: Span,
    pub source: ResolvedExpression,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedBreak {
    pub target: LoopId,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedWhile {
    pub loop_id: LoopId,
    pub condition: ResolvedExpression,
    pub body: ResolvedBlock,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedArrayAssignment {
    pub destination: ResolvedExpression,
    pub equal_span: Span,
    pub source: ResolvedExpression,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedOptionalAssignment {
    pub destination: BindingId,
    pub target: super::ResolvedTypeKind,
    pub equal_span: Span,
    pub source: ResolvedExpression,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedSharedAssignment {
    pub destination: BindingId,
    pub target: super::ResolvedSharedTarget,
    pub equal_span: Span,
    pub source: ResolvedExpression,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedBaseInitialization {
    pub base: ClassId,
    pub arguments: Vec<ResolvedExpression>,
    pub super_span: Span,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedObjectAssignment {
    pub destination: ResolvedObjectPlace,
    pub equal_span: Span,
    pub source: ResolvedExpression,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedFieldAssignment {
    pub receiver: super::ResolvedObjectReceiver,
    pub field: FieldId,
    pub member_span: Span,
    pub equal_span: Span,
    pub value: ResolvedExpression,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedLocalDecl {
    pub local: LocalId,
    pub initializer: ResolvedExpression,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedReturn {
    pub value: Option<ResolvedExpression>,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedExpressionStatement {
    pub expression: ResolvedExpression,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedConditional {
    pub arms: Vec<ResolvedConditionalArm>,
    pub else_block: Option<ResolvedBlock>,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedConditionalArm {
    pub condition: ResolvedExpression,
    pub body: ResolvedBlock,
    pub span: Span,
}
