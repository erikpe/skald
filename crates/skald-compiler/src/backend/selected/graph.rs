//! Checked projection of opcode operands into shared structural analysis.
use super::{storage::SelectedDraft, OperandRole, Payload, Representation};
use crate::backend::graph;
#[cfg_attr(not(test), allow(dead_code))]
impl<P: Payload> graph::GraphView for SelectedDraft<'_, P> {
    type Type = Representation;
    fn identity(&self) -> graph::GraphIdentity {
        graph::GraphIdentity {
            stage: graph::GraphStage::Selected,
            target: self.owner.context().profile(),
            callable: self.owner.key(),
        }
    }
    fn describe(&self) -> Result<graph::GraphDescription<Representation>, graph::GraphFailure> {
        let fail = |location, reason| graph::GraphFailure {
            identity: Box::new(self.identity()),
            location,
            reason,
            origin: None,
        };
        let value = |id| {
            self.values.get_id(id).map_err(|_| {
                fail(
                    graph::GraphLocation::Value(id.index()),
                    graph::GraphReason::OutOfBounds,
                )
            })
        };
        let uses =
            |payload: &P| -> Result<Vec<graph::TypedUse<Representation>>, graph::GraphFailure> {
                payload
                    .describe()
                    .operands
                    .iter()
                    .filter(|op| op.role == OperandRole::Use)
                    .map(|op| {
                        value(op.value)?;
                        Ok(graph::TypedUse {
                            value: op.value.index(),
                            expected: Some(op.representation),
                        })
                    })
                    .collect()
            };
        let mut blocks = vec![];
        if let Some(entry) = self.entry {
            self.blocks.get_id(entry).map_err(|_| {
                fail(
                    graph::GraphLocation::Entry,
                    graph::GraphReason::InvalidEntry,
                )
            })?;
        }
        for (id, block) in self.blocks.iter() {
            for parameter in &block.parameters {
                value(*parameter)?;
            }
            let mut instructions = vec![];
            for payload in &block.instructions {
                let mut results = vec![];
                for op in &*payload.describe().operands {
                    value(op.value)?;
                    if op.role == OperandRole::Definition {
                        results.push((op.value.index(), op.representation));
                    }
                }
                instructions.push(graph::InstructionDescription {
                    uses: uses(payload)?,
                    results,
                });
            }
            let terminal = if let Some(terminal) = &block.terminal {
                let payload = &terminal.payload;
                let targets = &terminal.edges;
                let mut edges = vec![];
                for (target, args) in targets {
                    self.blocks.get_id(*target).map_err(|_| {
                        fail(
                            graph::GraphLocation::Block(id.index()),
                            graph::GraphReason::OutOfBounds,
                        )
                    })?;
                    let arguments = args
                        .iter()
                        .map(|v| {
                            value(*v)?;
                            Ok(graph::TypedUse {
                                value: v.index(),
                                expected: None,
                            })
                        })
                        .collect::<Result<_, _>>()?;
                    edges.push(graph::EdgeDescription {
                        target: target.index(),
                        arguments,
                    });
                }
                Some((uses(payload)?, edges))
            } else {
                None
            };
            blocks.push(graph::BlockDescription {
                parameters: Some(block.parameters.iter().map(|v| v.index()).collect()),
                instructions,
                terminal,
            });
        }
        Ok(graph::GraphDescription {
            entry: self.entry.map(|id| id.index()),
            inputs: self
                .inputs
                .iter()
                .map(|id| Ok((id.index(), value(*id)?.ty)))
                .collect::<Result<_, _>>()?,
            values: self
                .values
                .iter()
                .map(|(_, v)| graph::ValueDescription {
                    ty: v.ty,
                    definition: v.definition,
                    origin: v.origin,
                })
                .collect(),
            blocks,
        })
    }
}
