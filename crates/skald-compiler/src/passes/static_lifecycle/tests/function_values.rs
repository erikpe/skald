//! Closed-world function-value candidates and static-effect propagation.

use crate::{
    identity::{CallableId, FunctionId},
    mir::{
        PreliminaryMirProgram, StaticEffectEdgeKind, StaticEffectNode,
        StaticFunctionValueCandidates,
    },
    test_support::lower_generic_source_to_preliminary_mir,
};

use super::super::{
    dump_static_effects, infer_static_effects, plan_static_lifetimes,
    STATIC_LIFECYCLE_DEPENDENCY_CYCLE, STATIC_LIFECYCLE_SELF_DEPENDENCY,
};
use super::{effect_fields, lower};

fn function(program: &PreliminaryMirProgram, name: &str) -> CallableId {
    program
        .program()
        .declarations
        .iter()
        .find(|declaration| declaration.name == name)
        .map(|declaration| CallableId::Function(declaration.id))
        .unwrap_or_else(|| panic!("missing function `{name}`"))
}

fn only_candidates(analysis: &crate::mir::StaticEffectAnalysis) -> &StaticFunctionValueCandidates {
    let candidates = analysis.function_value_candidates().collect::<Vec<_>>();
    assert_eq!(candidates.len(), 1);
    candidates[0]
}

#[test]
fn expands_each_indirect_call_to_every_exact_signature_target() {
    let preliminary = lower(
        "fn read_left() -> i64 { return State.left; }
         fn read_right() -> i64 { return State.right; }
         fn invoke(callback: fn() -> i64) -> i64 { return callback(); }
         fn retain_only() -> unit { var callback: fn() -> i64 = read_right; }
         class State {
           static left: i64 = 10;
           static right: i64 = 20;
           static result: i64 = invoke(read_left);
           init() {}
         }
         fn main() -> i64 { return State.result; }",
    );
    let fields = preliminary
        .static_fields()
        .map(|field| field.field)
        .collect::<Vec<_>>();
    let analysis = infer_static_effects(&preliminary);
    let candidates = only_candidates(&analysis);

    assert_eq!(
        candidates
            .targets
            .iter()
            .map(|target| target.callable)
            .collect::<Vec<_>>(),
        [
            function(&preliminary, "read_left"),
            function(&preliminary, "read_right")
        ]
    );
    let invoke = analysis
        .summary(StaticEffectNode::Callable(function(&preliminary, "invoke")))
        .unwrap();
    assert_eq!(
        invoke
            .possible_targets
            .iter()
            .filter(|edge| edge.kind == StaticEffectEdgeKind::IndirectCall)
            .map(|edge| edge.target)
            .collect::<Vec<_>>(),
        candidates
            .targets
            .iter()
            .map(|target| StaticEffectNode::Callable(target.callable))
            .collect::<Vec<_>>()
    );
    assert_eq!(
        effect_fields(
            &analysis,
            StaticEffectNode::Callable(function(&preliminary, "invoke"))
        ),
        fields[..2]
    );

    let retain_only = analysis
        .summary(StaticEffectNode::Callable(function(
            &preliminary,
            "retain_only",
        )))
        .unwrap();
    assert!(retain_only.direct_effects.is_empty());
    assert!(retain_only.possible_targets.is_empty());
    assert!(retain_only.effects.is_empty());

    let dump = dump_static_effects(&analysis);
    assert!(dump.contains("FunctionValueCandidates\n    Type ft0"));
    assert!(dump.contains("Retain f0"));
    assert!(dump.contains("Retain f1"));
    assert!(dump.contains("IndirectCall"));
    assert_eq!(
        dump,
        dump_static_effects(&infer_static_effects(&preliminary))
    );

    let planned = plan_static_lifetimes(preliminary).unwrap();
    let result = fields[2];
    assert!(planned
        .dependencies()
        .iter()
        .filter(|dependency| dependency.dependent == result)
        .any(|dependency| dependency.prerequisite == fields[0]
            && dependency
                .evidence
                .witness
                .iter()
                .any(|edge| edge.kind == StaticEffectEdgeKind::IndirectCall)));
    assert!(planned
        .dependencies()
        .iter()
        .any(|dependency| dependency.dependent == result && dependency.prerequisite == fields[1]));
}

