//! Target-independent mid-level IR.
//!
//! MIR makes storage, evaluation order, temporaries, calls, and control-flow
//! termination explicit. It is not SSA, but value and block identities leave a
//! clean path to SSA conversion later.

mod build;
mod dump;
mod lower;
mod model;
// Program-level executable-definition retention is deliberately separate
// from callable-local identity rewriting. The final-MIR pass capability is
// the only production caller permitted to consume its prepared plan.
pub(crate) mod retain;
// The final-MIR pass runner is the only production owner permitted to consume
// this atomic rewrite boundary.
#[allow(dead_code, unused_imports)]
pub(crate) mod rewrite;
mod verify;

#[cfg(test)]
pub(crate) mod test_fixtures;

pub use dump::{dump_mir, dump_preliminary_mir};
pub use lower::{lower_hir, lower_preliminary_hir};
pub(crate) use model::mir_execution_node_key;
pub(crate) use model::MirProgramStatistics;
pub use model::{
    BlockId, MirAggregateOptionalAssign, MirAggregateOptionalCleanup,
    MirAggregateOptionalInitialize, MirAggregateOptionalPublish, MirAggregateOptionalSource,
    MirAliasAccess, MirArgument, MirArrayAnchorKind, MirArrayAssignElement, MirArrayBoundary,
    MirArrayCopyElement, MirArrayDefaultElement, MirArrayDestroyElement, MirArrayFailure,
    MirArrayInstruction, MirArrayLifecycle, MirArrayLifecycleOperation, MirArrayOwnership,
    MirArrayPositionKind, MirArrayType, MirArrayTypeTable, MirAssignment, MirBaseCopy,
    MirBasicBlock, MirBinaryOperation, MirBody, MirCall, MirCallReceiver, MirCallTarget,
    MirCallableAddress, MirCallableSignature, MirCellWriteAuthorization, MirCheckedViewBinding,
    MirCheckedViewEnd, MirClassDeclaration, MirClassDeclarationTable, MirClassLifecycleOperation,
    MirClassOptionalAssign, MirClassOptionalCleanup, MirClassOptionalInitialize,
    MirClassOptionalPublish, MirClassOptionalSource, MirCleanup, MirComparisonOperand,
    MirComparisonPredicate, MirCopyAssignment, MirCopyAssignmentDeclaration, MirCopyCapability,
    MirCopyConstruction, MirCopyConstructorDeclaration, MirDefinitionRef, MirDestructionPlan,
    MirDestructionStep, MirDestructorDeclaration, MirDirectBase, MirEndFullExpression,
    MirExecutionNode, MirF64ToIntegerRange, MirF64ToIntegerRounding, MirFieldDeclaration,
    MirFinalWriteAuthorization, MirFunctionDeclaration, MirFunctionDeclarationTable,
    MirFunctionDefinition, MirFunctionDefinitionTable, MirFunctionLinkage, MirFunctionType,
    MirFunctionTypeTable, MirIndirectCallTarget, MirInitialize, MirInitializerDeclaration,
    MirInstruction, MirIntegerBitwiseOperation, MirIntegerDivisionKind,
    MirIntegerDivisionOperation, MirIntegerDivisorCheck, MirIntegerType, MirInterfaceCallTarget,
    MirInterfaceConformance, MirInterfaceDeclaration, MirInterfaceDeclarationTable,
    MirInterfaceRequirement, MirIoBuffer, MirIoInstruction, MirIoOperation, MirLiteralData,
    MirLiteralDataTable, MirLogicalExpression, MirLogicalOperation, MirMemberDefinition,
    MirMemberDefinitionTable, MirMethodCallTarget, MirMethodDeclaration, MirMethodKind,
    MirMethodReceiver, MirObjectOrigin, MirObjectView, MirOptionalAssign,
    MirOptionalAssignmentPlan, MirOptionalBoundaryPlan, MirOptionalBoundaryPlans,
    MirOptionalBoxCompletion, MirOptionalBoxType, MirOptionalBoxTypeTable, MirOptionalBoxViewBegin,
    MirOptionalBoxViewEnd, MirOptionalCheckedAccess, MirOptionalCleanupPlan, MirOptionalCopyPlan,
    MirOptionalInitializationPlan, MirOptionalInitialize, MirOptionalInjectionPlan,
    MirOptionalLifecycle, MirOptionalPresencePlan, MirOptionalRepresentation,
    MirOptionalSharedAssign, MirOptionalSharedCleanup, MirOptionalSharedInitialize,
    MirOptionalSharedSource, MirOptionalSharedUnwrap, MirOptionalSource, MirOptionalStorage,
    MirOptionalType, MirOptionalTypeTable, MirOptionalUnwrapPlan, MirOptionalViewBegin,
    MirOptionalViewEnd, MirParameter, MirParameterMode, MirPathCondition, MirPathConditionValue,
    MirPlace, MirPlaceBase, MirPlaceProjection, MirPlannedLifecycle, MirPresenceTestKind,
    MirPrimitiveCast, MirPrimitiveCastKind, MirPrimitiveCastRangeCheck, MirPrimitiveComparison,
    MirPrimitiveType, MirProgram, MirProgramLifecycle, MirReceiverAccess,
    MirRequirementImplementation, MirRightShiftFlavor, MirRvalue, MirRvalueKind,
    MirSelectedCopyOperation, MirSharedAdopt, MirSharedAllocate, MirSharedAllocationMode,
    MirSharedAllocationOrigin, MirSharedAllocationTarget, MirSharedCast, MirSharedCastSource,
    MirSharedCastTransfer, MirSharedCopy, MirSharedFieldCopy, MirSharedFieldInitialize,
    MirSharedFieldReplace, MirSharedInitialize, MirSharedMove, MirSharedPublish, MirSharedRelease,
    MirSharedStatic, MirSharedTarget, MirShiftCountCheck, MirShiftDirection, MirShiftOperation,
    MirSignedIntegerDivisionSemantics, MirSignedMinimumPairResult, MirSignedQuotientRounding,
    MirSignedRemainderSign, MirStaticActivationRegion, MirStaticActivationWork,
    MirStaticAllocationOrigin, MirStaticDataMutability, MirStaticDestructionRegion,
    MirStaticFieldInitialization, MirStaticInitializerBody, MirStaticLifecycleCoordinator,
    MirStaticLifecycleDefinition, MirStaticLifecycleProof, MirStaticLifecycleTransition,
    MirStaticLifecycleTransitionKind, MirStaticPublication, MirStaticSharedCleanup,
    MirStaticValueCleanup, MirStorage, MirStorageDead, MirStorageKind, MirStorageLive, MirStore,
    MirStringInitialize, MirStringLanguageItem, MirSynthesizedCopy, MirSynthesizedFieldCopy,
    MirTerminationReason, MirTerminator, MirType, MirUnaryOperation, MirUserCopy, MirValue,
    MirViewProvenance, MirViewTarget, MirVirtualFamily, MirVirtualFamilyTable, OptionalGuardId,
    PathConditionId, PreliminaryMirProgram, PreliminaryMirSharedLifecycleTarget,
    PreliminaryMirStaticField, PreliminaryMirStaticInitializer, StaticAccessKind,
    StaticActivationAuthority, StaticEffectPhase, StaticLifecycleAuthority,
    StaticLifecycleEffectFact, StaticLifecyclePlan, StaticLifecycleRootAuthority, StorageId,
    ValueId,
};
pub(crate) use verify::preliminary::{
    destination_completed_on_every_publication_path, reachable_static_initializer_blocks,
};
pub(crate) use verify::{check_normalized_mir, check_preliminary_mir};
pub(crate) use verify::{checked_scalar_dominates, checked_scalar_predecessors};
pub(crate) use verify::{classify_local_identity_site, MirIdentitySiteRole};
pub use verify::{
    verify_mir, verify_preliminary_mir, MirVerificationError, MirVerificationErrors,
    VerifiedPreliminaryMirProgram,
};

#[cfg(test)]
mod tests;
