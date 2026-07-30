//! Data model for target-independent MIR.

mod array;
mod control_flow;
mod declarations;
mod definition;
mod ids;
mod instruction;
mod interface;
mod logical;
mod optional;
mod path_condition;
mod shared;
mod strings;
mod value;

pub use array::{
    MirArrayAnchorKind, MirArrayAssignElement, MirArrayBoundary, MirArrayCopyElement,
    MirArrayDefaultElement, MirArrayDestroyElement, MirArrayFailure, MirArrayInstruction,
    MirArrayLifecycle, MirArrayOwnership, MirArrayPositionKind, MirArrayType, MirArrayTypeTable,
};
pub use control_flow::{MirBasicBlock, MirBody, MirTerminationReason, MirTerminator};
pub use declarations::{
    MirBaseCopy, MirCallableSignature, MirClassDeclaration, MirClassDeclarationTable,
    MirCopyAssignmentDeclaration, MirCopyCapability, MirCopyConstructorDeclaration,
    MirDestructionPlan, MirDestructionStep, MirDestructorDeclaration, MirDirectBase,
    MirFieldDeclaration, MirFunctionDeclaration, MirFunctionDeclarationTable, MirFunctionLinkage,
    MirInitializerDeclaration, MirMethodDeclaration, MirMethodKind, MirParameter, MirParameterMode,
    MirProgram, MirReceiverAccess, MirSelectedCopyOperation, MirSynthesizedCopy,
    MirSynthesizedFieldCopy, MirUserCopy, MirVirtualFamily, MirVirtualFamilyTable,
};
pub use definition::{
    MirAliasAccess, MirDefinitionRef, MirFunctionDefinition, MirFunctionDefinitionTable,
    MirMemberDefinition, MirMemberDefinitionTable, MirStorage, MirStorageKind,
};
pub use ids::{BlockId, OptionalGuardId, PathConditionId, StorageId, ValueId};
pub use instruction::{
    MirArgument, MirAssignment, MirCall, MirCallReceiver, MirCallTarget, MirCheckedViewBinding,
    MirCheckedViewEnd, MirCleanup, MirCopyAssignment, MirCopyConstruction, MirEndFullExpression,
    MirInitialize, MirInstruction, MirInterfaceCallTarget, MirMethodCallTarget, MirMethodReceiver,
    MirObjectOrigin, MirObjectView, MirStorageDead, MirStorageLive, MirStore, MirViewTarget,
};
pub use interface::{
    MirInterfaceConformance, MirInterfaceDeclaration, MirInterfaceDeclarationTable,
    MirInterfaceRequirement, MirRequirementImplementation,
};
pub use logical::{MirLogicalExpression, MirLogicalOperation};
pub use optional::{
    MirClassOptionalAssign, MirClassOptionalCleanup, MirClassOptionalInitialize,
    MirClassOptionalPublish, MirClassOptionalSource, MirOptionalAssign, MirOptionalInitialize,
    MirOptionalSharedAssign, MirOptionalSharedCleanup, MirOptionalSharedInitialize,
    MirOptionalSharedSource, MirOptionalSharedUnwrap, MirOptionalSource, MirOptionalViewBegin,
    MirOptionalViewEnd, MirPresenceTestKind, MirPrimitiveType,
};
pub use path_condition::MirPathCondition;
pub use shared::{
    MirSharedAdopt, MirSharedAllocate, MirSharedAllocationMode, MirSharedAllocationOrigin,
    MirSharedCast, MirSharedCastSource, MirSharedCastTransfer, MirSharedCopy, MirSharedFieldCopy,
    MirSharedFieldInitialize, MirSharedFieldReplace, MirSharedInitialize, MirSharedMove,
    MirSharedPublish, MirSharedRelease, MirSharedStatic, MirSharedTarget,
};
pub use strings::{
    MirLiteralData, MirLiteralDataTable, MirStaticAllocationOrigin, MirStaticDataMutability,
    MirStringInitialize, MirStringLanguageItem,
};
pub use value::{
    MirBinaryOperation, MirComparisonOperand, MirComparisonPredicate, MirIntegerCast,
    MirIntegerType, MirPathConditionValue, MirPlace, MirPlaceBase, MirPlaceProjection,
    MirPrimitiveComparison, MirRvalue, MirRvalueKind, MirType, MirUnaryOperation, MirValue,
};
