//! Data model for target-independent MIR.

mod control_flow;
mod declarations;
mod definition;
mod ids;
mod instruction;
mod value;

pub use control_flow::{MirBasicBlock, MirBody, MirTerminator};
pub use declarations::{
    MirCallableSignature, MirClassDeclaration, MirClassDeclarationTable,
    MirCopyAssignmentDeclaration, MirCopyCapability, MirDestructionPlan, MirDestructionStep,
    MirDestructorDeclaration, MirFieldDeclaration, MirFunctionDeclaration,
    MirFunctionDeclarationTable, MirFunctionLinkage, MirInitializerDeclaration,
    MirMethodDeclaration, MirParameter, MirParameterMode, MirProgram, MirReceiverAccess,
    MirSelectedCopyOperation, MirSynthesizedCopy, MirSynthesizedFieldCopy,
};
pub use definition::{
    MirAliasAccess, MirDefinitionRef, MirFunctionDefinition, MirFunctionDefinitionTable,
    MirMemberDefinition, MirMemberDefinitionTable, MirStorage, MirStorageKind,
};
pub use ids::{BlockId, StorageId, ValueId};
pub use instruction::{
    MirArgument, MirAssignment, MirCall, MirCallTarget, MirCleanup, MirCopyAssignment,
    MirCopyConstruction, MirEndFullExpression, MirInitialize, MirInstruction, MirStore,
};
pub use value::{
    MirBinaryOperation, MirPlace, MirPlaceBase, MirPlaceProjection, MirRvalue, MirRvalueKind,
    MirType, MirUnaryOperation, MirValue,
};
