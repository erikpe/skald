//! Compiler-owned static evidence for canonical primitive operator protocols.

use super::{
    CanonicalOperatorProtocol, CanonicalOperatorProtocolShape, ResolvedPrimitiveType,
    ResolvedProgram, ResolvedTypeKind,
};
use crate::identity::InterfaceId;

/// One existing target-independent primitive operation.
///
/// The variants preserve the semantic distinctions needed by specialization:
/// integer division and shifts are checked operations, while floating division
/// and ordinary arithmetic are scalar operations. No object or witness identity
/// is represented here.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ResolvedPrimitiveOperatorOperation {
    Negate(ResolvedPrimitiveType),
    BitwiseComplement(ResolvedPrimitiveType),
    Compare {
        predicate: CanonicalOperatorProtocol,
        operand: ResolvedPrimitiveType,
    },
    Arithmetic {
        operator: CanonicalOperatorProtocol,
        operand: ResolvedPrimitiveType,
    },
    CheckedIntegerDivision {
        remainder: bool,
        operand: ResolvedPrimitiveType,
    },
    IntegerBitwise {
        operator: CanonicalOperatorProtocol,
        operand: ResolvedPrimitiveType,
    },
    CheckedShift {
        operator: CanonicalOperatorProtocol,
        left: ResolvedPrimitiveType,
    },
}

impl ResolvedPrimitiveOperatorOperation {
    pub const fn protocol(self) -> CanonicalOperatorProtocol {
        match self {
            Self::Negate(_) => CanonicalOperatorProtocol::Neg,
            Self::BitwiseComplement(_) => CanonicalOperatorProtocol::BitNot,
            Self::Compare { predicate, .. }
            | Self::Arithmetic {
                operator: predicate,
                ..
            }
            | Self::IntegerBitwise {
                operator: predicate,
                ..
            }
            | Self::CheckedShift {
                operator: predicate,
                ..
            } => predicate,
            Self::CheckedIntegerDivision {
                remainder: false, ..
            } => CanonicalOperatorProtocol::Div,
            Self::CheckedIntegerDivision {
                remainder: true, ..
            } => CanonicalOperatorProtocol::Rem,
        }
    }

    pub const fn receiver(self) -> ResolvedPrimitiveType {
        match self {
            Self::Negate(operand)
            | Self::BitwiseComplement(operand)
            | Self::Compare { operand, .. }
            | Self::Arithmetic { operand, .. }
            | Self::CheckedIntegerDivision { operand, .. }
            | Self::IntegerBitwise { operand, .. } => operand,
            Self::CheckedShift { left, .. } => left,
        }
    }

    pub const fn rhs(self) -> Option<ResolvedPrimitiveType> {
        match self {
            Self::Negate(_) | Self::BitwiseComplement(_) => None,
            Self::CheckedShift { .. } => Some(ResolvedPrimitiveType::U64),
            Self::Compare { operand, .. }
            | Self::Arithmetic { operand, .. }
            | Self::CheckedIntegerDivision { operand, .. }
            | Self::IntegerBitwise { operand, .. } => Some(operand),
        }
    }

    pub const fn output(self) -> ResolvedPrimitiveType {
        match self {
            Self::Compare { .. } => ResolvedPrimitiveType::Bool,
            _ => self.receiver(),
        }
    }

    pub fn semantic_name(self) -> String {
        let suffix = primitive_suffix(self.receiver());
        match self {
            Self::Negate(_) => format!("Negate{suffix}"),
            Self::BitwiseComplement(_) => format!("BitwiseComplement{suffix}"),
            Self::Compare { predicate, .. } => {
                format!("{}{suffix}", protocol_stem(predicate))
            }
            Self::Arithmetic { operator, .. }
            | Self::IntegerBitwise { operator, .. }
            | Self::CheckedShift { operator, .. } => {
                format!("{}{suffix}", protocol_stem(operator))
            }
            Self::CheckedIntegerDivision {
                remainder: false, ..
            } => format!("Divide{suffix}"),
            Self::CheckedIntegerDivision {
                remainder: true, ..
            } => format!("Remainder{suffix}"),
        }
    }
}

