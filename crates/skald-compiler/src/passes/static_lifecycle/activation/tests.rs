//! Activation vocabulary, fixture, and current-behavior baselines.

use std::process::Command;

use crate::{
    identity::StaticFieldId,
    mir::{
        lower_preliminary_hir, verify_preliminary_mir, PreliminaryMirProgram, StaticAccessKind,
        StaticEffectPhase,
    },
    passes::static_lifecycle::{plan_static_lifetimes, verify_planned_mir},
    resolve::resolve_module_graph,
    test_support::{
        load_module_sources, load_module_sources_with_standard_library,
        lower_generic_source_to_preliminary_mir, type_check_source,
    },
    typeck::type_check,
};

use super::test_support::{
    activation_analysis_fixture, activation_identity_fixture, CYCLE_SOURCE, DIRECT_ACCESS_SOURCE,
    DYNAMIC_AND_INDIRECT_SOURCE, IMPLICIT_LIFECYCLE_SOURCE, INACTIVE_ONLY_DEPENDENCY_SOURCE,
    SELF_DEPENDENCY_SOURCE, STORED_FAMILY_SOURCE,
};
use super::{
    analyze_static_activation, dump_static_activation, static_activation_edge_key,
    static_activation_node_key, StaticActivationNode, StaticActivationTrigger,
};
use crate::passes::static_lifecycle::StaticActivationInspection;

const DETERMINISM_CHILD: &str = "SKALD_STATIC_ACTIVATION_DETERMINISM_CHILD";
const FINGERPRINT_BEGIN: &str = "-- static activation fingerprint begin --";
const FINGERPRINT_END: &str = "-- static activation fingerprint end --";

#[test]
fn node_and_edge_keys_define_stable_semantic_order() {
    let fixture = activation_identity_fixture();
    let analysis = activation_analysis_fixture(true);

    assert!(
        static_activation_node_key(StaticActivationNode::execution(fixture.entry))
            < static_activation_node_key(StaticActivationNode::field(fixture.active))
    );
    assert!(analysis
        .edges()
        .windows(2)
        .all(|pair| static_activation_edge_key(&pair[0]) < static_activation_edge_key(&pair[1])));
    assert_eq!(
        analysis
            .reachable_execution()
            .iter()
            .map(|execution| execution.node())
            .collect::<Vec<_>>(),
        vec![
            fixture.entry,
            fixture.helper,
            fixture.initializer,
            fixture.destruction,
        ]
    );
}

#[test]
fn immutable_queries_keep_fields_execution_witnesses_and_counts_coherent() {
    let fixture = activation_identity_fixture();
    let first = activation_analysis_fixture(false);
    let reordered = activation_analysis_fixture(true);

    assert_eq!(first, reordered);
    assert!(first.is_active(fixture.active));
    assert!(!first.is_active(fixture.inactive));
    assert_eq!(first.inactive_fields(), &[fixture.inactive]);
    assert!(first.is_execution_reachable(fixture.entry));
    assert!(first.is_execution_reachable(fixture.initializer));
    assert!(first.execution(fixture.destruction).is_some());
    assert_eq!(
        first
            .field(fixture.active)
            .unwrap()
            .first_trigger()
            .trigger(),
        StaticActivationTrigger::StaticAccess {
            access: StaticAccessKind::Read,
            phase: StaticEffectPhase::Ordinary,
        }
    );
    assert_eq!(
        first
            .field(fixture.active)
            .unwrap()
            .witness()
            .root()
            .entry(),
        fixture.entry
    );
    assert_eq!(first.active_fields().len(), 1);
    assert_eq!(first.counts().declared_fields, 2);
    assert_eq!(first.counts().active_fields, 1);
    assert_eq!(first.counts().inactive_fields, 1);
    assert_eq!(first.counts().reachable_execution_nodes, 4);
    assert_eq!(first.counts().edges, 4);
    assert_eq!(first.counts().execution_dependencies, 1);
    assert_eq!(first.counts().static_accesses, 1);
    assert_eq!(first.counts().initializer_roots, 1);
    assert_eq!(first.counts().destruction_roots, 1);
    assert_eq!(
        first
            .outgoing_dependencies(StaticActivationNode::execution(fixture.entry))
            .len(),
        1
    );
    assert_eq!(
        first.target_count(crate::passes::reachability::MirDependencyEdgeKind::DirectCall),
        1
    );
    assert_eq!(
        first.target_count(crate::passes::reachability::MirDependencyEdgeKind::Initializer),
        1
    );
    assert_eq!(
        first.target_count(crate::passes::reachability::MirDependencyEdgeKind::ArrayDestruction),
        1
    );
}

