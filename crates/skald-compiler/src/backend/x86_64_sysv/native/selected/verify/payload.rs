use super::super::{Instruction, Opcode, ValueRef};
use super::{float_condition, integer_condition, Verifier};
use crate::backend::{
    effects::{Effect, Effects, MemoryRegion},
    lir::{BinaryOperation, Constant, UnaryOperation},
    plan::{ArtifactId, ScalarType},
    selected::{BankKind, Representation, RepresentationKind, Timing},
};
impl Verifier {
    pub(super) fn check_payload(
        &self,
        node: &Instruction,
        terminal: bool,
    ) -> Result<(), &'static str> {
        let bits =
            |r: Representation| r.kind == RepresentationKind::Bits && matches!(r.bits(), 8 | 64);
        let float = |r: Representation| r.kind == RepresentationKind::Float && r.bits() == 64;
        let pointer =
            |r: Representation| r.kind == RepresentationKind::DataAddress && r.bits() == 64;
        let address = |r: Representation| {
            matches!(
                r.kind,
                RepresentationKind::DataAddress | RepresentationKind::CodeAddress(_)
            ) && r.bits() == 64
        };
        let bool_rep = Representation::from_scalar(ScalarType::Bool, 64).unwrap();
        let same = |a: ValueRef, b: ValueRef| a.representation == b.representation;
        let (valid, flags) = match &node.opcode {
            Opcode::Call(call) => (
                call.arguments.len() == call.inputs.len()
                    && call.results.len() == call.outputs.len()
                    && call
                        .arguments
                        .iter()
                        .zip(&call.inputs)
                        .chain(call.results.iter().zip(&call.outputs))
                        .all(|(v, b)| v.representation == b.representation)
                    && (!call.never || call.results.is_empty()),
                false,
            ),
            Opcode::HardTrap => (true, false),
            Opcode::TlsAddress { out } => (pointer(out.representation), false),
            Opcode::TraceLoad { address, out, .. } => (
                pointer(address.representation) && pointer(out.representation),
                false,
            ),
            Opcode::TraceStore { address, value, .. } => (
                pointer(address.representation) && pointer(value.representation),
                false,
            ),
            Opcode::Numeric(n) => (true, super::numeric::fields(n)?),
            Opcode::CheckBranch { condition, .. } => (condition.representation == bool_rep, true),
            Opcode::Failure {
                arguments,
                bindings,
                ..
            } => (
                arguments.len() == 2
                    && bindings.len() == 2
                    && arguments
                        .iter()
                        .zip(bindings)
                        .all(|(v, b)| v.representation == b.representation),
                false,
            ),
            Opcode::Constant { constant, out } => {
                let ty = match constant {
                    Constant::I64(_) => ScalarType::I64,
                    Constant::U64(_) => ScalarType::U64,
                    Constant::U8(_) => ScalarType::U8,
                    Constant::Bool(_) => ScalarType::Bool,
                    Constant::F64(_) => ScalarType::F64,
                    Constant::Null(ty) => *ty,
                };
                (
                    Some(out.representation) == Representation::from_scalar(ty, 64)
                        && (!matches!(constant, Constant::Null(_))
                            || matches!(ty, ScalarType::DataAddress | ScalarType::CodeAddress(_))),
                    false,
                )
            }
            Opcode::Unary {
                operation,
                input,
                out,
            } => {
                let valid = same(*input, *out)
                    && match operation {
                        UnaryOperation::Negate => {
                            bits(input.representation) || float(input.representation)
                        }
                        UnaryOperation::Complement => bits(input.representation),
                        UnaryOperation::LogicalNot => input.representation == bool_rep,
                    };
                (valid, !matches!(operation, UnaryOperation::Complement))
            }
            Opcode::Alu {
                operation,
                left,
                right,
                out,
            } => {
                let valid = same(*left, *right)
                    && same(*left, *out)
                    && match operation {
                        BinaryOperation::Add
                        | BinaryOperation::Subtract
                        | BinaryOperation::Multiply => {
                            bits(left.representation) || float(left.representation)
                        }
                        BinaryOperation::And | BinaryOperation::Or | BinaryOperation::Xor => {
                            bits(left.representation)
                        }
                        BinaryOperation::FloatDivide => float(left.representation),
                    };
                (valid, left.representation.bank() == BankKind::Integer)
            }
            Opcode::IntegerCompare {
                condition,
                predicate,
                signed,
                left,
                right,
                out,
            } => (
                same(*left, *right)
                    && (bits(left.representation)
                        || (address(left.representation) && predicate.is_equality()))
                    && out.representation == bool_rep
                    && *condition == integer_condition(*predicate, *signed)
                    && (!*signed || bits(left.representation) && left.representation.bits() == 64),
                true,
            ),
            Opcode::FloatCompare {
                condition,
                predicate,
                left,
                right,
                out,
            } => (
                same(*left, *right)
                    && float(left.representation)
                    && out.representation == bool_rep
                    && *condition == float_condition(*predicate),
                true,
            ),
            Opcode::SymbolAddress { symbol, out } => {
                let valid = matches!(
                    (symbol, out.representation.kind),
                    (
                        ArtifactId::Callable(_) | ArtifactId::External(_) | ArtifactId::Runtime(_),
                        RepresentationKind::CodeAddress(_)
                    ) | (ArtifactId::Data(_), RepresentationKind::DataAddress)
                );
                (valid && out.representation.bits() == 64, false)
            }
            Opcode::ObjectAddress { out, .. } => (pointer(out.representation), false),
            Opcode::ByteOffset { base, offset, out } => (
                pointer(base.representation)
                    && same(*base, *out)
                    && offset.representation.kind == RepresentationKind::Bits
                    && offset.representation.bits() == 64,
                true,
            ),
            Opcode::Load {
                address,
                out,
                bytes,
                alignment,
                ..
            } => (
                pointer(address.representation)
                    && valid_memory(out.representation, *bytes, *alignment),
                false,
            ),
            Opcode::Store {
                address,
                value,
                bytes,
                alignment,
                ..
            } => (
                pointer(address.representation)
                    && valid_memory(value.representation, *bytes, *alignment),
                false,
            ),
            Opcode::Lifetime {
                marker,
                site,
                sites,
                ..
            } => (
                *site < *sites
                    && matches!(
                        marker,
                        crate::backend::lir::LifetimeMarker::Start
                            | crate::backend::lir::LifetimeMarker::End
                    ),
                false,
            ),
            Opcode::Jump => (true, false),
            Opcode::Branch { condition } => (condition.representation == bool_rep, true),
            Opcode::Return { values, bindings } => (
                values.len() == bindings.len()
                    && values
                        .iter()
                        .zip(bindings)
                        .all(|(v, b)| v.representation == b.representation),
                false,
            ),
        };
        if !valid {
            return Err("illegal x86 opcode field or representation");
        }
        let expected_terminal = matches!(
            node.opcode,
            Opcode::Jump
                | Opcode::Branch { .. }
                | Opcode::CheckBranch { .. }
                | Opcode::Failure { .. }
                | Opcode::HardTrap
                | Opcode::Return { .. }
        ) || matches!(&node.opcode, Opcode::Call(call) if call.never);
        if terminal != expected_terminal {
            return Err("illegal x86 instruction/terminal position");
        }
        let clobbers = if matches!(node.opcode, Opcode::Failure { .. } | Opcode::Call(_)) {
            self.resources
                .caller_clobbers()
                .iter()
                .map(|u| (Timing::Late, *u))
                .collect()
        } else if flags {
            vec![(Timing::Late, self.resources.flags())]
        } else {
            vec![]
        };
        if node.clobbers != clobbers {
            return Err("missing or extraneous x86 flag clobber");
        }
        // Mandatory native memory effects and references are checked from fields,
        // not by asking describe to agree with itself.
        let effects = match node.opcode {
            Opcode::TlsAddress { .. } => Effects::new([Effect::Read(MemoryRegion::Unknown)]),
            Opcode::Call(ref call) => {
                let base = match call.target {
                    crate::backend::lir::CallTarget::Direct(ArtifactId::Runtime(service)) => {
                        crate::backend::plan::service_effects(service)
                    }
                    _ => Effects::conservative_call(),
                };
                if call.never {
                    Effects::new(base.iter().copied().chain([Effect::HardTrap]))
                } else {
                    base
                }
            }
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
        if node.effects != effects {
            return Err("incorrect x86 memory effects");
        }
        let artifacts = match node.opcode {
            Opcode::Call(ref call) => {
                let mut refs = vec![];
                if let crate::backend::lir::CallTarget::Direct(target) = call.target {
                    refs.push((target, target.category()));
                }
                if let Some(artifact) = super::super::calls::attribution_artifact(&call.attribution)
                {
                    refs.push((artifact, artifact.category()));
                }
                refs
            }
            Opcode::TlsAddress { .. } | Opcode::TraceLoad { .. } | Opcode::TraceStore { .. } => {
                vec![(
                    ArtifactId::TraceTls,
                    crate::backend::plan::ArtifactCategory::Tls,
                )]
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
                if let Some(artifact) = super::super::calls::attribution_artifact(attribution) {
                    refs.push((artifact, artifact.category()));
                }
                refs
            }
            Opcode::SymbolAddress { symbol, .. } => vec![(symbol, symbol.category())],
            _ => vec![],
        };
        if node.artifacts != artifacts {
            return Err("incorrect x86 symbol reference");
        }
        let objects = match node.opcode {
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
        if node.objects != objects {
            return Err("incorrect x86 object reference");
        }
        Ok(())
    }
}
fn valid_memory(r: Representation, bytes: usize, alignment: usize) -> bool {
    matches!(r.bits(), 8 | 64)
        && bytes == usize::from(r.bits() / 8)
        && alignment.is_power_of_two()
        && alignment <= bytes
        && (!matches!(
            r.kind,
            RepresentationKind::DataAddress | RepresentationKind::CodeAddress(_)
        ) || r.bits() == 64)
        && (r.kind != RepresentationKind::Float || r.bits() == 64)
}