/// One registry entry. Its closed application key is derived from the semantic
/// operation, preventing result and operand declarations from drifting apart.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ResolvedPrimitiveOperatorEvidence {
    pub operation: ResolvedPrimitiveOperatorOperation,
    receiver: ResolvedPrimitiveType,
    protocol: CanonicalOperatorProtocol,
    rhs: Option<ResolvedPrimitiveType>,
    output: ResolvedPrimitiveType,
}

impl ResolvedPrimitiveOperatorEvidence {
    pub const fn receiver(self) -> ResolvedPrimitiveType {
        self.receiver
    }

    pub const fn protocol(self) -> CanonicalOperatorProtocol {
        self.protocol
    }

    pub const fn rhs(self) -> Option<ResolvedPrimitiveType> {
        self.rhs
    }

    pub const fn output(self) -> ResolvedPrimitiveType {
        self.output
    }
}

const fn evidence(
    operation: ResolvedPrimitiveOperatorOperation,
) -> ResolvedPrimitiveOperatorEvidence {
    ResolvedPrimitiveOperatorEvidence {
        operation,
        receiver: operation.receiver(),
        protocol: operation.protocol(),
        rhs: operation.rhs(),
        output: operation.output(),
    }
}

const I64: ResolvedPrimitiveType = ResolvedPrimitiveType::I64;
const U64: ResolvedPrimitiveType = ResolvedPrimitiveType::U64;
const U8: ResolvedPrimitiveType = ResolvedPrimitiveType::U8;
const F64: ResolvedPrimitiveType = ResolvedPrimitiveType::F64;
const BOOL: ResolvedPrimitiveType = ResolvedPrimitiveType::Bool;

