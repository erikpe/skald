//! Fully typed HIR consumed by MIR lowering.

use std::borrow::Cow;

use crate::identity::ClassId;

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
    HirCopyAssignmentDeclaration, HirDestructionPlan, HirDestructionStep, HirDestructorDeclaration,
    HirDirectBase, HirFieldDeclaration, HirFunctionDeclaration, HirFunctionDeclarationTable,
    HirFunctionLinkage, HirInitializerDeclaration, HirLocal, HirMethodDeclaration, HirParameter,
    HirParameterMode, HirProgram,
};
pub use expression::{
    HirBinaryOperation, HirCallArgument, HirCopyArgument, HirExpression, HirExpressionKind,
    HirUnaryOperation,
};
pub use object::{
    HirBaseCopy, HirBaseInitialization, HirConstruction, HirCopyAssignment, HirCopyCapability,
    HirCopyConstruction, HirFieldAssignment, HirFieldConstruction, HirFieldCopyAssignment,
    HirFieldCopyConstruction, HirFieldPlace, HirObjectCall, HirObjectCallTarget,
    HirObjectInitialization, HirObjectPath, HirObjectPlace, HirObjectProducer, HirObjectReturn,
    HirObjectSource, HirSelectedCopyOperation, HirSynthesizedCopy, HirSynthesizedFieldCopy,
    HirUserCopy,
};

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Type {
    I64,
    U64,
    U8,
    F64,
    Bool,
    Unit,
    Class(ClassId),
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
            Self::Class(class) => Cow::Owned(format!("class {class}")),
        }
    }

    /// Returns the English indefinite article used before this type's name in
    /// diagnostics.
    pub const fn indefinite_article(self) -> &'static str {
        match self {
            Self::I64 => "an",
            Self::U64 | Self::U8 | Self::F64 | Self::Bool | Self::Unit | Self::Class(_) => "a",
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
