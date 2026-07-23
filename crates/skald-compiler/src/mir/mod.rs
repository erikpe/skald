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
pub use lower::{lower_hir, HirLoweringError};
pub use model::{
    BlockId, MirAliasAccess, MirArgument, MirAssignment, MirBaseCopy, MirBasicBlock,
    MirBinaryOperation, MirBody, MirCall, MirCallTarget, MirCallableSignature, MirClassDeclaration,
    MirClassDeclarationTable, MirCleanup, MirCopyAssignment, MirCopyAssignmentDeclaration,
    MirCopyCapability, MirCopyConstruction, MirDefinitionRef, MirDestructionPlan,
    MirDestructionStep, MirDestructorDeclaration, MirDirectBase, MirEndFullExpression,
    MirFieldDeclaration, MirFunctionDeclaration, MirFunctionDeclarationTable,
    MirFunctionDefinition, MirFunctionDefinitionTable, MirFunctionLinkage, MirInitialize,
    MirInitializerDeclaration, MirInstruction, MirMemberDefinition, MirMemberDefinitionTable,
    MirMethodDeclaration, MirObjectView, MirParameter, MirParameterMode, MirPlace, MirPlaceBase,
    MirPlaceProjection, MirProgram, MirReceiverAccess, MirRvalue, MirRvalueKind,
    MirSelectedCopyOperation, MirStorage, MirStorageKind, MirStore, MirSynthesizedCopy,
    MirSynthesizedFieldCopy, MirTerminator, MirType, MirUnaryOperation, MirUserCopy, MirValue,
    MirViewTarget, StorageId, ValueId,
};
pub use verify::{verify_mir, MirVerificationError, MirVerificationErrors};

#[cfg(test)]
mod tests;