const PRIMITIVE_OPERATOR_REGISTRY: [ResolvedPrimitiveOperatorEvidence; 60] = [
    evidence(ResolvedPrimitiveOperatorOperation::Negate(I64)),
    evidence(ResolvedPrimitiveOperatorOperation::Negate(F64)),
    evidence(ResolvedPrimitiveOperatorOperation::BitwiseComplement(I64)),
    evidence(ResolvedPrimitiveOperatorOperation::BitwiseComplement(U64)),
    evidence(ResolvedPrimitiveOperatorOperation::BitwiseComplement(U8)),
    evidence(ResolvedPrimitiveOperatorOperation::Compare {
        predicate: CanonicalOperatorProtocol::Eq,
        operand: I64,
    }),
    evidence(ResolvedPrimitiveOperatorOperation::Compare {
        predicate: CanonicalOperatorProtocol::Eq,
        operand: U64,
    }),
    evidence(ResolvedPrimitiveOperatorOperation::Compare {
        predicate: CanonicalOperatorProtocol::Eq,
        operand: U8,
    }),
    evidence(ResolvedPrimitiveOperatorOperation::Compare {
        predicate: CanonicalOperatorProtocol::Eq,
        operand: F64,
    }),
    evidence(ResolvedPrimitiveOperatorOperation::Compare {
        predicate: CanonicalOperatorProtocol::Eq,
        operand: BOOL,
    }),
    evidence(ResolvedPrimitiveOperatorOperation::Compare {
        predicate: CanonicalOperatorProtocol::Less,
        operand: I64,
    }),
    evidence(ResolvedPrimitiveOperatorOperation::Compare {
        predicate: CanonicalOperatorProtocol::Less,
        operand: U64,
    }),
    evidence(ResolvedPrimitiveOperatorOperation::Compare {
        predicate: CanonicalOperatorProtocol::Less,
        operand: U8,
    }),
    evidence(ResolvedPrimitiveOperatorOperation::Compare {
        predicate: CanonicalOperatorProtocol::Less,
        operand: F64,
    }),
    evidence(ResolvedPrimitiveOperatorOperation::Compare {
        predicate: CanonicalOperatorProtocol::LessEq,
        operand: I64,
    }),
    evidence(ResolvedPrimitiveOperatorOperation::Compare {
        predicate: CanonicalOperatorProtocol::LessEq,
        operand: U64,
    }),
    evidence(ResolvedPrimitiveOperatorOperation::Compare {
        predicate: CanonicalOperatorProtocol::LessEq,
        operand: U8,
    }),
    evidence(ResolvedPrimitiveOperatorOperation::Compare {
        predicate: CanonicalOperatorProtocol::LessEq,
        operand: F64,
    }),
    evidence(ResolvedPrimitiveOperatorOperation::Compare {
        predicate: CanonicalOperatorProtocol::Greater,
        operand: I64,
    }),
    evidence(ResolvedPrimitiveOperatorOperation::Compare {
        predicate: CanonicalOperatorProtocol::Greater,
        operand: U64,
    }),
    evidence(ResolvedPrimitiveOperatorOperation::Compare {
        predicate: CanonicalOperatorProtocol::Greater,
        operand: U8,
    }),
    evidence(ResolvedPrimitiveOperatorOperation::Compare {
        predicate: CanonicalOperatorProtocol::Greater,
        operand: F64,
    }),
    evidence(ResolvedPrimitiveOperatorOperation::Compare {
        predicate: CanonicalOperatorProtocol::GreaterEq,
        operand: I64,
    }),
    evidence(ResolvedPrimitiveOperatorOperation::Compare {
        predicate: CanonicalOperatorProtocol::GreaterEq,
        operand: U64,
    }),
    evidence(ResolvedPrimitiveOperatorOperation::Compare {
        predicate: CanonicalOperatorProtocol::GreaterEq,
        operand: U8,
    }),
    evidence(ResolvedPrimitiveOperatorOperation::Compare {
        predicate: CanonicalOperatorProtocol::GreaterEq,
        operand: F64,
    }),
    evidence(ResolvedPrimitiveOperatorOperation::Arithmetic {
        operator: CanonicalOperatorProtocol::Add,
        operand: I64,
    }),
    evidence(ResolvedPrimitiveOperatorOperation::Arithmetic {
        operator: CanonicalOperatorProtocol::Add,
        operand: U64,
    }),
    evidence(ResolvedPrimitiveOperatorOperation::Arithmetic {
        operator: CanonicalOperatorProtocol::Add,
        operand: U8,
    }),
    evidence(ResolvedPrimitiveOperatorOperation::Arithmetic {
        operator: CanonicalOperatorProtocol::Add,
        operand: F64,
    }),
    evidence(ResolvedPrimitiveOperatorOperation::Arithmetic {
        operator: CanonicalOperatorProtocol::Sub,
        operand: I64,
    }),
    evidence(ResolvedPrimitiveOperatorOperation::Arithmetic {
        operator: CanonicalOperatorProtocol::Sub,
        operand: U64,
    }),
    evidence(ResolvedPrimitiveOperatorOperation::Arithmetic {
        operator: CanonicalOperatorProtocol::Sub,
        operand: U8,
    }),
    evidence(ResolvedPrimitiveOperatorOperation::Arithmetic {
        operator: CanonicalOperatorProtocol::Sub,
        operand: F64,
    }),
    evidence(ResolvedPrimitiveOperatorOperation::Arithmetic {
        operator: CanonicalOperatorProtocol::Mul,
        operand: I64,
    }),
    evidence(ResolvedPrimitiveOperatorOperation::Arithmetic {
        operator: CanonicalOperatorProtocol::Mul,
        operand: U64,
    }),
    evidence(ResolvedPrimitiveOperatorOperation::Arithmetic {
        operator: CanonicalOperatorProtocol::Mul,
        operand: U8,
    }),
    evidence(ResolvedPrimitiveOperatorOperation::Arithmetic {
        operator: CanonicalOperatorProtocol::Mul,
        operand: F64,
    }),
    evidence(ResolvedPrimitiveOperatorOperation::CheckedIntegerDivision {
        remainder: false,
        operand: I64,
    }),
    evidence(ResolvedPrimitiveOperatorOperation::CheckedIntegerDivision {
        remainder: false,
        operand: U64,
    }),
    evidence(ResolvedPrimitiveOperatorOperation::CheckedIntegerDivision {
        remainder: false,
        operand: U8,
    }),
    evidence(ResolvedPrimitiveOperatorOperation::Arithmetic {
        operator: CanonicalOperatorProtocol::Div,
        operand: F64,
    }),
    evidence(ResolvedPrimitiveOperatorOperation::CheckedIntegerDivision {
        remainder: true,
        operand: I64,
    }),
    evidence(ResolvedPrimitiveOperatorOperation::CheckedIntegerDivision {
        remainder: true,
        operand: U64,
    }),
    evidence(ResolvedPrimitiveOperatorOperation::CheckedIntegerDivision {
        remainder: true,
        operand: U8,
    }),
    evidence(ResolvedPrimitiveOperatorOperation::IntegerBitwise {
        operator: CanonicalOperatorProtocol::BitAnd,
        operand: I64,
    }),
    evidence(ResolvedPrimitiveOperatorOperation::IntegerBitwise {
        operator: CanonicalOperatorProtocol::BitAnd,
        operand: U64,
    }),
    evidence(ResolvedPrimitiveOperatorOperation::IntegerBitwise {
        operator: CanonicalOperatorProtocol::BitAnd,
        operand: U8,
    }),
    evidence(ResolvedPrimitiveOperatorOperation::IntegerBitwise {
        operator: CanonicalOperatorProtocol::BitOr,
        operand: I64,
    }),
    evidence(ResolvedPrimitiveOperatorOperation::IntegerBitwise {
        operator: CanonicalOperatorProtocol::BitOr,
        operand: U64,
    }),
    evidence(ResolvedPrimitiveOperatorOperation::IntegerBitwise {
        operator: CanonicalOperatorProtocol::BitOr,
        operand: U8,
    }),
    evidence(ResolvedPrimitiveOperatorOperation::IntegerBitwise {
        operator: CanonicalOperatorProtocol::BitXor,
        operand: I64,
    }),
    evidence(ResolvedPrimitiveOperatorOperation::IntegerBitwise {
        operator: CanonicalOperatorProtocol::BitXor,
        operand: U64,
    }),
    evidence(ResolvedPrimitiveOperatorOperation::IntegerBitwise {
        operator: CanonicalOperatorProtocol::BitXor,
        operand: U8,
    }),
    evidence(ResolvedPrimitiveOperatorOperation::CheckedShift {
        operator: CanonicalOperatorProtocol::ShiftLeft,
        left: I64,
    }),
    evidence(ResolvedPrimitiveOperatorOperation::CheckedShift {
        operator: CanonicalOperatorProtocol::ShiftLeft,
        left: U64,
    }),
    evidence(ResolvedPrimitiveOperatorOperation::CheckedShift {
        operator: CanonicalOperatorProtocol::ShiftLeft,
        left: U8,
    }),
    evidence(ResolvedPrimitiveOperatorOperation::CheckedShift {
        operator: CanonicalOperatorProtocol::ShiftRight,
        left: I64,
    }),
    evidence(ResolvedPrimitiveOperatorOperation::CheckedShift {
        operator: CanonicalOperatorProtocol::ShiftRight,
        left: U64,
    }),
    evidence(ResolvedPrimitiveOperatorOperation::CheckedShift {
        operator: CanonicalOperatorProtocol::ShiftRight,
        left: U8,
    }),
];

