//! Typed high-level intermediate representation.
//!
//! HIR retains source spans useful to diagnostics while replacing resolved
//! syntax with explicit typed operations and exact call targets.

mod dump;
mod ir;

pub use dump::dump_hir;
pub use ir::{
    BlockFlow, HirAccess, HirArrayAliasArgument, HirArrayAliasSource, HirArrayAnchor,
    HirArrayAssignElement, HirArrayAssignment, HirArrayConstruction, HirArrayConstructionMode,
    HirArrayCopyElement, HirArrayDefaultElement, HirArrayDestroyElement, HirArrayElementAssignment,
    HirArrayElementPlace, HirArrayElementValue, HirArrayEvaluationOrder, HirArrayFieldInitialize,
    HirArrayIndex, HirArrayIndexNormalization, HirArrayInitialize, HirArrayLength,
    HirArrayLifecycle, HirArrayOwnership, HirArrayPlace, HirArrayProvenance, HirArrayReceiver,
    HirArrayReceiverOwnership, HirArrayReceiverSource, HirArrayRuntimeFailure, HirArraySlice,
    HirArraySliceAssignment, HirArraySliceBounds, HirArraySource, HirArrayTransfer, HirArrayType,
    HirArrayTypeTable, HirBaseCopy, HirBaseInitialization, HirBinaryOperation, HirBlock,
    HirCallArgument, HirCallStatement, HirCallableSignature, HirCheckedObjectView,
    HirCheckedObjectViewKind, HirCheckedOptionalView, HirClassDeclaration,
    HirClassDeclarationTable, HirClassDefinition, HirClassDefinitionTable,
    HirClassOptionalAssignment, HirClassOptionalInitialize, HirClassOptionalPlace,
    HirClassOptionalSource, HirConditional, HirConditionalArm, HirConstruction,
    HirConstructionMode, HirCopyArgument, HirCopyAssignment, HirCopyAssignmentDeclaration,
    HirCopyCapability, HirCopyConstruction, HirCopyConstructorDeclaration, HirDestructionPlan,
    HirDestructionStep, HirDestructorDeclaration, HirDirectBase, HirExpression, HirExpressionKind,
    HirFieldAssignment, HirFieldConstruction, HirFieldCopyAssignment, HirFieldCopyConstruction,
    HirFieldDeclaration, HirFieldPlace, HirFunctionDeclaration, HirFunctionDeclarationTable,
    HirFunctionDefinition, HirFunctionDefinitionTable, HirFunctionLinkage,
    HirInitializerDeclaration, HirInterfaceCallTarget, HirInterfaceConformance,
    HirInterfaceDeclaration, HirInterfaceDeclarationTable, HirInterfaceParameter,
    HirInterfaceReceiver, HirInterfaceRequirement, HirLocal, HirLocalDecl, HirLocalInitializer,
    HirMemberDefinition, HirMethodCallTarget, HirMethodDeclaration, HirMethodDispatch,
    HirMethodKind, HirMethodReceiver, HirObjectCall, HirObjectCallTarget, HirObjectInitialization,
    HirObjectOrigin, HirObjectPath, HirObjectPlace, HirObjectProducer, HirObjectReturn,
    HirObjectSlice, HirObjectSource, HirObjectView, HirOptionalAliasPlace, HirOptionalAssignment,
    HirOptionalOperand, HirOptionalPlace, HirOptionalSharedAssignment, HirOptionalSharedInitialize,
    HirOptionalSharedPlace, HirOptionalSharedSource, HirOptionalSource, HirOptionalStorage,
    HirOptionalWriteKind, HirOwnerTransfer, HirParameter, HirParameterMode, HirPresenceTestKind,
    HirPrimitiveType, HirProgram, HirRequirementImplementation, HirReturn, HirReturnValue,
    HirSelectedCopyOperation, HirSharedAllocation, HirSharedAllocationMode, HirSharedAssignment,
    HirSharedCast, HirSharedCastKind, HirSharedFieldWrite, HirSharedFieldWriteKind, HirSharedPlace,
    HirSharedProducer, HirSharedSource, HirSharedTarget, HirSharedTransfer, HirStatement,
    HirSynthesizedCopy, HirSynthesizedFieldCopy, HirTypeTest, HirTypeTestKind, HirUnaryOperation,
    HirUserCopy, HirViewSource, HirViewTarget, HirVirtualFamily, HirVirtualFamilyTable,
    ObjectProjection, Type,
};
