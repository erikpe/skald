//! Static-effect equivalence between operator syntax and explicit calls.

use crate::{
    identity::CallableId,
    mir::{PreliminaryMirProgram, StaticEffectNode},
    resolve::resolve_module_graph,
    test_support::load_module_sources_with_standard_library,
    typeck::type_check,
};

use super::super::super::plan_static_lifetimes;
use super::super::{infer_static_effects, StaticEffectEdgeKind, StaticEffectSummary};

fn operator_program() -> PreliminaryMirProgram {
    let (_workspace, graph) = load_module_sources_with_standard_library(
        "app",
        &[(
            "app.ska",
            r#"
from std::ops import OpAdd;

class State {
    static base: i64 = 10;
    static child: i64 = 20;
    init() {}
}

class Base implements OpAdd<Base, i64> {
    init() {}
    virtual fn op_add(ref rhs: Base) -> i64 { return State.base; }
}

class Child extends Base {
    init() { super(); }
    override fn op_add(ref rhs: Base) -> i64 { return State.child; }
}

fn punctuation(ref left: OpAdd<Base, i64>, ref right: Base) -> i64 {
    return left + right;
}

fn explicit(ref left: OpAdd<Base, i64>, ref right: Base) -> i64 {
    return left.op_add(right);
}

fn main() -> i64 { return 0; }
"#,
        )],
    );
    let resolved = resolve_module_graph(&graph);
    assert!(
        resolved.diagnostics.is_empty(),
        "{:?}",
        resolved.diagnostics
    );
    let checked = type_check(&resolved.program);
    assert!(checked.diagnostics.is_empty(), "{:?}", checked.diagnostics);
    crate::mir::lower_preliminary_hir(&checked.hir.unwrap())
}

fn function(program: &PreliminaryMirProgram, name: &str) -> CallableId {
    program
        .program()
        .declarations
        .iter()
        .find(|declaration| declaration.name == name)
        .map(|declaration| CallableId::Function(declaration.id))
        .unwrap_or_else(|| panic!("missing function `{name}`"))
}

#[test]
fn punctuation_and_explicit_protocol_calls_feed_identical_effect_and_target_owners() {
    let preliminary = operator_program();
    let punctuation = StaticEffectNode::Callable(function(&preliminary, "punctuation"));
    let explicit = StaticEffectNode::Callable(function(&preliminary, "explicit"));
    let analysis = infer_static_effects(&preliminary);
    let punctuation_summary = analysis.summary(punctuation).unwrap();
    let explicit_summary = analysis.summary(explicit).unwrap();

    let targets = |summary: &StaticEffectSummary| {
        summary
            .possible_targets
            .iter()
            .map(|edge| (edge.target, edge.kind))
            .collect::<Vec<_>>()
    };
    assert_eq!(targets(punctuation_summary), targets(explicit_summary));
    assert_eq!(targets(punctuation_summary).len(), 2);
    assert!(targets(punctuation_summary)
        .iter()
        .all(|(_, kind)| *kind == StaticEffectEdgeKind::InterfaceDispatch));

    let fields = |summary: &StaticEffectSummary| {
        summary
            .effects
            .iter()
            .map(|effect| effect.field)
            .collect::<Vec<_>>()
    };
    assert_eq!(fields(punctuation_summary), fields(explicit_summary));
    assert_eq!(fields(punctuation_summary).len(), 2);

    let planned = plan_static_lifetimes(preliminary)
        .expect("operator interface targets must survive lifecycle planning");
    crate::passes::static_lifecycle::verify_planned_mir(&planned)
        .expect("operator calls must retain valid static-lifecycle authority");
}
