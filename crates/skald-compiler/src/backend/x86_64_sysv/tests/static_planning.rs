use super::*;

use crate::{
    backend::{
        x86_64_sysv::{
            emit_assembly_observed,
            planning::{PlanningObserver, StaticPlanningPhase},
            symbol,
        },
        RuntimeTracePolicy,
    },
    identity::StaticFieldId,
    passes::{run_mir_pipeline, VerifiedFinalMirProgram},
    test_support::{
        load_module_sources_with_standard_library, lower_hir_to_final_mir,
        lower_source_to_complete_final_mir_with_sources,
    },
};

#[derive(Default)]
struct StaticVisits(Vec<(StaticPlanningPhase, StaticFieldId)>);

impl PlanningObserver for StaticVisits {
    fn visits_static_field(&mut self, phase: StaticPlanningPhase, field: StaticFieldId) {
        self.0.push((phase, field));
    }
}

impl StaticVisits {
    fn fields(&self, phase: StaticPlanningPhase) -> Vec<StaticFieldId> {
        self.0
            .iter()
            .filter_map(|(candidate, field)| (*candidate == phase).then_some(*field))
            .collect()
    }
}

#[test]
fn backend_storage_planning_distinguishes_active_and_dead_body_fallback_slots() {
    let fixture = lower_source_to_complete_final_mir_with_sources(
        "static-slot-domain.ska",
        concat!(
            "class State {\n",
            "  static live: i64 = 41;\n",
            "  static dormant: i64 = 7;\n",
            "  init() {}\n",
            "}\n",
            "fn dead() -> i64 { return State.dormant; }\n",
            "fn main() -> i64 { return State.live + 1; }\n",
        ),
    );
    let fields = fixture
        .mir
        .classes
        .iter()
        .find(|class| class.name == "State")
        .unwrap()
        .static_fields
        .iter()
        .map(|field| field.id)
        .collect::<Vec<_>>();
    let [live, dormant] = fields.as_slice() else {
        panic!("fixture must declare exactly two statics")
    };
    assert_eq!(
        fixture
            .backend_input(RuntimeTracePolicy::Omitted)
            .active_static_fields(),
        [*live]
    );

    let mut complete_visits = StaticVisits::default();
    let complete = emit_assembly_observed(
        fixture.backend_input(RuntimeTracePolicy::Omitted),
        &mut complete_visits,
    )
    .unwrap();
    assert_eq!(
        complete_visits.fields(StaticPlanningPhase::Declared),
        [*live, *dormant]
    );
    assert_eq!(complete_visits.fields(StaticPlanningPhase::Active), [*live]);
    assert_eq!(
        complete_visits.fields(StaticPlanningPhase::ConservativeFallback),
        [*dormant]
    );
    assert_eq!(
        complete_visits.fields(StaticPlanningPhase::Initializer),
        [*live]
    );
    assert_eq!(
        complete_visits.fields(StaticPlanningPhase::Finalizer),
        [*live]
    );
    assert_eq!(
        complete_visits.fields(StaticPlanningPhase::Retained),
        [*live, *dormant]
    );
    assert_eq!(
        complete_visits.fields(StaticPlanningPhase::Emitted),
        [*live, *dormant]
    );
    assert!(complete.contains(&symbol::static_field(&fixture.mir, *live)));
    assert!(complete.contains(&symbol::static_field(&fixture.mir, *dormant)));

    let mut retained_visits = StaticVisits::default();
    let retained = emit_assembly_observed(
        fixture
            .backend_input(RuntimeTracePolicy::Omitted)
            .with_reachable_artifacts_only(),
        &mut retained_visits,
    )
    .unwrap();
    assert_eq!(
        retained_visits.fields(StaticPlanningPhase::ConservativeFallback),
        [*dormant]
    );
    assert_eq!(
        retained_visits.fields(StaticPlanningPhase::Retained),
        [*live]
    );
    assert_eq!(
        retained_visits.fields(StaticPlanningPhase::Emitted),
        [*live]
    );
    assert!(retained.contains(&symbol::static_field(&fixture.mir, *live)));
    assert!(!retained.contains(&symbol::static_field(&fixture.mir, *dormant)));
    let dead = fixture
        .mir
        .declarations
        .iter()
        .find(|declaration| declaration.name == "dead")
        .unwrap();
    assert!(!retained.contains(&symbol::callable(&fixture.mir, dead.id.into())));
    assert_system_assembler_accepts(&retained);
}