#[test]
fn focused_source_fixtures_cover_future_activation_inputs() {
    for source in [
        DIRECT_ACCESS_SOURCE,
        DYNAMIC_AND_INDIRECT_SOURCE,
        IMPLICIT_LIFECYCLE_SOURCE,
        INACTIVE_ONLY_DEPENDENCY_SOURCE,
    ] {
        let checked = type_check_source(source);
        assert!(checked.diagnostics.is_empty(), "{:?}", checked.diagnostics);
        let preliminary = lower_preliminary_hir(&checked.hir.unwrap());
        assert!(preliminary.executable_definitions().next().is_some());
    }

    let (_workspace, graph) =
        load_module_sources_with_standard_library("app", &[("app.ska", STORED_FAMILY_SOURCE)]);
    let resolved = resolve_module_graph(&graph);
    assert!(
        resolved.diagnostics.is_empty(),
        "{:?}",
        resolved.diagnostics
    );
    let checked = type_check(&resolved.program);
    assert!(checked.diagnostics.is_empty(), "{:?}", checked.diagnostics);
    let preliminary = lower_preliminary_hir(&checked.hir.unwrap());
    assert!(preliminary.static_fields().len() >= 12);
}

#[test]
fn inactive_self_dependencies_and_cycles_do_not_enter_lifecycle_planning() {
    for source in [SELF_DEPENDENCY_SOURCE, CYCLE_SOURCE] {
        let checked = type_check_source(source);
        assert!(checked.diagnostics.is_empty(), "{:?}", checked.diagnostics);
        let preliminary = lower_preliminary_hir(&checked.hir.unwrap());
        let planned = plan_static_lifetimes(preliminary).unwrap();
        assert!(planned.activation_authority().is_empty());
        assert!(planned.lifecycle().activation().is_empty());
    }
}

#[test]
fn closure_handles_empty_direct_transitive_recursive_and_structural_roots() {
    let empty = lower("fn main() -> i64 { return 0; }");
    let empty_analysis = analyze_static_activation(&empty).unwrap();
    assert!(empty_analysis.active_fields().is_empty());
    assert_eq!(empty_analysis.reachable_execution().len(), 1);

    let program = lower(
        "class State {
           static direct: i64 = 1;
           static structural: i64 = 2;
           static dormant: i64 = 3;
           init() {}
         }
         fn recursive(count: i64) -> i64 {
           if (count == 0) { return State.direct; }
           return recursive(count - 1);
         }
         fn hidden() -> i64 { return State.dormant; }
         fn main() -> i64 {
           if (false) { return State.structural; }
           return recursive(1);
         }",
    );
    let analysis = analyze_static_activation(&program).unwrap();

    assert_active(&program, &analysis, "State.direct");
    assert_active(&program, &analysis, "State.structural");
    assert_inactive(&program, &analysis, "State.dormant");
    assert!(analysis.counts().initializer_roots >= 2);
}

#[test]
fn active_initializers_and_eventual_destruction_expand_the_same_fixed_point() {
    let program = lower(
        "class Tracker {
           value: i64;
           init() { self.value = 0; }
           fn read() -> i64 { return self.value; }
           destroy { var observed: i64 = State.shutdown; }
         }
         class State {
           static base: i64 = 4;
           static derived: i64 = State.base + 1;
           static tracker: Tracker = Tracker();
           static shutdown: i64 = 9;
           static dormant: i64 = 10;
           init() {}
         }
         fn main() -> i64 { return State.derived + State.tracker.read(); }",
    );
    let analysis = analyze_static_activation(&program).unwrap();

    for name in [
        "State.base",
        "State.derived",
        "State.tracker",
        "State.shutdown",
    ] {
        assert_active(&program, &analysis, name);
    }
    assert_inactive(&program, &analysis, "State.dormant");
    assert!(analysis.counts().destruction_roots >= 1);
    assert!(analysis.target_counts().iter().any(|count| count.kind()
        == crate::passes::reachability::MirDependencyEdgeKind::CompleteFinalizer));
}

