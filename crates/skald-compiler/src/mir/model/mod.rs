//! Data model for target-independent MIR.

mod control_flow;
mod declarations;
mod definition;
mod ids;
mod instruction;
mod interface;
mod value;

pub use control_flow::{MirBasicBlock, MirBody, MirTerminationReason, MirTerminator};
pub use declarations::{
    MirBaseCopy, MirCallableSignature, MirClassDeclaration, MirClassDeclarationTable,
    MirCopyAssignmentDeclaration, MirCopyCapability, MirDestructionPlan, MirDestructionStep,
    MirDestructorDeclaration, MirDirectBase, MirFieldDeclaration, MirFunctionDeclaration,
    MirFunctionDeclarationTable, MirFunctionLinkage, MirInitializerDeclaration,
    MirMethodDeclaration, MirParameter, MirParameterMode, MirProgram, MirReceiverAccess,
    MirSelectedCopyOperation, MirSynthesizedCopy, MirSynthesizedFieldCopy, MirUserCopy,
    MirVirtualFamily, MirVirtualFamilyTable,
};
pub use definition::{
    MirAliasAccess, MirDefinitionRef, MirFunctionDefinition, MirFunctionDefinitionTable,
    MirMemberDefinition, MirMemberDefinitionTable, MirStorage, MirStorageKind,
};
pub use ids::{BlockId, StorageId, ValueId};
pub use instruction::{
    MirArgument, MirAssignment, MirCall, MirCallReceiver, MirCallTarget, MirCleanup,
    MirCopyAssignment, MirCopyConstruction, MirEndFullExpression, MirInitialize, MirInstruction,
    MirInterfaceCallTarget, MirMethodCallTarget, MirMethodReceiver, MirNarrowedAliasBinding,
    MirNarrowedAliasEnd, MirObjectOrigin, MirObjectView, MirStore, MirViewTarget,
};
pub use interface::{
    MirInterfaceConformance, MirInterfaceDeclaration, MirInterfaceDeclarationTable,
    MirInterfaceRequirement, MirRequirementImplementation,
};
pub use value::{
    MirBinaryOperation, MirPlace, MirPlaceBase, MirPlaceProjection, MirRvalue, MirRvalueKind,
    MirType, MirUnaryOperation, MirValue,
};
