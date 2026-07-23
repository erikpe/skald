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
    HirInitializerDeclaration, HirInterfaceCallTarget, HirInterfaceConformance,
    HirInterfaceDeclaration, HirInterfaceDeclarationTable, HirInterfaceParameter,
    HirInterfaceRequirement, HirLocal, HirLocalDecl, HirLocalInitializer, HirMemberDefinition,
    HirMethodCallTarget, HirMethodDeclaration, HirMethodDispatch, HirMethodReceiver, HirObjectCall,
    HirObjectCallTarget, HirObjectInitialization, HirObjectOrigin, HirObjectPath, HirObjectPlace,
    HirObjectProducer, HirObjectReturn, HirObjectSlice, HirObjectSource, HirObjectView,
    HirParameter, HirParameterMode, HirProgram, HirRequirementImplementation, HirReturn,
    HirReturnValue, HirSelectedCopyOperation, HirStatement, HirSynthesizedCopy,
    HirSynthesizedFieldCopy, HirUnaryOperation, HirUserCopy, HirViewSource, HirViewTarget,
    HirVirtualFamily, HirVirtualFamilyTable, ObjectProjection, Type,
};
