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

pub use build::{MirBodyBuilder, MirBuildError};
pub use dump::dump_mir;
pub use lower::lower_hir;
pub use model::{
    BlockId, MirAssignment, MirBasicBlock, MirBinaryOperation, MirBody, MirCall, MirCallTarget,
    MirClassDeclaration, MirClassDeclarationTable, MirFieldDeclaration, MirFunctionDeclaration,
    MirFunctionDeclarationTable, MirFunctionDefinition, MirFunctionDefinitionTable,
    MirFunctionLinkage, MirInitialize, MirInitializerDeclaration, MirInstruction,
    MirMethodDeclaration, MirPlace, MirPlaceProjection, MirProgram, MirReceiverAccess, MirRvalue,
    MirRvalueKind, MirStorage, MirStorageKind, MirStore, MirTerminator, MirType, MirUnaryOperation,
    MirValue, StorageId, ValueId,
};
pub use verify::{verify_mir, MirVerificationError, MirVerificationErrors};

#[cfg(test)]
mod tests;
