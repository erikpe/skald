//! Name-resolved, but not yet type-checked, program representation.

mod body;
mod declarations;
mod expression;
mod hierarchy;
mod object_place;

pub use body::{
    ResolvedBlock, ResolvedClassDefinition, ResolvedClassDefinitionTable, ResolvedConditional,
    ResolvedConditionalArm, ResolvedExpressionStatement, ResolvedFieldAssignment,
    ResolvedFunctionDefinition, ResolvedFunctionDefinitionTable, ResolvedLocalDecl,
    ResolvedMemberDefinition, ResolvedObjectAssignment, ResolvedReturn, ResolvedStatement,
};
pub use declarations::{
    ResolvedClassDeclaration, ResolvedClassDeclarationTable, ResolvedCopyAssignmentDeclaration,
    ResolvedCopyOperation, ResolvedDestructorDeclaration, ResolvedDirectBase,
    ResolvedFieldDeclaration, ResolvedFunctionDeclaration, ResolvedFunctionDeclarationTable,
    ResolvedFunctionLinkage, ResolvedInitializerDeclaration, ResolvedLocal,
    ResolvedMethodDeclaration, ResolvedParameter, ResolvedParameterBindingMode, ResolvedProgram,
    ResolvedReceiverAccess, ResolvedType, ResolvedTypeKind,
};
pub use expression::{
    ResolvedBinaryExpr, ResolvedBinaryOperator, ResolvedBindingExpr, ResolvedBooleanExpr,
    ResolvedConstructExpr, ResolvedDirectCallExpr, ResolvedExpression, ResolvedFieldAccessExpr,
    ResolvedGroupedExpr, ResolvedMethodCallExpr, ResolvedNumericLiteralExpr, ResolvedUnaryExpr,
    ResolvedUnaryOperator,
};
pub use hierarchy::{ResolvedClassHierarchy, ResolvedClassMember};
pub use object_place::ResolvedObjectPlace;

pub(crate) use hierarchy::ResolvedClassHierarchyEntry;
