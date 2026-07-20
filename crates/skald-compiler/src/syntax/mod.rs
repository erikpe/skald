//! Parser and source-oriented abstract syntax tree.
//!
//! Syntax nodes preserve source spans and source spellings, but do not contain
//! resolved symbols or inferred semantic types. Name lookup belongs to M3.

mod ast;
mod dump;
mod parser;

pub use ast::{
    BinaryExpr, BinaryOperator, Block, BooleanExpr, CallExpr, CompilationUnit, ConditionalArm,
    ConditionalStatement, Expression, ExpressionStatement, ExternalFunctionDecl, FunctionDecl,
    GroupedExpr, IdentifierExpr, LocalDecl, Name, NumericLiteralExpr, Parameter, ReturnStatement,
    Statement, TopLevelDeclaration, TypeKind, TypeSyntax, UnaryExpr, UnaryOperator,
};
pub use dump::dump_ast;
pub use parser::{
    parse, ParseOutput, EXPECTED_DECLARATION, EXPECTED_EXPRESSION, EXPECTED_STATEMENT,
    EXPECTED_TOKEN,
};

#[cfg(test)]
mod tests;
