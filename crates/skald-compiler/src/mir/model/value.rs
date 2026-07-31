//! MIR value metadata, rvalues, and storage places.

use std::fmt;

use crate::{
    identity::{ArrayTypeId, ClassId, FieldId, InterfaceId},
    source::Span,
};

use super::ids::{PathConditionId, StorageId, ValueId};

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum MirType {
    I64,
    U64,
    U8,
    F64,
    Bool,
    Array(ArrayTypeId),
    Class(ClassId),
    /// A non-owning interface-view target. It is valid only for alias storage
    /// and never materializes as a scalar or inline object.
    Interface(InterfaceId),
    /// The universal non-owning object-view target. It has no owning storage
    /// or target layout of its own.
    Obj,
    /// A non-null strong owner carrying one object view of a live allocation.
    Shared(super::shared::MirSharedTarget),
    OptionalShared(super::shared::MirSharedTarget),
    OptionalPrimitive(super::MirPrimitiveType),
    OptionalClass(ClassId),
    Unit,
}

impl MirType {
    pub const fn is_scalar_value(self) -> bool {
        !matches!(
            self,
            Self::Class(_)
                | Self::Array(_)
                | Self::Interface(_)
                | Self::Obj
                | Self::Shared(_)
                | Self::OptionalShared(_)
                | Self::Unit
                | Self::OptionalPrimitive(_)
                | Self::OptionalClass(_)
        )
    }
}

