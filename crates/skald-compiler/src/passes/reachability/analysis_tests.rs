//! Focused behavior tests for deterministic whole-world reachability.

use std::process::Command;

use crate::{
    identity::{CallableId, StaticFieldId},
    mir::{MirExecutionNode, MirFunctionLinkage, MirProgram},
    test_support::lower_generic_source_to_final_mir,
};

use super::{
    analyze_reachability, dump_reachability, MirDependencyEdgeKind, MirReachabilityAnalysis,
    MirReachabilityRootReason, MirRuntimeEntity,
};

const DETERMINISM_CHILD: &str = "SKALD_REACHABILITY_DETERMINISM_CHILD";
const FINGERPRINT_BEGIN: &str = "SKALD_REACHABILITY_FINGERPRINT_BEGIN";
const FINGERPRINT_END: &str = "SKALD_REACHABILITY_FINGERPRINT_END";

fn analyze(source: &str) -> (MirProgram, MirReachabilityAnalysis) {
    let program = lower_generic_source_to_final_mir(source);
    let analysis = analyze_reachability(&program).expect("valid final MIR has reachability facts");
    (program, analysis)
}

fn function(program: &MirProgram, name: &str) -> CallableId {
    program
        .declarations
        .iter()
        .find(|declaration| declaration.name == name)
        .unwrap_or_else(|| panic!("missing function {name}"))
        .id
        .into()
}

fn node(program: &MirProgram, name: &str) -> MirExecutionNode {
    MirExecutionNode::callable(function(program, name))
}

fn static_field(program: &MirProgram, class_name: &str, name: &str) -> StaticFieldId {
    program
        .classes
        .iter()
        .find(|class| class.name == class_name)
        .unwrap_or_else(|| panic!("missing class {class_name}"))
        .static_fields
        .iter()
        .find(|field| field.name == name)
        .unwrap_or_else(|| panic!("missing static field {class_name}.{name}"))
        .id
}

#[test]
fn entry_only_program_has_one_reachable_node_and_no_outgoing_dependencies() {
    let (program, analysis) = analyze(
        "fn dead() -> i64 { return 9; }
         fn main() -> i64 { return 0; }",
    );

    assert_eq!(analysis.reachable_nodes(), &[node(&program, "main")]);
    assert!(analysis.outgoing().is_empty());
    assert!(!analysis.is_reachable(node(&program, "dead")));
}

#[test]
fn closure_reaches_transitive_and_recursive_calls_but_not_unreferenced_definitions() {
    let (program, analysis) = analyze(
        "fn leaf() -> i64 { return 1; }
         fn middle() -> i64 { return leaf(); }
         fn dead() -> i64 { return 9; }
         fn self_cycle(value: i64) -> i64 {
           if (value == 0) { return 0; }
           return self_cycle(value - 1);
         }
         fn left(value: i64) -> i64 {
           if (value == 0) { return 0; }
           return right(value - 1);
         }
         fn right(value: i64) -> i64 {
           if (value == 0) { return 0; }
           return left(value - 1);
         }
         fn main() -> i64 { return middle() + self_cycle(1) + left(1); }",
    );

    for name in ["main", "middle", "leaf", "self_cycle", "left", "right"] {
        assert!(analysis.is_reachable(node(&program, name)), "{name}");
    }
    assert!(!analysis.is_reachable(node(&program, "dead")));
    assert!(analysis.has_retained_definition(function(&program, "dead")));
    assert_eq!(analysis.roots().len(), 1);
    assert_eq!(
        analysis.roots()[0].reason(),
        MirReachabilityRootReason::Entry
    );

    let explanation = analysis.explanation(node(&program, "leaf")).unwrap();
    assert_eq!(
        explanation.root().reason(),
        MirReachabilityRootReason::Entry
    );
    assert_eq!(explanation.dependencies().len(), 2);
}

#[test]
fn reached_callables_scan_all_structurally_retained_blocks() {
    let (program, analysis) = analyze(
        "fn hidden() -> i64 { return 7; }
         fn branch() -> i64 {
           if (false) { return hidden(); }
           return 0;
         }
         fn main() -> i64 { return branch(); }",
    );

    assert!(analysis.is_reachable(node(&program, "hidden")));
}

