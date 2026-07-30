//! Name-resolved, but not yet type-checked, program representation.

mod array_types;
mod body;
mod declarations;
mod expression;
mod hierarchy;
mod modules;
mod object_place;
mod strings;

pub use array_types::{ResolvedArrayType, ResolvedArrayTypeTable};
pub use body::{
    ResolvedArrayAssignment, ResolvedBaseInitialization, ResolvedBlock, ResolvedBreak,
    ResolvedClassDefinition, ResolvedClassDefinitionTable, ResolvedConditional,
    ResolvedConditionalArm, ResolvedContinue, ResolvedExpressionStatement, ResolvedFieldAssignment,
    ResolvedFunctionDefinition, ResolvedFunctionDefinitionTable, ResolvedLocalDecl,
    ResolvedMemberDefinition, ResolvedObjectAssignment, ResolvedOptionalAssignment,
    ResolvedPrimitiveBindingAssignment, ResolvedReturn, ResolvedSharedAssignment,
    ResolvedStatement, ResolvedWhile,
};
pub use declarations::{
    ResolvedClassDeclaration, ResolvedClassDeclarationTable, ResolvedCopyAssignmentDeclaration,
    ResolvedCopyConstructorDeclaration, ResolvedCopyOperation, ResolvedDestructorDeclaration,
    ResolvedDirectBase, ResolvedFieldDeclaration, ResolvedFunctionDeclaration,
    ResolvedFunctionDeclarationTable, ResolvedFunctionLinkage, ResolvedInitializerDeclaration,
    ResolvedInterfaceClaim, ResolvedInterfaceDeclaration, ResolvedInterfaceDeclarationTable,
    ResolvedInterfaceParameter, ResolvedInterfaceRequirement, ResolvedLocal,
    ResolvedMemberVisibility, ResolvedMethodDeclaration, ResolvedMethodDispatch,
    ResolvedMethodKind, ResolvedMethodModifier, ResolvedOptionalPayload, ResolvedParameter,
    ResolvedParameterBindingMode, ResolvedProgram, ResolvedReceiverAccess, ResolvedSharedTarget,
    ResolvedType, ResolvedTypeKind, ResolvedVirtualFamily, ResolvedVirtualFamilyTable,
};
pub use expression::{
    ResolvedAbsentExpr, ResolvedAllocationExpr, ResolvedArrayConstructionArguments,
    ResolvedArrayConstructionExpr, ResolvedArrayLengthExpr, ResolvedArrayLengthOperator,
    ResolvedArrayProjectionBounds, ResolvedArrayProjectionExpr, ResolvedArrayProjectionOperator,
    ResolvedBinaryExpr, ResolvedBinaryOperator, ResolvedBindingExpr, ResolvedBooleanExpr,
    ResolvedConstructExpr, ResolvedConstructionMode, ResolvedDereferenceExpr,
    ResolvedDereferenceOperator, ResolvedDirectCallExpr, ResolvedExpression,
    ResolvedFieldAccessExpr, ResolvedGroupedExpr, ResolvedIntegerCastExpr, ResolvedIntegerType,
    ResolvedInterfaceCallExpr, ResolvedInterfaceReceiver, ResolvedLogicalExpr,
    ResolvedLogicalOperator, ResolvedMethodCallExpr, ResolvedNumericLiteralExpr,
    ResolvedObjectCastExpr, ResolvedObjectCastTargetMode, ResolvedPresenceTestExpr,
    ResolvedPresenceTestKind, ResolvedStaticCallExpr, ResolvedStringLiteralExpr,
    ResolvedTypeTestExpr, ResolvedUnaryExpr, ResolvedUnaryOperator, ResolvedUnwrapExpr,
};
pub use hierarchy::{ResolvedClassHierarchy, ResolvedClassMember};
pub use modules::{
    ResolvedModuleBinding, ResolvedModuleBindingTable, ResolvedModuleBindings,
    ResolvedModuleDeclaration, ResolvedModuleDeclarationTable, ResolvedModuleDeclarations,
    ResolvedOrdinaryBinding, ResolvedOrdinaryBindingTable, ResolvedOrdinaryBindings,
    ResolvedTopLevelId, ResolvedVisibility,
};
pub use object_place::{ResolvedObjectPlace, ResolvedObjectReceiver};
pub use strings::{ResolvedLiteralData, ResolvedLiteralDataTable, ResolvedStringLanguageItem};

pub(crate) use hierarchy::ResolvedClassHierarchyEntry;
