//! Source-shaped AST for the implemented language subset.

use std::{fmt, ops::Deref};

use crate::{literal::NumericLiteralKind, source::Span};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompilationUnit {
    pub imports: Vec<ImportDeclaration>,
    pub declarations: Vec<TopLevelDeclaration>,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ImportDeclaration {
    Module(ModuleImport),
    Selective(SelectiveImport),
}

impl ImportDeclaration {
    pub const fn span(&self) -> Span {
        match self {
            Self::Module(import) => import.span,
            Self::Selective(import) => import.span,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ModuleImport {
    pub import_span: Span,
    pub module: Name,
    pub as_span: Option<Span>,
    pub alias: Option<Name>,
    pub semicolon_span: Span,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SelectiveImport {
    pub from_span: Span,
    pub module: Name,
    pub import_span: Span,
    pub items: Vec<SelectiveImportItem>,
    pub comma_spans: Vec<Span>,
    pub semicolon_span: Span,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SelectiveImportItem {
    pub name: Name,
    pub as_span: Option<Span>,
    pub alias: Option<Name>,
    pub span: Span,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Visibility {
    Private,
    Public { span: Span },
}

impl Visibility {
    pub const fn start_span(self, fallback: Span) -> Span {
        match self {
            Self::Private => fallback,
            Self::Public { span } => span,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MemberVisibility {
    Public,
    Private { span: Span },
}

impl MemberVisibility {
    pub const fn start_span(self, fallback: Span) -> Span {
        match self {
            Self::Public => fallback,
            Self::Private { span } => span,
        }
    }
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

    pub const fn visibility(&self) -> Visibility {
        match self {
            Self::Function(function) => function.visibility,
            Self::ExternalFunction(function) => function.visibility,
            Self::Class(class) => class.visibility,
            Self::Interface(interface) => interface.visibility,
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
    pub visibility: Visibility,
    pub name: Name,
    pub direct_base: Option<Name>,
    pub implemented_interfaces: Vec<Name>,
    pub members: Vec<ClassMember>,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InterfaceDecl {
    pub visibility: Visibility,
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
    pub visibility: MemberVisibility,
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
    pub visibility: MemberVisibility,
    pub static_span: Option<Span>,
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
    pub visibility: Visibility,
    pub name: Name,
    pub parameters: Vec<Parameter>,
    pub return_type: TypeSyntax,
    pub body: Block,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExternalFunctionDecl {
    pub visibility: Visibility,
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
    pub text: NameText,
    pub span: Span,
}

impl Name {
    pub fn unqualified(text: String, span: Span) -> Self {
        Self {
            text: NameText::Unqualified(text),
            span,
        }
    }

    pub fn qualified(
        text: String,
        span: Span,
        components: Vec<NameComponent>,
        separator_spans: Vec<Span>,
    ) -> Self {
        Self {
            text: NameText::Qualified(Box::new(NameQualification {
                text,
                components,
                separator_spans,
            })),
            span,
        }
    }

    pub const fn is_qualified(&self) -> bool {
        matches!(self.text, NameText::Qualified(_))
    }

    pub fn components(&self) -> NameComponents<'_> {
        match &self.text {
            NameText::Qualified(qualification) => {
                NameComponents::Qualified(qualification.components.iter())
            }
            NameText::Unqualified(_) => NameComponents::Unqualified(Some(NameComponentRef {
                text: &self.text,
                span: self.span,
            })),
        }
    }

    pub fn separator_spans(&self) -> &[Span] {
        match &self.text {
            NameText::Unqualified(_) => &[],
            NameText::Qualified(qualification) => &qualification.separator_spans,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum NameText {
    Unqualified(String),
    Qualified(Box<NameQualification>),
}

impl NameText {
    pub fn as_str(&self) -> &str {
        match self {
            Self::Unqualified(text) => text,
            Self::Qualified(qualification) => &qualification.text,
        }
    }
}

impl Deref for NameText {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        self.as_str()
    }
}

impl fmt::Display for NameText {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self)
    }
}

impl PartialEq<str> for NameText {
    fn eq(&self, other: &str) -> bool {
        self.as_str() == other
    }
}

impl PartialEq<&str> for NameText {
    fn eq(&self, other: &&str) -> bool {
        self.as_str() == *other
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NameComponent {
    pub text: String,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NameQualification {
    pub text: String,
    pub components: Vec<NameComponent>,
    pub separator_spans: Vec<Span>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NameComponentRef<'name> {
    pub text: &'name str,
    pub span: Span,
}

pub enum NameComponents<'name> {
    Unqualified(Option<NameComponentRef<'name>>),
    Qualified(std::slice::Iter<'name, NameComponent>),
}

impl<'name> Iterator for NameComponents<'name> {
    type Item = NameComponentRef<'name>;

    fn next(&mut self) -> Option<Self::Item> {
        match self {
            Self::Unqualified(component) => component.take(),
            Self::Qualified(components) => components.next().map(|component| NameComponentRef {
                text: &component.text,
                span: component.span,
            }),
        }
    }
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
    Shared {
        shared_span: Span,
        target: Box<TypeSyntax>,
    },
    Optional {
        payload: OptionalPayloadKind,
        payload_span: Span,
        question_span: Span,
    },
    OptionalShared {
        shared_span: Span,
        question_span: Span,
        target: Box<TypeSyntax>,
    },
    Grouped {
        left_paren_span: Span,
        inner: Box<TypeSyntax>,
        right_paren_span: Span,
    },
    Array {
        element: Box<TypeSyntax>,
        left_bracket_span: Span,
        right_bracket_span: Span,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum OptionalPayloadKind {
    I64,
    U64,
    U8,
    F64,
    Bool,
    Named(Name),
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
    Absent(AbsentExpr),
    Identifier(IdentifierExpr),
    NumericLiteral(NumericLiteralExpr),
    Boolean(BooleanExpr),
    Unary(UnaryExpr),
    Binary(BinaryExpr),
    TypeTest(TypeTestExpr),
    PresenceTest(PresenceTestExpr),
    Unwrap(UnwrapExpr),
    ObjectCast(ObjectCastExpr),
    Allocation(Box<AllocationExpr>),
    ArrayConstruction(Box<ArrayConstructionExpr>),
    Call(CallExpr),
    Grouped(GroupedExpr),
    SelfValue(SelfExpr),
    MemberAccess(MemberAccessExpr),
    ArrayProjection(Box<ArrayProjectionExpr>),
}

impl Expression {
    pub const fn span(&self) -> Span {
        match self {
            Self::Absent(expression) => expression.span,
            Self::Identifier(expression) => expression.span,
            Self::NumericLiteral(expression) => expression.span,
            Self::Boolean(expression) => expression.span,
            Self::Unary(expression) => expression.span,
            Self::Binary(expression) => expression.span,
            Self::TypeTest(expression) => expression.span,
            Self::PresenceTest(expression) => expression.span,
            Self::Unwrap(expression) => expression.span,
            Self::ObjectCast(expression) => expression.span,
            Self::Allocation(expression) => expression.span,
            Self::ArrayConstruction(expression) => expression.span,
            Self::Call(expression) => expression.span,
            Self::Grouped(expression) => expression.span,
            Self::SelfValue(expression) => expression.span,
            Self::MemberAccess(expression) => expression.span,
            Self::ArrayProjection(expression) => expression.span,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AbsentExpr {
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PresenceTestExpr {
    pub source: Box<Expression>,
    pub is_span: Span,
    pub kind: PresenceTestKind,
    pub target_span: Span,
    pub span: Span,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PresenceTestKind {
    Some,
    None,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UnwrapExpr {
    pub source: Box<Expression>,
    pub bang_span: Span,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AllocationExpr {
    pub new_span: Span,
    pub target: Name,
    pub arguments: CallArguments,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ArrayConstructionExpr {
    pub new_span: Option<Span>,
    pub array_type: TypeSyntax,
    pub arguments: ArrayConstructionArguments,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ArrayConstructionArguments {
    Empty {
        left_paren_span: Span,
        right_paren_span: Span,
    },
    Length {
        left_paren_span: Span,
        length: Box<Expression>,
        right_paren_span: Span,
    },
    Copy {
        left_paren_span: Span,
        copy_span: Span,
        source: Box<Expression>,
        right_paren_span: Span,
    },
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
    pub operator: MemberAccessOperator,
    pub member: Name,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ArrayProjectionExpr {
    pub receiver: Box<Expression>,
    pub operator: ArrayProjectionOperator,
    pub bounds: ArrayProjectionBounds,
    pub right_bracket_span: Span,
    pub span: Span,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ArrayProjectionOperator {
    Ordinary {
        left_bracket_span: Span,
    },
    Shared {
        arrow_span: Span,
        left_bracket_span: Span,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ArrayProjectionBounds {
    Index(Box<Expression>),
    Slice {
        start: Option<Box<Expression>>,
        colon_span: Span,
        end: Option<Box<Expression>>,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MemberAccessOperator {
    Dot { span: Span },
    Arrow { span: Span },
}

impl MemberAccessOperator {
    pub const fn span(self) -> Span {
        match self {
            Self::Dot { span } | Self::Arrow { span } => span,
        }
    }
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
    Dereference,
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
