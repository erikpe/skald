//! Parser and source-oriented abstract syntax tree.
//!
//! `docs/language/GRAMMAR.md` defines the accepted source shape.
//!
//! Syntax nodes preserve source spans and source spellings, but do not contain
//! resolved symbols or inferred semantic types. Name lookup belongs to the
//! resolution phase.

mod ast;
mod dump;
mod parser;

pub use ast::{
    AbsentExpr, AllocationExpr, ArrayConstructionArguments, ArrayConstructionExpr,
    ArrayProjectionBounds, ArrayProjectionExpr, ArrayProjectionOperator,
    BaseInitializationStatement, BinaryExpr, BinaryOperator, Block, BooleanExpr, CallArguments,
    CallExpr, ClassDecl, ClassMember, CompilationUnit, ConditionalArm, ConditionalStatement,
    CopyAssignmentDecl, CopyConstructorDecl, DestructorDecl, Expression, ExpressionStatement,
    ExternalFunctionDecl, FieldAssignmentStatement, FieldDecl, FunctionDecl, GroupedExpr,
    IdentifierExpr, ImportDeclaration, InitializerDecl, IntegerCastExpr, IntrinsicFunctionDecl,
    LocalDecl, MemberAccessExpr, MemberAccessOperator, MemberVisibility, MethodDecl,
    MethodModifier, ModuleImport, Name, NameComponent, NameComponentRef, NameComponents,
    NameQualification, NameText, NumericLiteralExpr, ObjectAssignmentStatement, ObjectCastExpr,
    ObjectCastTargetMode, OptionalPayloadKind, Parameter, ParameterBindingMode, PresenceTestExpr,
    PresenceTestKind, PrimitiveIntegerType, ReturnStatement, SelectiveImport, SelectiveImportItem,
    SelfExpr, Statement, StringLiteralExpr, TopLevelDeclaration, TypeKind, TypeSyntax,
    TypeTestExpr, UnaryExpr, UnaryOperator, UnwrapExpr, Visibility, WhileStatement,
};
pub use dump::dump_ast;
pub use parser::{
    parse, ParseOutput, EXCESSIVE_NESTING, EXPECTED_DECLARATION, EXPECTED_EXPRESSION,
    EXPECTED_STATEMENT, EXPECTED_TOKEN, INVALID_CLASS_HEADER, INVALID_CLASS_MEMBER,
    INVALID_COMPARISON, INVALID_IMPORT, INVALID_OPTIONAL_TYPE, INVALID_TYPE_TEST,
    INVALID_VISIBILITY, MAX_SYNTAX_NESTING, MISPLACED_IMPORT, UNSUPPORTED_LOOP_EXIT,
};

#[cfg(test)]
mod nesting_tests;
#[cfg(test)]
mod tests;
