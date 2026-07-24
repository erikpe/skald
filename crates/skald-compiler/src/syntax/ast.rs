//! Source-shaped AST for the implemented language subset.

use crate::{literal::NumericLiteralKind, source::Span};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompilationUnit {
    pub declarations: Vec<TopLevelDeclaration>,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TopLevelDeclaration {
    Function(FunctionDecl),
    ExternalFunction(ExternalFunctionDecl),
    Class(ClassDecl),
    Interface(InterfaceDecl),
}

impl TopLevelDeclaration {
    pub const fn name(&self) -> &Name {
        match self {
            Self::Function(function) => &function.name,
            Self::ExternalFunction(function) => &function.name,
            Self::Class(class) => &class.name,
            Self::Interface(interface) => &interface.name,
        }
    }

    pub const fn span(&self) -> Span {
        match self {
            Self::Function(function) => function.span,
            Self::ExternalFunction(function) => function.span,
            Self::Class(class) => class.span,
            Self::Interface(interface) => interface.span,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClassDecl {
    pub name: Name,
    pub direct_base: Option<Name>,
    pub implemented_interfaces: Vec<Name>,
    pub members: Vec<ClassMember>,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InterfaceDecl {
    pub name: Name,
    pub requirements: Vec<InterfaceRequirementDecl>,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InterfaceRequirementDecl {
    pub mut_span: Option<Span>,
    pub name: Name,
    pub parameters: Vec<Parameter>,
    pub return_type: TypeSyntax,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ClassMember {
    Field(FieldDecl),
    Initializer(InitializerDecl),
    CopyConstructor(CopyConstructorDecl),
    CopyAssignment(CopyAssignmentDecl),
    Destructor(DestructorDecl),
    Method(MethodDecl),
}

impl ClassMember {
    pub const fn span(&self) -> Span {
        match self {
            Self::Field(field) => field.span,
            Self::Initializer(initializer) => initializer.span,
            Self::CopyConstructor(constructor) => constructor.span,
            Self::CopyAssignment(assignment) => assignment.span,
            Self::Destructor(destructor) => destructor.span,
            Self::Method(method) => method.span,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FieldDecl {
    pub name: Name,
    pub type_syntax: TypeSyntax,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InitializerDecl {
    pub introducer_span: Span,
    pub parameters: Vec<Parameter>,
    pub body: Block,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CopyConstructorDecl {
    pub introducer_span: Span,
    pub parameters: Vec<Parameter>,
    pub body: Block,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CopyAssignmentDecl {
    pub introducer_span: Span,
    pub parameters: Vec<Parameter>,
    pub body: Block,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DestructorDecl {
    pub introducer_span: Span,
    pub body: Block,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MethodDecl {
    pub modifier: Option<MethodModifier>,
    pub mut_span: Option<Span>,
    pub name: Name,
    pub parameters: Vec<Parameter>,
    pub return_type: TypeSyntax,
    pub body: Block,
    pub span: Span,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MethodModifier {
    Virtual { span: Span },
    Override { span: Span },
}

impl MethodModifier {
    pub const fn span(self) -> Span {
        match self {
            Self::Virtual { span } | Self::Override { span } => span,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FunctionDecl {
    pub name: Name,
    pub parameters: Vec<Parameter>,
    pub return_type: TypeSyntax,
    pub body: Block,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExternalFunctionDecl {
    pub name: Name,
    pub parameters: Vec<Parameter>,
    pub return_type: TypeSyntax,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Parameter {
    pub binding_mode: ParameterBindingMode,
    pub name: Name,
    pub type_syntax: TypeSyntax,
    pub span: Span,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ParameterBindingMode {
    Value,
    ReadOnlyAlias { ref_span: Span },
    MutableAlias { mut_span: Span, ref_span: Span },
}

impl ParameterBindingMode {
    pub const fn start_span(self, fallback: Span) -> Span {
        match self {
            Self::Value => fallback,
            Self::ReadOnlyAlias { ref_span } => ref_span,
            Self::MutableAlias { mut_span, .. } => mut_span,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Name {
    pub text: String,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TypeKind {
    I64,
    U64,
    U8,
    F64,
    Bool,
    Unit,
    Named(Name),
    Shared { shared_span: Span, target: Name },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TypeSyntax {
    pub kind: TypeKind,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Block {
    pub statements: Vec<Statement>,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Statement {
    BaseInitialization(BaseInitializationStatement),
    Local(LocalDecl),
    Return(ReturnStatement),
    Expression(ExpressionStatement),
    Conditional(ConditionalStatement),
    Block(Block),
    FieldAssignment(FieldAssignmentStatement),
    ObjectAssignment(ObjectAssignmentStatement),
}

impl Statement {
    pub const fn span(&self) -> Span {
        match self {
            Self::BaseInitialization(statement) => statement.span,
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
pub struct BaseInitializationStatement {
    pub super_span: Span,
    pub arguments: Vec<Expression>,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ObjectAssignmentStatement {
    pub place: Expression,
    pub equal_span: Span,
    pub value: Expression,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FieldAssignmentStatement {
    pub place: MemberAccessExpr,
    pub equal_span: Span,
    pub value: Expression,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LocalDecl {
    pub name: Name,
    pub type_syntax: TypeSyntax,
    pub initializer: Expression,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReturnStatement {
    pub value: Option<Expression>,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExpressionStatement {
    pub expression: Expression,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConditionalStatement {
    pub if_arm: ConditionalArm,
    pub elif_arms: Vec<ConditionalArm>,
    pub else_block: Option<Block>,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConditionalArm {
    pub condition: Expression,
    pub body: Block,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Expression {
    Identifier(IdentifierExpr),
    NumericLiteral(NumericLiteralExpr),
    Boolean(BooleanExpr),
    Unary(UnaryExpr),
    Binary(BinaryExpr),
    TypeTest(TypeTestExpr),
    ObjectCast(ObjectCastExpr),
    Allocation(AllocationExpr),
    Call(CallExpr),
    Grouped(GroupedExpr),
    SelfValue(SelfExpr),
    MemberAccess(MemberAccessExpr),
}

impl Expression {
    pub const fn span(&self) -> Span {
        match self {
            Self::Identifier(expression) => expression.span,
            Self::NumericLiteral(expression) => expression.span,
            Self::Boolean(expression) => expression.span,
            Self::Unary(expression) => expression.span,
            Self::Binary(expression) => expression.span,
            Self::TypeTest(expression) => expression.span,
            Self::ObjectCast(expression) => expression.span,
            Self::Allocation(expression) => expression.span,
            Self::Call(expression) => expression.span,
            Self::Grouped(expression) => expression.span,
            Self::SelfValue(expression) => expression.span,
            Self::MemberAccess(expression) => expression.span,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AllocationExpr {
    pub new_span: Span,
    pub target: Name,
    pub arguments: CallArguments,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ObjectCastExpr {
    pub target: Name,
    pub target_mode: ObjectCastTargetMode,
    pub source: Box<Expression>,
    pub span: Span,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ObjectCastTargetMode {
    Plain,
    Shared { shared_span: Span },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TypeTestExpr {
    pub source: Box<Expression>,
    pub is_span: Span,
    pub target: Name,
    pub span: Span,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SelfExpr {
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MemberAccessExpr {
    pub receiver: Box<Expression>,
    pub dot_span: Span,
    pub member: Name,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IdentifierExpr {
    pub name: Name,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NumericLiteralExpr {
    /// Lexically determined kind; semantic conversion and ranges belong to type checking.
    pub kind: NumericLiteralKind,
    /// The complete original source spelling, retained for diagnostics.
    pub spelling: String,
    pub span: Span,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BooleanExpr {
    pub value: bool,
    pub span: Span,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UnaryOperator {
    Negate,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UnaryExpr {
    pub operator: UnaryOperator,
    pub operator_span: Span,
    pub operand: Box<Expression>,
    pub span: Span,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BinaryOperator {
    Add,
    Subtract,
    Multiply,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BinaryExpr {
    pub left: Box<Expression>,
    pub operator: BinaryOperator,
    pub operator_span: Span,
    pub right: Box<Expression>,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CallExpr {
    pub callee: Box<Expression>,
    pub arguments: CallArguments,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CallArguments {
    Ordinary(Vec<Expression>),
    Copy {
        copy_span: Span,
        source: Box<Expression>,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GroupedExpr {
    pub expression: Box<Expression>,
    pub span: Span,
}
