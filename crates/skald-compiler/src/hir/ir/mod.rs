//! Fully typed HIR consumed by MIR lowering.

use std::borrow::Cow;

use crate::identity::{ClassId, InterfaceId, OptionalTypeId};
pub use crate::object_path::ObjectProjection;

mod array;
mod body;
mod control_flow;
mod declarations;
mod expression;
mod integer_division;
mod io;
mod object;
mod optional;
mod optional_box_type;
mod optional_type;
mod primitive;
mod shared;
mod shift;
mod static_field;
mod stored_value;
mod strings;

pub use array::{
    HirArrayAliasArgument, HirArrayAliasSource, HirArrayAnchor, HirArrayAssignElement,
    HirArrayAssignment, HirArrayConstruction, HirArrayConstructionMode, HirArrayCopyElement,
    HirArrayDefaultElement, HirArrayDestroyElement, HirArrayElementAssignment,
    HirArrayElementInitialization, HirArrayElementList, HirArrayElementPlace, HirArrayElementValue,
    HirArrayEvaluationOrder, HirArrayFieldInitialize, HirArrayIndex, HirArrayIndexNormalization,
    HirArrayInitialize, HirArrayLength, HirArrayLifecycle, HirArrayOwnership, HirArrayPlace,
    HirArrayProvenance, HirArrayReceiver, HirArrayReceiverOwnership, HirArrayReceiverSource,
    HirArrayRuntimeFailure, HirArraySlice, HirArraySliceAssignment, HirArraySliceBounds,
    HirArraySource, HirArrayTransfer, HirArrayType, HirArrayTypeTable,
};
pub use body::{
    HirBlock, HirBreak, HirCallStatement, HirClassDefinition, HirClassDefinitionTable,
    HirConditional, HirConditionalArm, HirContinue, HirFunctionDefinition,
    HirFunctionDefinitionTable, HirLocalDecl, HirLocalInitializer, HirMemberDefinition, HirPanic,
    HirPrimitiveAssignment, HirPrimitivePlace, HirPrimitiveStorage, HirReturn, HirReturnValue,
    HirStatement, HirWhile,
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
    HirBinaryOperation, HirCallArgument, HirComparisonOperand, HirComparisonPredicate,
    HirCopyArgument, HirExpression, HirExpressionKind, HirIntegerBitwiseOperation, HirIntegerType,
    HirInterfaceCallTarget, HirInterfaceReceiver, HirLogicalExpression, HirLogicalOperation,
    HirMethodCallTarget, HirPrimitiveComparison, HirTypeTest, HirTypeTestKind, HirUnaryOperation,
};
pub use integer_division::{
    HirCheckedIntegerDivision, HirIntegerDivisionFailure, HirIntegerDivisionKind,
    HirIntegerDivisionOperation, HirSignedIntegerDivisionSemantics, HirSignedMinimumPairResult,
    HirSignedQuotientRounding, HirSignedRemainderSign,
};
pub use io::HirIoOperation;
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
    HirAggregateOptionalAssignment, HirCheckedOptionalView, HirClassOptionalAssignment,
    HirClassOptionalInitialize, HirClassOptionalPlace, HirClassOptionalSource,
    HirNestedOptionalUnwrap, HirOptionalAliasPlace, HirOptionalArrayUnwrap, HirOptionalAssignment,
    HirOptionalBoxPointee, HirOptionalOperand, HirOptionalPlace, HirOptionalSharedAssignment,
    HirOptionalSharedInitialize, HirOptionalSharedPlace, HirOptionalSharedSource,
    HirOptionalSource, HirOptionalStorage, HirOptionalValue, HirOptionalValuePlace,
    HirOptionalValueSource, HirOptionalWriteKind, HirPresenceTestKind,
};
pub use optional_box_type::{HirOptionalBoxType, HirOptionalBoxTypeTable};
pub use optional_type::{
    HirOptionalAssignmentPlan, HirOptionalBoundaryPlan, HirOptionalBoundaryPlans,
    HirOptionalCheckedAccess, HirOptionalCopyPlan, HirOptionalDestructionPlan,
    HirOptionalInitializationPlan, HirOptionalInjectionPlan, HirOptionalLifecycle,
    HirOptionalPresenceTestPlan, HirOptionalRepresentation, HirOptionalStorageCategory,
    HirOptionalType, HirOptionalTypeTable, HirOptionalUnwrapPlan,
};
pub use primitive::{HirPrimitiveCast, HirPrimitiveCastKind, HirPrimitiveType};
pub use shared::{
    HirOptionalBoxAllocation, HirOptionalBoxEvaluationOrder, HirOwnerTransfer, HirSharedAllocation,
    HirSharedAllocationMode, HirSharedAssignment, HirSharedCast, HirSharedCastKind,
    HirSharedFieldWrite, HirSharedFieldWriteKind, HirSharedPlace, HirSharedProducer,
    HirSharedSource, HirSharedTarget, HirSharedTransfer,
};
pub use shift::{
    HirCheckedShift, HirRightShiftFlavor, HirShiftDirection, HirShiftFailure, HirShiftOperation,
};
pub use static_field::{HirStaticFieldDeclaration, HirStaticFieldInitializer, HirStaticPlace};
pub use stored_value::{
    HirClassOptionalDestinationInitialization, HirObjectDestinationInitialization,
    HirStoredValueInitialization,
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
    Optional(OptionalTypeId),
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
                HirSharedTarget::OptionalBox(target) => format!("shared optional-box {target}"),
            }),
            Self::Optional(optional) => Cow::Owned(format!("optional {optional}")),
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
            | Self::Optional(_) => "a",
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
