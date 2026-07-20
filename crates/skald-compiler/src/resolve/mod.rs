//! Declaration collection, lexical name resolution, and stable identity assignment.
//!
//! Resolution produces a separate representation with stable typed IDs. Later
//! phases never choose declarations by comparing source names.

mod dump;
mod ir;
mod resolver;

pub use dump::dump_resolved;
pub use ir::{
    ResolvedBinaryExpr, ResolvedBinaryOperator, ResolvedBindingExpr, ResolvedBlock,
    ResolvedBooleanExpr, ResolvedConditional, ResolvedConditionalArm, ResolvedDirectCallExpr,
    ResolvedExpression, ResolvedExpressionStatement, ResolvedFunctionDeclaration,
    ResolvedFunctionDeclarationTable, ResolvedFunctionDefinition, ResolvedFunctionDefinitionTable,
    ResolvedFunctionLinkage, ResolvedGroupedExpr, ResolvedLocal, ResolvedLocalDecl,
    ResolvedNumericLiteralExpr, ResolvedParameter, ResolvedProgram, ResolvedReturn,
    ResolvedStatement, ResolvedType, ResolvedTypeKind, ResolvedUnaryExpr, ResolvedUnaryOperator,
};
pub use resolver::{
    resolve, ResolveOutput, DUPLICATE_BINDING, DUPLICATE_FUNCTION, FUNCTION_USED_AS_VALUE,
    INVALID_CALL_TARGET, OBJECT_SYNTAX_NOT_RESOLVED, UNKNOWN_NAME,
};

#[cfg(test)]
mod tests;
