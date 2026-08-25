//! Declaration collection, lexical name resolution, and stable identity assignment.
//!
//! Resolution produces a separate representation with stable typed IDs. Later
//! phases never choose declarations by comparing source names.

mod dump;
mod ir;
mod resolver;

pub use dump::dump_resolved;
pub(crate) use ir::{ClosedGenericRequirementSubject, GenericSpecializationState};
pub use ir::{
    GenericAliasAccess, GenericCapability, GenericInterfaceApplicationOrigin,
    GenericInterfaceInstanceKey, GenericInterfaceRequirementMapping,
    GenericInterfaceSpecialization, GenericInterfaceSpecializationProvenance,
    GenericInterfaceSpecializationState, GenericInterfaceSpecializationTable,
    GenericInterfaceSpecializationTransition, GenericRequirement, GenericRequirementReason,
    ResolvedAbsentExpr, ResolvedAddressTakenCallable, ResolvedAddressTakenCallableTable,
    ResolvedAllocationExpr, ResolvedArrayAssignment, ResolvedArrayConstructionArguments,
    ResolvedArrayConstructionExpr, ResolvedArrayElementList, ResolvedArrayLengthExpr,
    ResolvedArrayLengthOperator, ResolvedArrayProjectionBounds, ResolvedArrayProjectionExpr,
    ResolvedArrayProjectionOperator, ResolvedArrayType, ResolvedArrayTypeTable,
    ResolvedBaseInitialization, ResolvedBinaryExpr, ResolvedBinaryOperator, ResolvedBindingExpr,
    ResolvedBlock, ResolvedBooleanExpr, ResolvedBreak, ResolvedByteLiteralExpr,
    ResolvedClassDeclaration, ResolvedClassDeclarationTable, ResolvedClassDefinition,
    ResolvedClassDefinitionTable, ResolvedClassHierarchy, ResolvedClassMember,
    ResolvedClassTemplate, ResolvedClassTemplateTable, ResolvedConditional, ResolvedConditionalArm,
    ResolvedConstructExpr, ResolvedConstructionMode, ResolvedContinue,
    ResolvedCopyAssignmentDeclaration, ResolvedCopyConstructorDeclaration, ResolvedCopyOperation,
    ResolvedDereferenceExpr, ResolvedDereferenceOperator, ResolvedDestructorDeclaration,
    ResolvedDirectBase, ResolvedDirectCallExpr, ResolvedExpression, ResolvedExpressionStatement,
    ResolvedFieldAccessExpr, ResolvedFieldAssignment, ResolvedFieldDeclaration, ResolvedForIn,
    ResolvedFunctionDeclaration, ResolvedFunctionDeclarationTable, ResolvedFunctionDefinition,
    ResolvedFunctionDefinitionTable, ResolvedFunctionLinkage, ResolvedFunctionReferenceExpr,
    ResolvedFunctionType, ResolvedFunctionTypeParameter, ResolvedFunctionTypeParameterMode,
    ResolvedFunctionTypeTable, ResolvedGroupedExpr, ResolvedIndirectCallExpr,
    ResolvedInitializerDeclaration, ResolvedInterfaceCallExpr, ResolvedInterfaceClaim,
    ResolvedInterfaceDeclaration, ResolvedInterfaceDeclarationTable, ResolvedInterfaceParameter,
    ResolvedInterfaceReceiver, ResolvedInterfaceRequirement, ResolvedInterfaceTemplate,
    ResolvedInterfaceTemplateBound, ResolvedInterfaceTemplateParameter,
    ResolvedInterfaceTemplateRequirement, ResolvedInterfaceTemplateRequirementSignature,
    ResolvedInterfaceTemplateSemanticTable, ResolvedInterfaceTemplateSemantics,
    ResolvedInterfaceTemplateTable, ResolvedInterfaceTemplateTypeUse,
    ResolvedInterfaceTemplateTypeUseContext, ResolvedInterfaceType, ResolvedIterableLanguageItem,
    ResolvedIterableSelection, ResolvedLiteralData, ResolvedLiteralDataTable, ResolvedLocal,
    ResolvedLocalDecl, ResolvedLogicalExpr, ResolvedLogicalOperator, ResolvedMemberDefinition,
    ResolvedMemberVisibility, ResolvedMethodCallExpr, ResolvedMethodDeclaration,
    ResolvedMethodDispatch, ResolvedMethodKind, ResolvedMethodModifier, ResolvedModuleBinding,
    ResolvedModuleBindingTable, ResolvedModuleBindings, ResolvedModuleDeclaration,
    ResolvedModuleDeclarationTable, ResolvedModuleDeclarations, ResolvedNumericLiteralExpr,
    ResolvedObjectAssignment, ResolvedObjectCastExpr, ResolvedObjectCastTargetMode,
    ResolvedObjectPlace, ResolvedObjectReceiver, ResolvedObjectTarget, ResolvedOptionalAssignment,
    ResolvedOptionalBoxAllocationExpr, ResolvedOptionalBoxInitializer, ResolvedOptionalBoxType,
    ResolvedOptionalBoxTypeTable, ResolvedOptionalType, ResolvedOptionalTypeTable,
    ResolvedOrdinaryBinding, ResolvedOrdinaryBindingTable, ResolvedOrdinaryBindings,
    ResolvedParameter, ResolvedParameterBindingMode, ResolvedPresenceTestExpr,
    ResolvedPresenceTestKind, ResolvedPrimitiveCastExpr, ResolvedPrimitiveType, ResolvedProgram,
    ResolvedReceiverAccess, ResolvedReturn, ResolvedScalarBindingAssignment,
    ResolvedSharedAssignment, ResolvedSharedTarget, ResolvedSharedTargetCategory,
    ResolvedStatement, ResolvedStaticCallExpr, ResolvedStaticFieldAccessExpr,
    ResolvedStaticFieldAssignment, ResolvedStaticFieldDeclaration, ResolvedStaticFieldInitializer,
    ResolvedStringLanguageItem, ResolvedStringLiteralExpr, ResolvedTemplateFunctionTypeParameter,
    ResolvedTemplateType, ResolvedTemplateTypeKind, ResolvedTopLevelId, ResolvedType,
    ResolvedTypeKind, ResolvedTypeParameter, ResolvedTypeParameterTable, ResolvedTypeParameters,
    ResolvedTypeTestExpr, ResolvedUnaryExpr, ResolvedUnaryOperator, ResolvedUnwrapExpr,
    ResolvedVirtualFamily, ResolvedVirtualFamilyTable, ResolvedVisibility, ResolvedWhile,
};
pub(crate) use resolver::resolve_with_source_path;
pub use resolver::{
    resolve, resolve_module_graph, ResolveOutput, AMBIGUOUS_GENERIC_BOUND_MEMBER,
    AMBIGUOUS_ITERABLE_APPLICATION, DUPLICATE_BINDING, DUPLICATE_GENERIC_BOUND, DUPLICATE_MEMBER,
    DUPLICATE_MODULE_BINDING, DUPLICATE_ORDINARY_BINDING, DUPLICATE_TOP_LEVEL,
    DUPLICATE_TYPE_PARAMETER, GENERIC_ARITY_MISMATCH, IMPLICIT_SHARED_DEREFERENCE,
    INCOMPATIBLE_EXTERNAL_ABI, INHERITANCE_CYCLE, INHERITED_MEMBER_COLLISION, INVALID_BASE_CLASS,
    INVALID_BASE_INITIALIZATION, INVALID_CALL_TARGET, INVALID_CONSTRUCTION_TARGET,
    INVALID_DEREFERENCE, INVALID_FUNCTION_REFERENCE, INVALID_GENERIC_APPLICATION,
    INVALID_GENERIC_BASE, INVALID_GENERIC_BOUND, INVALID_GENERIC_INTERFACE_REQUIREMENT,
    INVALID_INDEX_PROTOCOL, INVALID_INTERFACE_CLAIM, INVALID_INTRINSIC_DECLARATION,
    INVALID_ITERABLE_LANGUAGE_ITEM, INVALID_LIFECYCLE_SIGNATURE, INVALID_MEMBER_SELECTION,
    INVALID_OPTIONAL_TYPE, INVALID_OVERRIDE, INVALID_POINTEE_ASSIGNMENT,
    INVALID_STRING_LANGUAGE_ITEM, ITERATION_ITEM_TYPE_MISMATCH, LOOP_EXIT_OUTSIDE_LOOP,
    MISSING_ITERABLE_APPLICATION, MISSING_STRING_LANGUAGE_ITEM, MODULE_CONTEXT_REQUIRED,
    NON_TERMINATING_GENERIC_SPECIALIZATION, PRIVATE_DECLARATION, PRIVATE_MEMBER_ACCESS,
    RAW_GENERIC_TYPE, SELF_OUTSIDE_MEMBER, TOP_LEVEL_USED_AS_VALUE,
    UNCONSTRAINED_TYPE_PARAMETER_MEMBER, UNKNOWN_IMPORTED_DECLARATION, UNKNOWN_MEMBER,
    UNKNOWN_MODULE_BINDING, UNKNOWN_NAME, UNKNOWN_QUALIFIED_DECLARATION, UNKNOWN_TYPE,
    UNSATISFIED_GENERIC_REQUIREMENT, UNSUPPORTED_PARAMETER_CONSTRUCTION,
};

#[cfg(test)]
mod tests;
