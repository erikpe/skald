//! Name-resolved, but not yet type-checked, program representation.

mod body;
mod declarations;
mod expression;
mod hierarchy;
mod object_place;

pub use body::{
    ResolvedBaseInitialization, ResolvedBlock, ResolvedClassDefinition,
    ResolvedClassDefinitionTable, ResolvedConditional, ResolvedConditionalArm,
    ResolvedExpressionStatement, ResolvedFieldAssignment, ResolvedFunctionDefinition,
    ResolvedFunctionDefinitionTable, ResolvedLocalDecl, ResolvedMemberDefinition,
    ResolvedNarrowedAlias, ResolvedNarrowing, ResolvedObjectAssignment, ResolvedReturn,
    ResolvedStatement,
};
pub use declarations::{
    ResolvedClassDeclaration, ResolvedClassDeclarationTable, ResolvedCopyAssignmentDeclaration,
    ResolvedCopyOperation, ResolvedDestructorDeclaration, ResolvedDirectBase,
    ResolvedFieldDeclaration, ResolvedFunctionDeclaration, ResolvedFunctionDeclarationTable,
    ResolvedFunctionLinkage, ResolvedInitializerDeclaration, ResolvedInterfaceClaim,
    ResolvedInterfaceDeclaration, ResolvedInterfaceDeclarationTable, ResolvedInterfaceParameter,
    ResolvedInterfaceRequirement, ResolvedLocal, ResolvedMethodDeclaration, ResolvedMethodDispatch,
    ResolvedMethodModifier, ResolvedParameter, ResolvedParameterBindingMode, ResolvedProgram,
    ResolvedReceiverAccess, ResolvedType, ResolvedTypeKind, ResolvedVirtualFamily,
    ResolvedVirtualFamilyTable,
};
pub use expression::{
    ResolvedBinaryExpr, ResolvedBinaryOperator, ResolvedBindingExpr, ResolvedBooleanExpr,
    ResolvedConstructExpr, ResolvedDirectCallExpr, ResolvedExpression, ResolvedFieldAccessExpr,
    ResolvedGroupedExpr, ResolvedInterfaceCallExpr, ResolvedMethodCallExpr,
    ResolvedNumericLiteralExpr, ResolvedTypeTestExpr, ResolvedUnaryExpr, ResolvedUnaryOperator,
};
pub use hierarchy::{ResolvedClassHierarchy, ResolvedClassMember};
pub use object_place::ResolvedObjectPlace;

pub(crate) use hierarchy::ResolvedClassHierarchyEntry;
