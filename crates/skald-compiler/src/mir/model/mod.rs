//! Data model for target-independent MIR.

mod control_flow;
mod declarations;
mod definition;
mod ids;
mod instruction;
mod interface;
mod optional;
mod shared;
mod value;

pub use control_flow::{MirBasicBlock, MirBody, MirTerminationReason, MirTerminator};
pub use declarations::{
    MirBaseCopy, MirCallableSignature, MirClassDeclaration, MirClassDeclarationTable,
    MirCopyAssignmentDeclaration, MirCopyCapability, MirCopyConstructorDeclaration,
    MirDestructionPlan, MirDestructionStep, MirDestructorDeclaration, MirDirectBase,
    MirFieldDeclaration, MirFunctionDeclaration, MirFunctionDeclarationTable, MirFunctionLinkage,
    MirInitializerDeclaration, MirMethodDeclaration, MirParameter, MirParameterMode, MirProgram,
    MirReceiverAccess, MirSelectedCopyOperation, MirSynthesizedCopy, MirSynthesizedFieldCopy,
    MirUserCopy, MirVirtualFamily, MirVirtualFamilyTable,
};
pub use definition::{
    MirAliasAccess, MirDefinitionRef, MirFunctionDefinition, MirFunctionDefinitionTable,
    MirMemberDefinition, MirMemberDefinitionTable, MirStorage, MirStorageKind,
};
pub use ids::{BlockId, OptionalGuardId, StorageId, ValueId};
pub use instruction::{
    MirArgument, MirAssignment, MirCall, MirCallReceiver, MirCallTarget, MirCheckedViewBinding,
    MirCheckedViewEnd, MirCleanup, MirCopyAssignment, MirCopyConstruction, MirEndFullExpression,
    MirInitialize, MirInstruction, MirInterfaceCallTarget, MirMethodCallTarget, MirMethodReceiver,
    MirObjectOrigin, MirObjectView, MirStore, MirViewTarget,
};
pub use interface::{
    MirInterfaceConformance, MirInterfaceDeclaration, MirInterfaceDeclarationTable,
    MirInterfaceRequirement, MirRequirementImplementation,
};
pub use optional::{
    MirClassOptionalAssign, MirClassOptionalCleanup, MirClassOptionalInitialize,
    MirClassOptionalPublish, MirClassOptionalSource, MirOptionalAssign, MirOptionalInitialize,
    MirOptionalSource, MirOptionalViewBegin, MirOptionalViewEnd, MirPresenceTestKind,
    MirPrimitiveType,
};
pub use shared::{
    MirSharedAdopt, MirSharedAllocate, MirSharedAllocationMode, MirSharedAllocationOrigin,
    MirSharedCast, MirSharedCastSource, MirSharedCastTransfer, MirSharedCopy, MirSharedFieldCopy,
    MirSharedFieldInitialize, MirSharedFieldReplace, MirSharedInitialize, MirSharedMove,
    MirSharedPublish, MirSharedRelease, MirSharedTarget,
};
pub use value::{
    MirBinaryOperation, MirPlace, MirPlaceBase, MirPlaceProjection, MirRvalue, MirRvalueKind,
    MirType, MirUnaryOperation, MirValue,
};
