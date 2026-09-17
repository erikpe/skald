use super::*;

// These are semantic synthetic opcodes. Descriptions are constructed from fields,
// rather than independently maintained generic use/def tables.
pub(super) enum Opcode {
    TwoAddress,
    ThreeAddress,
    Call,
    EarlyClobber,
    Flags,
    Return,
}
pub(super) struct Synthetic {
    pub(super) opcode: Opcode,
    pub(super) inputs: Vec<graph::SelectedValueId>,
    pub(super) result: Option<graph::SelectedValueId>,
    pub(super) views: Vec<ViewId>,
    pub(super) clobbers: Vec<(Timing, UnitId)>,
    pub(super) effects: Effects<graph::SelectedObjectId>,
    pub(super) abi: Vec<AbiBinding>,
    pub(super) scratch_views: Vec<ViewId>,
    pub(super) objects: Vec<graph::SelectedObjectId>,
    pub(super) refs: Vec<(plan::ArtifactId, plan::ArtifactCategory)>,
}
impl Synthetic {
    pub(super) fn new(
        opcode: Opcode,
        inputs: Vec<graph::SelectedValueId>,
        result: Option<graph::SelectedValueId>,
        view: ViewId,
    ) -> Self {
        Self {
            opcode,
            inputs,
            result,
            views: vec![view],
            clobbers: vec![],
            effects: Effects::default(),
            abi: vec![],
            scratch_views: vec![view],
            objects: vec![],
            refs: vec![],
        }
    }
}
impl Payload for Synthetic {
    fn describe(&self) -> Description<'_> {
        let fixed = matches!(self.opcode, Opcode::Call);
        let constraint = if fixed {
            Constraint::Fixed(self.views[0])
        } else {
            Constraint::Resources {
                views: &self.views,
                memory: false,
            }
        };
        let mut operands: Vec<_> = self
            .inputs
            .iter()
            .map(|value| Operand {
                value: *value,
                representation: repr(),
                role: OperandRole::Use,
                timing: if matches!(self.opcode, Opcode::Flags) {
                    Timing::Late
                } else {
                    Timing::Early
                },
                constraint,
            })
            .collect();
        if let Some(value) = self.result {
            operands.push(Operand {
                value,
                representation: repr(),
                role: OperandRole::Definition,
                timing: if matches!(self.opcode, Opcode::EarlyClobber) {
                    Timing::Early
                } else {
                    Timing::Late
                },
                constraint,
            });
        }
        Description {
            flow: match self.opcode {
                Opcode::Flags => Flow::Branch,
                Opcode::Return => Flow::Return,
                _ => Flow::Instruction,
            },
            call_signature: None,
            call_attribution: None,
            operands: Cow::Owned(operands),
            ties: if matches!(self.opcode, Opcode::TwoAddress) {
                &[Tie {
                    input: 0,
                    output: 1,
                }]
            } else {
                &[]
            },
            clobbers: &self.clobbers,
            effects: &self.effects,
            artifacts: &self.refs,
            objects: &self.objects,
            abi_inputs: &self.abi,
            abi_results: &[],
            indirect_target: None,
            bundle: if matches!(self.opcode, Opcode::Flags) {
                Some(Bundle::Atomic)
            } else if matches!(self.opcode, Opcode::ThreeAddress) {
                Some(Bundle::Bounded {
                    steps: NonZeroU16::new(2).unwrap(),
                    scratch: Cow::Owned(vec![Scratch {
                        representation: repr(),
                        views: &self.scratch_views,
                        count: NonZeroU16::new(1).unwrap(),
                    }]),
                })
            } else {
                None
            },
            successors: if matches!(self.opcode, Opcode::Flags) {
                2
            } else {
                0
            },
        }
    }
}
