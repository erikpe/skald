use super::*;

use crate::{
    backend::x86_64_sysv::{
        emit_assembly_observed,
        planning::{DefinitionPlanningPhase, PlanningObserver},
    },
    identity::CallableId,
    mir::retain::{prepare_reachable_definition_retention, MirDefinitionRetention},
    test_support::lower_source_to_final_mir_with_sources,
};

#[derive(Default)]
struct DefinitionVisits(Vec<(DefinitionPlanningPhase, CallableId)>);

impl PlanningObserver for DefinitionVisits {
    fn visits_definition(&mut self, phase: DefinitionPlanningPhase, callable: CallableId) {
        self.0.push((phase, callable));
    }
}

impl DefinitionVisits {
    fn callables(&self, phase: DefinitionPlanningPhase) -> Vec<CallableId> {
        self.0
            .iter()
            .filter_map(|(candidate, callable)| (*candidate == phase).then_some(*callable))
            .collect()
    }
}

fn retain_reachable_definitions(
    verified: &crate::passes::VerifiedFinalMirProgram,
) -> crate::passes::VerifiedFinalMirProgram {
    let retention =
        prepare_reachable_definition_retention(verified.program(), verified.reachability())
            .unwrap();
    let MirDefinitionRetention::Changed(prepared) = retention else {
        panic!("fixture must contain definitions outside the reachable closure")
    };
    let change = prepared.apply(verified.program().clone());
    crate::passes::verify_final_mir(change.program).unwrap()
}

fn callable(program: &MirProgram, name: &str) -> CallableId {
    program
        .declarations
        .iter()
        .find(|declaration| declaration.name == name)
        .unwrap_or_else(|| panic!("missing function `{name}`"))
        .id
        .into()
}

#[test]
fn sparse_backend_planning_visits_only_physically_retained_definitions() {
    let fixture = lower_source_to_final_mir_with_sources(
        "retained-domain.ska",
        concat!(
            "interface Reader { fn read() -> i64; }\n",
            "class Dormant implements Reader {\n",
            "  init() {}\n",
            "  virtual fn read() -> i64 { return 7; }\n",
            "}\n",
            "fn dead() -> i64 { return 9; }\n",
            "fn main() -> i64 { var values: i64[] = i64[](); return 0; }\n",
        ),
    );
    let full = &fixture.mir;
    let main = callable(full.program(), "main");
    let dead = callable(full.program(), "dead");

    let mut full_visits = DefinitionVisits::default();
    let full_assembly = emit_assembly_observed(
        BackendInput::with_runtime_trace(full, &fixture.sources),
        &mut full_visits,
    )
    .unwrap();
    assert!(full_visits
        .callables(DefinitionPlanningPhase::Legality)
        .contains(&dead));
    assert!(full_assembly.contains(".fn.main.dead.f"));

    let sparse = retain_reachable_definitions(full);
    assert_eq!(
        sparse
            .program()
            .executable_definitions()
            .map(|definition| definition.callable())
            .collect::<Vec<_>>(),
        [main]
    );

    let mut sparse_visits = DefinitionVisits::default();
    let sparse_assembly = emit_assembly_observed(
        BackendInput::with_runtime_trace(&sparse, &fixture.sources),
        &mut sparse_visits,
    )
    .unwrap();
    for phase in [
        DefinitionPlanningPhase::ArrayLegality,
        DefinitionPlanningPhase::Legality,
        DefinitionPlanningPhase::RuntimeTraceActivation,
        DefinitionPlanningPhase::Frame,
        DefinitionPlanningPhase::InstructionSelection,
    ] {
        assert_eq!(sparse_visits.callables(phase), [main], "phase {phase:?}");
    }
    assert!(!sparse_assembly.contains(".fn.main.dead.f"));
    assert!(!sparse_assembly.contains("method.read"));
    assert!(sparse_assembly.contains(".quad 0"));
    assert_system_assembler_accepts(&sparse_assembly);
}

#[test]
fn sparse_complete_and_artifact_retained_emission_never_resurrect_absent_bodies() {
    let fixture = lower_source_to_final_mir_with_sources(
        "sparse-emission.ska",
        "fn dead() -> i64 { return 9; }\nfn main() -> i64 { return 0; }\n",
    );
    let sparse = retain_reachable_definitions(&fixture.mir);

    let complete = crate::backend::emit_assembly(
        Target::X86_64SysV,
        BackendInput::without_runtime_trace(&sparse),
    )
    .unwrap();
    let retained = crate::backend::emit_assembly(
        Target::X86_64SysV,
        BackendInput::without_runtime_trace(&sparse).with_reachable_artifacts_only(),
    )
    .unwrap();
    let repeated = crate::backend::emit_assembly(
        Target::X86_64SysV,
        BackendInput::without_runtime_trace(&sparse).with_reachable_artifacts_only(),
    )
    .unwrap();

    assert!(!complete.contains(".fn.main.dead.f"));
    assert!(!retained.contains(".fn.main.dead.f"));
    assert!(complete.contains(".globl main"));
    assert!(retained.contains(".globl main"));
    assert!(retained.contains("call ska_rt_abi_v9"));
    assert_eq!(retained, repeated);
    assert_system_assembler_accepts(&complete);
    assert_system_assembler_accepts(&retained);
}

#[test]
fn sparse_function_value_targets_survive_target_artifact_retention() {
    let fixture = lower_source_to_final_mir_with_sources(
        "sparse-function-values.ska",
        concat!(
            "fn live() -> i64 { return 7; }\n",
            "fn dead() -> i64 { return 9; }\n",
            "fn invoke(callback: fn() -> i64) -> i64 { return callback(); }\n",
            "fn main() -> i64 { return invoke(live); }\n",
        ),
    );
    let sparse = retain_reachable_definitions(&fixture.mir);
    let assembly = crate::backend::emit_assembly(
        Target::X86_64SysV,
        BackendInput::without_runtime_trace(&sparse).with_reachable_artifacts_only(),
    )
    .unwrap();

    assert!(assembly.contains(".fn.main.live.f"));
    assert!(!assembly.contains(".fn.main.dead.f"));
    assert_eq!(run_native_assembly(&assembly).code(), Some(7));
}

#[test]
fn sparse_static_lifecycle_and_array_helpers_execute_natively() {
    let fixture = lower_source_to_final_mir_with_sources(
        "sparse-lifecycle.ska",
        concat!(
            "class Item {\n",
            "  value: i64;\n",
            "  init(value: i64) { self.value = value; }\n",
            "  destroy {}\n",
            "}\n",
            "class State { static item: Item = Item(7); init() {} }\n",
            "fn dead() -> i64 { return 9; }\n",
            "fn main() -> i64 {\n",
            "  var values: i64[] = i64[]{3, 4};\n",
            "  return State.item.value + values[1];\n",
            "}\n",
        ),
    );
    let sparse = retain_reachable_definitions(&fixture.mir);
    let assembly = crate::backend::emit_assembly(
        Target::X86_64SysV,
        BackendInput::without_runtime_trace(&sparse).with_reachable_artifacts_only(),
    )
    .unwrap();

    assert!(!assembly.contains(".fn.main.dead.f"));
    assert!(assembly.contains(".Lska.static.initialize"));
    assert!(assembly.contains(".Lska.static.finalize"));
    let linked = format!(
        "{assembly}{}{}",
        native_allocator(),
        native_panic_reporter()
    );
    assert_eq!(run_native_assembly(&linked).code(), Some(11));
}