#[test]
fn dynamic_dispatch_and_reached_function_values_use_conservative_exact_targets() {
    let dispatch = lower(
        "class State {
           static base: i64 = 1;
           static child: i64 = 2;
           init() {}
         }
         interface View { fn read() -> i64; }
         class Base implements View {
           init() {}
           virtual fn read() -> i64 { return State.base; }
         }
         class Child extends Base {
           init() { super(); }
           override fn read() -> i64 { return State.child; }
         }
         fn read_virtual(ref value: Base) -> i64 { return value.read(); }
         fn read_interface(ref value: View) -> i64 { return value.read(); }
         fn main() -> i64 {
           return read_virtual(Child()) + read_interface(Child());
         }",
    );
    let dispatch_analysis = analyze_static_activation(&dispatch).unwrap();
    assert_active(&dispatch, &dispatch_analysis, "State.base");
    assert_active(&dispatch, &dispatch_analysis, "State.child");
    assert!(
        dispatch_analysis
            .target_count(crate::passes::reachability::MirDependencyEdgeKind::VirtualDispatch)
            >= 2
    );
    assert!(
        dispatch_analysis
            .target_count(crate::passes::reachability::MirDependencyEdgeKind::InterfaceDispatch)
            >= 2
    );

    let function_values = lower(
        "class State {
           static live: i64 = 3;
           static dead: i64 = 4;
           init() {}
         }
         fn live() -> i64 { return State.live; }
         fn dead() -> i64 { return State.dead; }
         fn invoke(callback: fn() -> i64) -> i64 { return callback(); }
         fn retain_dead() -> unit { var callback: fn() -> i64 = dead; }
         fn main() -> i64 { return invoke(live); }",
    );
    let function_value_analysis = analyze_static_activation(&function_values).unwrap();
    assert_active(&function_values, &function_value_analysis, "State.live");
    assert_inactive(&function_values, &function_value_analysis, "State.dead");
    assert_eq!(
        function_value_analysis
            .target_count(crate::passes::reachability::MirDependencyEdgeKind::IndirectCall),
        1
    );
}

#[test]
fn sibling_fields_and_generic_specializations_activate_independently() {
    let program = lower(
        "class Marker { init() {} }
         class Cache<T> {
           static seed: i64 = 1;
           static selected: i64 = Cache<T>.seed;
           static sibling: i64 = 2;
           init() {}
         }
         fn ignore(ref value: Cache<Marker>) -> i64 { return 0; }
         fn main() -> i64 {
           return Cache<i64>.selected + ignore(Cache<Marker>());
         }",
    );
    let analysis = analyze_static_activation(&program).unwrap();

    assert_active(&program, &analysis, "Cache<i64>.seed");
    assert_active(&program, &analysis, "Cache<i64>.selected");
    assert_inactive(&program, &analysis, "Cache<i64>.sibling");
    for name in [
        "Cache<Marker>.seed",
        "Cache<Marker>.selected",
        "Cache<Marker>.sibling",
    ] {
        if find_field(&program, name).is_some() {
            assert_inactive(&program, &analysis, name);
        }
    }
}

#[test]
fn provider_discovery_order_does_not_change_queries_witnesses_or_dump() {
    let sources = [
        (
            "app.ska",
            "from dep import read; fn main() -> i64 { return read(); }",
        ),
        (
            "dep.ska",
            "class State {
               static live: i64 = 1;
               static dormant: i64 = 2;
               init() {}
             }
             public fn read() -> i64 { return State.live; }",
        ),
    ];
    let first = lower_modules(&sources);
    let reversed = [sources[1], sources[0]];
    let second = lower_modules(&reversed);
    let first_analysis = analyze_static_activation(&first).unwrap();
    let second_analysis = analyze_static_activation(&second).unwrap();

    assert_eq!(first_analysis, second_analysis);
    assert_eq!(
        dump_static_activation(&first, &first_analysis),
        dump_static_activation(&second, &second_analysis)
    );
}

