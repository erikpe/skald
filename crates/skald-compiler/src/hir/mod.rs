//! Typed high-level intermediate representation.
//!
//! HIR retains source spans useful to diagnostics while replacing resolved
//! syntax with explicit typed operations and exact call targets.

mod dump;
mod ir;

pub use dump::dump_hir;
pub use ir::{
    BlockFlow, HirAccess, HirBaseCopy, HirBaseInitialization, HirBinaryOperation, HirBlock,
    HirCallArgument, HirCallStatement, HirCallableSignature, HirCheckedObjectView,
    HirCheckedObjectViewKind, HirClassDeclaration, HirClassDeclarationTable, HirClassDefinition,
    HirClassDefinitionTable, HirConditional, HirConditionalArm, HirConstruction,
    HirConstructionMode, HirCopyArgument, HirCopyAssignment, HirCopyAssignmentDeclaration,
    HirCopyCapability, HirCopyConstruction, HirCopyConstructorDeclaration, HirDestructionPlan,
    HirDestructionStep, HirDestructorDeclaration, HirDirectBase, HirExpression, HirExpressionKind,
    HirFieldAssignment, HirFieldConstruction, HirFieldCopyAssignment, HirFieldCopyConstruction,
    HirFieldDeclaration, HirFieldPlace, HirFunctionDeclaration, HirFunctionDeclarationTable,
    HirFunctionDefinition, HirFunctionDefinitionTable, HirFunctionLinkage,
    HirInitializerDeclaration, HirInterfaceCallTarget, HirInterfaceConformance,
    HirInterfaceDeclaration, HirInterfaceDeclarationTable, HirInterfaceParameter,
    HirInterfaceReceiver, HirInterfaceRequirement, HirLocal, HirLocalDecl, HirLocalInitializer,
    HirMemberDefinition, HirMethodCallTarget, HirMethodDeclaration, HirMethodDispatch,
    HirMethodReceiver, HirObjectCall, HirObjectCallTarget, HirObjectInitialization,
    HirObjectOrigin, HirObjectPath, HirObjectPlace, HirObjectProducer, HirObjectReturn,
    HirObjectSlice, HirObjectSource, HirObjectView, HirOwnerTransfer, HirParameter,
    HirParameterMode, HirProgram, HirRequirementImplementation, HirReturn, HirReturnValue,
    HirSelectedCopyOperation, HirSharedAllocation, HirSharedAssignment, HirSharedFieldWrite,
    HirSharedFieldWriteKind, HirSharedPlace, HirSharedProducer, HirSharedSource, HirSharedTarget,
    HirSharedTransfer, HirStatement, HirSynthesizedCopy, HirSynthesizedFieldCopy, HirTypeTest,
    HirTypeTestKind, HirUnaryOperation, HirUserCopy, HirViewSource, HirViewTarget,
    HirVirtualFamily, HirVirtualFamilyTable, ObjectProjection, Type,
};
