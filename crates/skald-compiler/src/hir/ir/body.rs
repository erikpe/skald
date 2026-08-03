//! Typed callable definitions, statements, and control-flow summaries.

use crate::{
    id_table::{DenseIdTable, SparseFunctionTable},
    identity::{BindingId, CallableId, ClassId, FunctionId, LocalId, LoopId},
    source::Span,
};

use super::{
    declarations::HirLocal,
    expression::HirExpression,
    object::{
        HirBaseInitialization, HirCopyAssignment, HirCopyConstruction, HirFieldAssignment,
        HirFieldConstruction, HirFieldCopyAssignment, HirFieldCopyConstruction,
        HirObjectInitialization, HirObjectReturn,
    },
    shared::{HirSharedAssignment, HirSharedFieldWrite, HirSharedTransfer},
    HirArrayInitialize, HirClassOptionalAssignment, HirClassOptionalInitialize, HirControlEffects,
    HirOptionalAssignment, HirOptionalSharedAssignment, HirOptionalSharedInitialize,
    HirOptionalSource,
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

    #[cfg(test)]
    pub(crate) fn entries_mut_for_test(&mut self) -> &mut [HirClassDefinition] {
        self.entries.entries_mut_for_test()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirClassDefinition {
    pub class: ClassId,
    pub initializers: Vec<HirMemberDefinition>,
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
            CallableId::Initializer(id) if id.class() == self.class => self
                .initializers
                .get(id.index())
                .filter(|definition| definition.callable == callable),
            CallableId::CopyConstructor(id) if id.class() == self.class => self
                .copy_constructor
                .as_ref()
                .filter(|definition| definition.callable == callable),
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
            | CallableId::CopyConstructor(_)
            | CallableId::CopyAssignment(_)
            | CallableId::Destructor(_)
            | CallableId::Method(_) => None,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirMemberDefinition {
    pub callable: CallableId,
    pub class_owner: ClassId,
    pub receiver_class: Option<ClassId>,
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

    #[cfg(test)]
    pub(crate) fn get_mut_for_test(
        &mut self,
        function: FunctionId,
    ) -> Option<&mut HirFunctionDefinition> {
        self.entries.get_mut_for_test(function)
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

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirBlock {
    pub statements: Vec<HirStatement>,
    pub effects: HirControlEffects,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HirStatement {
    BaseInitialization(HirBaseInitialization),
    Local(HirLocalDecl),
    Return(HirReturn),
    Break(HirBreak),
    Continue(HirContinue),
    Panic(HirPanic),
    Call(HirCallStatement),
    Conditional(HirConditional),
    While(HirWhile),
    Block(HirBlock),
    PrimitiveAssignment(HirPrimitiveAssignment),
    FieldAssignment(HirFieldAssignment),
    FieldConstruction(HirFieldConstruction),
    FieldCopyConstruction(HirFieldCopyConstruction),
    FieldCopyAssignment(HirFieldCopyAssignment),
    CopyAssignment(HirCopyAssignment),
    SharedFieldWrite(HirSharedFieldWrite),
    SharedAssignment(HirSharedAssignment),
    OptionalAssignment(HirOptionalAssignment),
    ClassOptionalAssignment(HirClassOptionalAssignment),
    OptionalSharedAssignment(HirOptionalSharedAssignment),
    ArrayFieldInitialize(super::HirArrayFieldInitialize),
    ArrayAssignment(super::HirArrayAssignment),
    ArrayElementAssignment(Box<super::HirArrayElementAssignment>),
    ArraySliceAssignment(super::HirArraySliceAssignment),
}

impl HirStatement {
    pub const fn span(&self) -> Span {
        match self {
            Self::BaseInitialization(statement) => statement.span,
            Self::Local(statement) => statement.span,
            Self::Return(statement) => statement.span,
            Self::Break(statement) => statement.span,
            Self::Continue(statement) => statement.span,
            Self::Panic(statement) => statement.span,
            Self::Call(statement) => statement.span,
            Self::Conditional(statement) => statement.span,
            Self::While(statement) => statement.span,
            Self::Block(block) => block.span,
            Self::PrimitiveAssignment(statement) => statement.span,
            Self::FieldAssignment(statement) => statement.span,
            Self::FieldConstruction(statement) => statement.span,
            Self::FieldCopyConstruction(statement) => statement.span,
            Self::FieldCopyAssignment(statement) => statement.span,
            Self::CopyAssignment(statement) => statement.span,
            Self::SharedFieldWrite(statement) => statement.span,
            Self::SharedAssignment(statement) => statement.span,
            Self::OptionalAssignment(statement) => statement.span,
            Self::ClassOptionalAssignment(statement) => statement.span,
            Self::OptionalSharedAssignment(statement) => statement.span,
            Self::ArrayFieldInitialize(statement) => statement.span,
            Self::ArrayAssignment(statement) => statement.span,
            Self::ArrayElementAssignment(statement) => statement.span,
            Self::ArraySliceAssignment(statement) => statement.span,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirPrimitiveAssignment {
    pub destination: HirPrimitivePlace,
    pub source: HirExpression,
    pub span: Span,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HirPrimitivePlace {
    pub storage: HirPrimitiveStorage,
    pub span: Span,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HirPrimitiveStorage {
    Binding(BindingId),
    Static(super::HirStaticPlace),
}

impl HirPrimitivePlace {
    pub const fn span(self) -> Span {
        self.span
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
    Shared(HirSharedTransfer),
    Optional(HirOptionalSource),
    ClassOptional(HirClassOptionalInitialize),
    OptionalShared(HirOptionalSharedInitialize),
    Array(HirArrayInitialize),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirReturn {
    pub value: Option<HirReturnValue>,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirBreak {
    pub target: LoopId,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirContinue {
    pub target: LoopId,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HirReturnValue {
    Scalar(HirExpression),
    Optional(HirOptionalSource),
    ClassOptional(HirClassOptionalInitialize),
    Object(HirObjectReturn),
    Shared(HirSharedTransfer),
    OptionalShared(HirOptionalSharedInitialize),
    Array(HirArrayInitialize),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirCallStatement {
    pub call: HirExpression,
    pub span: Span,
}

/// A checked call of the canonical `std::error::panic` intrinsic.
///
/// Keeping this separate from ordinary calls makes its abrupt control flow
/// explicit and prevents later phases from treating the intrinsic declaration
/// as an executable Skald function.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirPanic {
    pub message: super::HirCopyArgument,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirConditional {
    pub arms: Vec<HirConditionalArm>,
    pub else_block: Option<HirBlock>,
    pub effects: HirControlEffects,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirWhile {
    pub loop_id: LoopId,
    pub condition: HirExpression,
    pub body: HirBlock,
    pub effects: HirControlEffects,
    pub span: Span,
}

impl HirWhile {
    pub fn new(loop_id: LoopId, condition: HirExpression, body: HirBlock, span: Span) -> Self {
        assert_eq!(
            condition.ty,
            super::Type::Bool,
            "typed while conditions must have exact bool type"
        );
        let effects = body.effects.clone().through_loop(loop_id);
        Self {
            loop_id,
            condition,
            body,
            effects,
            span,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirConditionalArm {
    pub condition: HirExpression,
    pub body: HirBlock,
    pub span: Span,
}
