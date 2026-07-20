//! Parser and source-oriented abstract syntax tree.
//!
//! Syntax nodes preserve source spans and source spellings, but do not contain
//! resolved symbols or inferred semantic types. Name lookup belongs to the
//! resolution phase.

mod ast;
mod dump;
mod parser;

pub use ast::{
    BinaryExpr, BinaryOperator, Block, BooleanExpr, CallExpr, ClassDecl, ClassMember,
    CompilationUnit, ConditionalArm, ConditionalStatement, Expression, ExpressionStatement,
    ExternalFunctionDecl, FieldAssignmentStatement, FieldDecl, FunctionDecl, GroupedExpr,
    IdentifierExpr, InitializerDecl, LocalDecl, MemberAccessExpr, MethodDecl, Name,
    NumericLiteralExpr, Parameter, ReturnStatement, SelfExpr, Statement, TopLevelDeclaration,
    TypeKind, TypeSyntax, UnaryExpr, UnaryOperator,
};
pub use dump::dump_ast;
pub use parser::{
    parse, ParseOutput, EXCESSIVE_NESTING, EXPECTED_DECLARATION, EXPECTED_EXPRESSION,
    EXPECTED_STATEMENT, EXPECTED_TOKEN, INVALID_CLASS_MEMBER, MAX_SYNTAX_NESTING,
};

#[cfg(test)]
mod nesting_tests;
#[cfg(test)]
mod tests;
