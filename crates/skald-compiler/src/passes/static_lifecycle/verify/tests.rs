//! Mutation tests for the planned-MIR trust boundary.

use crate::{
    identity::{ClassId, StaticFieldId},
    mir::{
        lower_preliminary_hir, MirProgramLifecycle, MirStaticFieldInitialization,
        MirStaticLifecycleCertificate, MirStaticLifecycleDefinition, MirStaticLifecycleIndices,
        MirStaticLifecycleTransition, MirStaticLifecycleTransitionKind, MirType, PlannedMirProgram,
        StaticEffectEdgeKind, StaticLifecyclePlan,
    },
    test_support::type_check_source,
};

use super::{
    super::{infer_static_effects, plan_static_lifetimes},
    verify_planned_mir,
};

const DEPENDENCY_SOURCE: &str = "fn read_base() -> i64 { return State.base; }
     class State {
       static result: i64 = read_base();
       static base: i64 = 1;
       init() {}
     }
     fn main() -> i64 { return 0; }";

fn plan(source: &str) -> PlannedMirProgram {
    let checked = type_check_source(source);
    assert!(checked.diagnostics.is_empty(), "{:?}", checked.diagnostics);
    let preliminary = lower_preliminary_hir(&checked.hir.unwrap());
    plan_static_lifetimes(preliminary).expect("test program must have an acyclic static plan")
}

fn errors(program: &PlannedMirProgram) -> String {
    verify_planned_mir(program).unwrap_err().to_string()
}

#[test]
fn accepts_a_complete_hand_built_phase_product() {
    let checked = type_check_source(
        "class State { static value: i64 = 1; init() {} }
         fn main() -> i64 { return 0; }",
    );
    let mut preliminary = lower_preliminary_hir(&checked.hir.unwrap());
    let field = *preliminary.static_fields().next().unwrap();
    let initializer_id = field.initializer.unwrap();
    let initializer = preliminary.static_initializer(initializer_id).unwrap();
    let initializer_span = initializer.span;
    let publication_span = initializer.publication.span;
    let indices = MirStaticLifecycleIndices {
        activation: 0,
        shutdown: 0,
    };
    preliminary
        .program_mut()
        .static_field_mut(field.field)
        .unwrap()
        .lifecycle = Some(indices);
    let effects = infer_static_effects(&preliminary);
    let plan = StaticLifecyclePlan::new(vec![field.field]);
    let lifecycle = MirProgramLifecycle::new(
        vec![MirStaticLifecycleDefinition {
            field: field.field,
            ty: field.ty,
            initialization: MirStaticFieldInitialization::Explicit(initializer_id),
            indices,
            span: field.span,
        }],
        vec![
            MirStaticLifecycleTransition {
                field: field.field,
                kind: MirStaticLifecycleTransitionKind::BeginInitialization,
                span: initializer_span,
            },
            MirStaticLifecycleTransition {
                field: field.field,
                kind: MirStaticLifecycleTransitionKind::PublishLive,
                span: publication_span,
            },
        ],
        vec![
            MirStaticLifecycleTransition {
                field: field.field,
                kind: MirStaticLifecycleTransitionKind::BeginDestruction,
                span: field.span,
            },
            MirStaticLifecycleTransition {
                field: field.field,
                kind: MirStaticLifecycleTransitionKind::FinishDestruction,
                span: field.span,
            },
        ],
        plan,
        MirStaticLifecycleCertificate::new(effects, Vec::new()),
    );
    let planned = PlannedMirProgram::new(preliminary, lifecycle);

    verify_planned_mir(&planned).unwrap();
}

#[test]
fn rejects_missing_direct_effects_and_summary_closure() {
    let mut missing_direct = plan(DEPENDENCY_SOURCE);
    let summary = missing_direct
        .lifecycle_mut_for_test()
        .certificate_mut_for_test()
        .effects_mut_for_test()
        .summaries_mut_for_test()
        .iter_mut()
        .find(|summary| !summary.direct_effects.is_empty())
        .unwrap();
    summary.direct_effects.pop();
    assert!(errors(&missing_direct).contains("direct effects"));

    let mut missing_closure = plan(DEPENDENCY_SOURCE);
    let summary = missing_closure
        .lifecycle_mut_for_test()
        .certificate_mut_for_test()
        .effects_mut_for_test()
        .summaries_mut_for_test()
        .iter_mut()
        .find(|summary| {
            summary
                .effects
                .iter()
                .any(|effect| !effect.witness.is_empty())
        })
        .unwrap();
    let index = summary
        .effects
        .iter()
        .position(|effect| !effect.witness.is_empty())
        .unwrap();
    summary.effects.remove(index);
    assert!(errors(&missing_closure).contains("not closed over target"));
}

#[test]
fn rejects_missing_call_targets_and_dynamic_targets() {
    let mut missing_call = plan(DEPENDENCY_SOURCE);
    let summary = missing_call
        .lifecycle_mut_for_test()
        .certificate_mut_for_test()
        .effects_mut_for_test()
        .summaries_mut_for_test()
        .iter_mut()
        .find(|summary| !summary.possible_targets.is_empty())
        .unwrap();
    summary.possible_targets.pop();
    assert!(errors(&missing_call).contains("possible call targets"));

    let mut missing_dynamic = plan(
        "class State { static base: i64 = 1; static child: i64 = 2; init() {} }
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
         fn main() -> i64 { return 0; }",
    );
    let summary = missing_dynamic
        .lifecycle_mut_for_test()
        .certificate_mut_for_test()
        .effects_mut_for_test()
        .summaries_mut_for_test()
        .iter_mut()
        .find(|summary| {
            summary
                .possible_targets
                .iter()
                .any(|edge| edge.kind == StaticEffectEdgeKind::VirtualDispatch)
        })
        .unwrap();
    let index = summary
        .possible_targets
        .iter()
        .position(|edge| edge.kind == StaticEffectEdgeKind::VirtualDispatch)
        .unwrap();
    summary.possible_targets.remove(index);
    assert!(errors(&missing_dynamic).contains("possible call targets"));
}

