//! Focused static-lifetime graph, order, evidence, and diagnostic tests.

use crate::{
    mir::{
        dump_mir, lower_preliminary_hir, MirClassLifecycleOperation, MirExecutionNode,
        MirStaticFieldInitialization, VerifiedPreliminaryMirProgram,
    },
    test_support::type_check_source,
};

use super::*;

fn lower(text: &str) -> VerifiedPreliminaryMirProgram {
    let checked = type_check_source(text);
    assert!(checked.diagnostics.is_empty(), "{:?}", checked.diagnostics);
    crate::mir::verify_preliminary_mir(lower_preliminary_hir(&checked.hir.unwrap()))
        .expect("static-planning fixture must produce verified preliminary MIR")
}

fn plan(text: &str) -> PlannedMirProgram {
    plan_static_lifetimes(lower(text)).unwrap_or_else(|failure| {
        panic!(
            "unexpected lifetime diagnostics: {:?}",
            failure.diagnostics().collect::<Vec<_>>()
        )
    })
}

#[test]
fn sparse_planning_orders_only_active_dependencies() {
    let planned = plan(
        "fn read_base() -> i64 { return State.base; }
         class State {
           static result: i64 = read_base();
           static inactive: i64 = 2;
           static base: i64 = 1;
           init() {}
         }
         fn main() -> i64 { return State.result; }",
    );
    let fields = planned
        .static_fields()
        .map(|field| field.field)
        .collect::<Vec<_>>();

    assert_eq!(planned.lifecycle().activation(), &[fields[2], fields[0]]);
    assert_eq!(
        planned.lifecycle().shutdown().collect::<Vec<_>>(),
        [fields[0], fields[2]]
    );
    assert_eq!(planned.dependencies().len(), 1);
    assert_eq!(planned.dependencies()[0].prerequisite, fields[2]);
    assert_eq!(planned.dependencies()[0].dependent, fields[0]);
}

#[test]
fn orders_initialization_dependencies_and_independent_fields_deterministically() {
    let planned = plan(
        "fn read_last() -> i64 { return State.last; }
         class State {
           static dependent: i64 = read_last();
           static independent: i64 = 2;
           static last: i64 = 1;
           init() {}
         }
         fn main() -> i64 { return State.dependent + State.independent; }",
    );
    let fields = planned
        .static_fields()
        .map(|field| field.field)
        .collect::<Vec<_>>();

    assert_eq!(
        planned.lifecycle().activation(),
        &[fields[1], fields[2], fields[0]]
    );
    assert_eq!(
        planned.lifecycle().shutdown().collect::<Vec<_>>(),
        [fields[0], fields[2], fields[1]]
    );
    let dependencies = planned.dependencies();
    let dependency = dependencies.first().unwrap();
    assert_eq!(dependency.prerequisite, fields[2]);
    assert_eq!(dependency.dependent, fields[0]);
    assert_eq!(
        dependency.evidence.phase,
        StaticLifetimePhase::Initialization
    );
    assert_eq!(dependency.evidence.witness.len(), 1);
}

#[test]
fn includes_destruction_of_initializer_free_replaceable_owning_fields() {
    let planned = plan(
        "class State {
           static item: Item?;
           static flag: i64;
           init() {}
         }
         class Item {
           init() {}
           destroy { var observed: i64 = State.flag; }
         }
         fn main() -> i64 { State.item = none; return State.flag; }",
    );
    let fields = planned
        .static_fields()
        .map(|field| field.field)
        .collect::<Vec<_>>();
    let dependencies = planned.dependencies();
    let dependency = dependencies.first().unwrap();

    assert_eq!(dependency.prerequisite, fields[1]);
    assert_eq!(dependency.dependent, fields[0]);
    assert_eq!(dependency.evidence.phase, StaticLifetimePhase::Destruction);
    assert_eq!(planned.lifecycle().activation(), &[fields[1], fields[0]]);
}

