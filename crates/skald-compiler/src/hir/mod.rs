//! Typed high-level intermediate representation.
//!
//! HIR retains source spans useful to diagnostics while replacing resolved
//! syntax with explicit typed operations and exact call targets.

mod dump;
mod ir;

pub use dump::dump_hir;
pub use ir::{
    BlockFlow, HirAccess, HirBaseCopy, HirBaseInitialization, HirBinaryOperation, HirBlock,
    HirCallArgument, HirCallStatement, HirCallableSignature, HirClassDeclaration,
    HirClassDeclarationTable, HirClassDefinition, HirClassDefinitionTable, HirConditional,
    HirConditionalArm, HirConstruction, HirCopyArgument, HirCopyAssignment,
    HirCopyAssignmentDeclaration, HirCopyCapability, HirCopyConstruction, HirDestructionPlan,
    HirDestructionStep, HirDestructorDeclaration, HirDirectBase, HirExpression, HirExpressionKind,
    HirFieldAssignment, HirFieldConstruction, HirFieldCopyAssignment, HirFieldCopyConstruction,
    HirFieldDeclaration, HirFieldPlace, HirFunctionDeclaration, HirFunctionDeclarationTable,
    HirFunctionDefinition, HirFunctionDefinitionTable, HirFunctionLinkage,
    HirInitializerDeclaration, HirLocal, HirLocalDecl, HirLocalInitializer, HirMemberDefinition,
    HirMethodDeclaration, HirObjectCall, HirObjectCallTarget, HirObjectInitialization,
    HirObjectPath, HirObjectPlace, HirObjectProducer, HirObjectReturn, HirObjectSource,
    HirParameter, HirParameterMode, HirProgram, HirReturn, HirReturnValue,
    HirSelectedCopyOperation, HirStatement, HirSynthesizedCopy, HirSynthesizedFieldCopy,
    HirUnaryOperation, HirUserCopy, Type,
};