pub(crate) fn primitive_operator_registry() -> &'static [ResolvedPrimitiveOperatorEvidence] {
    debug_assert!(validate_primitive_operator_registry(&PRIMITIVE_OPERATOR_REGISTRY).is_ok());
    &PRIMITIVE_OPERATOR_REGISTRY
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PrimitiveOperatorRegistryError {
    WrongEntryCount { actual: usize },
    DuplicateKey { first: usize, duplicate: usize },
    UnsupportedCell { index: usize },
    OperationMismatch { index: usize },
}

fn validate_primitive_operator_registry(
    registry: &[ResolvedPrimitiveOperatorEvidence],
) -> Result<(), PrimitiveOperatorRegistryError> {
    if registry.len() != PRIMITIVE_OPERATOR_REGISTRY.len() {
        return Err(PrimitiveOperatorRegistryError::WrongEntryCount {
            actual: registry.len(),
        });
    }
    for (index, entry) in registry.iter().copied().enumerate() {
        if entry.receiver != entry.operation.receiver()
            || entry.protocol != entry.operation.protocol()
            || entry.rhs != entry.operation.rhs()
            || entry.output != entry.operation.output()
        {
            return Err(PrimitiveOperatorRegistryError::OperationMismatch { index });
        }
        if !supported_cell(entry.receiver, entry.protocol, entry.rhs, entry.output) {
            return Err(PrimitiveOperatorRegistryError::UnsupportedCell { index });
        }
        if let Some(first) = registry[..index].iter().position(|previous| {
            previous.receiver == entry.receiver
                && previous.protocol == entry.protocol
                && previous.rhs == entry.rhs
                && previous.output == entry.output
        }) {
            return Err(PrimitiveOperatorRegistryError::DuplicateKey {
                first,
                duplicate: index,
            });
        }
    }
    Ok(())
}

