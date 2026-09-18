use super::super::Gpr;
use super::numeric::Numeric;
use super::{Instruction, Opcode};
use crate::backend::{
    lir::{BinaryOperation, UnaryOperation},
    selected::{
        AbiLocation, Bundle, Constraint, Description, Flow, InspectPayload, Operand, OperandRole,
        Payload, Representation, RepresentationKind, Scratch, Tie, Timing,
    },
};
use std::{borrow::Cow, num::NonZeroU16};

impl Payload for Instruction {
    fn span(&self) -> Option<crate::source::Span> {
        self.origin.span
    }
    fn describe(&self) -> Description<'_> {
        let operands = self
            .operands()
            .into_iter()
            .enumerate()
            .map(|(slot, (v, definition))| {
                let constraint = if let Opcode::Call(call) = &self.opcode {
                    let target =
                        matches!(call.target, crate::backend::lir::CallTarget::Indirect(_));
                    if target && slot == call.arguments.len() {
                        Constraint::Fixed(self.resources.gpr(Gpr::R11, 64).expect("secured target"))
                    } else {
                        let binding = if slot < call.arguments.len() {
                            call.inputs.get(slot)
                        } else {
                            call.outputs
                                .get(slot - call.arguments.len() - usize::from(target))
                        };
                        match binding.map(|b| b.location) {
                            Some(AbiLocation::Fixed(view)) => Constraint::Fixed(view),
                            Some(AbiLocation::Slot { area, index }) => {
                                Constraint::AbiSlot { area, index }
                            }
                            None => Constraint::Resources {
                                views: &[],
                                memory: false,
                            },
                        }
                    }
                } else if let Opcode::Numeric(n) = &self.opcode {
                    let fixed = |gpr, bits| {
                        Constraint::Fixed(
                            self.resources
                                .gpr(gpr, bits)
                                .expect("canonical native register"),
                        )
                    };
                    match (n, slot) {
                        (Numeric::Dividend { .. }, 0) | (Numeric::Divide { .. }, 0 | 3) => {
                            fixed(Gpr::Rax, 64)
                        }
                        (Numeric::Dividend { .. }, 1) | (Numeric::Divide { .. }, 1 | 4) => {
                            fixed(Gpr::Rdx, 64)
                        }
                        (Numeric::Divide { .. }, 2) => Constraint::Resources {
                            views: self.resources.divisor_views(),
                            memory: false,
                        },
                        (Numeric::Shift { .. }, 1) => fixed(Gpr::Rcx, 8),
                        _ => Constraint::Resources {
                            views: self.views(v.representation),
                            memory: false,
                        },
                    }
                } else if let Opcode::Failure { bindings, .. } = &self.opcode {
                    match bindings.get(slot).map(|b| b.location) {
                        Some(AbiLocation::Fixed(view)) => Constraint::Fixed(view),
                        Some(AbiLocation::Slot { area, index }) => {
                            Constraint::AbiSlot { area, index }
                        }
                        None => Constraint::Resources {
                            views: &[],
                            memory: false,
                        },
                    }
                } else if let Opcode::Return { bindings, .. } = &self.opcode {
                    match bindings.get(slot).map(|b| b.location) {
                        Some(AbiLocation::Fixed(view)) => Constraint::Fixed(view),
                        Some(AbiLocation::Slot { area, index }) => {
                            Constraint::AbiSlot { area, index }
                        }
                        None => Constraint::Resources {
                            views: &[],
                            memory: false,
                        },
                    }
                } else {
                    Constraint::Resources {
                        views: self.views(v.representation),
                        memory: false,
                    }
                };
                Operand {
                    value: v.value,
                    representation: v.representation,
                    role: if definition {
                        OperandRole::Definition
                    } else {
                        OperandRole::Use
                    },
                    timing: if matches!(
                        self.opcode,
                        Opcode::Numeric(Numeric::Divide { .. } | Numeric::Shift { .. })
                            | Opcode::Failure { .. }
                            | Opcode::Call(_)
                    ) || definition
                    {
                        Timing::Late
                    } else {
                        Timing::Early
                    },
                    constraint,
                }
            })
            .collect::<Vec<_>>();
        let ties = match &self.opcode {
            Opcode::Numeric(Numeric::Shift { .. }) => &[Tie {
                input: 0,
                output: 2,
            }][..],
            Opcode::Numeric(Numeric::ShiftOne { .. }) => &[Tie {
                input: 0,
                output: 1,
            }][..],
            Opcode::Alu {
                operation: BinaryOperation::Multiply,
                left,
                ..
            } if left.representation.bits() == 8 => &[][..],
            Opcode::Alu { .. } | Opcode::ByteOffset { .. } => &[Tie {
                input: 0,
                output: 2,
            }][..],
            Opcode::Unary {
                operation: UnaryOperation::Negate,
                input,
                ..
            } if input.representation.kind == RepresentationKind::Float => &[][..],
            Opcode::Unary {
                operation: UnaryOperation::LogicalNot,
                ..
            } => &[][..],
            Opcode::Unary { .. } => &[Tie {
                input: 0,
                output: 1,
            }][..],
            _ => &[][..],
        };
        let scratch = |bits, count| Scratch {
            representation: Representation::new(RepresentationKind::Bits, bits)
                .expect("nonzero width"),
            views: self
                .resources
                .allocatable(crate::backend::selected::BankKind::Integer, bits),
            count: NonZeroU16::new(count).expect("nonzero scratch"),
        };
        let bundle = match &self.opcode {
            Opcode::TlsAddress { .. } => Some(Bundle::Bounded {
                steps: NonZeroU16::new(2).unwrap(),
                scratch: Cow::Borrowed(&[]),
            }),
            Opcode::Constant {
                constant: crate::backend::lir::Constant::F64(_),
                ..
            } => Some(Bundle::Bounded {
                steps: NonZeroU16::new(2).unwrap(),
                scratch: Cow::Owned(vec![scratch(64, 1)]),
            }),
            Opcode::FloatCompare { condition, .. } if condition.parity() => Some(Bundle::Bounded {
                steps: NonZeroU16::new(4).unwrap(),
                scratch: Cow::Owned(vec![scratch(8, 1)]),
            }),
            Opcode::Unary {
                operation: UnaryOperation::Negate,
                input,
                ..
            } if input.representation.kind == RepresentationKind::Float => Some(Bundle::Bounded {
                steps: NonZeroU16::new(4).unwrap(),
                scratch: Cow::Owned(vec![scratch(64, 2)]),
            }),
            Opcode::Alu {
                operation: BinaryOperation::Multiply,
                left,
                ..
            } if left.representation.bits() == 8 => Some(Bundle::Bounded {
                steps: NonZeroU16::new(4).unwrap(),
                scratch: Cow::Owned(vec![scratch(32, 2)]),
            }),
            Opcode::IntegerCompare { .. }
            | Opcode::FloatCompare { .. }
            | Opcode::Unary {
                operation: UnaryOperation::LogicalNot,
                ..
            }
            | Opcode::CheckBranch { .. }
            | Opcode::Branch { .. } => Some(Bundle::Atomic),
            Opcode::Failure { .. } | Opcode::Call(_) => Some(Bundle::Atomic),
            _ => None,
        };
        let (flow, successors) = match self.opcode {
            Opcode::Jump => (Flow::Branch, 1),
            Opcode::Branch { .. } | Opcode::CheckBranch { .. } => (Flow::Branch, 2),
            Opcode::Failure { .. } | Opcode::HardTrap => (Flow::Never, 0),
            Opcode::Call(ref call) if call.never => (Flow::Never, 0),
            Opcode::Return { .. } => (Flow::Return, 0),
            _ => (Flow::Instruction, 0),
        };
        Description {
            operands: Cow::Owned(operands),
            ties,
            clobbers: &self.clobbers,
            effects: &self.effects,
            artifacts: &self.artifacts,
            objects: &self.objects,
            abi_inputs: if let Opcode::Failure { bindings, .. } = &self.opcode {
                bindings
            } else if let Opcode::Call(call) = &self.opcode {
                &call.inputs
            } else {
                &[]
            },
            abi_results: if let Opcode::Call(call) = &self.opcode {
                &call.outputs
            } else if let Opcode::Return { bindings, .. } = &self.opcode {
                bindings
            } else {
                &[]
            },
            indirect_target: if let Opcode::Call(call) = &self.opcode {
                matches!(call.target, crate::backend::lir::CallTarget::Indirect(_))
                    .then_some(call.arguments.len())
            } else {
                None
            },
            bundle,
            successors,
            flow,
            call_signature: if let Opcode::Failure { signature, .. } = self.opcode {
                Some(signature)
            } else if let Opcode::Call(call) = &self.opcode {
                Some(call.signature)
            } else {
                None
            },
            call_attribution: if let Opcode::Failure { attribution, .. } = &self.opcode {
                Some(attribution)
            } else if let Opcode::Call(call) = &self.opcode {
                Some(&call.attribution)
            } else {
                None
            },
        }
    }
}
impl InspectPayload for Instruction {
    fn fmt_opcode(&self, out: &mut dyn std::fmt::Write) -> std::fmt::Result {
        write!(
            out,
            "x86 {:?} span={:?} ",
            self.opcode(),
            self.origin().span
        )?;
        match self.origin().site {
            super::Site::Instruction { block, ordinal } => {
                write!(out, "origin=instruction {block:?}/{ordinal}")
            }
            super::Site::Terminator(block) => write!(out, "origin=terminal {block:?}"),
            super::Site::Edge { block, slot } => write!(out, "origin=edge {block:?} slot={slot}"),
        }
    }
}
