use std::collections::HashSet;

use crate::{
    identity::FunctionId,
    mir::{
        rewrite::{MirReferenceFailure, MirRewriteError},
        BlockId, MirLogicalOperation, MirTerminator, PathConditionId,
    },
    test_support::lower_source_to_final_mir,
};

use super::*;

fn observations(
    program: &crate::mir::MirProgram,
) -> Vec<(crate::identity::CallableId, LogicalTopologyObservation)> {
    program
        .executable_definitions()
        .flat_map(|definition| {
            observe_logical_topologies(definition)
                .unwrap()
                .into_iter()
                .map(move |observation| (definition.callable(), observation))
        })
        .collect()
}

fn entry_definition_mut(
    program: &mut crate::mir::MirProgram,
) -> &mut crate::mir::MirFunctionDefinition {
    program
        .definitions
        .get_mut_for_test(program.entry_function)
        .unwrap()
}

#[test]
fn observes_all_operations_and_nested_records_without_constant_requirements() {
    let program = lower_source_to_final_mir(concat!(
        "fn choose(a: bool, b: bool, c: bool) -> bool { return (a && b) || (c && a); }\n",
        "fn main() -> i64 { return 0; }\n",
    ));
    let observed = observations(&program);

    assert_eq!(observed.len(), 3);
    assert!(observed
        .iter()
        .all(|(_, observation)| matches!(observation, LogicalTopologyObservation::Protocol(_))));
    assert_eq!(
        observed
            .iter()
            .map(|(_, observation)| match observation {
                LogicalTopologyObservation::Protocol(topology) => topology.operation,
                LogicalTopologyObservation::Rejected { .. } => unreachable!(),
            })
            .collect::<Vec<_>>(),
        [
            MirLogicalOperation::Or,
            MirLogicalOperation::And,
            MirLogicalOperation::And,
        ]
    );
    assert!(observed.iter().any(|(_, observation)| matches!(
        observation,
        LogicalTopologyObservation::Protocol(topology) if topology.parent_condition.is_some()
    )));
}

#[test]
fn records_exact_path_condition_relationships_and_spans_without_mutation() {
    let program = lower_source_to_final_mir(
        "fn choose(left: bool, right: bool) -> bool { return left && right; } fn main() -> i64 { return 0; }",
    );
    let original = program.clone();
    let definition = program
        .executable_definitions()
        .find(|definition| !definition.logical_expressions().is_empty())
        .unwrap();
    let first = observe_logical_topologies(definition).unwrap();
    let LogicalTopologyObservation::Protocol(topology) = &first[0] else {
        panic!("expected logical topology");
    };
    let condition = definition.path_condition(topology.condition).unwrap();

    assert_eq!(program, original);
    assert_eq!(topology.record_index, 0);
    assert_eq!(topology.activation, condition.activation);
    assert_eq!(topology.active_predecessor, condition.active_predecessor);
    assert_eq!(
        topology.inactive_predecessor,
        condition.inactive_predecessor
    );
    assert_eq!(topology.selection, condition.merge);
    assert_eq!(topology.condition_span, condition.span);
    assert_eq!(
        topology.logical_span,
        definition.logical_expressions()[0].span
    );
    assert_eq!(first, observe_logical_topologies(definition).unwrap());
}

