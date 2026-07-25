//! Declaration collection, lexical name resolution, and stable identity assignment.
//!
//! Resolution produces a separate representation with stable typed IDs. Later
//! phases never choose declarations by comparing source names.

mod dump;
mod ir;
mod resolver;

pub use dump::dump_resolved;
pub use ir::{
    ResolvedAllocationExpr, ResolvedBaseInitialization, ResolvedBinaryExpr, ResolvedBinaryOperator,
    ResolvedBindingExpr, ResolvedBlock, ResolvedBooleanExpr, ResolvedClassDeclaration,
    ResolvedClassDeclarationTable, ResolvedClassDefinition, ResolvedClassDefinitionTable,
    ResolvedClassHierarchy, ResolvedClassMember, ResolvedConditional, ResolvedConditionalArm,
    ResolvedConstructExpr, ResolvedConstructionMode, ResolvedCopyAssignmentDeclaration,
    ResolvedCopyConstructorDeclaration, ResolvedCopyOperation, ResolvedDereferenceExpr,
    ResolvedDereferenceOperator, ResolvedDestructorDeclaration, ResolvedDirectBase,
    ResolvedDirectCallExpr, ResolvedExpression, ResolvedExpressionStatement,
    ResolvedFieldAccessExpr, ResolvedFieldAssignment, ResolvedFieldDeclaration,
    ResolvedFunctionDeclaration, ResolvedFunctionDeclarationTable, ResolvedFunctionDefinition,
    ResolvedFunctionDefinitionTable, ResolvedFunctionLinkage, ResolvedGroupedExpr,
    ResolvedInitializerDeclaration, ResolvedInterfaceCallExpr, ResolvedInterfaceClaim,
    ResolvedInterfaceDeclaration, ResolvedInterfaceDeclarationTable, ResolvedInterfaceParameter,
    ResolvedInterfaceReceiver, ResolvedInterfaceRequirement, ResolvedLocal, ResolvedLocalDecl,
    ResolvedMemberDefinition, ResolvedMethodCallExpr, ResolvedMethodDeclaration,
    ResolvedMethodDispatch, ResolvedMethodModifier, ResolvedNumericLiteralExpr,
    ResolvedObjectAssignment, ResolvedObjectCastExpr, ResolvedObjectCastTargetMode,
    ResolvedObjectPlace, ResolvedObjectReceiver, ResolvedParameter, ResolvedParameterBindingMode,
    ResolvedProgram, ResolvedReceiverAccess, ResolvedReturn, ResolvedSharedAssignment,
    ResolvedSharedTarget, ResolvedStatement, ResolvedType, ResolvedTypeKind, ResolvedTypeTestExpr,
    ResolvedUnaryExpr, ResolvedUnaryOperator, ResolvedVirtualFamily, ResolvedVirtualFamilyTable,
};
pub use resolver::{
    resolve, ResolveOutput, DUPLICATE_BINDING, DUPLICATE_MEMBER, DUPLICATE_TOP_LEVEL,
    IMPLICIT_SHARED_DEREFERENCE, INHERITANCE_CYCLE, INHERITED_MEMBER_COLLISION, INVALID_BASE_CLASS,
    INVALID_BASE_INITIALIZATION, INVALID_CALL_TARGET, INVALID_CONSTRUCTION_TARGET,
    INVALID_DEREFERENCE, INVALID_INTERFACE_CLAIM, INVALID_LIFECYCLE_SIGNATURE,
    INVALID_MEMBER_SELECTION, INVALID_OVERRIDE, INVALID_POINTEE_ASSIGNMENT, SELF_OUTSIDE_MEMBER,
    TOP_LEVEL_USED_AS_VALUE, UNKNOWN_MEMBER, UNKNOWN_NAME, UNKNOWN_TYPE,
};

#[cfg(test)]
mod tests;