const fn supported_cell(
    receiver: ResolvedPrimitiveType,
    protocol: CanonicalOperatorProtocol,
    rhs: Option<ResolvedPrimitiveType>,
    output: ResolvedPrimitiveType,
) -> bool {
    let integer = matches!(
        receiver,
        ResolvedPrimitiveType::I64 | ResolvedPrimitiveType::U64 | ResolvedPrimitiveType::U8
    );
    let numeric = !matches!(receiver, ResolvedPrimitiveType::Bool);
    let same_binary = matches!(rhs, Some(actual) if primitive_types_equal(actual, receiver))
        && primitive_types_equal(output, receiver);
    match protocol {
        CanonicalOperatorProtocol::Neg => {
            rhs.is_none()
                && primitive_types_equal(output, receiver)
                && matches!(
                    receiver,
                    ResolvedPrimitiveType::I64 | ResolvedPrimitiveType::F64
                )
        }
        CanonicalOperatorProtocol::BitNot => {
            rhs.is_none() && primitive_types_equal(output, receiver) && integer
        }
        CanonicalOperatorProtocol::Eq => {
            matches!(rhs, Some(actual) if primitive_types_equal(actual, receiver))
                && matches!(output, ResolvedPrimitiveType::Bool)
        }
        CanonicalOperatorProtocol::Less
        | CanonicalOperatorProtocol::LessEq
        | CanonicalOperatorProtocol::Greater
        | CanonicalOperatorProtocol::GreaterEq => {
            numeric
                && matches!(rhs, Some(actual) if primitive_types_equal(actual, receiver))
                && matches!(output, ResolvedPrimitiveType::Bool)
        }
        CanonicalOperatorProtocol::Add
        | CanonicalOperatorProtocol::Sub
        | CanonicalOperatorProtocol::Mul
        | CanonicalOperatorProtocol::Div => numeric && same_binary,
        CanonicalOperatorProtocol::Rem
        | CanonicalOperatorProtocol::BitAnd
        | CanonicalOperatorProtocol::BitOr
        | CanonicalOperatorProtocol::BitXor => integer && same_binary,
        CanonicalOperatorProtocol::ShiftLeft | CanonicalOperatorProtocol::ShiftRight => {
            integer
                && matches!(rhs, Some(ResolvedPrimitiveType::U64))
                && primitive_types_equal(output, receiver)
        }
    }
}

const fn primitive_types_equal(left: ResolvedPrimitiveType, right: ResolvedPrimitiveType) -> bool {
    matches!(
        (left, right),
        (ResolvedPrimitiveType::I64, ResolvedPrimitiveType::I64)
            | (ResolvedPrimitiveType::U64, ResolvedPrimitiveType::U64)
            | (ResolvedPrimitiveType::U8, ResolvedPrimitiveType::U8)
            | (ResolvedPrimitiveType::F64, ResolvedPrimitiveType::F64)
            | (ResolvedPrimitiveType::Bool, ResolvedPrimitiveType::Bool)
    )
}

/// Finds static evidence only when `interface` is an exact closed application
/// of one validated canonical operator template.
pub(crate) fn primitive_operator_evidence(
    program: &ResolvedProgram,
    receiver: ResolvedTypeKind,
    interface: InterfaceId,
) -> Option<ResolvedPrimitiveOperatorEvidence> {
    let receiver = primitive_type(receiver)?;
    let item = program.operator_language_item.as_ref()?;
    let application = program
        .generic_interface_specializations
        .for_interface(interface)?;
    let protocol = item
        .iter()
        .find(|protocol| protocol.template == application.key.template)?;

    primitive_operator_registry()
        .iter()
        .copied()
        .find(|evidence| {
            evidence.receiver() == receiver
                && evidence.protocol() == protocol.kind
                && application_arguments_match(*evidence, &application.key.arguments)
        })
}

