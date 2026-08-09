use crate::{
    backend::{emit_assembly, Target},
    mir::{
        dump_mir, lower_preliminary_hir, MirArgument, MirInstruction, MirPlace,
        MirStaticActivationWork, MirStaticValueCleanup, MirTerminator,
    },
    test_support::type_check_source,
};

use super::{
    super::{plan_static_lifetimes, verify_synthesized_mir},
    synthesize_static_lifecycle,
};

const STORAGE_MATRIX: &str = concat!(
    "class Item { init() {} copy(ref other: Item) {} destroy {} }\n",
    "class State {\n",
    "  static zero: i64;\n",
    "  static number: i64 = 1;\n",
    "  static item: Item = (Item());\n",
    "  static maybe_item: Item? = none;\n",
    "  static owner: shared Item = new Item();\n",
    "  static maybe_owner: shared? Item = none;\n",
    "  static values: i64[] = i64[]{2, 3};\n",
    "  init() {}\n",
    "}\n",
    "fn main() -> i64 { return 0; }\n",
);

fn planned(source: &str) -> crate::mir::PlannedMirProgram {
    let checked = type_check_source(source);
    assert!(checked.diagnostics.is_empty(), "{:?}", checked.diagnostics);
    let preliminary = lower_preliminary_hir(&checked.hir.unwrap());
    plan_static_lifetimes(preliminary).expect("test source must have an acyclic lifecycle")
}

fn synthesized(source: &str) -> crate::mir::MirProgram {
    synthesize_static_lifecycle(planned(source)).expect("planned MIR must synthesize")
}

fn errors(program: &crate::mir::MirProgram) -> String {
    verify_synthesized_mir(program).unwrap_err().to_string()
}

#[test]
fn moves_initializer_bodies_unchanged_into_planned_activation_regions() {
    let planned = planned(STORAGE_MATRIX);
    let expected_bodies = planned.static_initializers().cloned().collect::<Vec<_>>();
    let expected_order = planned.lifecycle().activation().to_vec();
    let program = synthesize_static_lifecycle(planned).unwrap();
    let coordinator = program.static_lifecycle.as_ref().unwrap();

    let mut actual_bodies = coordinator.initializers().to_vec();
    actual_bodies.sort_by_key(|body| body.id);
    let mut expected_bodies = expected_bodies;
    expected_bodies.sort_by_key(|body| body.id);
    assert_eq!(actual_bodies, expected_bodies);
    assert_eq!(
        coordinator
            .activation()
            .iter()
            .map(|region| region.field)
            .collect::<Vec<_>>(),
        expected_order
    );
    assert!(matches!(
        coordinator.activation()[0].work,
        MirStaticActivationWork::ZeroDefault
    ));
    assert_eq!(coordinator.activation()[0].transitions.len(), 1);
    assert!(coordinator.activation()[1..]
        .iter()
        .all(|region| region.transitions.len() == 2));
    verify_synthesized_mir(&program).unwrap();
}

#[test]
fn synthesizes_exact_reverse_cleanup_category_matrix() {
    let program = synthesized(STORAGE_MATRIX);
    let coordinator = program.static_lifecycle.as_ref().unwrap();
    assert!(coordinator
        .shutdown()
        .iter()
        .map(|region| region.field)
        .eq(coordinator
            .activation()
            .iter()
            .rev()
            .map(|region| region.field)));

    let mut names = coordinator
        .shutdown()
        .iter()
        .map(|region| match region.cleanup {
            MirStaticValueCleanup::None => "none",
            MirStaticValueCleanup::CompleteObject(_) => "class",
            MirStaticValueCleanup::OptionalClass(_) => "optional-class",
            MirStaticValueCleanup::Shared(_) => "shared",
            MirStaticValueCleanup::OptionalShared(_) => "optional-shared",
            MirStaticValueCleanup::Array(_) => "array",
        })
        .collect::<Vec<_>>();
    names.sort_unstable();
    assert_eq!(
        names,
        [
            "array",
            "class",
            "none",
            "none",
            "optional-class",
            "optional-shared",
            "shared",
        ]
    );
}

#[test]
fn publication_precedes_preserved_full_expression_cleanup() {
    let program = synthesized(
        "class Item { init() {} copy(ref other: Item) {} destroy {} }
         class State { static item: Item = (Item()); init() {} }
         fn main() -> i64 { return 0; }",
    );
    let coordinator = program.static_lifecycle.as_ref().unwrap();
    let body = &coordinator.initializers()[0];
    let cleanup = body.block(body.publication.cleanup_entry).unwrap();
    assert!(cleanup.instructions.iter().any(
        |instruction| matches!(instruction, MirInstruction::EndFullExpression(end)
            if !end.temporaries.is_empty())
    ));
    assert!(matches!(
        coordinator.activation()[0].work,
        MirStaticActivationWork::Explicit(_)
    ));
}

#[test]
fn final_coordinator_dump_is_exactly_deterministic() {
    let expected = dump_mir(&synthesized(STORAGE_MATRIX));
    assert!(
        expected.contains("StaticLifecycleCoordinator"),
        "{expected}"
    );
    assert!(expected.contains("ActivateZeroDefault"), "{expected}");
    assert!(expected.contains("Cleanup optional-shared"), "{expected}");
    assert!(expected.contains("Cleanup array"), "{expected}");

    for _ in 0..8 {
        assert_eq!(dump_mir(&synthesized(STORAGE_MATRIX)), expected);
    }
}

