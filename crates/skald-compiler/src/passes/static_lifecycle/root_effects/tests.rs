//! Focused normalized lifecycle-root analysis tests.

use std::collections::BTreeSet;

use crate::{
    identity::{ClassId, StaticFieldId},
    mir::{
        lower_preliminary_hir, PreliminaryMirProgram, StaticAccessKind,
        StaticArrayLifecycleOperation, StaticClassLifecycleOperation, StaticEffectNode,
        StaticEffectPhase,
    },
    test_support::type_check_source,
};

use super::{
    super::{extract, plan_static_lifetimes},
    analyze,
    model::{StaticLifecycleEffectFact, StaticLifecycleRootEffectError},
};

fn lower(text: &str) -> PreliminaryMirProgram {
    let checked = type_check_source(text);
    assert!(checked.diagnostics.is_empty(), "{:?}", checked.diagnostics);
    lower_preliminary_hir(&checked.hir.unwrap())
}

fn analyze_program(program: &PreliminaryMirProgram) -> super::StaticLifecycleRootEffectAnalysis {
    let graph = extract::extract(program);
    analyze(program, &graph).expect("valid preliminary MIR must have valid lifecycle roots")
}

fn field(program: &PreliminaryMirProgram, index: usize) -> StaticFieldId {
    program.static_fields().nth(index).unwrap().field
}

fn fact(
    target: StaticFieldId,
    access: StaticAccessKind,
    phase: StaticEffectPhase,
    lifecycle_owned: bool,
) -> StaticLifecycleEffectFact {
    StaticLifecycleEffectFact {
        target,
        access,
        phase,
        lifecycle_owned,
    }
}

#[test]
fn normalizes_direct_transitive_and_post_publication_effects_by_root() {
    let program = lower(
        "fn select(ref item: Item) -> i64 { return State.base; }
         class State {
           static base: i64 = 1;
           static cleaned: i64 = 2;
           static result: i64 = select(Item());
           init() {}
         }
         class Item {
           init() {}
           destroy { var observed: i64 = State.cleaned; }
         }
         fn main() -> i64 { return 0; }",
    );
    let result = field(&program, 2);
    let initializer = program
        .static_initializers()
        .find(|initializer| initializer.field == result)
        .unwrap();
    let analysis = analyze_program(&program);
    let effects = &analysis
        .summary(StaticEffectNode::callable(initializer.callable()))
        .unwrap()
        .effects;

    assert!(effects.contains(&fact(
        field(&program, 0),
        StaticAccessKind::Read,
        StaticEffectPhase::InitializerBeforePublication,
        false,
    )));
    assert!(effects.contains(&fact(
        field(&program, 1),
        StaticAccessKind::Read,
        StaticEffectPhase::InitializerAfterPublication,
        false,
    )));
    assert!(effects.contains(&fact(
        result,
        StaticAccessKind::Initialize,
        StaticEffectPhase::InitializerBeforePublication,
        true,
    )));
}

#[test]
fn inventories_initializer_free_optional_shared_and_array_destruction_roots() {
    let program = lower(
        "class State {
           static observed: i64;
           static maybe_item: Item?;
           static maybe_owner: shared? Base;
           static items: Item[];
           init() {}
         }
         class Item {
           init() {}
           destroy { var value: i64 = State.observed; }
         }
         class Base {
           init() {}
           destroy { var value: i64 = State.observed; }
         }
         class Child extends Base {
           init() { super(); }
           destroy { var value: i64 = State.observed; }
         }
         fn main() -> i64 { return 0; }",
    );
    let analysis = analyze_program(&program);
    let roots = analysis
        .summaries()
        .map(|summary| summary.root)
        .collect::<BTreeSet<_>>();

    assert!(roots.contains(&StaticEffectNode::class(
        ClassId::new(1),
        StaticClassLifecycleOperation::CompleteFinalizer,
    )));
    assert!(roots.contains(&StaticEffectNode::class(
        ClassId::new(2),
        StaticClassLifecycleOperation::CompleteFinalizer,
    )));
    assert!(roots.contains(&StaticEffectNode::class(
        ClassId::new(3),
        StaticClassLifecycleOperation::CompleteFinalizer,
    )));
    assert!(roots.iter().any(|root| matches!(
        root,
        StaticEffectNode::ArrayLifecycle {
            operation: StaticArrayLifecycleOperation::Destruction,
            ..
        }
    )));

    let observed = field(&program, 0);
    for dependent in [field(&program, 1), field(&program, 2), field(&program, 3)] {
        assert!(analysis
            .dependency_pairs(&program)
            .contains(&(observed, dependent)));
    }
}

#[test]
fn closed_world_indirect_targets_contribute_normalized_initializer_effects() {
    let program = lower(
        "fn read_left() -> i64 { return State.left; }
         fn read_right() -> i64 { return State.right; }
         fn invoke(callback: fn() -> i64) -> i64 { return callback(); }
         fn retain() -> unit { var callback: fn() -> i64 = read_right; }
         class State {
           static left: i64 = 1;
           static right: i64 = 2;
           static result: i64 = invoke(read_left);
           init() {}
         }
         fn main() -> i64 { return State.result; }",
    );
    let initializer = program.static_initializers().nth(2).unwrap();
    let analysis = analyze_program(&program);
    let effects = &analysis
        .summary(StaticEffectNode::callable(initializer.callable()))
        .unwrap()
        .effects;

    for target in [field(&program, 0), field(&program, 1)] {
        assert!(effects.contains(&fact(
            target,
            StaticAccessKind::Read,
            StaticEffectPhase::InitializerBeforePublication,
            false,
        )));
    }
}