#[test]
fn imported_unused_decimal_table_is_inactive_in_activation_analysis() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/golden/control_flow/loop_lifecycle_matrix.ska"
    ));
    let (_workspace, graph) =
        load_module_sources_with_standard_library("app", &[("app.ska", source)]);
    let resolved = resolve_module_graph(&graph);
    assert!(
        resolved.diagnostics.is_empty(),
        "{:?}",
        resolved.diagnostics
    );
    let checked = type_check(&resolved.program);
    assert!(checked.diagnostics.is_empty(), "{:?}", checked.diagnostics);
    let program = lower_preliminary_hir(&checked.hir.unwrap());
    let analysis = analyze_static_activation(&program).unwrap();
    let decimal_table = program
        .static_fields()
        .map(|field| field.field)
        .find(|field| {
            program
                .static_field_qualified_name(*field)
                .is_some_and(|name| name.ends_with("_EiselPowers._words"))
        })
        .expect("the imported decimal parser declares its power table");

    assert!(!analysis.is_active(decimal_table));
    assert!(analysis.inactive_fields().contains(&decimal_table));
}

#[test]
fn activation_inspection_dump_is_deterministic_across_processes() {
    if std::env::var_os(DETERMINISM_CHILD).is_some() {
        let program = lower(determinism_source());
        let verified = verify_planned_mir(plan_static_lifetimes(program).unwrap()).unwrap();
        println!("{FINGERPRINT_BEGIN}");
        println!(
            "{}",
            StaticActivationInspection::new(&verified).activation_dump()
        );
        println!("{FINGERPRINT_END}");
        return;
    }

    assert_eq!(fingerprint_from_child(), fingerprint_from_child());
}

fn lower(source: &str) -> PreliminaryMirProgram {
    let program = lower_generic_source_to_preliminary_mir(source);
    verify_preliminary_mir(&program)
        .expect("activation fixture must produce verified preliminary MIR");
    program
}

fn lower_modules(sources: &[(&str, &str)]) -> PreliminaryMirProgram {
    let (_workspace, graph) = load_module_sources("app", sources);
    let resolved = resolve_module_graph(&graph);
    assert!(
        resolved.diagnostics.is_empty(),
        "{:?}",
        resolved.diagnostics
    );
    let checked = type_check(&resolved.program);
    assert!(checked.diagnostics.is_empty(), "{:?}", checked.diagnostics);
    let program = lower_preliminary_hir(&checked.hir.unwrap());
    verify_preliminary_mir(&program).expect("module fixture must produce verified preliminary MIR");
    program
}

fn find_field(program: &PreliminaryMirProgram, name: &str) -> Option<StaticFieldId> {
    program
        .static_fields()
        .map(|field| field.field)
        .find(|field| program.static_field_qualified_name(*field).as_deref() == Some(name))
}

fn field(program: &PreliminaryMirProgram, name: &str) -> StaticFieldId {
    find_field(program, name).unwrap_or_else(|| panic!("missing static field `{name}`"))
}

fn assert_active(
    program: &PreliminaryMirProgram,
    analysis: &super::StaticActivationAnalysis,
    name: &str,
) {
    assert!(
        analysis.is_active(field(program, name)),
        "`{name}` is inactive"
    );
}

fn assert_inactive(
    program: &PreliminaryMirProgram,
    analysis: &super::StaticActivationAnalysis,
    name: &str,
) {
    assert!(
        !analysis.is_active(field(program, name)),
        "`{name}` is active"
    );
}

fn determinism_source() -> &'static str {
    "class State {
       static base: i64 = 1;
       static selected: i64 = State.base + 1;
       static dormant: i64 = 3;
       init() {}
     }
     fn target() -> i64 { return State.selected; }
     fn invoke(callback: fn() -> i64) -> i64 { return callback(); }
     fn main() -> i64 { return invoke(target); }"
}

fn fingerprint_from_child() -> String {
    let output = Command::new(std::env::current_exe().expect("unit-test executable path"))
        .args([
            "--exact",
            "passes::static_lifecycle::activation::tests::activation_inspection_dump_is_deterministic_across_processes",
            "--nocapture",
        ])
        .env(DETERMINISM_CHILD, "1")
        .output()
        .expect("activation determinism child starts");
    assert!(
        output.status.success(),
        "activation determinism child failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("test output is UTF-8");
    let (_, fingerprint) = stdout
        .split_once(FINGERPRINT_BEGIN)
        .expect("child emitted fingerprint start marker");
    let (fingerprint, _) = fingerprint
        .split_once(FINGERPRINT_END)
        .expect("child emitted fingerprint end marker");
    fingerprint.trim().to_owned()
}
