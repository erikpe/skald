//! Target-independent mid-level IR.
//!
//! MIR makes storage, evaluation order, temporaries, calls, and control-flow
//! termination explicit. It is not SSA, but value and block identities leave a
//! clean path to SSA conversion later.

mod build;
mod dump;
mod lower;
mod model;
mod verify;

#[cfg(test)]
pub(crate) mod test_fixtures;

pub use dump::{dump_mir, dump_preliminary_mir};
pub use lower::{lower_hir, lower_preliminary_hir};
pub use model::{
    BlockId, MirAliasAccess, MirArgument, MirArrayAnchorKind, MirArrayAssignElement,
    MirArrayBoundary, MirArrayCopyElement, MirArrayDefaultElement, MirArrayDestroyElement,
    MirArrayFailure, MirArrayInstruction, MirArrayLifecycle, MirArrayOwnership,
    MirArrayPositionKind, MirArrayType, MirArrayTypeTable, MirAssignment, MirBaseCopy,
    MirBasicBlock, MirBinaryOperation, MirBody, MirCall, MirCallReceiver, MirCallTarget,
    MirCallableSignature, MirCheckedViewBinding, MirCheckedViewEnd, MirClassDeclaration,
    MirClassDeclarationTable, MirClassOptionalAssign, MirClassOptionalCleanup,
    MirClassOptionalInitialize, MirClassOptionalPublish, MirClassOptionalSource, MirCleanup,
    MirComparisonOperand, MirComparisonPredicate, MirCopyAssignment, MirCopyAssignmentDeclaration,
    MirCopyCapability, MirCopyConstruction, MirCopyConstructorDeclaration, MirDefinitionRef,
    MirDestructionPlan, MirDestructionStep, MirDestructorDeclaration, MirDirectBase,
    MirEndFullExpression, MirF64ToIntegerRange, MirF64ToIntegerRounding, MirFieldDeclaration,
    MirFunctionDeclaration, MirFunctionDeclarationTable, MirFunctionDefinition,
    MirFunctionDefinitionTable, MirFunctionLinkage, MirInitialize, MirInitializerDeclaration,
    MirInstruction, MirIntegerBitwiseOperation, MirIntegerDivisionKind,
    MirIntegerDivisionOperation, MirIntegerDivisorCheck, MirIntegerType, MirInterfaceCallTarget,
    MirInterfaceConformance, MirInterfaceDeclaration, MirInterfaceDeclarationTable,
    MirInterfaceRequirement, MirIoBuffer, MirIoInstruction, MirIoOperation, MirLiteralData,
    MirLiteralDataTable, MirLogicalExpression, MirLogicalOperation, MirMemberDefinition,
    MirMemberDefinitionTable, MirMethodCallTarget, MirMethodDeclaration, MirMethodKind,
    MirMethodReceiver, MirNestedOptionalAssign, MirNestedOptionalCleanup,
    MirNestedOptionalInitialize, MirNestedOptionalPublish, MirNestedOptionalSource,
    MirObjectOrigin, MirObjectView, MirOptionalAssign, MirOptionalAssignmentPlan,
    MirOptionalBoundaryPlan, MirOptionalBoundaryPlans, MirOptionalCheckedAccess,
    MirOptionalCleanupPlan, MirOptionalCopyPlan, MirOptionalInitializationPlan,
    MirOptionalInitialize, MirOptionalInjectionPlan, MirOptionalLifecycle, MirOptionalPresencePlan,
    MirOptionalRepresentation, MirOptionalSharedAssign, MirOptionalSharedCleanup,
    MirOptionalSharedInitialize, MirOptionalSharedSource, MirOptionalSharedUnwrap,
    MirOptionalSource, MirOptionalStorage, MirOptionalType, MirOptionalTypeTable,
    MirOptionalUnwrapPlan, MirOptionalViewBegin, MirOptionalViewEnd, MirParameter,
    MirParameterMode, MirPathCondition, MirPathConditionValue, MirPlace, MirPlaceBase,
    MirPlaceProjection, MirPresenceTestKind, MirPrimitiveCast, MirPrimitiveCastKind,
    MirPrimitiveCastRangeCheck, MirPrimitiveComparison, MirPrimitiveType, MirProgram,
    MirProgramLifecycle, MirReceiverAccess, MirRequirementImplementation, MirRightShiftFlavor,
    MirRvalue, MirRvalueKind, MirSelectedCopyOperation, MirSharedAdopt, MirSharedAllocate,
    MirSharedAllocationMode, MirSharedAllocationOrigin, MirSharedCast, MirSharedCastSource,
    MirSharedCastTransfer, MirSharedCopy, MirSharedFieldCopy, MirSharedFieldInitialize,
    MirSharedFieldReplace, MirSharedInitialize, MirSharedMove, MirSharedPublish, MirSharedRelease,
    MirSharedStatic, MirSharedTarget, MirShiftCountCheck, MirShiftDirection, MirShiftOperation,
    MirSignedIntegerDivisionSemantics, MirSignedMinimumPairResult, MirSignedQuotientRounding,
    MirSignedRemainderSign, MirStaticActivationRegion, MirStaticActivationWork,
    MirStaticAllocationOrigin, MirStaticDataMutability, MirStaticDestructionRegion,
    MirStaticFieldInitialization, MirStaticInitializerBody, MirStaticLifecycleCertificate,
    MirStaticLifecycleCoordinator, MirStaticLifecycleDefinition, MirStaticLifecycleIndices,
    MirStaticLifecycleTransition, MirStaticLifecycleTransitionKind, MirStaticPublication,
    MirStaticSharedCleanup, MirStaticValueCleanup, MirStorage, MirStorageDead, MirStorageKind,
    MirStorageLive, MirStore, MirStringInitialize, MirStringLanguageItem, MirSynthesizedCopy,
    MirSynthesizedFieldCopy, MirTerminationReason, MirTerminator, MirType, MirUnaryOperation,
    MirUserCopy, MirValue, MirViewTarget, MirVirtualFamily, MirVirtualFamilyTable, OptionalGuardId,
    PathConditionId, PlannedMirProgram, PreliminaryMirProgram, PreliminaryMirSharedLifecycleTarget,
    PreliminaryMirStaticField, PreliminaryMirStaticInitializer, StaticAccessEvidence,
    StaticAccessKind, StaticArrayLifecycleOperation, StaticClassLifecycleOperation,
    StaticEffectAnalysis, StaticEffectEdge, StaticEffectEdgeKind, StaticEffectNode,
    StaticEffectPhase, StaticEffectSummary, StaticLifecyclePlan, StaticLifetimeDependency,
    StaticLifetimeEvidence, StaticLifetimePhase, StorageId, ValueId,
};
pub(crate) use verify::preliminary::{
    destination_completed_on_every_publication_path, reachable_static_initializer_blocks,
};
pub use verify::{verify_mir, verify_preliminary_mir, MirVerificationError, MirVerificationErrors};

#[cfg(test)]
mod tests;