#[test]
fn indirect_effects_participate_in_self_and_cycle_diagnostics() {
    let self_failure = plan_static_lifetimes(lower(
        "fn read() -> i64 { return State.value; }
         fn invoke(callback: fn() -> i64) -> i64 { return callback(); }
         class State {
           static value: i64 = invoke(read);
           init() {}
         }
         fn main() -> i64 { return 0; }",
    ))
    .unwrap_err();
    let self_diagnostic = self_failure.diagnostics().next().unwrap();
    assert_eq!(self_diagnostic.code, STATIC_LIFECYCLE_SELF_DEPENDENCY);
    assert!(self_diagnostic
        .labels
        .iter()
        .any(|label| label.message.contains("IndirectCall")));

    let cycle_failure = plan_static_lifetimes(lower(
        "fn read_left(value: i64) -> i64 { return State.left + value; }
         fn read_right(flag: bool) -> i64 {
           if (flag) { return State.right; }
           return State.right;
         }
         fn invoke_i64(callback: fn(i64) -> i64) -> i64 { return callback(0); }
         fn invoke_bool(callback: fn(bool) -> i64) -> i64 { return callback(false); }
         class State {
           static left: i64 = invoke_bool(read_right);
           static right: i64 = invoke_i64(read_left);
           init() {}
         }
         fn main() -> i64 { return 0; }",
    ))
    .unwrap_err();
    let cycle_diagnostic = cycle_failure.diagnostics().next().unwrap();
    assert_eq!(cycle_diagnostic.code, STATIC_LIFECYCLE_DEPENDENCY_CYCLE);
    assert!(cycle_failure
        .dependencies()
        .iter()
        .all(|dependency| dependency
            .evidence
            .witness
            .iter()
            .any(|edge| edge.kind == StaticEffectEdgeKind::IndirectCall)));
}

#[test]
fn same_signature_generic_targets_keep_independent_effect_nodes() {
    let preliminary = lower_generic_source_to_preliminary_mir(
        "class Cell<T> {
           static value: i64 = 1;
           init() {}
           static fn read() -> i64 { return Cell<T>.value; }
         }
         fn invoke(callback: fn() -> i64) -> i64 { return callback(); }
         fn retain() -> unit {
           var first: fn() -> i64 = Cell<i64>.read;
           var second: fn() -> i64 = Cell<bool>.read;
         }
         class State {
           static result: i64 = invoke(Cell<i64>.read);
           init() {}
         }
         fn main() -> i64 { return State.result; }",
    );
    let analysis = infer_static_effects(&preliminary);
    let candidates = only_candidates(&analysis);
    assert_eq!(candidates.targets.len(), 2);
    assert!(candidates
        .targets
        .iter()
        .all(|target| matches!(target.callable, CallableId::Method(_))));

    let target_effects = candidates
        .targets
        .iter()
        .map(|target| {
            let effects = effect_fields(&analysis, StaticEffectNode::Callable(target.callable));
            assert_eq!(effects.len(), 1);
            effects[0]
        })
        .collect::<Vec<_>>();
    assert_ne!(target_effects[0], target_effects[1]);
    assert_ne!(target_effects[0].class(), target_effects[1].class());

    let invoke = function(&preliminary, "invoke");
    let edges = &analysis
        .summary(StaticEffectNode::Callable(invoke))
        .unwrap()
        .possible_targets;
    assert_eq!(
        edges
            .iter()
            .filter(|edge| edge.kind == StaticEffectEdgeKind::IndirectCall)
            .count(),
        2
    );
}

#[test]
fn candidate_identity_order_is_not_source_formation_order() {
    let preliminary = lower(
        "fn first() -> i64 { return 1; }
         fn second() -> i64 { return 2; }
         fn retain() -> unit {
           var later_identity: fn() -> i64 = second;
           var earlier_identity: fn() -> i64 = first;
         }
         fn main() -> i64 { return 0; }",
    );
    let analysis = infer_static_effects(&preliminary);

    assert_eq!(
        only_candidates(&analysis)
            .targets
            .iter()
            .map(|target| target.callable)
            .collect::<Vec<_>>(),
        [
            CallableId::Function(FunctionId::new(0)),
            CallableId::Function(FunctionId::new(1)),
        ]
    );
}
