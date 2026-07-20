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
    ResolvedBooleanExpr, ResolvedClassDeclaration, ResolvedClassDeclarationTable,
    ResolvedClassDefinition, ResolvedClassDefinitionTable, ResolvedConditional,
    ResolvedConditionalArm, ResolvedConstructExpr, ResolvedDirectCallExpr, ResolvedExpression,
    ResolvedExpressionStatement, ResolvedFieldAccessExpr, ResolvedFieldAssignment,
    ResolvedFieldDeclaration, ResolvedFunctionDeclaration, ResolvedFunctionDeclarationTable,
    ResolvedFunctionDefinition, ResolvedFunctionDefinitionTable, ResolvedFunctionLinkage,
    ResolvedGroupedExpr, ResolvedInitializerDeclaration, ResolvedLocal, ResolvedLocalDecl,
    ResolvedMemberDefinition, ResolvedMethodCallExpr, ResolvedMethodDeclaration,
    ResolvedNumericLiteralExpr, ResolvedObjectPlace, ResolvedParameter, ResolvedProgram,
    ResolvedReceiverAccess, ResolvedReturn, ResolvedStatement, ResolvedType, ResolvedTypeKind,
    ResolvedUnaryExpr, ResolvedUnaryOperator,
};
pub use resolver::{
    resolve, ResolveOutput, DUPLICATE_BINDING, DUPLICATE_MEMBER, DUPLICATE_TOP_LEVEL,
    INVALID_CALL_TARGET, INVALID_CONSTRUCTION_TARGET, INVALID_MEMBER_SELECTION,
    SELF_OUTSIDE_MEMBER, TOP_LEVEL_USED_AS_VALUE, UNKNOWN_MEMBER, UNKNOWN_NAME, UNKNOWN_TYPE,
};

#[cfg(test)]
mod tests;
