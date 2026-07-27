//! Name-resolved, but not yet type-checked, program representation.

mod array_types;
mod body;
mod declarations;
mod expression;
mod hierarchy;
mod modules;
mod object_place;

pub use array_types::{ResolvedArrayType, ResolvedArrayTypeTable};
pub use body::{
    ResolvedArrayAssignment, ResolvedBaseInitialization, ResolvedBlock, ResolvedClassDefinition,
    ResolvedClassDefinitionTable, ResolvedConditional, ResolvedConditionalArm,
    ResolvedExpressionStatement, ResolvedFieldAssignment, ResolvedFunctionDefinition,
    ResolvedFunctionDefinitionTable, ResolvedLocalDecl, ResolvedMemberDefinition,
    ResolvedObjectAssignment, ResolvedOptionalAssignment, ResolvedReturn, ResolvedSharedAssignment,
    ResolvedStatement,
};
pub use declarations::{
    ResolvedClassDeclaration, ResolvedClassDeclarationTable, ResolvedCopyAssignmentDeclaration,
    ResolvedCopyConstructorDeclaration, ResolvedCopyOperation, ResolvedDestructorDeclaration,
    ResolvedDirectBase, ResolvedFieldDeclaration, ResolvedFunctionDeclaration,
    ResolvedFunctionDeclarationTable, ResolvedFunctionLinkage, ResolvedInitializerDeclaration,
    ResolvedInterfaceClaim, ResolvedInterfaceDeclaration, ResolvedInterfaceDeclarationTable,
    ResolvedInterfaceParameter, ResolvedInterfaceRequirement, ResolvedLocal,
    ResolvedMethodDeclaration, ResolvedMethodDispatch, ResolvedMethodModifier,
    ResolvedOptionalPayload, ResolvedParameter, ResolvedParameterBindingMode, ResolvedProgram,
    ResolvedReceiverAccess, ResolvedSharedTarget, ResolvedType, ResolvedTypeKind,
    ResolvedVirtualFamily, ResolvedVirtualFamilyTable,
};
pub use expression::{
    ResolvedAbsentExpr, ResolvedAllocationExpr, ResolvedArrayConstructionArguments,
    ResolvedArrayConstructionExpr, ResolvedArrayLengthExpr, ResolvedArrayLengthOperator,
    ResolvedArrayProjectionBounds, ResolvedArrayProjectionExpr, ResolvedArrayProjectionOperator,
    ResolvedBinaryExpr, ResolvedBinaryOperator, ResolvedBindingExpr, ResolvedBooleanExpr,
    ResolvedConstructExpr, ResolvedConstructionMode, ResolvedDereferenceExpr,
    ResolvedDereferenceOperator, ResolvedDirectCallExpr, ResolvedExpression,
    ResolvedFieldAccessExpr, ResolvedGroupedExpr, ResolvedInterfaceCallExpr,
    ResolvedInterfaceReceiver, ResolvedMethodCallExpr, ResolvedNumericLiteralExpr,
    ResolvedObjectCastExpr, ResolvedObjectCastTargetMode, ResolvedPresenceTestExpr,
    ResolvedPresenceTestKind, ResolvedTypeTestExpr, ResolvedUnaryExpr, ResolvedUnaryOperator,
    ResolvedUnwrapExpr,
};
pub use hierarchy::{ResolvedClassHierarchy, ResolvedClassMember};
pub use modules::{
    ResolvedModuleDeclaration, ResolvedModuleDeclarationTable, ResolvedModuleDeclarations,
    ResolvedTopLevelId, ResolvedVisibility,
};
pub use object_place::{ResolvedObjectPlace, ResolvedObjectReceiver};

pub(crate) use hierarchy::ResolvedClassHierarchyEntry;
