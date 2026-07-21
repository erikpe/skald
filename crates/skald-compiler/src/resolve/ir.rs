//! Name-resolved, but not yet type-checked, program representation.

use crate::{
    function_table::{DenseFunctionTable, SparseFunctionTable},
    identity::{
        BindingId, CallableId, ClassId, CopyAssignmentId, DestructorId, FieldId, FunctionId,
        InitializerId, LocalId, MethodId, ParameterId,
    },
    literal::NumericLiteralKind,
    object_path::ObjectPath,
    source::Span,
};

pub type ResolvedObjectPlace = ObjectPath;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedProgram {
    pub declarations: ResolvedFunctionDeclarationTable,
    pub definitions: ResolvedFunctionDefinitionTable,
    pub classes: ResolvedClassDeclarationTable,
    pub class_definitions: ResolvedClassDefinitionTable,
    /// Function named `main`, selected during resolution. Type checking
    /// validates its signature and diagnoses its absence.
    pub entry_function: Option<FunctionId>,
    pub span: Span,
}

impl ResolvedProgram {
    pub fn class(&self, id: ClassId) -> Option<&ResolvedClassDeclaration> {
        self.classes.get(id)
    }

    pub fn field(&self, id: FieldId) -> Option<&ResolvedFieldDeclaration> {
        self.class(id.class())?.field(id)
    }

    pub fn initializer(&self, id: InitializerId) -> Option<&ResolvedInitializerDeclaration> {
        self.class(id.class())?.initializer(id)
    }

    pub fn destructor(&self, id: DestructorId) -> Option<&ResolvedDestructorDeclaration> {
        self.class(id.class())?.destructor(id)
    }

    pub fn copy_assignment(
        &self,
        id: CopyAssignmentId,
    ) -> Option<&ResolvedCopyAssignmentDeclaration> {
        self.class(id.class())?.copy_assignment_declaration(id)
    }

    pub fn method(&self, id: MethodId) -> Option<&ResolvedMethodDeclaration> {
        self.class(id.class())?.method(id)
    }

