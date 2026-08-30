//! Data model for target-independent MIR.

mod array;
mod cell_write;
mod control_flow;
mod declarations;
mod definition;
mod final_write;
mod function_type;
mod ids;
mod instruction;
mod integer_division;
mod interface;
mod io;
mod logical;
mod optional;
mod optional_box_type;
mod optional_type;
mod path_condition;
mod preliminary;
mod primitive;
mod shared;
mod shift;
mod static_lifecycle;
mod statistics;
mod strings;
mod value;

pub use array::{
    MirArrayAnchorKind, MirArrayAssignElement, MirArrayBoundary, MirArrayCopyElement,
    MirArrayDefaultElement, MirArrayDestroyElement, MirArrayFailure, MirArrayInstruction,
    MirArrayLifecycle, MirArrayOwnership, MirArrayPositionKind, MirArrayType, MirArrayTypeTable,
};
pub use cell_write::MirCellWriteAuthorization;
pub use control_flow::{MirBasicBlock, MirBody, MirTerminationReason, MirTerminator};
pub use declarations::{
    MirBaseCopy, MirCallableSignature, MirClassDeclaration, MirClassDeclarationTable,
    MirCopyAssignmentDeclaration, MirCopyCapability, MirCopyConstructorDeclaration,
    MirDestructionPlan, MirDestructionStep, MirDestructorDeclaration, MirDirectBase,
    MirFieldDeclaration, MirFunctionDeclaration, MirFunctionDeclarationTable, MirFunctionLinkage,
    MirInitializerDeclaration, MirMethodDeclaration, MirMethodKind, MirParameter, MirParameterMode,
    MirProgram, MirReceiverAccess, MirSelectedCopyOperation, MirStaticFieldDeclaration,
    MirSynthesizedCopy, MirSynthesizedFieldCopy, MirUserCopy, MirVirtualFamily,
    MirVirtualFamilyTable,
};
pub use definition::{
    MirAliasAccess, MirDefinitionRef, MirFunctionDefinition, MirFunctionDefinitionTable,
    MirMemberDefinition, MirMemberDefinitionTable, MirStorage, MirStorageKind,
};
pub use final_write::MirFinalWriteAuthorization;
pub use function_type::{MirFunctionType, MirFunctionTypeTable};
pub use ids::{BlockId, OptionalGuardId, PathConditionId, StorageId, ValueId};
pub use instruction::{
    MirArgument, MirAssignment, MirCall, MirCallReceiver, MirCallTarget, MirCallableAddress,
    MirCheckedViewBinding, MirCheckedViewEnd, MirCleanup, MirCopyAssignment, MirCopyConstruction,
    MirEndFullExpression, MirIndirectCallTarget, MirInitialize, MirInstruction,
    MirInterfaceCallTarget, MirMethodCallTarget, MirMethodReceiver, MirObjectOrigin, MirObjectView,
    MirStorageDead, MirStorageLive, MirStore, MirViewProvenance, MirViewTarget,
};
pub use integer_division::{
    MirIntegerDivisionKind, MirIntegerDivisionOperation, MirIntegerDivisorCheck,
    MirSignedIntegerDivisionSemantics, MirSignedMinimumPairResult, MirSignedQuotientRounding,
    MirSignedRemainderSign,
};
pub use interface::{
    MirInterfaceConformance, MirInterfaceDeclaration, MirInterfaceDeclarationTable,
    MirInterfaceRequirement, MirRequirementImplementation,
};
pub use io::{MirIoBuffer, MirIoInstruction, MirIoOperation};
pub use logical::{MirLogicalExpression, MirLogicalOperation};
pub use optional::{
    MirAggregateOptionalAssign, MirAggregateOptionalCleanup, MirAggregateOptionalInitialize,
    MirAggregateOptionalPublish, MirAggregateOptionalSource, MirClassOptionalAssign,
    MirClassOptionalCleanup, MirClassOptionalInitialize, MirClassOptionalPublish,
    MirClassOptionalSource, MirOptionalAssign, MirOptionalBoxViewBegin, MirOptionalBoxViewEnd,
    MirOptionalInitialize, MirOptionalSharedAssign, MirOptionalSharedCleanup,
    MirOptionalSharedInitialize, MirOptionalSharedSource, MirOptionalSharedUnwrap,
    MirOptionalSource, MirOptionalViewBegin, MirOptionalViewEnd, MirPresenceTestKind,
};
pub use optional_box_type::{MirOptionalBoxType, MirOptionalBoxTypeTable};
pub use optional_type::{
    MirOptionalAssignmentPlan, MirOptionalBoundaryPlan, MirOptionalBoundaryPlans,
    MirOptionalCheckedAccess, MirOptionalCleanupPlan, MirOptionalCopyPlan,
    MirOptionalInitializationPlan, MirOptionalInjectionPlan, MirOptionalLifecycle,
    MirOptionalPresencePlan, MirOptionalRepresentation, MirOptionalStorage, MirOptionalType,
    MirOptionalTypeTable, MirOptionalUnwrapPlan,
};
pub use path_condition::MirPathCondition;
pub use preliminary::{
    MirStaticInitializerBody, MirStaticPublication, PreliminaryMirProgram,
    PreliminaryMirSharedLifecycleTarget, PreliminaryMirStaticField,
    PreliminaryMirStaticInitializer,
};
pub use primitive::{
    MirF64ToIntegerRange, MirF64ToIntegerRounding, MirPrimitiveCast, MirPrimitiveCastKind,
    MirPrimitiveCastRangeCheck, MirPrimitiveType,
};
pub use shared::{
    MirOptionalBoxCompletion, MirSharedAdopt, MirSharedAllocate, MirSharedAllocationMode,
    MirSharedAllocationOrigin, MirSharedAllocationTarget, MirSharedCast, MirSharedCastSource,
    MirSharedCastTransfer, MirSharedCopy, MirSharedFieldCopy, MirSharedFieldInitialize,
    MirSharedFieldReplace, MirSharedInitialize, MirSharedMove, MirSharedPublish, MirSharedRelease,
    MirSharedStatic, MirSharedTarget,
};
pub use shift::{MirRightShiftFlavor, MirShiftCountCheck, MirShiftDirection, MirShiftOperation};
pub use static_lifecycle::{
    MirProgramLifecycle, MirStaticActivationRegion, MirStaticActivationWork,
    MirStaticDestructionRegion, MirStaticFieldInitialization, MirStaticLifecycleCertificate,
    MirStaticLifecycleCoordinator, MirStaticLifecycleDefinition, MirStaticLifecycleIndices,
    MirStaticLifecycleTransition, MirStaticLifecycleTransitionKind, MirStaticSharedCleanup,
    MirStaticValueCleanup, PlannedMirProgram, StaticAccessEvidence, StaticAccessKind,
    StaticArrayLifecycleOperation, StaticClassLifecycleOperation, StaticEffectAnalysis,
    StaticEffectEdge, StaticEffectEdgeKind, StaticEffectNode, StaticEffectPhase,
    StaticEffectSummary, StaticFunctionValueCandidates, StaticFunctionValueTarget,
    StaticLifecycleAuthority, StaticLifecycleEffectFact, StaticLifecyclePlan,
    StaticLifecycleRootAuthority, StaticLifetimeDependency, StaticLifetimeEvidence,
    StaticLifetimePhase,
};
pub(crate) use statistics::MirProgramStatistics;
pub use strings::{
    MirLiteralData, MirLiteralDataTable, MirStaticAllocationOrigin, MirStaticDataMutability,
    MirStringInitialize, MirStringLanguageItem,
};
pub use value::{
    MirBinaryOperation, MirComparisonOperand, MirComparisonPredicate, MirIntegerBitwiseOperation,
    MirIntegerType, MirPathConditionValue, MirPlace, MirPlaceBase, MirPlaceProjection,
    MirPrimitiveComparison, MirRvalue, MirRvalueKind, MirType, MirUnaryOperation, MirValue,
};
