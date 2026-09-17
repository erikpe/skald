use super::*;
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Shape {
    Two,
    Three,
}
#[derive(Clone, Copy)]
pub(super) struct ValueRef {
    pub id: SelectedValueId,
    pub ty: Representation,
}
pub(super) enum Op {
    Load {
        address: ValueRef,
        out: ValueRef,
    },
    Address {
        out: ValueRef,
        artifact: ArtifactId,
    },
    Divide {
        numerator: ValueRef,
        divisor: ValueRef,
        quotient: ValueRef,
        remainder: ValueRef,
    },
    Add {
        a: ValueRef,
        b: ValueRef,
        out: ValueRef,
        destructive: bool,
    },
    Call {
        target: Option<ValueRef>,
        args: Vec<ValueRef>,
        out: Vec<ValueRef>,
        signature: SignatureId,
        abi: AbiBindings,
        artifact: ArtifactId,
    },
    Branch {
        condition: ValueRef,
        successors: usize,
    },
    Jump,
    Return {
        values: Vec<ValueRef>,
        bindings: Vec<AbiBinding>,
    },
    Trap,
    Trace,
}
#[derive(Clone, Copy)]
pub(super) enum Corruption {
    TieSlot,
    TieTiming,
    FixedWidth,
    MissingScratch,
    MissingReference,
    MissingEffect,
    WrongFlow,
    WrongAbi,
    UnknownClobber,
}
pub(super) struct Node {
    pub op: Op,
    pub views: Vec<ViewId>,
    pub clobbers: Vec<(Timing, UnitId)>,
    pub effects: Effects<graph::SelectedObjectId>,
    pub refs: Vec<(ArtifactId, ArtifactCategory)>,
    pub corrupt: Option<Corruption>,
    pub attribution: lir::CallAttribution,
    empty: Effects<graph::SelectedObjectId>,
}
impl Node {
    pub fn new(op: Op, views: &[ViewId]) -> Self {
        let effects = match &op {
            Op::Call { artifact, .. } => {
                if *artifact == ArtifactId::Runtime(plan::RuntimeService::Free) {
                    Effects::new([Effect::Call, Effect::Free])
                } else {
                    Effects::new([Effect::Call])
                }
            }
            Op::Load { .. } => {
                Effects::new([Effect::Read(crate::backend::effects::MemoryRegion::Unknown)])
            }
            Op::Trap => Effects::new([Effect::HardTrap]),
            Op::Trace => Effects::new([Effect::TraceState]),
            _ => Effects::default(),
        };
        let refs = if let Op::Call { artifact, .. } | Op::Address { artifact, .. } = &op {
            vec![(*artifact, artifact.category())]
        } else {
            vec![]
        };
        Self {
            op,
            views: views.to_vec(),
            clobbers: vec![],
            effects,
            refs,
            corrupt: None,
            attribution: lir::CallAttribution::NonReporting,
            empty: Effects::default(),
        }
    }
}
fn operand(value: ValueRef, role: OperandRole, constraint: Constraint<'_>) -> Operand<'_> {
    Operand {
        value: value.id,
        representation: value.ty,
        role,
        timing: if role == OperandRole::Use {
            Timing::Early
        } else {
            Timing::Late
        },
        constraint,
    }
}
fn abi_constraint(binding: &AbiBinding) -> Constraint<'_> {
    match binding.location {
        AbiLocation::Fixed(v) => Constraint::Fixed(v),
        AbiLocation::Slot { area, index } => Constraint::AbiSlot { area, index },
    }
}
impl Payload for Node {
    fn describe(&self) -> Description<'_> {
        let c = Constraint::Resources {
            views: &self.views,
            memory: false,
        };
        let mut operands = vec![];
        let mut ties: &[Tie] = &[];
        let mut signature = None;
        let mut indirect_target = None;
        let mut abi_inputs: &[AbiBinding] = &[];
        let mut abi_results: &[AbiBinding] = &[];
        let mut bundle = None;
        let (flow, successors) = match &self.op {
            Op::Load { address, out } => {
                operands.extend([
                    operand(*address, OperandRole::Use, c),
                    operand(*out, OperandRole::Definition, c),
                ]);
                (Flow::Instruction, 0)
            }
            Op::Address { out, .. } => {
                operands.push(operand(*out, OperandRole::Definition, c));
                (Flow::Instruction, 0)
            }
            Op::Divide {
                numerator,
                divisor,
                quotient,
                remainder,
            } => {
                operands.extend([
                    operand(*numerator, OperandRole::Use, c),
                    operand(*divisor, OperandRole::Use, c),
                    operand(*quotient, OperandRole::Definition, c),
                    operand(*remainder, OperandRole::Definition, c),
                ]);
                (Flow::Instruction, 0)
            }
            Op::Add {
                a,
                b,
                out,
                destructive,
            } => {
                operands.extend([
                    operand(*a, OperandRole::Use, c),
                    operand(*b, OperandRole::Use, c),
                    operand(*out, OperandRole::Definition, c),
                ]);
                if *destructive {
                    ties = &[Tie {
                        input: 0,
                        output: 2,
                    }];
                } else {
                    bundle = Some(Bundle::Bounded {
                        steps: NonZeroU16::new(2).unwrap(),
                        scratch: Cow::Owned(vec![Scratch {
                            representation: out.ty,
                            views: &self.views,
                            count: NonZeroU16::new(1).unwrap(),
                        }]),
                    });
                }
                (Flow::Instruction, 0)
            }
            Op::Call {
                target,
                args,
                out,
                signature: s,
                abi,
                ..
            } => {
                signature = Some(*s);
                abi_inputs = abi.inputs();
                abi_results = abi.results();
                for (i, v) in args.iter().enumerate() {
                    operands.push(operand(
                        *v,
                        OperandRole::Use,
                        abi_constraint(&abi_inputs[i]),
                    ));
                }
                if let Some(target) = target {
                    indirect_target = Some(operands.len());
                    operands.push(operand(*target, OperandRole::Use, c));
                }
                for (i, v) in out.iter().enumerate() {
                    operands.push(operand(
                        *v,
                        OperandRole::Definition,
                        abi_constraint(&abi_results[i]),
                    ));
                }
                (Flow::Instruction, 0)
            }
            Op::Branch {
                condition,
                successors,
            } => {
                operands.push(operand(*condition, OperandRole::Use, c));
                bundle = Some(Bundle::Atomic);
                (Flow::Branch, *successors)
            }
            Op::Jump => (Flow::Branch, 1),
            Op::Return { values, bindings } => {
                abi_results = bindings;
                for (i, v) in values.iter().enumerate() {
                    operands.push(operand(*v, OperandRole::Use, abi_constraint(&bindings[i])));
                }
                (Flow::Return, 0)
            }
            Op::Trap => (Flow::Never, 0),
            Op::Trace => (Flow::Instruction, 0),
        };
        let mut desc = Description {
            operands: Cow::Owned(operands),
            ties,
            clobbers: &self.clobbers,
            effects: &self.effects,
            artifacts: &self.refs,
            objects: &[],
            abi_inputs,
            abi_results,
            indirect_target,
            bundle,
            successors,
            flow,
            call_signature: signature,
            call_attribution: signature.map(|_| &self.attribution),
        };
        match self.corrupt {
            Some(Corruption::TieSlot) => {
                desc.ties = &[Tie {
                    input: 0,
                    output: 99,
                }]
            }
            Some(Corruption::TieTiming) => {
                desc.operands.to_mut()[0].timing = Timing::Late;
                desc.operands.to_mut()[2].timing = Timing::Early;
            }
            Some(Corruption::FixedWidth) => {
                desc.operands.to_mut()[0].constraint = Constraint::Fixed(self.views[0])
            }
            Some(Corruption::MissingScratch) => {
                desc.bundle = Some(Bundle::Bounded {
                    steps: NonZeroU16::new(1).unwrap(),
                    scratch: Cow::Owned(vec![Scratch {
                        representation: repr(),
                        views: &[],
                        count: NonZeroU16::new(1).unwrap(),
                    }]),
                })
            }
            Some(Corruption::MissingReference) => desc.artifacts = &[],
            Some(Corruption::MissingEffect) => desc.effects = &self.empty,
            Some(Corruption::WrongFlow) => desc.flow = Flow::Return,
            Some(Corruption::WrongAbi) => desc.abi_inputs = &[],
            Some(Corruption::UnknownClobber) => {}
            None => {}
        }
        desc
    }
}
pub(super) struct WitnessTarget {
    pub profile: plan::TargetProfile,
    pub shape: Shape,
    pub reject: bool,
}
impl TargetVerifier<Node> for WitnessTarget {
    fn profile(&self) -> plan::TargetProfile {
        self.profile
    }
    fn verify_payload(&self, payload: &Node, terminal: bool) -> Result<(), &'static str> {
        let desc = payload.describe();
        if terminal
            != matches!(
                payload.op,
                Op::Branch { .. } | Op::Jump | Op::Return { .. } | Op::Trap
            )
        {
            return Err("opcode position");
        }
        if let Op::Call { artifact, .. } = &payload.op {
            if !desc.artifacts.contains(&(*artifact, artifact.category())) {
                return Err("missing call dependency");
            }
            if !desc.effects.contains(Effect::Call) {
                return Err("missing call effect");
            }
            if *artifact == ArtifactId::Runtime(plan::RuntimeService::Free)
                && !desc.effects.contains(Effect::Free)
            {
                return Err("missing free effect");
            }
        }
        if matches!(payload.op, Op::Branch { .. }) && !matches!(desc.bundle, Some(Bundle::Atomic)) {
            return Err("flag bundle missing");
        }
        if matches!(payload.op, Op::Trap) && !desc.effects.contains(Effect::HardTrap) {
            return Err("trap effect missing");
        }
        if matches!(payload.op, Op::Trace) && !desc.effects.contains(Effect::TraceState) {
            return Err("trace effect missing");
        }
        Ok(())
    }
    fn verify_callable(&self, draft: &SelectedDraft<'_, Node>) -> Result<(), &'static str> {
        if self.reject {
            return Err("adversarial target rejection");
        }
        let graph = graph::check_graph(draft).map_err(|_| "target graph invalid")?;
        for (block_id, block) in draft.blocks.iter() {
            for node in &block.instructions {
                if let Op::Call {
                    signature,
                    artifact,
                    ..
                } = node.op
                {
                    let view = draft.context.extension.parent().parent();
                    let signature_fact = view
                        .signature(
                            view.signature_id(signature.index())
                                .map_err(|_| "unknown signature")?,
                        )
                        .map_err(|_| "unknown signature")?;
                    let matches = if let ArtifactId::Callable(key) = artifact {
                        draft
                            .context
                            .extension
                            .selection_binding(key)
                            .map_err(|_| "unknown callable")?
                            .signature()
                            .map_err(|_| "unknown signature")?
                            == signature_fact
                    } else {
                        view.artifact(
                            view.artifact_id(artifact).map_err(|_| "unknown artifact")?,
                            artifact.category(),
                        )
                        .map_err(|_| "unknown artifact")?
                        .signature
                            == Some(signature)
                    };
                    if !matches {
                        return Err("call dependency signature");
                    }
                }
                if let Op::Divide { divisor, .. } = node.op {
                    let guarded = draft.blocks.iter().any(|(guard_id, guard)| {
                        guard
                            .terminal
                            .as_ref()
                            .is_some_and(|term| match term.payload.op {
                                Op::Branch { condition, .. } if condition.id == divisor.id => {
                                    term.edges.first().is_some_and(|(success, _)| {
                                        graph.dominates(guard_id.index(), block_id.index())
                                            == Some(true)
                                            && graph.dominates(success.index(), block_id.index())
                                                == Some(true)
                                            && graph
                                                .predecessors(success.index())
                                                .is_some_and(|p| p == [(guard_id.index(), 0)])
                                    })
                                }
                                _ => false,
                            })
                    });
                    if !guarded {
                        return Err("unsecured divisor");
                    }
                }
            }
        }
        for (_, block) in draft.blocks.iter() {
            for node in &block.instructions {
                if let Op::Add { destructive, .. } = node.op {
                    if destructive != (self.shape == Shape::Two) {
                        return Err("wrong target add recipe");
                    }
                }
            }
        }
        Ok(())
    }
}