    pub fn member_definition(&self, callable: CallableId) -> Option<&ResolvedMemberDefinition> {
        let class = callable.class()?;
        self.class_definitions.get(class)?.member(callable)
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ResolvedClassDeclarationTable {
    entries: Vec<ResolvedClassDeclaration>,
}

impl ResolvedClassDeclarationTable {
    pub(crate) fn new(entries: Vec<ResolvedClassDeclaration>) -> Self {
        assert!(
            entries
                .iter()
                .enumerate()
                .all(|(index, class)| class.id.index() == index),
            "class declarations must be ordered by dense class ID"
        );
        Self { entries }
    }

    pub fn get(&self, id: ClassId) -> Option<&ResolvedClassDeclaration> {
        self.entries.get(id.index()).filter(|class| class.id == id)
    }

    pub fn iter(&self) -> impl ExactSizeIterator<Item = &ResolvedClassDeclaration> {
        self.entries.iter()
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    #[cfg(test)]
    pub(crate) fn entries_mut_for_test(&mut self) -> &mut [ResolvedClassDeclaration] {
        &mut self.entries
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedClassDeclaration {
    pub id: ClassId,
    pub name: String,
    pub name_span: Span,
    pub fields: Vec<ResolvedFieldDeclaration>,
    pub initializer: Option<ResolvedInitializerDeclaration>,
    pub copy_constructor_declaration: Option<ResolvedInitializerDeclaration>,
    pub copy_constructor: ResolvedCopyOperation<InitializerId>,
    pub copy_assignment_declaration: Option<ResolvedCopyAssignmentDeclaration>,
    pub copy_assignment: ResolvedCopyOperation<CopyAssignmentId>,
    pub destructor: Option<ResolvedDestructorDeclaration>,
    pub methods: Vec<ResolvedMethodDeclaration>,
    pub span: Span,
}

impl ResolvedClassDeclaration {
    pub fn field(&self, id: FieldId) -> Option<&ResolvedFieldDeclaration> {
        if id.class() != self.id {
            return None;
        }
        self.fields.get(id.index()).filter(|field| field.id == id)
    }

    pub fn initializer(&self, id: InitializerId) -> Option<&ResolvedInitializerDeclaration> {
        if id.class() != self.id {
            return None;
        }
        self.initializer
            .as_ref()
            .filter(|initializer| initializer.id == id)
            .or_else(|| {
                self.copy_constructor_declaration
                    .as_ref()
                    .filter(|initializer| initializer.id == id)
            })
    }

    pub fn copy_assignment_declaration(
        &self,
        id: CopyAssignmentId,
    ) -> Option<&ResolvedCopyAssignmentDeclaration> {
        if id.class() != self.id {
            return None;
        }
        self.copy_assignment_declaration
            .as_ref()
            .filter(|assignment| assignment.id == id)
    }

    pub fn destructor(&self, id: DestructorId) -> Option<&ResolvedDestructorDeclaration> {
        if id.class() != self.id {
            return None;
        }
        self.destructor
            .as_ref()
            .filter(|destructor| destructor.id == id)
    }

    pub fn method(&self, id: MethodId) -> Option<&ResolvedMethodDeclaration> {
        if id.class() != self.id {
            return None;
        }
        self.methods
            .get(id.index())
            .filter(|method| method.id == id)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedFieldDeclaration {
    pub id: FieldId,
    pub name: String,
    pub name_span: Span,
    pub type_syntax: ResolvedType,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedInitializerDeclaration {
    pub id: InitializerId,
    pub parameters: Vec<ResolvedParameter>,
    pub span: Span,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ResolvedCopyOperation<I> {
    User(I),
    Synthesized(ClassId),
    Unavailable,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedCopyAssignmentDeclaration {
    pub id: CopyAssignmentId,
    pub parameter: ResolvedParameter,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedDestructorDeclaration {
    pub id: DestructorId,
    pub span: Span,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ResolvedReceiverAccess {
    ReadOnly,
    Mutable,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedMethodDeclaration {
    pub id: MethodId,
    pub name: String,
    pub name_span: Span,
    pub receiver_access: ResolvedReceiverAccess,
    pub parameters: Vec<ResolvedParameter>,
    pub return_type: ResolvedType,
    pub span: Span,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ResolvedClassDefinitionTable {
    entries: Vec<ResolvedClassDefinition>,
}

impl ResolvedClassDefinitionTable {
    pub(crate) fn new(entries: Vec<ResolvedClassDefinition>) -> Self {
        assert!(
            entries
                .iter()
                .enumerate()
                .all(|(index, class)| class.class.index() == index),
            "class definitions must be ordered by dense class ID"
        );
        Self { entries }
    }

    pub fn get(&self, id: ClassId) -> Option<&ResolvedClassDefinition> {
        self.entries
            .get(id.index())
            .filter(|definition| definition.class == id)
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
    pub initializer: Option<ResolvedMemberDefinition>,
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
                .initializer
                .as_ref()
                .or(self.copy_constructor.as_ref())
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
pub struct ResolvedFunctionDeclarationTable {
    entries: DenseFunctionTable<ResolvedFunctionDeclaration>,
}

impl ResolvedFunctionDeclarationTable {
    pub(crate) fn new(entries: Vec<ResolvedFunctionDeclaration>) -> Self {
        Self {
            entries: DenseFunctionTable::new(entries, |declaration| declaration.id),
        }
    }

    pub fn get(&self, id: FunctionId) -> Option<&ResolvedFunctionDeclaration> {
        self.entries.get(id, |declaration| declaration.id)
    }

    pub fn iter(&self) -> impl ExactSizeIterator<Item = &ResolvedFunctionDeclaration> {
        self.entries.iter()
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    #[cfg(test)]
    pub(crate) fn entries_mut_for_test(&mut self) -> &mut [ResolvedFunctionDeclaration] {
        self.entries.entries_mut_for_test()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedFunctionDeclaration {
    pub id: FunctionId,
    /// Retained for diagnostics, dumps, and external-linkage selection.
    pub name: String,
    pub name_span: Span,
    pub parameters: Vec<ResolvedParameter>,
    pub return_type: ResolvedType,
    pub linkage: ResolvedFunctionLinkage,
    pub span: Span,
}

impl ResolvedFunctionDeclaration {
    pub fn parameter(&self, id: ParameterId) -> Option<&ResolvedParameter> {
        (id.callable() == self.id.into())
            .then(|| self.parameters.get(id.index()))
            .flatten()
            .filter(|parameter| parameter.id == id)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ResolvedFunctionLinkage {
    Internal,
    External { symbol: String },
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
pub struct ResolvedParameter {
    pub id: ParameterId,
    pub binding_mode: ResolvedParameterBindingMode,
    pub name: String,
    pub name_span: Span,
    pub type_syntax: ResolvedType,
    pub span: Span,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ResolvedParameterBindingMode {
    Value,
    ReadOnlyAlias { ref_span: Span },
    MutableAlias { mut_span: Span, ref_span: Span },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedLocal {
    pub id: LocalId,
    pub name: String,
    pub name_span: Span,
    pub type_syntax: ResolvedType,
    pub span: Span,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ResolvedTypeKind {
    I64,
    U64,
    U8,
    F64,
    Bool,
    Unit,
    Class(ClassId),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedType {
    pub kind: ResolvedTypeKind,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedBlock {
    pub statements: Vec<ResolvedStatement>,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ResolvedStatement {
    Local(ResolvedLocalDecl),
    Return(ResolvedReturn),
    Expression(ResolvedExpressionStatement),
    Conditional(ResolvedConditional),
    Block(ResolvedBlock),
    FieldAssignment(ResolvedFieldAssignment),
    ObjectAssignment(ResolvedObjectAssignment),
}

impl ResolvedStatement {
    pub const fn span(&self) -> Span {
        match self {
            Self::Local(statement) => statement.span,
            Self::Return(statement) => statement.span,
            Self::Expression(statement) => statement.span,
            Self::Conditional(statement) => statement.span,
            Self::Block(block) => block.span,
            Self::FieldAssignment(statement) => statement.span,
            Self::ObjectAssignment(statement) => statement.span,
        }
    }
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
    pub receiver: ResolvedObjectPlace,
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

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ResolvedExpression {
    Binding(ResolvedBindingExpr),
    NumericLiteral(ResolvedNumericLiteralExpr),
    Boolean(ResolvedBooleanExpr),
    Unary(ResolvedUnaryExpr),
    Binary(ResolvedBinaryExpr),
    DirectCall(ResolvedDirectCallExpr),
    Grouped(ResolvedGroupedExpr),
    FieldAccess(ResolvedFieldAccessExpr),
    MethodCall(ResolvedMethodCallExpr),
    Construct(ResolvedConstructExpr),
}

impl ResolvedExpression {
    pub const fn span(&self) -> Span {
        match self {
            Self::Binding(expression) => expression.span,
            Self::NumericLiteral(expression) => expression.span,
            Self::Boolean(expression) => expression.span,
            Self::Unary(expression) => expression.span,
            Self::Binary(expression) => expression.span,
            Self::DirectCall(expression) => expression.span,
            Self::Grouped(expression) => expression.span,
            Self::FieldAccess(expression) => expression.span,
            Self::MethodCall(expression) => expression.span,
            Self::Construct(expression) => expression.span,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedFieldAccessExpr {
    pub receiver: ResolvedObjectPlace,
    pub field: FieldId,
    pub member_span: Span,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedMethodCallExpr {
    pub receiver: ResolvedObjectPlace,
    pub method: MethodId,
    pub member_span: Span,
    pub arguments: Vec<ResolvedExpression>,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedConstructExpr {
    pub class: ClassId,
    pub initializer: InitializerId,
    pub callee_span: Span,
    pub arguments: Vec<ResolvedExpression>,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedBindingExpr {
    pub binding: BindingId,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedNumericLiteralExpr {
    pub kind: NumericLiteralKind,
    pub spelling: String,
    pub span: Span,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ResolvedBooleanExpr {
    pub value: bool,
    pub span: Span,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ResolvedUnaryOperator {
    Negate,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedUnaryExpr {
    pub operator: ResolvedUnaryOperator,
    pub operator_span: Span,
    pub operand: Box<ResolvedExpression>,
    pub span: Span,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ResolvedBinaryOperator {
    Add,
    Subtract,
    Multiply,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedBinaryExpr {
    pub left: Box<ResolvedExpression>,
    pub operator: ResolvedBinaryOperator,
    pub operator_span: Span,
    pub right: Box<ResolvedExpression>,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedDirectCallExpr {
    pub function: FunctionId,
    pub callee_span: Span,
    pub arguments: Vec<ResolvedExpression>,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedGroupedExpr {
    pub expression: Box<ResolvedExpression>,
    pub span: Span,
}