#[test]
fn rejects_missing_reordered_and_wrong_cleanup_regions() {
    let valid = synthesized(STORAGE_MATRIX);

    let mut missing = valid.clone();
    missing
        .static_lifecycle
        .as_mut()
        .unwrap()
        .activation_mut_for_test()
        .pop();
    assert!(errors(&missing).contains("activation regions do not cover"));

    let mut duplicated_transition = valid.clone();
    let coordinator = duplicated_transition.static_lifecycle.as_mut().unwrap();
    let duplicate = coordinator.lifecycle().activation()[0];
    coordinator
        .lifecycle_mut_for_test()
        .activation_mut_for_test()
        .push(duplicate);
    assert!(errors(&duplicated_transition).contains("certified transitions"));

    let mut reordered = valid.clone();
    reordered
        .static_lifecycle
        .as_mut()
        .unwrap()
        .shutdown_mut_for_test()
        .swap(0, 1);
    assert!(errors(&reordered).contains("reverse order"));

    let mut wrong_cleanup = valid;
    wrong_cleanup
        .static_lifecycle
        .as_mut()
        .unwrap()
        .shutdown_mut_for_test()[0]
        .cleanup = MirStaticValueCleanup::None;
    assert!(errors(&wrong_cleanup).contains("cleanup"));

    let mut unstorable = synthesized(STORAGE_MATRIX);
    unstorable
        .static_lifecycle
        .as_mut()
        .unwrap()
        .lifecycle_mut_for_test()
        .definitions_mut_for_test()[0]
        .ty = crate::mir::MirType::Unit;
    let message = errors(&unstorable);
    assert!(
        message.contains("unstorable type") || message.contains("disagrees with its declaration"),
        "{message}"
    );
}

#[test]
fn rejects_publication_bypass_and_initializer_destination_escape() {
    let source = "class Item { init() {} copy(ref other: Item) {} destroy {} }
                  class State { static item: Item = (Item()); init() {} }
                  fn main() -> i64 { return 0; }";
    let valid = synthesized(source);

    let mut bypass = valid.clone();
    let body = &mut bypass
        .static_lifecycle
        .as_mut()
        .unwrap()
        .initializers_mut_for_test()[0];
    let exit = body.publication.initialization_exit.index();
    body.body.blocks[exit].terminator = Some(MirTerminator::Goto {
        target: body.body.entry,
        span: body.span,
    });
    let message = errors(&bypass);
    assert!(message.contains("bypass publication"), "{message}");

    let mut escaped = synthesized(
        "class Item { init() {} copy(ref other: Item) {} destroy {} }
         fn forward(ref item: Item) -> Item { return item; }
         class State {
           static item: Item = forward(Item());
           init() {}
         }
         fn main() -> i64 { return 0; }",
    );
    let body = escaped
        .static_lifecycle
        .as_mut()
        .unwrap()
        .initializers_mut_for_test()
        .iter_mut()
        .find(|body| {
            body.body
                .blocks
                .iter()
                .flat_map(|block| &block.instructions)
                .any(|instruction| {
                    matches!(instruction, MirInstruction::Call(call)
                        if !call.arguments.is_empty())
                })
        })
        .unwrap();
    let field = body.field;
    let argument = body
        .body
        .blocks
        .iter_mut()
        .flat_map(|block| &mut block.instructions)
        .find_map(|instruction| match instruction {
            MirInstruction::Call(call) if !call.arguments.is_empty() => {
                Some(&mut call.arguments[0])
            }
            _ => None,
        })
        .unwrap();
    *argument = MirArgument::Place(MirPlace::static_lifecycle_destination(field));
    let message = errors(&escaped);
    assert!(
        message.contains("direct effects")
            || message.contains("invalid lifecycle-owned destination access"),
        "{message}"
    );
}

#[test]
fn ordinary_ownership_verification_covers_moved_initializer_bodies() {
    let mut program = synthesized(
        "class Item { init() {} destroy {} }
         class State { static owner: shared Item = new Item(); init() {} }
         fn main() -> i64 { return 0; }",
    );
    let body = &mut program
        .static_lifecycle
        .as_mut()
        .unwrap()
        .initializers_mut_for_test()[0];
    let callable = body.callable();
    let initialize = body
        .body
        .blocks
        .iter_mut()
        .flat_map(|block| &mut block.instructions)
        .find_map(|instruction| match instruction {
            MirInstruction::SharedFieldInitialize(initialize) => Some(initialize),
            _ => None,
        })
        .unwrap();
    initialize.source = crate::mir::StorageId::new(callable, usize::MAX);

    let message = errors(&program);
    assert!(
        message.contains("not declared") || message.contains("shared"),
        "{message}"
    );
}

#[test]
fn backend_boundary_accepts_only_final_mir_and_rejects_unimplemented_startup_structurally() {
    let program = synthesized(
        "class State { static value: i64 = 1; init() {} }
         fn main() -> i64 { return 0; }",
    );
    let error = emit_assembly(Target::X86_64SysV, &program).unwrap_err();
    assert_eq!(
        error.message(),
        "verified static lifecycle startup lowering is not implemented"
    );
}