#[test]
fn issues_exact_authority_for_explicit_zero_default_and_destructible_statics() {
    let planned = plan(
        "class State {
           static explicit: i64 = 1;
           static zero: i64;
           static item: Item?;
           init() {}
         }
         class Item {
           init() {}
           destroy { var observed: i64 = State.zero; }
         }
         fn main() -> i64 {
           State.item = none;
           return State.explicit + State.zero;
         }",
    );
    let definitions = planned.lifecycle_mir().definitions();
    let explicit = match definitions[0].initialization {
        MirStaticFieldInitialization::Explicit(initializer) => initializer,
        MirStaticFieldInitialization::ZeroDefault => panic!("explicit static lost initializer"),
    };

    assert_eq!(
        definitions[1].initialization,
        MirStaticFieldInitialization::ZeroDefault
    );
    assert_eq!(
        definitions[2].initialization,
        MirStaticFieldInitialization::ZeroDefault
    );
    assert!(planned
        .authority()
        .root(MirExecutionNode::callable(explicit.into()))
        .is_some());
    assert!(planned.authority().roots().any(|root| matches!(
        root.root(),
        MirExecutionNode::ClassLifecycle {
            operation: MirClassLifecycleOperation::CompleteFinalizer,
            ..
        }
    )));

    let authority_pairs = super::super::analysis::root_effects::dependency_pairs_for_definitions(
        planned.preliminary().program(),
        definitions,
        planned.authority(),
    )
    .unwrap();
    let report_pairs = planned
        .dependencies()
        .iter()
        .map(|dependency| (dependency.prerequisite, dependency.dependent))
        .collect();
    assert_eq!(authority_pairs, report_pairs);
}

#[test]
fn permits_post_publication_cleanup_to_access_the_newly_live_field() {
    let planned = plan(
        "fn select(ref item: Item) -> i64 { return 1; }
         class State {
           static value: i64 = select(Item());
           init() {}
         }
         class Item {
           init() {}
           destroy { var observed: i64 = State.value; }
         }
         fn main() -> i64 { return State.value; }",
    );

    assert!(planned.dependencies().is_empty());
    assert_eq!(planned.lifecycle().activation().len(), 1);
}

#[test]
fn rejects_pre_publication_self_dependencies_with_a_call_witness() {
    let failure = plan_static_lifetimes(lower(
        "fn read_value() -> i64 { return State.value; }
         class State {
           static value: i64 = read_value();
           init() {}
         }
         fn main() -> i64 { return State.value; }",
    ))
    .unwrap_err();
    let diagnostic = failure.diagnostics().next().unwrap();

    assert_eq!(diagnostic.code, STATIC_LIFECYCLE_SELF_DEPENDENCY);
    assert!(diagnostic
        .message
        .contains("initialization self-dependency"));
    assert!(diagnostic
        .labels
        .iter()
        .any(|label| label.message.contains("DirectCall")));
    assert_eq!(failure.dependencies()[0].evidence.witness.len(), 1);
}

#[test]
fn rejects_destruction_self_dependencies_for_initializer_free_arrays() {
    let failure = plan_static_lifetimes(lower(
        "class State {
           static items: Item[];
           init() {}
         }
         class Item {
           init() {}
           destroy { var count: u64 = State.items.len(); }
         }
         fn main() -> i64 { State.items = Item[]{}; return 0; }",
    ))
    .unwrap_err();
    let diagnostic = failure.diagnostics().next().unwrap();

    assert_eq!(diagnostic.code, STATIC_LIFECYCLE_SELF_DEPENDENCY);
    assert!(diagnostic.message.contains("destruction self-dependency"));
    assert_eq!(
        failure.dependencies()[0].evidence.phase,
        StaticLifetimePhase::Destruction
    );
}

#[test]
fn rejects_mixed_initialization_and_destruction_cycles() {
    let failure = plan_static_lifetimes(lower(
        "class State {
           static count: u64 = State.items.len();
           static items: Item[] = Item[]{};
           init() {}
         }
         class Item {
           init() {}
           destroy { var observed: u64 = State.count; }
         }
         fn main() -> i64 { State.count = State.count; return 0; }",
    ))
    .unwrap_err();
    let phases = failure
        .dependencies()
        .iter()
        .map(|dependency| dependency.evidence.phase)
        .collect::<Vec<_>>();
    let diagnostic = failure.diagnostics().next().unwrap();

    assert!(phases.contains(&StaticLifetimePhase::Initialization));
    assert!(phases.contains(&StaticLifetimePhase::Destruction));
    assert_eq!(diagnostic.code, STATIC_LIFECYCLE_DEPENDENCY_CYCLE);
    assert!(diagnostic
        .notes
        .iter()
        .any(|note| note.contains("required activation order closes a cycle")));
}

#[test]
fn callable_recursion_remains_separate_from_static_lifetime_cycles() {
    let planned = plan(
        "fn left(flag: bool) -> i64 {
           if (flag) { return State.base; }
           return right(true);
         }
         fn right(flag: bool) -> i64 {
           if (flag) { return left(true); }
           return 0;
         }
         class State {
           static result: i64 = left(false);
           static base: i64 = 1;
           init() {}
         }
         fn main() -> i64 { return State.result; }",
    );

    assert!(planned.planning_report().analysis().recursive_components() >= 1);
    assert_eq!(planned.dependencies().len(), 1);
}