#[test]
fn closed_world_virtual_and_interface_targets_contribute_root_effects() {
    let program = lower(
        "interface View { fn read() -> i64; }
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
         class State {
           static base: i64 = 1;
           static child: i64 = 2;
           static virtual_result: i64 = read_virtual(Child());
           static interface_result: i64 = read_interface(Child());
           init() {}
         }
         fn main() -> i64 { return 0; }",
    );
    let analysis = analyze_program(&program);

    for initializer in program.static_initializers().skip(2) {
        let effects = &analysis
            .summary(StaticEffectNode::callable(initializer.callable()))
            .unwrap()
            .effects;
        for target in [field(&program, 0), field(&program, 1)] {
            assert!(effects.contains(&fact(
                target,
                StaticAccessKind::Read,
                StaticEffectPhase::InitializerBeforePublication,
                false,
            )));
        }
    }
}

#[test]
fn preserves_access_kinds_in_normalized_facts() {
    let program = lower(
        "fn inspect(ref item: Item) -> i64 { return item.value; }
         fn mutate(ref replacement: Item) -> i64 {
           State.number = 3;
           State.item = replacement;
           return inspect(State.item!);
         }
         class State {
           static number: i64 = 1;
           static item: Item? = Item(2);
           static result: i64 = mutate(Item(4));
           init() {}
         }
         class Item {
           value: i64;
           init(value: i64) { self.value = value; }
           copy(ref other: Item) { self.value = other.value; }
           assign(ref other: Item) { self.value = other.value; }
           destroy {}
         }
         fn main() -> i64 { return 0; }",
    );
    let analysis = analyze_program(&program);
    let effects = &analysis
        .summary(StaticEffectNode::callable(
            program.static_initializers().nth(2).unwrap().callable(),
        ))
        .unwrap()
        .effects;

    assert!(effects.contains(&fact(
        field(&program, 0),
        StaticAccessKind::Write,
        StaticEffectPhase::InitializerBeforePublication,
        false,
    )));
    assert!(effects.iter().any(|effect| {
        effect.target == field(&program, 1)
            && effect.access == StaticAccessKind::Replace
            && effect.phase == StaticEffectPhase::InitializerBeforePublication
    }));
    assert!(effects.iter().any(|effect| {
        effect.target == field(&program, 1)
            && effect.access == StaticAccessKind::Borrow
            && effect.phase == StaticEffectPhase::InitializerBeforePublication
    }));
}

#[test]
fn normalized_dependencies_match_the_existing_planner_oracle() {
    let program = lower(
        "fn read_base() -> i64 { return State.base; }
         class State {
           static result: i64 = read_base();
           static item: Item?;
           static base: i64 = 1;
           static cleanup: i64 = 2;
           init() {}
         }
         class Item {
           init() {}
           destroy { var observed: i64 = State.cleanup; }
         }
         fn main() -> i64 { return 0; }",
    );
    let expected = analyze_program(&program).dependency_pairs(&program);
    let planned = plan_static_lifetimes(program).unwrap();
    let actual = planned
        .dependencies()
        .iter()
        .map(|dependency| (dependency.prerequisite, dependency.dependent))
        .collect::<BTreeSet<_>>();

    assert_eq!(actual, expected);
}

#[test]
fn normalized_root_effects_are_deterministic() {
    let program = lower(
        "fn left(flag: bool) -> i64 {
           if (flag) { return State.value; }
           return right(true);
         }
         fn right(flag: bool) -> i64 {
           if (flag) { return left(true); }
           return State.value;
         }
         class State {
           static value: i64 = 1;
           static result: i64 = left(false);
           init() {}
         }
         fn main() -> i64 { return 0; }",
    );
    let first = analyze_program(&program);

    for _ in 0..16 {
        assert_eq!(analyze_program(&program), first);
    }
}

#[test]
fn rejects_missing_roots_foreign_edges_and_foreign_static_fields() {
    let program = lower(
        "fn read() -> i64 { return State.base; }
         class State {
           static base: i64 = 1;
           static result: i64 = read();
           init() {}
         }
         fn main() -> i64 { return 0; }",
    );
    let result_initializer = program.static_initializers().nth(1).unwrap();
    let root = StaticEffectNode::callable(result_initializer.callable());

    let mut missing_root = extract::extract(&program);
    missing_root.nodes.remove(&root);
    assert_eq!(
        analyze(&program, &missing_root),
        Err(StaticLifecycleRootEffectError::MissingRoot(root))
    );

    let mut foreign_source = extract::extract(&program);
    let edge = foreign_source
        .nodes
        .get_mut(&root)
        .unwrap()
        .edges
        .first_mut()
        .unwrap();
    edge.source = StaticEffectNode::class(
        ClassId::new(0),
        StaticClassLifecycleOperation::CopyConstructor,
    );
    assert!(matches!(
        analyze(&program, &foreign_source),
        Err(StaticLifecycleRootEffectError::ForeignEdgeSource { .. })
    ));

    let mut foreign_target = extract::extract(&program);
    let target = foreign_target.nodes[&root].edges[0].target;
    foreign_target.nodes.remove(&target);
    assert_eq!(
        analyze(&program, &foreign_target),
        Err(StaticLifecycleRootEffectError::ForeignEdgeTarget {
            source: root,
            target,
        })
    );

    let mut foreign_field = extract::extract(&program);
    let direct_node = foreign_field.nodes[&root].edges[0].target;
    foreign_field.nodes.get_mut(&direct_node).unwrap().direct[0].field =
        StaticFieldId::new(ClassId::new(99), 0);
    assert!(matches!(
        analyze(&program, &foreign_field),
        Err(StaticLifecycleRootEffectError::ForeignStaticField { .. })
    ));
}