#[test]
fn final_facts_retain_only_static_accesses_from_reachable_execution() {
    let (program, analysis) = analyze(
        "class State {
           static live: i64 = 1;
           static dead: i64 = 2;
           init() {}
         }
         fn unreachable() -> i64 { return State.dead; }
         fn read_live() -> i64 { return State.live; }
         fn main() -> i64 { return read_live(); }",
    );
    let live = static_field(&program, "State", "live");
    let unreachable = node(&program, "unreachable");

    assert!(analysis
        .static_accesses()
        .iter()
        .any(|access| access.target() == live));
    assert!(analysis.static_accesses_from(unreachable).is_empty());
    assert_eq!(
        analysis.counts().static_accesses,
        analysis.static_accesses().len()
    );
    for access in analysis.static_accesses() {
        analysis
            .static_access_explanation(access)
            .expect("every retained access has selecting evidence");
    }
    let live_read = analysis
        .static_accesses_from(node(&program, "read_live"))
        .iter()
        .find(|access| access.target() == live)
        .unwrap();
    assert_eq!(
        analysis
            .static_access_explanation(live_read)
            .unwrap()
            .root()
            .reason(),
        MirReachabilityRootReason::Entry
    );
    let dump = dump_reachability(&analysis);
    assert!(dump.contains("static-accesses="));
    assert!(dump.contains(&format!("StaticAccess {live}")));
    assert!(!dump.contains(&format!(
        "Node callable {}",
        function(&program, "unreachable")
    )));
}

#[test]
fn function_value_candidates_are_coupled_to_reached_formations() {
    let (program, analysis) = analyze(
        "fn live() -> i64 { return 1; }
         fn dead() -> i64 { return 2; }
         fn invoke(callback: fn() -> i64) -> i64 { return callback(); }
         fn retain_dead() -> unit { var callback: fn() -> i64 = dead; }
         fn main() -> i64 { return invoke(live); }",
    );

    assert!(analysis.is_reachable(node(&program, "live")));
    assert!(!analysis.is_reachable(node(&program, "dead")));
    assert!(!analysis.is_reachable(node(&program, "retain_dead")));
    assert_eq!(analysis.function_value_candidates().len(), 1);
    assert_eq!(analysis.function_value_candidates()[0].targets().len(), 1);
    assert_eq!(
        analysis
            .candidates_for_function_type(analysis.function_value_candidates()[0].function_type()),
        analysis.function_value_candidates()[0].targets()
    );
    assert_eq!(
        analysis.function_value_candidates()[0].targets()[0].callable(),
        function(&program, "live")
    );
    assert!(analysis
        .outgoing_dependencies(node(&program, "invoke"))
        .iter()
        .any(|dependency| dependency.kind() == MirDependencyEdgeKind::IndirectCall));
}

#[test]
fn dispatch_queries_record_reached_virtual_families_and_interface_requirements() {
    let (_, analysis) = analyze(
        "interface View { fn read() -> i64; }
         class Base implements View {
           init() {}
           virtual fn read() -> i64 { return 1; }
         }
         class Child extends Base {
           init() { super(); }
           override fn read() -> i64 { return 2; }
         }
         fn read_virtual(ref value: Base) -> i64 { return value.read(); }
         fn read_interface(ref value: View) -> i64 { return value.read(); }
         fn main() -> i64 {
           return read_virtual(Child()) + read_interface(Child());
         }",
    );

    assert_eq!(analysis.used_virtual_families().len(), 1);
    assert_eq!(analysis.used_interface_requirements().len(), 1);
    let kinds = analysis
        .outgoing()
        .iter()
        .flat_map(|outgoing| outgoing.dependencies())
        .map(|dependency| dependency.kind())
        .collect::<Vec<_>>();
    assert!(kinds.contains(&MirDependencyEdgeKind::VirtualDispatch));
    assert!(kinds.contains(&MirDependencyEdgeKind::InterfaceDispatch));
}

#[test]
fn static_activation_and_reverse_shutdown_are_explicit_typed_roots() {
    let (_, analysis) = analyze(
        "class Item {
           init() {}
           destroy {}
         }
         class State {
           static zero: i64;
           static item: Item = Item();
           init() {}
         }
         fn main() -> i64 { return State.zero; }",
    );

    let activation_roots = analysis
        .roots()
        .iter()
        .filter(|root| {
            matches!(
                root.reason(),
                MirReachabilityRootReason::StaticActivation(_)
            )
        })
        .count();
    let shutdown_roots = analysis
        .roots()
        .iter()
        .filter(|root| matches!(root.reason(), MirReachabilityRootReason::StaticShutdown(_)))
        .count();
    assert_eq!(activation_roots, 2);
    assert_eq!(shutdown_roots, 2);
    assert!(
        analysis
            .runtime_entities()
            .iter()
            .filter(|entity| matches!(entity, MirRuntimeEntity::StaticStorage(_)))
            .count()
            >= 2
    );
}

