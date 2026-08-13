//! Name-resolved, but not yet type-checked, program representation.

mod array_types;
mod body;
mod declarations;
mod expression;
mod generic_requirements;
mod generic_specializations;
mod generic_templates;
mod hierarchy;
mod modules;
mod object_place;
mod optional_box_types;
mod optional_types;
mod shared_targets;
mod strings;

pub use array_types::{ResolvedArrayType, ResolvedArrayTypeTable};
pub use body::{
    ResolvedArrayAssignment, ResolvedBaseInitialization, ResolvedBlock, ResolvedBreak,
    ResolvedClassDefinition, ResolvedClassDefinitionTable, ResolvedConditional,
    ResolvedConditionalArm, ResolvedContinue, ResolvedExpressionStatement, ResolvedFieldAssignment,
    ResolvedFunctionDefinition, ResolvedFunctionDefinitionTable, ResolvedLocalDecl,
    ResolvedMemberDefinition, ResolvedObjectAssignment, ResolvedOptionalAssignment,
    ResolvedPrimitiveBindingAssignment, ResolvedReturn, ResolvedSharedAssignment,
    ResolvedStatement, ResolvedStaticFieldAssignment, ResolvedWhile,
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
    ResolvedInterfaceCallExpr, ResolvedInterfaceReceiver, ResolvedLogicalExpr,
    ResolvedLogicalOperator, ResolvedMethodCallExpr, ResolvedNumericLiteralExpr,
    ResolvedObjectCastExpr, ResolvedObjectCastTargetMode, ResolvedOptionalBoxAllocationExpr,
    ResolvedOptionalBoxInitializer, ResolvedPresenceTestExpr, ResolvedPresenceTestKind,
    ResolvedPresentExpr, ResolvedPrimitiveCastExpr, ResolvedPrimitiveType, ResolvedStaticCallExpr,
    ResolvedStaticFieldAccessExpr, ResolvedStringLiteralExpr, ResolvedTypeTestExpr,
    ResolvedUnaryExpr, ResolvedUnaryOperator, ResolvedUnwrapExpr,
};
pub(crate) use generic_requirements::{
    GenericAliasAccess, GenericCapability, GenericRequirement, GenericRequirementReason,
};
pub(crate) use generic_specializations::{
    ClosedGenericRequirementSubject, GenericApplicationOrigin, GenericClassInstanceKey,
    GenericSpecialization, GenericSpecializationProvenance, GenericSpecializationState,
    GenericSpecializationTable, GenericSpecializationTransition,
};
pub use generic_templates::{
    ResolvedClassTemplate, ResolvedClassTemplateTable, ResolvedTypeParameter,
    ResolvedTypeParameterTable, ResolvedTypeParameters,
};
pub(crate) use generic_templates::{
    ResolvedClassTemplateSemanticTable, ResolvedClassTemplateSemantics, ResolvedTemplateBound,
    ResolvedTemplateConstructionMode, ResolvedTemplateDependentSelectionKind,
    ResolvedTemplateSelection, ResolvedTemplateType, ResolvedTemplateTypeKind,
    ResolvedTemplateTypeUse, ResolvedTemplateTypeUseContext,
};
pub use hierarchy::{ResolvedClassHierarchy, ResolvedClassMember};
pub use modules::{
    ResolvedModuleBinding, ResolvedModuleBindingTable, ResolvedModuleBindings,
    ResolvedModuleDeclaration, ResolvedModuleDeclarationTable, ResolvedModuleDeclarations,
    ResolvedOrdinaryBinding, ResolvedOrdinaryBindingTable, ResolvedOrdinaryBindings,
    ResolvedTopLevelId, ResolvedVisibility,
};
pub use object_place::{ResolvedObjectPlace, ResolvedObjectReceiver};
pub use optional_box_types::{ResolvedOptionalBoxType, ResolvedOptionalBoxTypeTable};
pub use optional_types::{ResolvedOptionalType, ResolvedOptionalTypeTable};
pub use shared_targets::{
    ResolvedObjectTarget, ResolvedSharedTarget, ResolvedSharedTargetCategory,
};
pub use strings::{ResolvedLiteralData, ResolvedLiteralDataTable, ResolvedStringLanguageItem};

pub(crate) use hierarchy::ResolvedClassHierarchyEntry;
