//! Name-resolved, but not yet type-checked, program representation.

mod array_types;
mod body;
mod declarations;
mod expression;
mod function_references;
mod function_types;
mod generic_interface_specializations;
mod generic_requirements;
mod generic_specializations;
mod generic_templates;
mod hierarchy;
mod interface_receiver;
mod iteration;
mod modules;
mod object_place;
mod operator_language_item;
mod operator_selection;
mod optional_box_types;
mod optional_types;
mod primitive_bound_realization;
mod primitive_operator_evidence;
mod primitive_successor_evidence;
mod range_language_item;
mod shared_targets;
mod strings;
mod type_names;

pub use array_types::{ResolvedArrayType, ResolvedArrayTypeTable};
pub use body::{
    ResolvedArrayAssignment, ResolvedBaseInitialization, ResolvedBlock, ResolvedBreak,
    ResolvedClassDefinition, ResolvedClassDefinitionTable, ResolvedConditional,
    ResolvedConditionalArm, ResolvedContinue, ResolvedExpressionStatement, ResolvedFieldAssignment,
    ResolvedForIn, ResolvedFunctionDefinition, ResolvedFunctionDefinitionTable,
    ResolvedIterableSelection, ResolvedLocalDecl, ResolvedMemberDefinition,
    ResolvedObjectAssignment, ResolvedOptionalAssignment, ResolvedReturn,
    ResolvedScalarBindingAssignment, ResolvedSharedAssignment, ResolvedStatement,
    ResolvedStaticFieldAssignment, ResolvedWhile,
};
pub use declarations::{
    ResolvedClassDeclaration, ResolvedClassDeclarationTable, ResolvedCopyAssignmentDeclaration,
    ResolvedCopyConstructorDeclaration, ResolvedCopyOperation, ResolvedDestructorDeclaration,
    ResolvedDirectBase, ResolvedFieldDeclaration, ResolvedFunctionDeclaration,
    ResolvedFunctionDeclarationTable, ResolvedFunctionLinkage, ResolvedInitializerDeclaration,
    ResolvedInterfaceClaim, ResolvedInterfaceDeclaration, ResolvedInterfaceDeclarationTable,
    ResolvedInterfaceParameter, ResolvedInterfaceRequirement, ResolvedLocal,
    ResolvedMemberVisibility, ResolvedMethodDeclaration, ResolvedMethodDispatch,
    ResolvedMethodKind, ResolvedMethodModifier, ResolvedParameter, ResolvedParameterBindingMode,
    ResolvedProgram, ResolvedReceiverAccess, ResolvedStaticFieldDeclaration,
    ResolvedStaticFieldInitializer, ResolvedType, ResolvedTypeKind, ResolvedVirtualFamily,
    ResolvedVirtualFamilyTable,
};
pub use expression::{
    ResolvedAbsentExpr, ResolvedAllocationExpr, ResolvedArrayConstructionArguments,
    ResolvedArrayConstructionExpr, ResolvedArrayElementList, ResolvedArrayLengthExpr,
    ResolvedArrayLengthOperator, ResolvedArrayProjectionBounds, ResolvedArrayProjectionExpr,
    ResolvedArrayProjectionOperator, ResolvedBinaryExpr, ResolvedBinaryOperator,
    ResolvedBindingExpr, ResolvedBooleanExpr, ResolvedByteLiteralExpr, ResolvedConstructExpr,
    ResolvedConstructionMode, ResolvedDereferenceExpr, ResolvedDereferenceOperator,
    ResolvedDirectCallExpr, ResolvedExpression, ResolvedFieldAccessExpr, ResolvedGroupedExpr,
    ResolvedIndirectCallExpr, ResolvedInterfaceCallExpr, ResolvedInterfaceReceiver,
    ResolvedLogicalExpr, ResolvedLogicalOperator, ResolvedMethodCallExpr,
    ResolvedNumericLiteralExpr, ResolvedObjectCastExpr, ResolvedObjectCastTargetMode,
    ResolvedOptionalBoxAllocationExpr, ResolvedOptionalBoxInitializer, ResolvedPresenceTestExpr,
    ResolvedPresenceTestKind, ResolvedPresentExpr, ResolvedPrimitiveCastExpr,
    ResolvedPrimitiveType, ResolvedRangeExpr, ResolvedRangeProtocolEvidence,
    ResolvedRangeProtocolRealization, ResolvedStaticCallExpr, ResolvedStaticFieldAccessExpr,
    ResolvedStringLiteralExpr, ResolvedTypeTestExpr, ResolvedUnaryExpr, ResolvedUnaryOperator,
    ResolvedUnwrapExpr,
};
pub use function_references::{
    ResolvedAddressTakenCallable, ResolvedAddressTakenCallableTable, ResolvedFunctionReferenceExpr,
};
pub use function_types::{
    ResolvedFunctionType, ResolvedFunctionTypeParameter, ResolvedFunctionTypeParameterMode,
    ResolvedFunctionTypeTable,
};
pub use generic_interface_specializations::{
    GenericInterfaceApplicationOrigin, GenericInterfaceInstanceKey,
    GenericInterfaceRequirementMapping, GenericInterfaceSpecialization,
    GenericInterfaceSpecializationProvenance, GenericInterfaceSpecializationState,
    GenericInterfaceSpecializationTable, GenericInterfaceSpecializationTransition,
};
pub use generic_requirements::{
    GenericAliasAccess, GenericCapability, GenericRequirement, GenericRequirementReason,
};
pub(crate) use generic_specializations::{
    ClosedGenericBoundMember, ClosedGenericIterationSelection, ClosedGenericOperatorSelection,
    ClosedGenericRequirementSubject, GenericApplicationOrigin, GenericClassInstanceKey,
    GenericSpecialization, GenericSpecializationKey, GenericSpecializationProvenance,
    GenericSpecializationState, GenericSpecializationTable, GenericSpecializationTransition,
};
pub use generic_templates::{
    ResolvedClassTemplate, ResolvedClassTemplateTable, ResolvedInterfaceTemplate,
    ResolvedInterfaceTemplateBound, ResolvedInterfaceTemplateParameter,
    ResolvedInterfaceTemplateRequirement, ResolvedInterfaceTemplateRequirementSignature,
    ResolvedInterfaceTemplateSemanticTable, ResolvedInterfaceTemplateSemantics,
    ResolvedInterfaceTemplateTable, ResolvedInterfaceTemplateTypeUse,
    ResolvedInterfaceTemplateTypeUseContext, ResolvedInterfaceType,
    ResolvedTemplateFunctionTypeParameter, ResolvedTemplateType, ResolvedTemplateTypeKind,
    ResolvedTypeParameter, ResolvedTypeParameterTable, ResolvedTypeParameters,
};
pub(crate) use generic_templates::{
    ResolvedClassTemplateSemanticTable, ResolvedClassTemplateSemantics, ResolvedTemplateBound,
    ResolvedTemplateBoundRequirement, ResolvedTemplateConstructionMode,
    ResolvedTemplateDependentSelectionKind, ResolvedTemplateOperatorSelection,
    ResolvedTemplateOperatorSyntax, ResolvedTemplateSelection, ResolvedTemplateTypeUse,
    ResolvedTemplateTypeUseContext,
};
pub use hierarchy::{ResolvedClassHierarchy, ResolvedClassMember};
pub use iteration::ResolvedIterableLanguageItem;
pub use modules::{
    ResolvedModuleBinding, ResolvedModuleBindingTable, ResolvedModuleBindings,
    ResolvedModuleDeclaration, ResolvedModuleDeclarationTable, ResolvedModuleDeclarations,
    ResolvedOrdinaryBinding, ResolvedOrdinaryBindingTable, ResolvedOrdinaryBindings,
    ResolvedTopLevelId, ResolvedVisibility,
};
pub use object_place::{ResolvedObjectPlace, ResolvedObjectReceiver};
pub use operator_language_item::{
    CanonicalOperatorProtocol, CanonicalOperatorProtocolShape, ResolvedOperatorLanguageItem,
    ResolvedOperatorProtocol, ResolvedOperatorProtocolParameters,
};
pub use operator_selection::{ResolvedOperatorResolution, ResolvedOperatorSelection};
pub use optional_box_types::{ResolvedOptionalBoxType, ResolvedOptionalBoxTypeTable};
pub use optional_types::{ResolvedOptionalType, ResolvedOptionalTypeTable};
pub(crate) use primitive_bound_realization::ResolvedPrimitiveBoundOperation;
pub(crate) use primitive_operator_evidence::{
    canonical_operator_application, primitive_operator_evidence, primitive_operator_operation,
    primitive_operator_registry, ResolvedPrimitiveOperatorOperation,
};
pub(crate) use primitive_successor_evidence::{
    canonical_successor_application, primitive_successor_evidence, primitive_successor_operation,
    primitive_successor_registry,
};
pub use range_language_item::ResolvedRangeLanguageItem;
pub use shared_targets::{
    ResolvedObjectTarget, ResolvedSharedTarget, ResolvedSharedTargetCategory,
};
pub use strings::{ResolvedLiteralData, ResolvedLiteralDataTable, ResolvedStringLanguageItem};

pub(crate) use hierarchy::ResolvedClassHierarchyEntry;
pub(crate) use type_names::{ResolvedTypeNameContext, ResolvedTypeNameRenderer};
