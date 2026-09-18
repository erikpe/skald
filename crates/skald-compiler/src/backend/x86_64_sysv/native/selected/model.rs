use super::super::NativeResources;
use crate::{
    backend::{
        effects::{Effect, Effects, MemoryRegion},
        graph::{LoweredBlockId, SelectedObjectId, SelectedValueId},
        lir::{BinaryOperation, Constant, LifetimeMarker, UnaryOperation},
        plan::{ArtifactCategory, ArtifactId},
        selected::{AbiBinding, BankKind, Representation, UnitId, ViewId},
    },
    primitive_comparison::PrimitiveComparisonPredicate,
    source::Span,
};

#[derive(Clone, Copy, Debug)]
pub(in crate::backend) struct ValueRef {
    pub value: SelectedValueId,
    pub representation: Representation,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::backend) enum Site {
    Instruction {
        block: LoweredBlockId,
        ordinal: usize,
    },
    Terminator(LoweredBlockId),
    Edge {
        block: LoweredBlockId,
        slot: usize,
    },
}
#[derive(Clone, Copy, Debug)]
pub(in crate::backend) struct Origin {
    pub site: Site,
    pub span: Option<Span>,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::backend) enum IntegerCondition {
    Equal,
    NotEqual,
    Less,
    LessEqual,
    Greater,
    GreaterEqual,
    Below,
    BelowEqual,
    Above,
    AboveEqual,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::backend) enum FloatCondition {
    EqualOrdered,
    NotEqualOrUnordered,
    BelowOrdered,
    BelowEqualOrdered,
    Above,
    AboveEqual,
}
impl FloatCondition {
    pub(super) fn parity(self) -> bool {
        !matches!(self, Self::Above | Self::AboveEqual)
    }
}

/// No frame offsets, value homes, MIR operands, or mutable descriptor tables.
#[derive(Clone, Debug)]
pub(in crate::backend) enum Opcode {
    Call(Box<super::calls::NativeCall>),
    TlsAddress {
        out: ValueRef,
    },
    TraceLoad {
        address: ValueRef,
        out: ValueRef,
        record: Option<SelectedObjectId>,
    },
    TraceStore {
        address: ValueRef,
        value: ValueRef,
        record: Option<SelectedObjectId>,
    },
    HardTrap,
    Numeric(super::numeric::Numeric),
    CheckBranch {
        condition: ValueRef,
        relation: crate::backend::lir::ScalarCheck<SelectedValueId>,
    },
    Failure {
        reason: crate::backend::failure::FailureMessage,
        signature: crate::backend::plan::SignatureId,
        arguments: Vec<ValueRef>,
        bindings: Vec<AbiBinding>,
        attribution: crate::backend::lir::CallAttribution,
    },
    Constant {
        constant: Constant,
        out: ValueRef,
    },
    Unary {
        operation: UnaryOperation,
        input: ValueRef,
        out: ValueRef,
    },
    Alu {
        operation: BinaryOperation,
        left: ValueRef,
        right: ValueRef,
        out: ValueRef,
    },
    IntegerCompare {
        condition: IntegerCondition,
        predicate: PrimitiveComparisonPredicate,
        signed: bool,
        left: ValueRef,
        right: ValueRef,
        out: ValueRef,
    },
    FloatCompare {
        condition: FloatCondition,
        predicate: PrimitiveComparisonPredicate,
        left: ValueRef,
        right: ValueRef,
        out: ValueRef,
    },
    SymbolAddress {
        symbol: ArtifactId,
        out: ValueRef,
    },
    ObjectAddress {
        object: SelectedObjectId,
        out: ValueRef,
    },
    ByteOffset {
        base: ValueRef,
        offset: ValueRef,
        out: ValueRef,
    },
    Load {
        address: ValueRef,
        out: ValueRef,
        bytes: usize,
        alignment: usize,
        region: MemoryRegion<SelectedObjectId>,
    },
    Store {
        address: ValueRef,
        value: ValueRef,
        bytes: usize,
        alignment: usize,
        region: MemoryRegion<SelectedObjectId>,
    },
    Lifetime {
        object: SelectedObjectId,
        marker: LifetimeMarker,
        site: usize,
        sites: usize,
    },
    Jump,
    Branch {
        condition: ValueRef,
    },
    Return {
        values: Vec<ValueRef>,
        bindings: Vec<AbiBinding>,
    },
}

#[derive(Clone)]
pub(in crate::backend) struct Instruction {
    pub(super) opcode: Opcode,
    pub(super) origin: Origin,
    pub(super) resources: std::sync::Arc<NativeResources>,
    pub(super) effects: Effects<SelectedObjectId>,
    pub(super) artifacts: Vec<(ArtifactId, ArtifactCategory)>,
    pub(super) objects: Vec<SelectedObjectId>,
    pub(super) clobbers: Vec<(crate::backend::selected::Timing, UnitId)>,
}
impl Instruction {
    pub(in crate::backend) fn new(
        opcode: Opcode,
        origin: Origin,
        resources: &std::sync::Arc<NativeResources>,
    ) -> Self {
        let mut instruction = Self {
            opcode,
            origin,
            resources: resources.clone(),
            effects: Effects::default(),
            artifacts: vec![],
            objects: vec![],
            clobbers: vec![],
        };
        instruction.refresh();
        instruction
    }
    pub(in crate::backend) fn opcode(&self) -> &Opcode {
        &self.opcode
    }
    pub(in crate::backend) fn origin(&self) -> Origin {
        self.origin
    }

    pub(super) fn views(&self, representation: Representation) -> &[ViewId] {
        self.resources
            .allocatable(representation.bank(), representation.bits())
    }

