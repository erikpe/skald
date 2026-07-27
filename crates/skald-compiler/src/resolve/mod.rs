//! Declaration collection, lexical name resolution, and stable identity assignment.
//!
//! Resolution produces a separate representation with stable typed IDs. Later
//! phases never choose declarations by comparing source names.

mod dump;
mod ir;
mod resolver;

pub use dump::dump_resolved;
pub use ir::{
    ResolvedAbsentExpr, ResolvedAllocationExpr, ResolvedArrayAssignment,
    ResolvedArrayConstructionArguments, ResolvedArrayConstructionExpr, ResolvedArrayLengthExpr,
    ResolvedArrayLengthOperator, ResolvedArrayProjectionBounds, ResolvedArrayProjectionExpr,
    ResolvedArrayProjectionOperator, ResolvedArrayType, ResolvedArrayTypeTable,
    ResolvedBaseInitialization, ResolvedBinaryExpr, ResolvedBinaryOperator, ResolvedBindingExpr,
    ResolvedBlock, ResolvedBooleanExpr, ResolvedClassDeclaration, ResolvedClassDeclarationTable,
    ResolvedClassDefinition, ResolvedClassDefinitionTable, ResolvedClassHierarchy,
    ResolvedClassMember, ResolvedConditional, ResolvedConditionalArm, ResolvedConstructExpr,
    ResolvedConstructionMode, ResolvedCopyAssignmentDeclaration,
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
    ResolvedMethodDispatch, ResolvedMethodModifier, ResolvedModuleBinding,
    ResolvedModuleBindingTable, ResolvedModuleBindings, ResolvedModuleDeclaration,
    ResolvedModuleDeclarationTable, ResolvedModuleDeclarations, ResolvedNumericLiteralExpr,
    ResolvedObjectAssignment, ResolvedObjectCastExpr, ResolvedObjectCastTargetMode,
    ResolvedObjectPlace, ResolvedObjectReceiver, ResolvedOptionalAssignment,
    ResolvedOptionalPayload, ResolvedOrdinaryBinding, ResolvedOrdinaryBindingTable,
    ResolvedOrdinaryBindings, ResolvedParameter, ResolvedParameterBindingMode,
    ResolvedPresenceTestExpr, ResolvedPresenceTestKind, ResolvedProgram, ResolvedReceiverAccess,
    ResolvedReturn, ResolvedSharedAssignment, ResolvedSharedTarget, ResolvedStatement,
    ResolvedTopLevelId, ResolvedType, ResolvedTypeKind, ResolvedTypeTestExpr, ResolvedUnaryExpr,
    ResolvedUnaryOperator, ResolvedUnwrapExpr, ResolvedVirtualFamily, ResolvedVirtualFamilyTable,
    ResolvedVisibility,
};
pub use resolver::{
    resolve, resolve_module_graph, ResolveOutput, DUPLICATE_BINDING, DUPLICATE_MEMBER,
    DUPLICATE_MODULE_BINDING, DUPLICATE_ORDINARY_BINDING, DUPLICATE_TOP_LEVEL,
    IMPLICIT_SHARED_DEREFERENCE, INHERITANCE_CYCLE, INHERITED_MEMBER_COLLISION, INVALID_BASE_CLASS,
    INVALID_BASE_INITIALIZATION, INVALID_CALL_TARGET, INVALID_CONSTRUCTION_TARGET,
    INVALID_DEREFERENCE, INVALID_INTERFACE_CLAIM, INVALID_LIFECYCLE_SIGNATURE,
    INVALID_MEMBER_SELECTION, INVALID_OPTIONAL_TYPE, INVALID_OVERRIDE, INVALID_POINTEE_ASSIGNMENT,
    PRIVATE_DECLARATION, SELF_OUTSIDE_MEMBER, TOP_LEVEL_USED_AS_VALUE,
    UNKNOWN_IMPORTED_DECLARATION, UNKNOWN_MEMBER, UNKNOWN_MODULE_BINDING, UNKNOWN_NAME,
    UNKNOWN_QUALIFIED_DECLARATION, UNKNOWN_TYPE, UNSUPPORTED_MODULE_SYNTAX,
};

#[cfg(test)]
mod tests;
