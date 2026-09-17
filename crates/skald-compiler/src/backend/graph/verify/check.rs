//! Validate all structure before analysis or dataflow indexing.
use super::analysis::{analyze, GraphSession};
use super::model::*;
#[cfg_attr(not(test), allow(dead_code))]
pub(in crate::backend) fn check_graph<G: GraphView>(
    owner: &G,
) -> Result<GraphSession<'_, G>, Vec<GraphFailure>> {
    let graph = owner.describe().map_err(|failure| vec![failure])?;
    let mut failures = Vec::new();
    let identity = owner.identity();
    let mut fail = |location, reason, origin| {
        failures.push(GraphFailure {
            identity: Box::new(identity),
            location,
            reason,
            origin,
        })
    };
    let entry = graph.entry.filter(|entry| *entry < graph.blocks.len());
    if entry.is_none() {
        fail(GraphLocation::Entry, GraphReason::InvalidEntry, None);
    }
    let mut definitions = vec![None; graph.values.len()];
    let mut define = |value: usize, site, expected: Option<G::Type>, location| {
        let Some(record) = graph.values.get(value) else {
            fail(location, GraphReason::OutOfBounds, None);
            return;
        };
        if definitions[value].replace(site).is_some() {
            fail(location, GraphReason::DuplicateDefinition, record.origin);
        }
        if record.definition != Some(site) {
            fail(location, GraphReason::DefinitionMismatch, record.origin);
        }
        if expected.is_some_and(|ty| ty != record.ty) {
            fail(location, GraphReason::TypeMismatch, record.origin);
        }
    };
    for (component, &(value, ty)) in graph.inputs.iter().enumerate() {
        define(
            value,
            DefinitionSite::EntryInput(component),
            Some(ty),
            GraphLocation::Entry,
        );
    }
    for (block, data) in graph.blocks.iter().enumerate() {
        if let Some(parameters) = &data.parameters {
            for (ordinal, &value) in parameters.iter().enumerate() {
                define(
                    value,
                    DefinitionSite::Parameter { block, ordinal },
                    None,
                    GraphLocation::Parameter { block, ordinal },
                );
            }
        }
        for (instruction, data) in data.instructions.iter().enumerate() {
            for (ordinal, &(value, ty)) in data.results.iter().enumerate() {
                define(
                    value,
                    DefinitionSite::Result {
                        block,
                        instruction,
                        ordinal,
                    },
                    Some(ty),
                    GraphLocation::Instruction {
                        block,
                        ordinal: instruction,
                        operand: ordinal,
                    },
                );
            }
        }
    }
    let mut check_use = |used: &TypedUse<G::Type>, location| match graph.values.get(used.value) {
        None => fail(location, GraphReason::OutOfBounds, None),
        Some(value) if used.expected.is_some_and(|ty| ty != value.ty) => {
            fail(location, GraphReason::TypeMismatch, value.origin)
        }
        _ => {}
    };
    for (block, data) in graph.blocks.iter().enumerate() {
        for (ordinal, instruction) in data.instructions.iter().enumerate() {
            for (operand, used) in instruction.uses.iter().enumerate() {
                check_use(
                    used,
                    GraphLocation::Instruction {
                        block,
                        ordinal,
                        operand,
                    },
                );
            }
        }
        if let Some((uses, edges)) = &data.terminal {
            for (operand, used) in uses.iter().enumerate() {
                check_use(used, GraphLocation::Terminator { block, operand });
            }
            for (slot, edge) in edges.iter().enumerate() {
                for (operand, used) in edge.arguments.iter().enumerate() {
                    check_use(
                        used,
                        GraphLocation::Edge {
                            block,
                            slot,
                            operand,
                        },
                    );
                }
            }
        }
    }
    for (value, record) in graph.values.iter().enumerate() {
        if definitions[value].is_none() || record.definition.is_none() {
            fail(
                GraphLocation::Value(value),
                GraphReason::UnresolvedValue,
                record.origin,
            );
        }
    }
    for (block, data) in graph.blocks.iter().enumerate() {
        if data.parameters.is_none() {
            fail(
                GraphLocation::Block(block),
                GraphReason::UnresolvedBlock,
                None,
            );
        }
        if Some(block) == entry && data.parameters.as_ref().is_some_and(|p| !p.is_empty()) {
            fail(GraphLocation::Entry, GraphReason::InvalidEntry, None);
        }
        let Some((_, edges)) = &data.terminal else {
            fail(
                GraphLocation::Block(block),
                GraphReason::MissingTerminator,
                None,
            );
            continue;
        };
        for (slot, edge) in edges.iter().enumerate() {
            let location = GraphLocation::Edge {
                block,
                slot,
                operand: 0,
            };
            let Some(target) = graph.blocks.get(edge.target) else {
                fail(location, GraphReason::OutOfBounds, None);
                continue;
            };
            if Some(edge.target) == entry {
                fail(location, GraphReason::EntryPredecessor, None);
            }
            if let Some(parameters) = &target.parameters {
                if parameters.len() != edge.arguments.len() {
                    fail(location, GraphReason::ArityMismatch, None);
                }
                for (operand, (parameter, argument)) in
                    parameters.iter().zip(&edge.arguments).enumerate()
                {
                    if let (Some(parameter), Some(argument)) = (
                        graph.values.get(*parameter),
                        graph.values.get(argument.value),
                    ) {
                        if parameter.ty != argument.ty {
                            fail(
                                GraphLocation::Edge {
                                    block,
                                    slot,
                                    operand,
                                },
                                GraphReason::TypeMismatch,
                                argument.origin,
                            );
                        }
                    }
                }
            }
        }
    }
    if !failures.is_empty() {
        return Err(canonical(failures));
    }
    let session = analyze(owner, &graph, entry.expect("entry checked"));
    for (block, data) in graph.blocks.iter().enumerate() {
        let check = |used: &TypedUse<G::Type>,
                     position: usize,
                     location,
                     failures: &mut Vec<GraphFailure>| {
            let value = &graph.values[used.value];
            let (definition_block, ordinal) = match value.definition.expect("definitions checked") {
                DefinitionSite::EntryInput(_) => return,
                DefinitionSite::Parameter { block, .. } => (block, None),
                DefinitionSite::Result {
                    block, instruction, ..
                } => (block, Some(instruction)),
            };
            let reason = if definition_block == block {
                ordinal
                    .filter(|ordinal| *ordinal >= position)
                    .map(|_| GraphReason::UseBeforeDefinition)
            } else if session.reachable[block]
                && session.dominates(definition_block, block) != Some(true)
            {
                Some(GraphReason::NonDominatingUse)
            } else {
                None
            };
            if let Some(reason) = reason {
                failures.push(GraphFailure {
                    identity: Box::new(identity),
                    location,
                    reason,
                    origin: value.origin,
                });
            }
        };
        for (ordinal, instruction) in data.instructions.iter().enumerate() {
            for (operand, used) in instruction.uses.iter().enumerate() {
                check(
                    used,
                    ordinal,
                    GraphLocation::Instruction {
                        block,
                        ordinal,
                        operand,
                    },
                    &mut failures,
                );
            }
        }
        let (uses, edges) = data.terminal.as_ref().expect("terminal checked");
        for (operand, used) in uses.iter().enumerate() {
            check(
                used,
                data.instructions.len(),
                GraphLocation::Terminator { block, operand },
                &mut failures,
            );
        }
        for (slot, edge) in edges.iter().enumerate() {
            for (operand, used) in edge.arguments.iter().enumerate() {
                check(
                    used,
                    data.instructions.len(),
                    GraphLocation::Edge {
                        block,
                        slot,
                        operand,
                    },
                    &mut failures,
                );
            }
        }
    }
    if failures.is_empty() {
        Ok(session)
    } else {
        Err(canonical(failures))
    }
}
#[cfg_attr(not(test), allow(dead_code))]
fn canonical(mut failures: Vec<GraphFailure>) -> Vec<GraphFailure> {
    failures.sort_by_key(|failure| {
        let location = match failure.location {
            GraphLocation::Entry => (0, 0, 0, 0, 0),
            GraphLocation::Value(value) => (1, value, 0, 0, 0),
            GraphLocation::Block(block) => (2, block, 0, 0, 0),
            GraphLocation::Parameter { block, ordinal } => (2, block, 1, ordinal, 0),
            GraphLocation::Instruction {
                block,
                ordinal,
                operand,
            } => (2, block, 2, ordinal, operand),
            GraphLocation::Terminator { block, operand } => (2, block, 3, 0, operand),
            GraphLocation::Edge {
                block,
                slot,
                operand,
            } => (2, block, 4, slot, operand),
        };
        (location, failure.reason)
    });
    failures.dedup();
    failures
}