#[test]
fn empty_sparse_and_full_activation_emit_exact_static_slot_domains() {
    let cases = [
        ("fn main() -> i64 { return 0; }", 0),
        ("fn main() -> i64 { return State.left; }", 1),
        ("fn main() -> i64 { return State.left + State.right; }", 2),
    ];
    for (entry, expected_slots) in cases {
        let fixture = lower_source_to_complete_final_mir_with_sources(
            "activation-slot-domain.ska",
            format!(
                "class State {{ static left: i64; static right: i64; init() {{}} }}\n{entry}\n"
            ),
        );
        let mut visits = StaticVisits::default();
        let assembly = emit_assembly_observed(
            fixture.backend_input(RuntimeTracePolicy::Omitted),
            &mut visits,
        )
        .unwrap();

        assert_eq!(visits.fields(StaticPlanningPhase::Declared).len(), 2);
        assert_eq!(
            visits.fields(StaticPlanningPhase::Active).len(),
            expected_slots
        );
        assert!(visits
            .fields(StaticPlanningPhase::ConservativeFallback)
            .is_empty());
        assert_eq!(
            visits.fields(StaticPlanningPhase::Emitted).len(),
            expected_slots
        );
        assert_system_assembler_accepts(&assembly);
    }
}

#[test]
fn decimal_power_table_artifacts_are_pay_for_use() {
    let unused_source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/golden/control_flow/loop_lifecycle_matrix.ska"
    ));
    let unused = lower_standard_library_program("unused", unused_source);
    let unused_field = eisel_power_table(&unused);
    let unused_symbol = symbol::static_field(unused.program(), unused_field);
    let unused_initializer_symbol = format!("{unused_symbol}.initialize");
    let unused_assembly = crate::backend::emit_assembly(
        Target::X86_64SysV,
        BackendInput::without_runtime_trace(&unused).with_reachable_artifacts_only(),
    )
    .unwrap();

    assert!(!BackendInput::without_runtime_trace(&unused)
        .active_static_fields()
        .contains(&unused_field));
    assert!(!unused_assembly.contains(&unused_symbol));
    assert!(!unused_assembly.contains(&unused_initializer_symbol));

    let used = lower_standard_library_program(
        "used",
        concat!(
            "from std::str import Str;\n",
            "fn main() -> i64 {\n",
            "  var text: Str = \"1.5\";\n",
            "  var parsed: f64? = text.to_f64();\n",
            "  if (parsed is some) { return 42; }\n",
            "  return 1;\n",
            "}\n",
        ),
    );
    let used_field = eisel_power_table(&used);
    let used_symbol = symbol::static_field(used.program(), used_field);
    let used_initializer_symbol = format!("{used_symbol}.initialize");
    let used_assembly = crate::backend::emit_assembly(
        Target::X86_64SysV,
        BackendInput::without_runtime_trace(&used).with_reachable_artifacts_only(),
    )
    .unwrap();

    assert!(BackendInput::without_runtime_trace(&used)
        .active_static_fields()
        .contains(&used_field));
    assert!(used_assembly.contains(&used_symbol));
    let initializer = function_assembly(&used_assembly, &used_initializer_symbol);
    assert!(initializer.contains("call ska_rt_alloc"), "{initializer}");
    let finalizer = function_assembly(&used_assembly, ".Lska.static.finalize");
    assert!(finalizer.contains(&used_symbol), "{finalizer}");
    assert_system_assembler_accepts(&unused_assembly);
    assert_system_assembler_accepts(&used_assembly);
}

fn lower_standard_library_program(entry: &str, source: &str) -> VerifiedFinalMirProgram {
    let source_path = format!("{entry}.ska");
    let (_workspace, graph) =
        load_module_sources_with_standard_library(entry, &[(source_path.as_str(), source)]);
    let resolved = crate::resolve::resolve_module_graph(&graph);
    assert!(
        resolved.diagnostics.is_empty(),
        "{:?}",
        resolved.diagnostics
    );
    let checked = crate::typeck::type_check(&resolved.program);
    assert!(checked.diagnostics.is_empty(), "{:?}", checked.diagnostics);
    let final_mir = lower_hir_to_final_mir(&checked.hir.unwrap());
    run_mir_pipeline(final_mir).expect("standard-library fixture must produce verified final MIR")
}

fn eisel_power_table(program: &VerifiedFinalMirProgram) -> StaticFieldId {
    program
        .classes
        .iter()
        .flat_map(|class| &class.static_fields)
        .map(|field| field.id)
        .find(|field| {
            program
                .static_field_qualified_name(*field)
                .is_some_and(|name| name.ends_with("_EiselPowers._words"))
        })
        .expect("standard string imports must declare the decimal power table")
}