    pub(super) fn refresh(&mut self) {
        self.effects = match self.opcode {
            Opcode::TlsAddress { .. } => Effects::new([Effect::Read(MemoryRegion::Unknown)]),
            Opcode::Call(ref call) => call.effects(),
            Opcode::HardTrap => Effects::new([Effect::HardTrap]),
            Opcode::TraceLoad { record, .. } => Effects::new([
                Effect::Read(record.map_or(MemoryRegion::Unknown, MemoryRegion::Object)),
                Effect::TraceState,
            ]),
            Opcode::TraceStore { record, .. } => Effects::new([
                Effect::Write(record.map_or(MemoryRegion::Unknown, MemoryRegion::Object)),
                Effect::TraceState,
            ]),
            Opcode::Failure { .. } => Effects::new([
                Effect::Call,
                Effect::Read(MemoryRegion::Unknown),
                Effect::Report,
                Effect::TraceState,
                Effect::HardTrap,
            ]),
            Opcode::Load { region, .. } => Effects::new([Effect::Read(region)]),
            Opcode::Store { region, .. } => Effects::new([Effect::Write(region)]),
            _ => Effects::default(),
        };
        self.artifacts = match self.opcode {
            Opcode::Call(ref call) => call.artifacts(),
            Opcode::TlsAddress { .. } | Opcode::TraceLoad { .. } | Opcode::TraceStore { .. } => {
                vec![(ArtifactId::TraceTls, ArtifactCategory::Tls)]
            }
            Opcode::Failure {
                reason,
                ref attribution,
                ..
            } => {
                let panic = ArtifactId::Runtime(crate::backend::plan::RuntimeService::Panic);
                let message =
                    ArtifactId::Data(crate::backend::plan::DataKey::FailureMessage(reason));
                let mut refs = vec![(panic, panic.category()), (message, message.category())];
                if let crate::backend::lir::CallAttribution::SourceOperation {
                    location: Some(location),
                    ..
                } = attribution
                {
                    refs.push((*location, location.category()));
                }
                refs
            }
            Opcode::SymbolAddress { symbol, .. } => vec![(symbol, symbol.category())],
            _ => vec![],
        };
        self.objects = match self.opcode {
            Opcode::TraceLoad {
                record: Some(object),
                ..
            }
            | Opcode::TraceStore {
                record: Some(object),
                ..
            } => vec![object],
            Opcode::ObjectAddress { object, .. } | Opcode::Lifetime { object, .. } => vec![object],
            Opcode::Load {
                region: MemoryRegion::Object(object),
                ..
            }
            | Opcode::Store {
                region: MemoryRegion::Object(object),
                ..
            } => vec![object],
            _ => vec![],
        };
        self.clobbers = if matches!(self.opcode, Opcode::Failure { .. } | Opcode::Call(_)) {
            self.resources
                .caller_clobbers()
                .iter()
                .map(|u| (crate::backend::selected::Timing::Late, *u))
                .collect()
        } else if self.writes_flags() {
            vec![(
                crate::backend::selected::Timing::Late,
                self.resources.flags(),
            )]
        } else {
            vec![]
        };
    }
    pub(super) fn writes_flags(&self) -> bool {
        match self.opcode {
            Opcode::CheckBranch { .. }
            | Opcode::Numeric(
                super::numeric::Numeric::Divide { .. }
                | super::numeric::Numeric::Shift { .. }
                | super::numeric::Numeric::ShiftOne { .. },
            ) => true,
            Opcode::Alu { left, .. } => left.representation.bank() == BankKind::Integer,
            Opcode::Unary {
                operation: UnaryOperation::Negate,
                input,
                ..
            } => {
                input.representation.bank() == BankKind::Integer
                    || input.representation.bank() == BankKind::Float
            }
            Opcode::Unary {
                operation: UnaryOperation::LogicalNot,
                ..
            }
            | Opcode::IntegerCompare { .. }
            | Opcode::FloatCompare { .. }
            | Opcode::Branch { .. }
            | Opcode::ByteOffset { .. } => true,
            _ => false,
        }
    }
    /// Stable descriptor positions derived solely from opcode fields.
    pub(super) fn operands(&self) -> Vec<(ValueRef, bool)> {
        match &self.opcode {
            Opcode::Call(call) => call.operands(),
            Opcode::TlsAddress { out } => vec![(*out, true)],
            Opcode::TraceLoad { address, out, .. } => vec![(*address, false), (*out, true)],
            Opcode::TraceStore { address, value, .. } => vec![(*address, false), (*value, false)],
            Opcode::HardTrap => vec![],
            Opcode::Numeric(n) => n.operands(),
            Opcode::CheckBranch { condition, .. } => vec![(*condition, false)],
            Opcode::Failure { arguments, .. } => arguments.iter().map(|v| (*v, false)).collect(),
            Opcode::Constant { out, .. }
            | Opcode::SymbolAddress { out, .. }
            | Opcode::ObjectAddress { out, .. } => vec![(*out, true)],
            Opcode::Unary { input, out, .. } => {
                vec![(*input, false), (*out, true)]
            }
            Opcode::Alu {
                left, right, out, ..
            }
            | Opcode::IntegerCompare {
                left, right, out, ..
            }
            | Opcode::FloatCompare {
                left, right, out, ..
            } => vec![(*left, false), (*right, false), (*out, true)],
            Opcode::ByteOffset { base, offset, out } => {
                vec![(*base, false), (*offset, false), (*out, true)]
            }
            Opcode::Load { address, out, .. } => vec![(*address, false), (*out, true)],
            Opcode::Store { address, value, .. } => vec![(*address, false), (*value, false)],
            Opcode::Branch { condition } => vec![(*condition, false)],
            Opcode::Return { values, .. } => values.iter().map(|v| (*v, false)).collect(),
            Opcode::Lifetime { .. } | Opcode::Jump => vec![],
        }
    }
}