#[test]
fn overlapping_cycles_select_one_stable_canonical_representative() {
    let source = "fn read_a() -> i64 { return State.a; }
         fn read_b() -> i64 { return State.b; }
         fn read_c() -> i64 { return State.c; }
         fn read_b_or_c(flag: bool) -> i64 {
           if (flag) { return read_b(); }
           return read_c();
         }
         class State {
           static a: i64 = read_b_or_c(true);
           static b: i64 = read_a();
           static c: i64 = read_a();
           init() {}
         }
         fn main() -> i64 { return State.a; }";
    let first = plan_static_lifetimes(lower(source)).unwrap_err();
    let first_dump = format!("{:?}", first.diagnostics().collect::<Vec<_>>());
    let diagnostic = first.diagnostics().next().unwrap();

    assert_eq!(diagnostic.code, STATIC_LIFECYCLE_DEPENDENCY_CYCLE);
    assert!(diagnostic
        .notes
        .iter()
        .any(|note| { note.contains("State.a -> State.b -> State.a") }));
    for _ in 0..8 {
        let failure = plan_static_lifetimes(lower(source)).unwrap_err();
        assert_eq!(
            first_dump,
            format!("{:?}", failure.diagnostics().collect::<Vec<_>>())
        );
    }
}

#[test]
fn exact_plan_dump_retains_effects_dependencies_witnesses_and_reverse_order() {
    let planned = plan(
        "fn read_base() -> i64 { return State.base; }
         class State {
           static result: i64 = read_base();
           static base: i64 = 1;
           init() {}
         }
         fn main() -> i64 { return State.result; }",
    );
    let dump = dump_static_lifetime_plan(&planned);
    let fields = planned
        .static_fields()
        .map(|field| field.field)
        .collect::<Vec<_>>();

    assert!(dump.starts_with("StaticLifetimePlan\n  Dependency "));
    assert!(dump.contains("Initialization root callable"));
    assert!(dump.contains("via callable"));
    assert!(dump.contains(&format!(
        "  Activation {} \"State.base\" {} \"State.result\"\n",
        fields[1], fields[0]
    )));
    assert!(dump.contains(&format!(
        "  Shutdown {} \"State.result\" {} \"State.base\"\n",
        fields[0], fields[1]
    )));
    assert_eq!(dump, dump_static_lifetime_plan(&planned));
    let planned_dump = dump_planned_mir(&planned);
    assert!(planned_dump.contains("StaticEffectAnalysis\n"));
    assert!(planned_dump.contains("StaticLifecycleBaselineAuthority\n"));
    assert!(planned_dump.contains("  Root callable"));
}

#[test]
fn planning_report_is_inspectable_but_synthesis_retains_only_compact_proof() {
    let planned = plan(
        "fn read() -> i64 { return State.base; }
         fn invoke(callback: fn() -> i64) -> i64 { return callback(); }
         class State {
           static base: i64 = 1;
           static result: i64 = invoke(read);
           init() {}
         }
         fn main() -> i64 { return State.result; }",
    );
    let report = planned.planning_report();
    assert!(report.analysis().function_value_candidates().len() > 0);
    assert!(report.analysis().summaries().len() > 0);
    assert_eq!(report.analysis().recursive_components(), 0);
    assert_eq!(
        report.activation().counts().declared_fields,
        planned.static_fields().len()
    );
    assert!(!report.activation().reachable_execution().is_empty());
    assert!(planned
        .dependencies()
        .iter()
        .any(|dependency| !dependency.evidence.witness.is_empty()));
    let authority = planned.authority().clone();

    let verified = super::super::verify_planned_mir(planned).unwrap();
    let final_program = super::super::synthesize_static_lifecycle(verified);
    let proof = final_program
        .static_lifecycle
        .as_ref()
        .unwrap()
        .lifecycle()
        .proof();
    assert_eq!(proof.authority(), &authority);

    let final_dump = dump_mir(&final_program);
    assert!(
        final_dump.contains("Proof authority-roots="),
        "{final_dump}"
    );
    for planning_only in [
        "StaticEffectAnalysis",
        "FunctionValueCandidates",
        "RecursiveComponents",
        "summaries=",
        "dependencies=",
    ] {
        assert!(!final_dump.contains(planning_only), "{final_dump}");
    }
}

mod generic_classes;
