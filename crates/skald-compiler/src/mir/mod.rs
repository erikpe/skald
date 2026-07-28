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

pub use dump::dump_mir;
pub use lower::lower_hir;
pub use model::{
    BlockId, MirAliasAccess, MirArgument, MirArrayAnchorKind, MirArrayAssignElement,
    MirArrayBoundary, MirArrayCopyElement, MirArrayDefaultElement, MirArrayDestroyElement,
    MirArrayFailure, MirArrayInstruction, MirArrayLifecycle, MirArrayOwnership,
    MirArrayPositionKind, MirArrayType, MirArrayTypeTable, MirAssignment, MirBaseCopy,
    MirBasicBlock, MirBinaryOperation, MirBody, MirCall, MirCallReceiver, MirCallTarget,
    MirCallableSignature, MirCheckedViewBinding, MirCheckedViewEnd, MirClassDeclaration,
    MirClassDeclarationTable, MirClassOptionalAssign, MirClassOptionalCleanup,
    MirClassOptionalInitialize, MirClassOptionalPublish, MirClassOptionalSource, MirCleanup,
    MirCopyAssignment, MirCopyAssignmentDeclaration, MirCopyCapability, MirCopyConstruction,
    MirCopyConstructorDeclaration, MirDefinitionRef, MirDestructionPlan, MirDestructionStep,
    MirDestructorDeclaration, MirDirectBase, MirEndFullExpression, MirFieldDeclaration,
    MirFunctionDeclaration, MirFunctionDeclarationTable, MirFunctionDefinition,
    MirFunctionDefinitionTable, MirFunctionLinkage, MirInitialize, MirInitializerDeclaration,
    MirInstruction, MirInterfaceCallTarget, MirInterfaceConformance, MirInterfaceDeclaration,
    MirInterfaceDeclarationTable, MirInterfaceRequirement, MirLiteralData, MirLiteralDataTable,
    MirMemberDefinition, MirMemberDefinitionTable, MirMethodCallTarget, MirMethodDeclaration,
    MirMethodKind, MirMethodReceiver, MirObjectOrigin, MirObjectView, MirOptionalAssign,
    MirOptionalInitialize, MirOptionalSharedAssign, MirOptionalSharedCleanup,
    MirOptionalSharedInitialize, MirOptionalSharedSource, MirOptionalSharedUnwrap,
    MirOptionalSource, MirOptionalViewBegin, MirOptionalViewEnd, MirParameter, MirParameterMode,
    MirPlace, MirPlaceBase, MirPlaceProjection, MirPresenceTestKind, MirPrimitiveType, MirProgram,
    MirReceiverAccess, MirRequirementImplementation, MirRvalue, MirRvalueKind,
    MirSelectedCopyOperation, MirSharedAdopt, MirSharedAllocate, MirSharedAllocationMode,
    MirSharedAllocationOrigin, MirSharedCast, MirSharedCastSource, MirSharedCastTransfer,
    MirSharedCopy, MirSharedFieldCopy, MirSharedFieldInitialize, MirSharedFieldReplace,
    MirSharedInitialize, MirSharedMove, MirSharedPublish, MirSharedRelease, MirSharedStatic,
    MirSharedTarget, MirStaticAllocationOrigin, MirStaticDataMutability, MirStorage,
    MirStorageKind, MirStore, MirStringInitialize, MirStringLanguageItem, MirSynthesizedCopy,
    MirSynthesizedFieldCopy, MirTerminationReason, MirTerminator, MirType, MirUnaryOperation,
    MirUserCopy, MirValue, MirViewTarget, MirVirtualFamily, MirVirtualFamilyTable, OptionalGuardId,
    StorageId, ValueId,
};
pub use verify::{verify_mir, MirVerificationError, MirVerificationErrors};

#[cfg(test)]
mod tests;
