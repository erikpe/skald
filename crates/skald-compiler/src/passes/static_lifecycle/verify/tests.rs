//! Mutation tests for the planned-MIR trust boundary.

use crate::{
    identity::{ClassId, StaticFieldId},
    mir::{
        lower_preliminary_hir, MirProgramLifecycle, MirStaticFieldInitialization,
        MirStaticLifecycleDefinition, MirStaticLifecycleIndices, MirStaticLifecycleProof,
        MirStaticLifecycleTransition, MirStaticLifecycleTransitionKind, MirType, StaticAccessKind,
        StaticClassLifecycleOperation, StaticEffectNode, StaticEffectPhase, StaticLifecyclePlan,
    },
    test_support::type_check_source,
};

use super::{
    super::{
        analysis::infer_static_effects_with_roots,
        plan::{PlannedMirProgram, StaticLifecyclePlanningReport},
        plan_static_lifetimes,
    },
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
    let (effects, authority) = infer_static_effects_with_roots(&preliminary);
    let plan = StaticLifecyclePlan::new(vec![field.field]);
    let lifecycle = MirProgramLifecycle::new(
        vec![MirStaticLifecycleDefinition {
            field: field.field,
            ty: field.ty,
            initialization: MirStaticFieldInitialization::Explicit(initializer_id),
            final_span: field.final_span,
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
        MirStaticLifecycleProof::new(authority),
    );
    let report = StaticLifecyclePlanningReport::new(effects, Vec::new());
    let planned = PlannedMirProgram::new(preliminary, lifecycle, report);

    verify_planned_mir(&planned).unwrap();
}

#[test]
fn rejects_missing_extra_and_duplicate_authority_entries() {
    let mut missing_root = plan(DEPENDENCY_SOURCE);
    missing_root
        .lifecycle_mut_for_test()
        .proof_mut_for_test()
        .authority_mut_for_test()
        .roots_mut_for_test()
        .pop();
    assert!(errors(&missing_root).contains("omits lifecycle root"));

    let mut duplicate_root = plan(DEPENDENCY_SOURCE);
    let roots = duplicate_root
        .lifecycle_mut_for_test()
        .proof_mut_for_test()
        .authority_mut_for_test()
        .roots_mut_for_test();
    roots.insert(1, roots[0].clone());
    assert!(errors(&duplicate_root).contains("duplicate lifecycle root"));

    let mut missing_fact = plan(DEPENDENCY_SOURCE);
    missing_fact
        .lifecycle_mut_for_test()
        .proof_mut_for_test()
        .authority_mut_for_test()
        .roots_mut_for_test()
        .iter_mut()
        .find(|root| !root.effects().is_empty())
        .unwrap()
        .effects_mut_for_test()
        .pop();
    assert!(errors(&missing_fact).contains("omits preliminary-MIR fact"));

    let mut duplicate_fact = plan(DEPENDENCY_SOURCE);
    let root = duplicate_fact
        .lifecycle_mut_for_test()
        .proof_mut_for_test()
        .authority_mut_for_test()
        .roots_mut_for_test()
        .iter_mut()
        .find(|root| !root.effects().is_empty())
        .unwrap();
    let duplicate = root.effects()[0];
    root.effects_mut_for_test().insert(1, duplicate);
    assert!(errors(&duplicate_fact).contains("duplicate fact"));

    let mut extra_fact = plan(DEPENDENCY_SOURCE);
    let root = extra_fact
        .lifecycle_mut_for_test()
        .proof_mut_for_test()
        .authority_mut_for_test()
        .roots_mut_for_test()
        .iter_mut()
        .find(|root| !root.effects().is_empty())
        .unwrap();
    let mut extra = root.effects()[0];
    extra.set_lifecycle_owned_for_test(!extra.is_lifecycle_owned());
    root.effects_mut_for_test().push(extra);
    root.effects_mut_for_test().sort_unstable();
    assert!(errors(&extra_fact).contains("contains extra fact"));

    let mut extra_root = plan(DEPENDENCY_SOURCE);
    let extra_node = extra_root
        .planning_report()
        .analysis()
        .summaries()
        .map(|summary| summary.node)
        .find(|node| extra_root.authority().root(*node).is_none())
        .expect("fixture must contain a non-lifecycle effect node");
    let roots = extra_root
        .lifecycle_mut_for_test()
        .proof_mut_for_test()
        .authority_mut_for_test()
        .roots_mut_for_test();
    let mut extra = roots[0].clone();
    extra.set_root_for_test(extra_node);
    roots.push(extra);
    roots.sort_by_key(|root| root.root());
    assert!(errors(&extra_root).contains("extra lifecycle root"));
}

#[test]
fn rejects_foreign_authority_identities() {
    let mut foreign_root = plan(DEPENDENCY_SOURCE);
    foreign_root
        .lifecycle_mut_for_test()
        .proof_mut_for_test()
        .authority_mut_for_test()
        .roots_mut_for_test()[0]
        .set_root_for_test(StaticEffectNode::class(
            ClassId::new(99),
            StaticClassLifecycleOperation::CompleteFinalizer,
        ));
    assert!(errors(&foreign_root).contains("foreign lifecycle root"));

    let mut foreign_field = plan(DEPENDENCY_SOURCE);
    foreign_field
        .lifecycle_mut_for_test()
        .proof_mut_for_test()
        .authority_mut_for_test()
        .roots_mut_for_test()
        .iter_mut()
        .find(|root| !root.effects().is_empty())
        .unwrap()
        .effects_mut_for_test()[0]
        .set_target_for_test(StaticFieldId::new(ClassId::new(99), 0));
    assert!(errors(&foreign_field).contains("foreign static field"));
}

#[test]
fn rejects_changed_authority_access_phase_and_lifecycle_ownership() {
    let mut wrong_access = plan(DEPENDENCY_SOURCE);
    let fact = wrong_access
        .lifecycle_mut_for_test()
        .proof_mut_for_test()
        .authority_mut_for_test()
        .roots_mut_for_test()
        .iter_mut()
        .find_map(|root| root.effects_mut_for_test().first_mut())
        .unwrap();
    fact.set_access_for_test(if fact.access() == StaticAccessKind::Read {
        StaticAccessKind::Write
    } else {
        StaticAccessKind::Read
    });
    assert!(errors(&wrong_access).contains("contains extra fact"));

    let mut wrong_phase = plan(DEPENDENCY_SOURCE);
    let fact = wrong_phase
        .lifecycle_mut_for_test()
        .proof_mut_for_test()
        .authority_mut_for_test()
        .roots_mut_for_test()
        .iter_mut()
        .find_map(|root| root.effects_mut_for_test().first_mut())
        .unwrap();
    fact.set_phase_for_test(if fact.phase() == StaticEffectPhase::Ordinary {
        StaticEffectPhase::Copy
    } else {
        StaticEffectPhase::Ordinary
    });
    assert!(errors(&wrong_phase).contains("contains extra fact"));

    let mut wrong_ownership = plan(DEPENDENCY_SOURCE);
    let fact = wrong_ownership
        .lifecycle_mut_for_test()
        .proof_mut_for_test()
        .authority_mut_for_test()
        .roots_mut_for_test()
        .iter_mut()
        .find_map(|root| root.effects_mut_for_test().first_mut())
        .unwrap();
    fact.set_lifecycle_owned_for_test(!fact.is_lifecycle_owned());
    assert!(errors(&wrong_ownership).contains("contains extra fact"));
}

#[test]
fn rejects_authority_derived_order_violations() {
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

#[test]
fn final_static_publication_metadata_is_verified_at_the_plan_boundary() {
    let source =
        "class State { final static value: i64 = 1; init() {} } fn main() -> i64 { return State.value; }";
    let planned = plan(source);
    verify_planned_mir(&planned).unwrap();
    let definition = planned.lifecycle_mir().definitions()[0];
    assert!(definition.final_span.is_some());
    assert!(matches!(
        definition.initialization,
        MirStaticFieldInitialization::Explicit(_)
    ));
    let dump = super::super::dump_static_lifetime_plan(&planned);
    assert!(dump.contains(" final i64 explicit"), "{dump}");

    let mut missing_marker = planned.clone();
    missing_marker
        .lifecycle_mut_for_test()
        .definitions_mut_for_test()[0]
        .final_span = None;
    let missing_marker_errors = errors(&missing_marker);
    assert!(
        missing_marker_errors.contains("disagrees with its declaration"),
        "{missing_marker_errors}"
    );

    let mut zero_default = planned;
    zero_default
        .lifecycle_mut_for_test()
        .definitions_mut_for_test()[0]
        .initialization = MirStaticFieldInitialization::ZeroDefault;
    let zero_default_errors = errors(&zero_default);
    assert!(
        zero_default_errors.contains("disagrees with its declaration"),
        "{zero_default_errors}"
    );
}