#[test]
fn lifecycle_cycles_terminate_and_keep_the_complete_cycle() {
    let (_, analysis) = analyze(
        "class Loop {
           init() {}
           destroy { var nested: Loop = Loop(); }
         }
         class State {
           static value: Loop = Loop();
           init() {}
         }
         fn main() -> i64 { return 0; }",
    );

    assert!(analysis
        .outgoing()
        .iter()
        .flat_map(|outgoing| outgoing.dependencies())
        .any(|dependency| dependency.kind() == MirDependencyEdgeKind::UserDestructor));
    assert!(analysis
        .outgoing()
        .iter()
        .flat_map(|outgoing| outgoing.dependencies())
        .any(|dependency| dependency.kind() == MirDependencyEdgeKind::CompleteFinalizer));
}

#[test]
fn optional_box_finalizers_retain_payload_lifecycle_bodies() {
    let (program, analysis) = analyze(
        "class Tracked {
           init() {}
           destroy {}
         }
         fn build() -> unit {
           var owner: shared Tracked? = new Tracked?(Tracked());
           return;
         }
         fn main() -> i64 { build(); return 0; }",
    );
    let destructor = program
        .classes
        .iter()
        .find(|class| class.name == "Tracked")
        .and_then(|class| class.destruction.destructor.as_ref())
        .expect("Tracked has a user destructor")
        .id
        .into();

    assert!(analysis.is_reachable(MirExecutionNode::callable(destructor)));
    assert!(analysis
        .runtime_entities()
        .iter()
        .any(|entity| matches!(entity, MirRuntimeEntity::OptionalBoxLayout(_))));
    assert!(analysis
        .outgoing()
        .iter()
        .flat_map(|outgoing| outgoing.dependencies())
        .any(|dependency| dependency.kind() == MirDependencyEdgeKind::UserDestructor));
}

#[test]
fn roots_queries_witnesses_and_dump_are_deterministic_on_repeated_analysis() {
    let program = lower_generic_source_to_final_mir(determinism_source());
    let first = analyze_reachability(&program).unwrap();
    let second = analyze_reachability(&program).unwrap();

    assert_eq!(first, second);
    assert_eq!(dump_reachability(&first), dump_reachability(&second));
    assert_eq!(first.counts(), second.counts());
    assert!(dump_reachability(&first).contains("MirReachabilityAnalysis\n  Summary roots="));
    assert!(dump_reachability(&first).contains("FunctionValues"));
    assert!(dump_reachability(&first).contains("Via"));
}

#[test]
fn reachability_dump_is_deterministic_across_processes() {
    if std::env::var_os(DETERMINISM_CHILD).is_some() {
        let (_, analysis) = analyze(determinism_source());
        println!("{FINGERPRINT_BEGIN}");
        println!("{}", dump_reachability(&analysis));
        println!("{FINGERPRINT_END}");
        return;
    }

    assert_eq!(fingerprint_from_child(), fingerprint_from_child());
}

fn determinism_source() -> &'static str {
    "fn target() -> i64 { return 3; }
     fn invoke(callback: fn() -> i64) -> i64 { return callback(); }
     fn chain() -> i64 { return invoke(target); }
     fn dead() -> i64 { return 9; }
     fn main() -> i64 { return chain(); }"
}

fn fingerprint_from_child() -> String {
    let output = Command::new(std::env::current_exe().expect("unit-test executable path"))
        .args([
            "--exact",
            "passes::reachability::analysis_tests::reachability_dump_is_deterministic_across_processes",
            "--nocapture",
        ])
        .env(DETERMINISM_CHILD, "1")
        .output()
        .expect("reachability determinism child starts");
    assert!(
        output.status.success(),
        "reachability determinism child failed: {}",
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

#[test]
fn imported_external_declarations_are_leaf_dependencies_not_roots() {
    let (mut program, analysis) = analyze(
        "extern fn foreign() -> i64;
         fn main() -> i64 { return foreign(); }",
    );

    assert_eq!(analysis.roots().len(), 1);
    assert_eq!(
        analysis.reachable_callables(),
        &[program.entry_function.into()]
    );
    assert!(analysis
        .outgoing_dependencies(node(&program, "main"))
        .iter()
        .any(|dependency| matches!(dependency.target(), super::MirDependencyTarget::External(_))));

    let external = program
        .declarations
        .iter()
        .find(|declaration| matches!(declaration.linkage, MirFunctionLinkage::External { .. }))
        .unwrap()
        .id;
    program.entry_function = external;
    assert_eq!(
        analyze_reachability(&program),
        Err(super::MirDependencyExtractionError::NonInternalEntry(
            external
        ))
    );
}
