//! Typed high-level intermediate representation.
//!
//! HIR retains source spans useful to diagnostics while replacing resolved
//! syntax with explicit typed operations and exact call targets.

mod dump;
mod ir;

pub use dump::dump_hir;
pub use ir::{
    BlockFlow, HirBinaryOperation, HirBlock, HirCallStatement, HirCallableSignature,
    HirClassDeclaration, HirClassDeclarationTable, HirClassDefinition, HirClassDefinitionTable,
    HirConditional, HirConditionalArm, HirConstruction, HirExpression, HirExpressionKind,
    HirFieldAssignment, HirFieldDeclaration, HirFieldPlace, HirFunctionDeclaration,
    HirFunctionDeclarationTable, HirFunctionDefinition, HirFunctionDefinitionTable,
    HirFunctionLinkage, HirInitializerDeclaration, HirLocal, HirLocalDecl, HirLocalInitializer,
    HirMemberDefinition, HirMethodDeclaration, HirObjectPlace, HirParameter, HirProgram,
    HirReceiverAccess, HirReturn, HirStatement, HirUnaryOperation, Type,
};
