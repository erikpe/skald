//! Fully typed HIR consumed by MIR lowering.

use std::borrow::Cow;

use crate::identity::{ClassId, InterfaceId};
pub use crate::object_path::ObjectProjection;

mod body;
mod declarations;
mod expression;
mod object;

pub use body::{
    BlockFlow, HirBlock, HirCallStatement, HirClassDefinition, HirClassDefinitionTable,
    HirConditional, HirConditionalArm, HirFunctionDefinition, HirFunctionDefinitionTable,
    HirLocalDecl, HirLocalInitializer, HirMemberDefinition, HirReturn, HirReturnValue,
    HirStatement,
};
pub use declarations::{
    HirCallableSignature, HirClassDeclaration, HirClassDeclarationTable,
    HirCopyAssignmentDeclaration, HirCopyConstructorDeclaration, HirDestructionPlan,
    HirDestructionStep, HirDestructorDeclaration, HirDirectBase, HirFieldDeclaration,
    HirFunctionDeclaration, HirFunctionDeclarationTable, HirFunctionLinkage,
    HirInitializerDeclaration, HirInterfaceConformance, HirInterfaceDeclaration,
    HirInterfaceDeclarationTable, HirInterfaceParameter, HirInterfaceRequirement, HirLocal,
    HirMethodDeclaration, HirMethodDispatch, HirParameter, HirParameterMode, HirProgram,
    HirRequirementImplementation, HirVirtualFamily, HirVirtualFamilyTable,
};
pub use expression::{
    HirBinaryOperation, HirCallArgument, HirCopyArgument, HirExpression, HirExpressionKind,
    HirInterfaceCallTarget, HirInterfaceReceiver, HirMethodCallTarget, HirTypeTest,
    HirTypeTestKind, HirUnaryOperation,
};
pub use object::{
    HirBaseCopy, HirBaseInitialization, HirCheckedObjectView, HirCheckedObjectViewKind,
    HirConstruction, HirConstructionMode, HirCopyAssignment, HirCopyCapability,
    HirCopyConstruction, HirFieldAssignment, HirFieldConstruction, HirFieldCopyAssignment,
    HirFieldCopyConstruction, HirFieldPlace, HirMethodReceiver, HirObjectCall, HirObjectCallTarget,
    HirObjectInitialization, HirObjectOrigin, HirObjectPath, HirObjectPlace, HirObjectProducer,
    HirObjectReturn, HirObjectSlice, HirObjectSource, HirObjectView, HirSelectedCopyOperation,
    HirSynthesizedCopy, HirSynthesizedFieldCopy, HirUserCopy, HirViewSource, HirViewTarget,
};

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Type {
    I64,
    U64,
    U8,
    F64,
    Bool,
    Unit,
    Obj,
    Class(ClassId),
    Interface(InterfaceId),
}

impl Type {
    pub fn name(self) -> Cow<'static, str> {
        match self {
            Self::I64 => Cow::Borrowed("i64"),
            Self::U64 => Cow::Borrowed("u64"),
            Self::U8 => Cow::Borrowed("u8"),
            Self::F64 => Cow::Borrowed("f64"),
            Self::Bool => Cow::Borrowed("bool"),
            Self::Unit => Cow::Borrowed("unit"),
            Self::Obj => Cow::Borrowed("Obj"),
            Self::Class(class) => Cow::Owned(format!("class {class}")),
            Self::Interface(interface) => Cow::Owned(format!("interface {interface}")),
        }
    }

    /// Returns the English indefinite article used before this type's name in
    /// diagnostics.
    pub const fn indefinite_article(self) -> &'static str {
        match self {
            Self::I64 | Self::Obj => "an",
            Self::U64
            | Self::U8
            | Self::F64
            | Self::Bool
            | Self::Unit
            | Self::Class(_)
            | Self::Interface(_) => "a",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HirAccess {
    ReadOnly,
    Mutable,
}

impl HirAccess {
    pub const fn permits(self, required: Self) -> bool {
        matches!(
            (self, required),
            (Self::Mutable, _) | (Self::ReadOnly, Self::ReadOnly)
        )
    }
}
