//! MIR value metadata, rvalues, and storage places.

use std::fmt;

use crate::{
    identity::{ClassId, FieldId, InterfaceId},
    source::Span,
};

use super::ids::{StorageId, ValueId};

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum MirType {
    I64,
    U64,
    U8,
    F64,
    Bool,
    Class(ClassId),
    /// A non-owning interface-view target. It is valid only for alias storage
    /// and never materializes as a scalar or inline object.
    Interface(InterfaceId),
    /// The universal non-owning object-view target. It has no owning storage
    /// or target layout of its own.
    Obj,
    /// A non-null strong owner carrying one object view of a live allocation.
    Shared(super::shared::MirSharedTarget),
    Unit,
}

impl MirType {
    pub const fn is_scalar_value(self) -> bool {
        !matches!(
            self,
            Self::Class(_) | Self::Interface(_) | Self::Obj | Self::Shared(_) | Self::Unit
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
            Self::Class(class) => write!(formatter, "class {class}"),
            Self::Interface(interface) => write!(formatter, "interface {interface}"),
            Self::Obj => formatter.write_str("Obj"),
            Self::Shared(target) => write!(formatter, "shared {target}"),
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

    pub fn project_field(mut self, field: FieldId) -> Self {
        self.projections.push(MirPlaceProjection::Field(field));
        self
    }

    pub fn project_base(mut self, base: ClassId) -> Self {
        self.projections.push(MirPlaceProjection::Base(base));
        self
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum MirPlaceBase {
    Storage(StorageId),
    AliasParameter(StorageId),
    CheckedView(StorageId),
}

impl MirPlaceBase {
    pub const fn storage(self) -> StorageId {
        match self {
            Self::Storage(storage) | Self::AliasParameter(storage) | Self::CheckedView(storage) => {
                storage
            }
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
    /// A runtime metadata query. Statically known outcomes are constants.
    TypeTest {
        source: super::instruction::MirObjectView,
        target: super::instruction::MirViewTarget,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MirUnaryOperation {
    NegateI64,
    NegateF64,
}

impl MirUnaryOperation {
    pub const fn operand_type(self) -> MirType {
        match self {
            Self::NegateI64 => MirType::I64,
            Self::NegateF64 => MirType::F64,
        }
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
}

impl MirBinaryOperation {
    pub const fn operand_type(self) -> MirType {
        match self {
            Self::AddI64 | Self::SubtractI64 | Self::MultiplyI64 => MirType::I64,
            Self::AddU64 | Self::SubtractU64 | Self::MultiplyU64 => MirType::U64,
            Self::AddU8 | Self::SubtractU8 | Self::MultiplyU8 => MirType::U8,
            Self::AddF64 | Self::SubtractF64 | Self::MultiplyF64 => MirType::F64,
        }
    }
}