impl fmt::Display for MirType {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::I64 => formatter.write_str("i64"),
            Self::U64 => formatter.write_str("u64"),
            Self::U8 => formatter.write_str("u8"),
            Self::F64 => formatter.write_str("f64"),
            Self::Bool => formatter.write_str("bool"),
            Self::Array(array) => write!(formatter, "array {array}"),
            Self::Class(class) => write!(formatter, "class {class}"),
            Self::Interface(interface) => write!(formatter, "interface {interface}"),
            Self::Obj => formatter.write_str("Obj"),
            Self::Shared(target) => write!(formatter, "shared {target}"),
            Self::OptionalShared(target) => write!(formatter, "shared? {target}"),
            Self::OptionalPrimitive(payload) => write!(formatter, "{payload}?"),
            Self::OptionalClass(class) => write!(formatter, "class {class}?"),
            Self::Unit => formatter.write_str("unit"),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MirValue {
    pub id: ValueId,
    pub ty: MirType,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct MirPlace {
    pub base: MirPlaceBase,
    pub projections: Vec<MirPlaceProjection>,
}

impl MirPlace {
    pub fn base(base: StorageId) -> Self {
        Self {
            base: MirPlaceBase::Storage(base),
            projections: Vec::new(),
        }
    }

    pub fn alias_parameter(base: StorageId) -> Self {
        Self {
            base: MirPlaceBase::AliasParameter(base),
            projections: Vec::new(),
        }
    }

    pub fn checked_view(base: StorageId) -> Self {
        Self {
            base: MirPlaceBase::CheckedView(base),
            projections: Vec::new(),
        }
    }

    pub fn array_alias(base: StorageId) -> Self {
        Self {
            base: MirPlaceBase::ArrayAlias(base),
            projections: Vec::new(),
        }
    }

    pub fn shared_pointee(owner: StorageId) -> Self {
        Self {
            base: MirPlaceBase::SharedPointee(owner),
            projections: Vec::new(),
        }
    }

    pub fn shared_allocation_payload(allocation: StorageId) -> Self {
        Self {
            base: MirPlaceBase::SharedAllocationPayload(allocation),
            projections: Vec::new(),
        }
    }

    pub fn project_field(mut self, field: FieldId) -> Self {
        self.projections.push(MirPlaceProjection::Field(field));
        self
    }

    pub fn project_base(mut self, base: ClassId) -> Self {
        self.projections.push(MirPlaceProjection::Base(base));
        self
    }

    pub fn project_optional_payload(mut self, class: ClassId) -> Self {
        self.projections
            .push(MirPlaceProjection::OptionalPayload(class));
        self
    }

    pub fn project_array_element(
        mut self,
        array: ArrayTypeId,
        normalized_index: StorageId,
    ) -> Self {
        self.projections.push(MirPlaceProjection::ArrayElement {
            array,
            normalized_index,
        });
        self
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum MirPlaceBase {
    Storage(StorageId),
    AliasParameter(StorageId),
    CheckedView(StorageId),
    ArrayAlias(StorageId),
    /// The complete payload of the allocation retained by one shared owner.
    SharedPointee(StorageId),
    /// The unpublished payload under construction in allocation storage.
    SharedAllocationPayload(StorageId),
}

impl MirPlaceBase {
    pub const fn storage(self) -> StorageId {
        match self {
            Self::Storage(storage)
            | Self::AliasParameter(storage)
            | Self::CheckedView(storage)
            | Self::ArrayAlias(storage)
            | Self::SharedPointee(storage)
            | Self::SharedAllocationPayload(storage) => storage,
        }
    }
}

impl From<StorageId> for MirPlace {
    fn from(storage: StorageId) -> Self {
        Self::base(storage)
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum MirPlaceProjection {
    /// Selects the declared direct base of the current class-typed place.
    Base(ClassId),
    Field(FieldId),
    /// Selects the reserved payload bytes of an inline-class optional.
    OptionalPayload(ClassId),
    ArrayElement {
        array: ArrayTypeId,
        normalized_index: StorageId,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MirRvalue {
    pub kind: MirRvalueKind,
    pub ty: MirType,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MirRvalueKind {
    ConstantI64(i64),
    ConstantU64(u64),
    ConstantU8(u8),
    /// IEEE-754 binary64 payload, stored as raw bits for deterministic IR.
    ConstantF64Bits(u64),
    ConstantBool(bool),
    /// Reads one verified canonical path activation from its storage.
    PathCondition(MirPathConditionValue),
    Load(MirPlace),
    Unary {
        operation: MirUnaryOperation,
        operand: ValueId,
    },
    Binary {
        operation: MirBinaryOperation,
        left: ValueId,
        right: ValueId,
    },
    /// A semantic integer quotient or remainder. Verification rejects this
    /// rvalue until it participates in the matching explicit divisor check.
    IntegerDivision {
        operation: super::integer_division::MirIntegerDivisionOperation,
        dividend: ValueId,
        divisor: ValueId,
    },
    Shift {
        operation: super::shift::MirShiftOperation,
        left: ValueId,
        count: ValueId,
    },
    PrimitiveComparison {
        operation: MirPrimitiveComparison,
        left: ValueId,
        right: ValueId,
    },
    PrimitiveCast {
        operation: super::primitive::MirPrimitiveCast,
        operand: ValueId,
    },
    /// A runtime metadata query. Statically known outcomes are constants.
    TypeTest {
        source: super::instruction::MirObjectView,
        target: super::instruction::MirViewTarget,
    },
    OptionalPresence {
        source: MirPlace,
        kind: super::optional::MirPresenceTestKind,
    },
    ArrayLength {
        source: MirPlace,
        array: ArrayTypeId,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MirPathConditionValue {
    pub condition: PathConditionId,
    pub activation: StorageId,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MirUnaryOperation {
    NegateI64,
    NegateF64,
    LogicalNotBool,
    BitwiseComplement(MirIntegerType),
}

impl MirUnaryOperation {
    pub const fn operand_type(self) -> MirType {
        match self {
            Self::NegateI64 => MirType::I64,
            Self::NegateF64 => MirType::F64,
            Self::LogicalNotBool => MirType::Bool,
            Self::BitwiseComplement(integer) => integer.operand_type(),
        }
    }

    pub const fn result_type(self) -> MirType {
        self.operand_type()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MirBinaryOperation {
    AddI64,
    SubtractI64,
    MultiplyI64,
    AddU64,
    SubtractU64,
    MultiplyU64,
    AddU8,
    SubtractU8,
    MultiplyU8,
    AddF64,
    SubtractF64,
    MultiplyF64,
    DivideF64,
    IntegerBitwise {
        operation: MirIntegerBitwiseOperation,
        operand: MirIntegerType,
    },
}

impl MirBinaryOperation {
    pub const fn operand_type(self) -> MirType {
        match self {
            Self::AddI64 | Self::SubtractI64 | Self::MultiplyI64 => MirType::I64,
            Self::AddU64 | Self::SubtractU64 | Self::MultiplyU64 => MirType::U64,
            Self::AddU8 | Self::SubtractU8 | Self::MultiplyU8 => MirType::U8,
            Self::AddF64 | Self::SubtractF64 | Self::MultiplyF64 | Self::DivideF64 => MirType::F64,
            Self::IntegerBitwise { operand, .. } => operand.operand_type(),
        }
    }

    pub const fn result_type(self) -> MirType {
        self.operand_type()
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum MirIntegerBitwiseOperation {
    And,
    Or,
    Xor,
}

impl MirIntegerBitwiseOperation {
    pub const fn mnemonic(self) -> &'static str {
        match self {
            Self::And => "and",
            Self::Or => "or",
            Self::Xor => "xor",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum MirIntegerType {
    I64,
    U64,
    U8,
}

impl MirIntegerType {
    pub const fn operand_type(self) -> MirType {
        match self {
            Self::I64 => MirType::I64,
            Self::U64 => MirType::U64,
            Self::U8 => MirType::U8,
        }
    }

    pub const fn name(self) -> &'static str {
        match self {
            Self::I64 => "i64",
            Self::U64 => "u64",
            Self::U8 => "u8",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum MirComparisonPredicate {
    Equal,
    NotEqual,
    LessThan,
    LessEqual,
    GreaterThan,
    GreaterEqual,
}

impl MirComparisonPredicate {
    pub const fn mnemonic(self) -> &'static str {
        match self {
            Self::Equal => "eq",
            Self::NotEqual => "ne",
            Self::LessThan => "lt",
            Self::LessEqual => "le",
            Self::GreaterThan => "gt",
            Self::GreaterEqual => "ge",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum MirComparisonOperand {
    Integer(MirIntegerType),
    F64,
    Bool,
}

impl MirComparisonOperand {
    pub const fn operand_type(self) -> MirType {
        match self {
            Self::Integer(integer) => integer.operand_type(),
            Self::F64 => MirType::F64,
            Self::Bool => MirType::Bool,
        }
    }

    pub const fn name(self) -> &'static str {
        match self {
            Self::Integer(integer) => integer.name(),
            Self::F64 => "f64",
            Self::Bool => "bool",
        }
    }

    pub const fn supports_predicate(self, predicate: MirComparisonPredicate) -> bool {
        match self {
            Self::Integer(_) | Self::F64 => true,
            Self::Bool => matches!(
                predicate,
                MirComparisonPredicate::Equal | MirComparisonPredicate::NotEqual
            ),
        }
    }
}

impl From<MirIntegerType> for MirComparisonOperand {
    fn from(integer: MirIntegerType) -> Self {
        Self::Integer(integer)
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct MirPrimitiveComparison {
    pub predicate: MirComparisonPredicate,
    pub operand: MirComparisonOperand,
}

impl MirPrimitiveComparison {
    pub const fn operand_type(self) -> MirType {
        self.operand.operand_type()
    }

    pub const fn result_type(self) -> MirType {
        MirType::Bool
    }

    pub const fn is_valid(self) -> bool {
        self.operand.supports_predicate(self.predicate)
    }
}
