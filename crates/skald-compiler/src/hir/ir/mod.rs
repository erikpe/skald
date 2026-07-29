//! Fully typed HIR consumed by MIR lowering.

use std::borrow::Cow;

use crate::identity::{ClassId, InterfaceId};
pub use crate::object_path::ObjectProjection;

mod array;
mod body;
mod control_flow;
mod declarations;
mod expression;
mod object;
mod optional;
mod shared;
mod strings;

pub use array::{
    HirArrayAliasArgument, HirArrayAliasSource, HirArrayAnchor, HirArrayAssignElement,
    HirArrayAssignment, HirArrayConstruction, HirArrayConstructionMode, HirArrayCopyElement,
    HirArrayDefaultElement, HirArrayDestroyElement, HirArrayElementAssignment,
    HirArrayElementPlace, HirArrayElementValue, HirArrayEvaluationOrder, HirArrayFieldInitialize,
    HirArrayIndex, HirArrayIndexNormalization, HirArrayInitialize, HirArrayLength,
    HirArrayLifecycle, HirArrayOwnership, HirArrayPlace, HirArrayProvenance, HirArrayReceiver,
    HirArrayReceiverOwnership, HirArrayReceiverSource, HirArrayRuntimeFailure, HirArraySlice,
    HirArraySliceAssignment, HirArraySliceBounds, HirArraySource, HirArrayTransfer, HirArrayType,
    HirArrayTypeTable,
};
pub use body::{
    HirBlock, HirCallStatement, HirClassDefinition, HirClassDefinitionTable, HirConditional,
    HirConditionalArm, HirFunctionDefinition, HirFunctionDefinitionTable, HirLocalDecl,
    HirLocalInitializer, HirMemberDefinition, HirPanic, HirPrimitiveBindingAssignment, HirReturn,
    HirReturnValue, HirStatement, HirWhile,
};
pub use control_flow::HirControlEffects;
pub use declarations::{
    HirCallableSignature, HirClassDeclaration, HirClassDeclarationTable,
    HirCopyAssignmentDeclaration, HirCopyConstructorDeclaration, HirDestructionPlan,
    HirDestructionStep, HirDestructorDeclaration, HirDirectBase, HirFieldDeclaration,
    HirFunctionDeclaration, HirFunctionDeclarationTable, HirFunctionLinkage,
    HirInitializerDeclaration, HirInterfaceConformance, HirInterfaceDeclaration,
    HirInterfaceDeclarationTable, HirInterfaceParameter, HirInterfaceRequirement, HirLocal,
    HirMethodDeclaration, HirMethodDispatch, HirMethodKind, HirParameter, HirParameterMode,
    HirProgram, HirRequirementImplementation, HirVirtualFamily, HirVirtualFamilyTable,
};
pub use expression::{
    HirBinaryOperation, HirCallArgument, HirComparisonPredicate, HirCopyArgument, HirExpression,
    HirExpressionKind, HirIntegerCast, HirIntegerComparison, HirIntegerType,
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
pub use optional::{
    HirCheckedOptionalView, HirClassOptionalAssignment, HirClassOptionalInitialize,
    HirClassOptionalPlace, HirClassOptionalSource, HirOptionalAliasPlace, HirOptionalAssignment,
    HirOptionalOperand, HirOptionalPlace, HirOptionalSharedAssignment, HirOptionalSharedInitialize,
    HirOptionalSharedPlace, HirOptionalSharedSource, HirOptionalSource, HirOptionalStorage,
    HirOptionalWriteKind, HirPresenceTestKind, HirPrimitiveType,
};
pub use shared::{
    HirOwnerTransfer, HirSharedAllocation, HirSharedAllocationMode, HirSharedAssignment,
    HirSharedCast, HirSharedCastKind, HirSharedFieldWrite, HirSharedFieldWriteKind, HirSharedPlace,
    HirSharedProducer, HirSharedSource, HirSharedTarget, HirSharedTransfer,
};
pub use strings::{HirLiteralData, HirLiteralDataTable, HirStringLanguageItem, HirStringLiteral};

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
    Array(crate::identity::ArrayTypeId),
    Shared(HirSharedTarget),
    OptionalShared(HirSharedTarget),
    OptionalPrimitive(HirPrimitiveType),
    OptionalClass(ClassId),
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
            Self::Array(array) => Cow::Owned(format!("array {array}")),
            Self::Shared(target) => Cow::Owned(match target {
                HirSharedTarget::Obj => "shared Obj".to_owned(),
                HirSharedTarget::Class(class) => format!("shared class {class}"),
                HirSharedTarget::Interface(interface) => format!("shared interface {interface}"),
                HirSharedTarget::Array(array) => format!("shared array {array}"),
            }),
            Self::OptionalShared(target) => Cow::Owned(match target {
                HirSharedTarget::Obj => "shared? Obj".to_owned(),
                HirSharedTarget::Class(class) => format!("shared? class {class}"),
                HirSharedTarget::Interface(interface) => {
                    format!("shared? interface {interface}")
                }
                HirSharedTarget::Array(array) => format!("shared? array {array}"),
            }),
            Self::OptionalPrimitive(payload) => Cow::Owned(format!("{}?", payload.name())),
            Self::OptionalClass(class) => Cow::Owned(format!("class {class}?")),
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
            | Self::Interface(_)
            | Self::Array(_)
            | Self::Shared(_)
            | Self::OptionalShared(_)
            | Self::OptionalPrimitive(_)
            | Self::OptionalClass(_) => "a",
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
