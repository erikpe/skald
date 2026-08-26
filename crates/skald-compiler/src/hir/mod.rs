//! Typed high-level intermediate representation.
//!
//! HIR retains source spans useful to diagnostics while replacing resolved
//! syntax with explicit typed operations and exact call targets.

#[cfg(test)]
mod cell_writes;
mod dump;
mod ir;

#[cfg(test)]
mod tests;

#[cfg(test)]
pub(crate) use cell_writes::collect_cell_writes;
pub use dump::dump_hir;
pub use ir::{
    HirAccess, HirAggregateOptionalAssignment, HirArrayAliasArgument, HirArrayAliasSource,
    HirArrayAnchor, HirArrayAssignElement, HirArrayAssignment, HirArrayConstruction,
    HirArrayConstructionMode, HirArrayCopyElement, HirArrayDefaultElement, HirArrayDestroyElement,
    HirArrayElementAssignment, HirArrayElementInitialization, HirArrayElementList,
    HirArrayElementPlace, HirArrayElementValue, HirArrayEvaluationOrder, HirArrayFieldInitialize,
    HirArrayIndex, HirArrayIndexNormalization, HirArrayInitialize, HirArrayLength,
    HirArrayLifecycle, HirArrayOwnership, HirArrayPlace, HirArrayProvenance, HirArrayReceiver,
    HirArrayReceiverOwnership, HirArrayReceiverSource, HirArrayRuntimeFailure, HirArraySlice,
    HirArraySliceAssignment, HirArraySliceBounds, HirArraySource, HirArrayTransfer, HirArrayType,
    HirArrayTypeTable, HirBaseCopy, HirBaseInitialization, HirBinaryOperation, HirBlock, HirBreak,
    HirCallArgument, HirCallStatement, HirCallableSignature, HirCheckedIntegerDivision,
    HirCheckedObjectView, HirCheckedObjectViewKind, HirCheckedOptionalView, HirCheckedShift,
    HirClassDeclaration, HirClassDeclarationTable, HirClassDefinition, HirClassDefinitionTable,
    HirClassOptionalAssignment, HirClassOptionalDestinationInitialization,
    HirClassOptionalInitialize, HirClassOptionalPlace, HirClassOptionalSource,
    HirComparisonOperand, HirComparisonPredicate, HirConditional, HirConditionalArm,
    HirConstruction, HirConstructionMode, HirContinue, HirControlEffects, HirCopyArgument,
    HirCopyAssignment, HirCopyAssignmentDeclaration, HirCopyCapability, HirCopyConstruction,
    HirCopyConstructorDeclaration, HirDestructionPlan, HirDestructionStep,
    HirDestructorDeclaration, HirDirectBase, HirExpression, HirExpressionKind, HirFieldAssignment,
    HirFieldConstruction, HirFieldCopyAssignment, HirFieldCopyConstruction, HirFieldDeclaration,
    HirFieldPlace, HirFieldWriteAuthorization, HirForIn, HirFunctionDeclaration,
    HirFunctionDeclarationTable, HirFunctionDefinition, HirFunctionDefinitionTable,
    HirFunctionLinkage, HirFunctionReference, HirFunctionType, HirFunctionTypeParameter,
    HirFunctionTypeParameterMode, HirFunctionTypeTable, HirIndirectCall, HirInitializerDeclaration,
    HirIntegerBitwiseOperation, HirIntegerDivisionFailure, HirIntegerDivisionKind,
    HirIntegerDivisionOperation, HirIntegerType, HirInterfaceCallTarget, HirInterfaceConformance,
    HirInterfaceDeclaration, HirInterfaceDeclarationTable, HirInterfaceParameter,
    HirInterfaceReceiver, HirInterfaceRequirement, HirIoOperation, HirIterationCallTarget,
    HirIterationItemPlan, HirIterationNextCallPlan, HirIterationProtocol, HirIterationReceiver,
    HirIterationReceiverCarrier, HirIterationReceiverLifetime, HirIterationResultPlan,
    HirIterationSpans, HirIterationStateAlias, HirIterationStateCallPlan, HirIterationStatePlan,
    HirIterationStoredValuePlan, HirIterationValueCopy, HirIterationValueDestruction,
    HirLiteralData, HirLiteralDataTable, HirLocal, HirLocalDecl, HirLocalInitializer,
    HirLogicalExpression, HirLogicalOperation, HirMemberDefinition, HirMethodCallTarget,
    HirMethodDeclaration, HirMethodDispatch, HirMethodKind, HirMethodReceiver,
    HirNestedOptionalUnwrap, HirObjectCall, HirObjectCallTarget,
    HirObjectDestinationInitialization, HirObjectInitialization, HirObjectOrigin, HirObjectPath,
    HirObjectPlace, HirObjectProducer, HirObjectReceiver, HirObjectReturn, HirObjectSlice,
    HirObjectSource, HirObjectView, HirOptionalAliasPlace, HirOptionalArrayUnwrap,
    HirOptionalAssignment, HirOptionalAssignmentPlan, HirOptionalBoundaryPlan,
    HirOptionalBoundaryPlans, HirOptionalBoxAllocation, HirOptionalBoxEvaluationOrder,
    HirOptionalBoxObjectView, HirOptionalBoxPointee, HirOptionalBoxPresence, HirOptionalBoxType,
    HirOptionalBoxTypeTable, HirOptionalCheckedAccess, HirOptionalCopyPlan,
    HirOptionalDestructionPlan, HirOptionalInitializationPlan, HirOptionalInjectionPlan,
    HirOptionalLifecycle, HirOptionalOperand, HirOptionalPlace, HirOptionalPresenceTestPlan,
    HirOptionalRepresentation, HirOptionalSharedAssignment, HirOptionalSharedInitialize,
    HirOptionalSharedPlace, HirOptionalSharedSource, HirOptionalSource, HirOptionalStorage,
    HirOptionalStorageCategory, HirOptionalType, HirOptionalTypeTable, HirOptionalUnwrapPlan,
    HirOptionalValue, HirOptionalValuePlace, HirOptionalValueSource, HirOptionalWriteKind,
    HirOwnerTransfer, HirPanic, HirParameter, HirParameterMode, HirPresenceTestKind,
    HirPrimitiveCast, HirPrimitiveCastKind, HirPrimitiveComparison, HirPrimitivePlace,
    HirPrimitiveStorage, HirPrimitiveType, HirProgram, HirRequirementImplementation, HirReturn,
    HirReturnValue, HirRightShiftFlavor, HirScalarAssignment, HirScalarPlace, HirScalarStorage,
    HirSelectedCopyOperation, HirSharedAllocation, HirSharedAllocationMode, HirSharedAssignment,
    HirSharedCast, HirSharedCastKind, HirSharedFieldWrite, HirSharedFieldWriteKind, HirSharedPlace,
    HirSharedProducer, HirSharedSource, HirSharedStaticAssignment, HirSharedTarget,
    HirSharedTransfer, HirShiftDirection, HirShiftFailure, HirShiftOperation,
    HirSignedIntegerDivisionSemantics, HirSignedMinimumPairResult, HirSignedQuotientRounding,
    HirSignedRemainderSign, HirStatement, HirStaticFieldDeclaration, HirStaticFieldInitializer,
    HirStaticPlace, HirStoredValueInitialization, HirStringLanguageItem, HirStringLiteral,
    HirSynthesizedCopy, HirSynthesizedFieldCopy, HirTypeTest, HirTypeTestKind, HirUnaryOperation,
    HirUserCopy, HirViewSource, HirViewTarget, HirVirtualFamily, HirVirtualFamilyTable, HirWhile,
    ObjectProjection, Type,
};
