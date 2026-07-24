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
    ResolvedObjectAssignment, ResolvedReturn, ResolvedStatement,
};
pub use declarations::{
    ResolvedClassDeclaration, ResolvedClassDeclarationTable, ResolvedCopyAssignmentDeclaration,
    ResolvedCopyConstructorDeclaration, ResolvedCopyOperation, ResolvedDestructorDeclaration,
    ResolvedDirectBase, ResolvedFieldDeclaration, ResolvedFunctionDeclaration,
    ResolvedFunctionDeclarationTable, ResolvedFunctionLinkage, ResolvedInitializerDeclaration,
    ResolvedInterfaceClaim, ResolvedInterfaceDeclaration, ResolvedInterfaceDeclarationTable,
    ResolvedInterfaceParameter, ResolvedInterfaceRequirement, ResolvedLocal,
    ResolvedMethodDeclaration, ResolvedMethodDispatch, ResolvedMethodModifier, ResolvedParameter,
    ResolvedParameterBindingMode, ResolvedProgram, ResolvedReceiverAccess, ResolvedSharedTarget,
    ResolvedType, ResolvedTypeKind, ResolvedVirtualFamily, ResolvedVirtualFamilyTable,
};
pub use expression::{
    ResolvedAllocationExpr, ResolvedBinaryExpr, ResolvedBinaryOperator, ResolvedBindingExpr,
    ResolvedBooleanExpr, ResolvedConstructExpr, ResolvedConstructionMode, ResolvedDirectCallExpr,
    ResolvedExpression, ResolvedFieldAccessExpr, ResolvedGroupedExpr, ResolvedInterfaceCallExpr,
    ResolvedInterfaceReceiver, ResolvedMethodCallExpr, ResolvedNumericLiteralExpr,
    ResolvedObjectCastExpr, ResolvedObjectCastTargetMode, ResolvedTypeTestExpr, ResolvedUnaryExpr,
    ResolvedUnaryOperator,
};
pub use hierarchy::{ResolvedClassHierarchy, ResolvedClassMember};
pub use object_place::{ResolvedObjectPlace, ResolvedObjectReceiver};

pub(crate) use hierarchy::ResolvedClassHierarchyEntry;
