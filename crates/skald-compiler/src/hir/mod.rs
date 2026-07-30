//! Typed high-level intermediate representation.
//!
//! HIR retains source spans useful to diagnostics while replacing resolved
//! syntax with explicit typed operations and exact call targets.

mod dump;
mod ir;

#[cfg(test)]
mod tests;

pub use dump::dump_hir;
pub use ir::{
    HirAccess, HirArrayAliasArgument, HirArrayAliasSource, HirArrayAnchor, HirArrayAssignElement,
    HirArrayAssignment, HirArrayConstruction, HirArrayConstructionMode, HirArrayCopyElement,
    HirArrayDefaultElement, HirArrayDestroyElement, HirArrayElementAssignment,
    HirArrayElementPlace, HirArrayElementValue, HirArrayEvaluationOrder, HirArrayFieldInitialize,
    HirArrayIndex, HirArrayIndexNormalization, HirArrayInitialize, HirArrayLength,
    HirArrayLifecycle, HirArrayOwnership, HirArrayPlace, HirArrayProvenance, HirArrayReceiver,
    HirArrayReceiverOwnership, HirArrayReceiverSource, HirArrayRuntimeFailure, HirArraySlice,
    HirArraySliceAssignment, HirArraySliceBounds, HirArraySource, HirArrayTransfer, HirArrayType,
    HirArrayTypeTable, HirBaseCopy, HirBaseInitialization, HirBinaryOperation, HirBlock, HirBreak,
    HirCallArgument, HirCallStatement, HirCallableSignature, HirCheckedObjectView,
    HirCheckedObjectViewKind, HirCheckedOptionalView, HirClassDeclaration,
    HirClassDeclarationTable, HirClassDefinition, HirClassDefinitionTable,
    HirClassOptionalAssignment, HirClassOptionalInitialize, HirClassOptionalPlace,
    HirClassOptionalSource, HirComparisonOperand, HirComparisonPredicate, HirConditional,
    HirConditionalArm, HirConstruction, HirConstructionMode, HirContinue, HirControlEffects,
    HirCopyArgument, HirCopyAssignment, HirCopyAssignmentDeclaration, HirCopyCapability,
    HirCopyConstruction, HirCopyConstructorDeclaration, HirDestructionPlan, HirDestructionStep,
    HirDestructorDeclaration, HirDirectBase, HirExpression, HirExpressionKind, HirFieldAssignment,
    HirFieldConstruction, HirFieldCopyAssignment, HirFieldCopyConstruction, HirFieldDeclaration,
    HirFieldPlace, HirFunctionDeclaration, HirFunctionDeclarationTable, HirFunctionDefinition,
    HirFunctionDefinitionTable, HirFunctionLinkage, HirInitializerDeclaration, HirIntegerCast,
    HirIntegerType, HirInterfaceCallTarget, HirInterfaceConformance, HirInterfaceDeclaration,
    HirInterfaceDeclarationTable, HirInterfaceParameter, HirInterfaceReceiver,
    HirInterfaceRequirement, HirLiteralData, HirLiteralDataTable, HirLocal, HirLocalDecl,
    HirLocalInitializer, HirLogicalExpression, HirLogicalOperation, HirMemberDefinition,
    HirMethodCallTarget, HirMethodDeclaration, HirMethodDispatch, HirMethodKind, HirMethodReceiver,
    HirObjectCall, HirObjectCallTarget, HirObjectInitialization, HirObjectOrigin, HirObjectPath,
    HirObjectPlace, HirObjectProducer, HirObjectReturn, HirObjectSlice, HirObjectSource,
    HirObjectView, HirOptionalAliasPlace, HirOptionalAssignment, HirOptionalOperand,
    HirOptionalPlace, HirOptionalSharedAssignment, HirOptionalSharedInitialize,
    HirOptionalSharedPlace, HirOptionalSharedSource, HirOptionalSource, HirOptionalStorage,
    HirOptionalWriteKind, HirOwnerTransfer, HirPanic, HirParameter, HirParameterMode,
    HirPresenceTestKind, HirPrimitiveBindingAssignment, HirPrimitiveComparison, HirPrimitiveType,
    HirProgram, HirRequirementImplementation, HirReturn, HirReturnValue, HirSelectedCopyOperation,
    HirSharedAllocation, HirSharedAllocationMode, HirSharedAssignment, HirSharedCast,
    HirSharedCastKind, HirSharedFieldWrite, HirSharedFieldWriteKind, HirSharedPlace,
    HirSharedProducer, HirSharedSource, HirSharedTarget, HirSharedTransfer, HirStatement,
    HirStringLanguageItem, HirStringLiteral, HirSynthesizedCopy, HirSynthesizedFieldCopy,
    HirTypeTest, HirTypeTestKind, HirUnaryOperation, HirUserCopy, HirViewSource, HirViewTarget,
    HirVirtualFamily, HirVirtualFamilyTable, HirWhile, ObjectProjection, Type,
};