pub(crate) fn canonical_operator_application(
    program: &ResolvedProgram,
    interface: InterfaceId,
) -> bool {
    let Some(item) = program.operator_language_item.as_ref() else {
        return false;
    };
    let Some(application) = program
        .generic_interface_specializations
        .for_interface(interface)
    else {
        return false;
    };
    item.iter()
        .any(|protocol| protocol.template == application.key.template)
}

fn application_arguments_match(
    evidence: ResolvedPrimitiveOperatorEvidence,
    arguments: &[ResolvedTypeKind],
) -> bool {
    let primitive = |kind| primitive_type(kind);
    match evidence.protocol().shape() {
        CanonicalOperatorProtocolShape::Unary => {
            arguments == [ResolvedTypeKind::from(evidence.output())]
        }
        CanonicalOperatorProtocolShape::Predicate => {
            arguments.len() == 1 && primitive(arguments[0]) == evidence.rhs()
        }
        CanonicalOperatorProtocolShape::Binary => {
            arguments.len() == 2
                && primitive(arguments[0]) == evidence.rhs()
                && primitive(arguments[1]) == Some(evidence.output())
        }
    }
}

const fn primitive_type(kind: ResolvedTypeKind) -> Option<ResolvedPrimitiveType> {
    match kind {
        ResolvedTypeKind::I64 => Some(I64),
        ResolvedTypeKind::U64 => Some(U64),
        ResolvedTypeKind::U8 => Some(U8),
        ResolvedTypeKind::F64 => Some(F64),
        ResolvedTypeKind::Bool => Some(BOOL),
        _ => None,
    }
}

impl From<ResolvedPrimitiveType> for ResolvedTypeKind {
    fn from(primitive: ResolvedPrimitiveType) -> Self {
        match primitive {
            ResolvedPrimitiveType::I64 => Self::I64,
            ResolvedPrimitiveType::U64 => Self::U64,
            ResolvedPrimitiveType::U8 => Self::U8,
            ResolvedPrimitiveType::F64 => Self::F64,
            ResolvedPrimitiveType::Bool => Self::Bool,
        }
    }
}

fn primitive_suffix(primitive: ResolvedPrimitiveType) -> &'static str {
    match primitive {
        ResolvedPrimitiveType::I64 => "I64",
        ResolvedPrimitiveType::U64 => "U64",
        ResolvedPrimitiveType::U8 => "U8",
        ResolvedPrimitiveType::F64 => "F64",
        ResolvedPrimitiveType::Bool => "Bool",
    }
}

fn protocol_stem(protocol: CanonicalOperatorProtocol) -> &'static str {
    match protocol {
        CanonicalOperatorProtocol::Neg => "Negate",
        CanonicalOperatorProtocol::BitNot => "BitwiseComplement",
        CanonicalOperatorProtocol::Eq => "Equal",
        CanonicalOperatorProtocol::Less => "Less",
        CanonicalOperatorProtocol::LessEq => "LessEqual",
        CanonicalOperatorProtocol::Greater => "Greater",
        CanonicalOperatorProtocol::GreaterEq => "GreaterEqual",
        CanonicalOperatorProtocol::Add => "Add",
        CanonicalOperatorProtocol::Sub => "Subtract",
        CanonicalOperatorProtocol::Mul => "Multiply",
        CanonicalOperatorProtocol::Div => "Divide",
        CanonicalOperatorProtocol::Rem => "Remainder",
        CanonicalOperatorProtocol::BitAnd => "BitwiseAnd",
        CanonicalOperatorProtocol::BitOr => "BitwiseOr",
        CanonicalOperatorProtocol::BitXor => "BitwiseXor",
        CanonicalOperatorProtocol::ShiftLeft => "ShiftLeft",
        CanonicalOperatorProtocol::ShiftRight => "ShiftRight",
    }
}

#[cfg(test)]
mod tests;