#[test]
fn rejects_missing_lifetime_edges_and_order_violations() {
    let mut missing_edge = plan(DEPENDENCY_SOURCE);
    missing_edge
        .lifecycle_mut_for_test()
        .certificate_mut_for_test()
        .dependencies_mut_for_test()
        .clear();
    assert!(errors(&missing_edge).contains("omits initialization edge"));

    let mut wrong_order = plan(DEPENDENCY_SOURCE);
    wrong_order
        .lifecycle_mut_for_test()
        .plan_mut_for_test()
        .activation_mut_for_test()
        .reverse();
    assert!(errors(&wrong_order).contains("violates activation order"));

    let mut wrong_shutdown = plan(DEPENDENCY_SOURCE);
    wrong_shutdown
        .lifecycle_mut_for_test()
        .plan_mut_for_test()
        .shutdown_mut_for_test()
        .reverse();
    assert!(errors(&wrong_shutdown).contains("exact reverse"));
}

#[test]
fn rejects_missing_fields_mistyped_definitions_and_foreign_identities() {
    let mut missing = plan(DEPENDENCY_SOURCE);
    missing
        .lifecycle_mut_for_test()
        .definitions_mut_for_test()
        .pop();
    assert!(errors(&missing).contains("does not cover every static field"));

    let mut mistyped = plan(DEPENDENCY_SOURCE);
    mistyped.lifecycle_mut_for_test().definitions_mut_for_test()[0].ty = MirType::Bool;
    assert!(errors(&mistyped).contains("disagrees with its declaration"));

    let mut foreign = plan(DEPENDENCY_SOURCE);
    foreign.lifecycle_mut_for_test().definitions_mut_for_test()[0].field =
        StaticFieldId::new(ClassId::new(99), 0);
    assert!(errors(&foreign).contains("foreign static field"));
}

#[test]
fn rejects_phase_partition_and_declaration_index_mutations() {
    let mut phases = plan(DEPENDENCY_SOURCE);
    phases.lifecycle_mut_for_test().activation_mut_for_test()[0].kind =
        MirStaticLifecycleTransitionKind::PublishLive;
    assert!(errors(&phases).contains("activation phase partition"));

    let mut shutdown_phases = plan(DEPENDENCY_SOURCE);
    shutdown_phases
        .lifecycle_mut_for_test()
        .shutdown_mut_for_test()[0]
        .kind = MirStaticLifecycleTransitionKind::FinishDestruction;
    assert!(errors(&shutdown_phases).contains("shutdown phase partition"));

    let mut indices = plan(DEPENDENCY_SOURCE);
    let field = indices.static_fields().next().unwrap().field;
    indices
        .preliminary_mut_for_test()
        .program_mut()
        .static_field_mut(field)
        .unwrap()
        .lifecycle
        .as_mut()
        .unwrap()
        .activation += 1;
    assert!(errors(&indices).contains("inconsistent lifecycle indices"));
}

#[test]
fn verification_and_lifecycle_dump_are_deterministic() {
    let planned = plan(DEPENDENCY_SOURCE);
    let expected = super::super::dump_planned_mir(&planned);

    for _ in 0..8 {
        let repeated = plan(DEPENDENCY_SOURCE);
        verify_planned_mir(&repeated).unwrap();
        assert_eq!(super::super::dump_planned_mir(&repeated), expected);
    }
}

#[test]
fn lifecycle_dump_has_an_exact_stable_schema() {
    let planned = plan(
        "class State { static value: i64 = 1; init() {} }
         fn main() -> i64 { return 0; }",
    );
    let definition = planned.lifecycle_mir().definitions()[0];
    let field = definition.field;
    let initializer = match definition.initialization {
        crate::mir::MirStaticFieldInitialization::Explicit(initializer) => initializer,
        crate::mir::MirStaticFieldInitialization::ZeroDefault => unreachable!(),
    };
    let span = definition.span.range();
    let transition_span = planned.lifecycle_mir().activation()[0].span.range();
    let field_reference = format!("{field} \"State.value\"");
    let expected = format!(
        concat!(
            "StaticLifetimePlan\n",
            "  Activation {field_reference}\n",
            "  Shutdown {field_reference}\n",
            "ProgramLifecycle\n",
            "  Field {field_reference} i64 explicit {initializer} activation=0 shutdown=0 @{start}..{end}\n",
            "  ActivationTransitions\n",
            "    {field_reference} BeginInitialization @{transition_start}..{transition_end}\n",
            "    {field_reference} PublishLive @{transition_start}..{transition_end}\n",
            "  ShutdownTransitions\n",
            "    {field_reference} BeginDestruction @{start}..{end}\n",
            "    {field_reference} FinishDestruction @{start}..{end}\n",
        ),
        field_reference = field_reference,
        initializer = initializer,
        start = span.start(),
        end = span.end(),
        transition_start = transition_span.start(),
        transition_end = transition_span.end(),
    );

    assert_eq!(super::super::dump_static_lifetime_plan(&planned), expected);
}