#[test]
fn observes_functions_methods_initializers_destructors_and_static_initializers() {
    let program = lower_source_to_final_mir(
        "class State {
           static selected: bool = true && false;
           initialized: bool;
           init() { self.initialized = true && false; }
           fn value() -> bool { return false || true; }
           destroy { var finished: bool = true && false; }
         }
         fn choose() -> bool { return true && false; }
         fn main() -> i64 {
           var state: State = State();
           if (state.value() || State.selected || choose()) { return 1; }
           return 0;
         }",
    );
    let observed = observations(&program);
    let owners = observed
        .iter()
        .map(|(owner, _)| *owner)
        .collect::<HashSet<_>>();

    assert!(owners
        .iter()
        .any(|owner| matches!(owner, crate::identity::CallableId::Function(_))));
    assert!(owners
        .iter()
        .any(|owner| matches!(owner, crate::identity::CallableId::Initializer(_))));
    assert!(owners
        .iter()
        .any(|owner| matches!(owner, crate::identity::CallableId::Method(_))));
    assert!(owners
        .iter()
        .any(|owner| matches!(owner, crate::identity::CallableId::Destructor(_))));
    assert!(owners
        .iter()
        .any(|owner| matches!(owner, crate::identity::CallableId::StaticInitializer(_))));
    assert!(observed
        .iter()
        .all(|(_, observation)| matches!(observation, LogicalTopologyObservation::Protocol(_))));
}

#[test]
fn duplicate_mismatched_and_malformed_records_have_structured_outcomes() {
    let mut duplicate = lower_source_to_final_mir(
        "fn main() -> i64 { if (true && false) { return 1; } return 0; }",
    );
    let definition = entry_definition_mut(&mut duplicate);
    definition
        .body
        .logical_expressions
        .push(definition.body.logical_expressions[0].clone());
    assert!(matches!(
        observe_logical_topologies((&*definition).into())
            .unwrap()
            .as_slice(),
        [
            LogicalTopologyObservation::Protocol(_),
            LogicalTopologyObservation::Rejected {
                reason: LogicalTopologyRejectionReason::DuplicateCondition,
                ..
            }
        ]
    ));

    let mut mismatched = lower_source_to_final_mir(
        "fn main() -> i64 { if (true || false) { return 1; } return 0; }",
    );
    let definition = entry_definition_mut(&mut mismatched);
    definition.body.path_conditions[0].merge = definition.body.logical_expressions[0].join;
    assert!(matches!(
        observe_logical_topologies((&*definition).into())
            .unwrap()
            .as_slice(),
        [LogicalTopologyObservation::Rejected {
            reason: LogicalTopologyRejectionReason::MismatchedPathCondition,
            ..
        }]
    ));

    let mut missing = lower_source_to_final_mir(
        "fn main() -> i64 { if (true && false) { return 1; } return 0; }",
    );
    let owner = missing.entry_function;
    let definition = entry_definition_mut(&mut missing);
    definition.body.logical_expressions[0].condition =
        PathConditionId::new(FunctionId::new(owner.index() + 1), 0);
    assert!(matches!(
        observe_logical_topologies((&*definition).into()),
        Err(MirRewriteError::InvalidReference {
            failure: MirReferenceFailure::Foreign,
            ..
        })
    ));

    let mut malformed = lower_source_to_final_mir(
        "fn main() -> i64 { if (true && false) { return 1; } return 0; }",
    );
    let definition = entry_definition_mut(&mut malformed);
    let owner = definition.callable();
    let split = definition.body.logical_expressions[0].split;
    let MirTerminator::Branch { true_target, .. } = definition.body.blocks[split.index()]
        .terminator
        .as_mut()
        .unwrap()
    else {
        unreachable!();
    };
    *true_target = BlockId::new(owner, split.index());
    assert!(matches!(
        observe_logical_topologies((&*definition).into())
            .unwrap()
            .as_slice(),
        [LogicalTopologyObservation::Rejected {
            reason: LogicalTopologyRejectionReason::NonCanonicalTopology,
            ..
        }]
    ));
}

#[test]
fn repeated_queries_preserve_definition_and_record_order() {
    let program = lower_source_to_final_mir(concat!(
        "fn first(a: bool, b: bool) -> bool { return a && b; }\n",
        "fn second(a: bool, b: bool) -> bool { return a || b; }\n",
        "fn main() -> i64 { return 0; }\n",
    ));
    let first = observations(&program);
    assert_eq!(first, observations(&program));
    assert!(first.windows(2).all(|window| window[0].0 <= window[1].0));
}
