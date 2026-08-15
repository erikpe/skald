//! Focused whole-program static-effect analysis tests.

use crate::{
    identity::{ClassId, FunctionId},
    mir::{lower_preliminary_hir, PreliminaryMirProgram},
    resolve::resolve_module_graph,
    test_support::{load_module_sources_with_standard_library, type_check_source},
    typeck::type_check,
};

use super::*;

fn lower(text: &str) -> PreliminaryMirProgram {
    let checked = type_check_source(text);
    assert!(checked.diagnostics.is_empty(), "{:?}", checked.diagnostics);
    lower_preliminary_hir(&checked.hir.unwrap())
}

fn effect_fields(
    analysis: &StaticEffectAnalysis,
    node: StaticEffectNode,
) -> Vec<crate::identity::StaticFieldId> {
    analysis
        .summary(node)
        .unwrap_or_else(|| panic!("missing summary for {node:?}"))
        .effects
        .iter()
        .map(|effect| effect.field)
        .collect()
}

#[test]
fn propagates_direct_deep_and_recursive_effects_with_minimum_witnesses() {
    let preliminary = lower(
        "fn leaf(flag: bool) -> i64 {
           if (flag) { return State.target; }
           return recurse(true);
         }
         fn recurse(flag: bool) -> i64 {
           if (flag) { return leaf(true); }
           return State.target;
         }
         fn outer() -> i64 { return leaf(false); }
         class State {
           static target: i64 = 1;
           static result: i64 = outer();
           init() {}
         }
         fn main() -> i64 { return 0; }",
    );
    let fields = preliminary
        .static_fields()
        .map(|field| field.field)
        .collect::<Vec<_>>();
    let analysis = infer_static_effects(&preliminary);

    let outer = StaticEffectNode::Callable(FunctionId::new(2).into());
    let outer_effect = analysis
        .summary(outer)
        .unwrap()
        .effects
        .iter()
        .find(|effect| effect.field == fields[0])
        .unwrap();
    assert_eq!(outer_effect.witness.len(), 1);
    assert_eq!(
        outer_effect.witness[0].kind,
        StaticEffectEdgeKind::DirectCall
    );

    let result_initializer = StaticEffectNode::Callable(
        preliminary
            .static_initializers()
            .find(|initializer| initializer.field == fields[1])
            .unwrap()
            .callable(),
    );
    let result_effect = analysis
        .summary(result_initializer)
        .unwrap()
        .effects
        .iter()
        .find(|effect| effect.field == fields[0])
        .unwrap();
    assert_eq!(result_effect.witness.len(), 2);
    assert_eq!(
        result_effect.phase,
        StaticEffectPhase::InitializerBeforePublication
    );
    assert!(analysis.recursive_components() >= 1);
}

#[test]
fn scans_unreachable_branches_conservatively() {
    let preliminary = lower(
        "fn maybe(flag: bool) -> i64 {
           if (flag) { return State.hidden; }
           return 0;
         }
         class State {
           static hidden: i64 = 1;
           static result: i64 = maybe(false);
           init() {}
         }
         fn main() -> i64 { return 0; }",
    );
    let hidden = preliminary.static_fields().next().unwrap().field;
    let analysis = infer_static_effects(&preliminary);

    assert!(effect_fields(
        &analysis,
        StaticEffectNode::Callable(FunctionId::new(0).into())
    )
    .contains(&hidden));
}

#[test]
fn expands_virtual_and_interface_dispatch_to_all_linked_implementations() {
    let preliminary = lower(
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
         fn main() -> i64 { return 0; }",
    );
    let fields = preliminary
        .static_fields()
        .map(|field| field.field)
        .collect::<Vec<_>>();
    let analysis = infer_static_effects(&preliminary);

    for function in [FunctionId::new(0), FunctionId::new(1)] {
        let effects = effect_fields(&analysis, StaticEffectNode::Callable(function.into()));
        assert!(effects.contains(&fields[0]), "{effects:?}");
        assert!(effects.contains(&fields[1]), "{effects:?}");
    }
    let virtual_summary = analysis
        .summary(StaticEffectNode::Callable(FunctionId::new(0).into()))
        .unwrap();
    assert!(virtual_summary
        .effects
        .iter()
        .flat_map(|effect| &effect.witness)
        .any(|edge| edge.kind == StaticEffectEdgeKind::VirtualDispatch));
    let interface_summary = analysis
        .summary(StaticEffectNode::Callable(FunctionId::new(1).into()))
        .unwrap();
    assert!(interface_summary
        .effects
        .iter()
        .flat_map(|effect| &effect.witness)
        .any(|edge| edge.kind == StaticEffectEdgeKind::InterfaceDispatch));
}

#[test]
fn models_constructor_copy_temporary_optional_and_array_lifecycle_effects() {
    let preliminary = lower(
        "class State {
           static constructed: i64 = 1;
           static copied: i64 = 2;
           static destroyed: i64 = 3;
           static item: Item = Item();
           static item_copy: Item = (Item());
           init() {}
         }
         class Item {
           value: i64;
           init() { self.value = State.constructed; }
           copy(ref other: Item) { self.value = State.copied; }
           assign(ref other: Item) { self.value = State.copied; }
           destroy { var observed: i64 = State.destroyed; }
         }
         fn temporary() -> i64 { var item: Item = Item(); return 0; }
         fn optional() -> i64 { var item: Item? = Item(); return 0; }
         fn array() -> i64 { var items: Item[] = Item[]{Item()}; return 0; }
         fn assignment() -> i64 {
           var left: Item = Item();
           var right: Item = Item();
           left = right;
           return 0;
         }
         fn main() -> i64 { return 0; }",
    );
    let fields = preliminary
        .static_fields()
        .map(|field| field.field)
        .collect::<Vec<_>>();
    let analysis = infer_static_effects(&preliminary);
    let item = ClassId::new(1);

    assert!(effect_fields(
        &analysis,
        StaticEffectNode::class(item, StaticClassLifecycleOperation::CopyConstructor)
    )
    .contains(&fields[1]));
    assert!(effect_fields(
        &analysis,
        StaticEffectNode::class(item, StaticClassLifecycleOperation::CompleteFinalizer)
    )
    .contains(&fields[2]));
    assert!(effect_fields(
        &analysis,
        StaticEffectNode::class(item, StaticClassLifecycleOperation::CopyAssignment)
    )
    .contains(&fields[1]));
    for function in [
        FunctionId::new(0),
        FunctionId::new(1),
        FunctionId::new(2),
        FunctionId::new(3),
    ] {
        let effects = effect_fields(&analysis, StaticEffectNode::Callable(function.into()));
        assert!(effects.contains(&fields[0]), "{function}: {effects:?}");
        assert!(effects.contains(&fields[2]), "{function}: {effects:?}");
    }
    assert!(analysis.summaries().any(|summary| {
        matches!(
            summary.node,
            StaticEffectNode::ArrayLifecycle {
                operation: StaticArrayLifecycleOperation::Destruction,
                ..
            }
        ) && summary
            .effects
            .iter()
            .any(|effect| effect.field == fields[2])
    }));

    let copied_initializer = preliminary
        .static_initializers()
        .find(|initializer| initializer.field == fields[4])
        .unwrap();
    let cleanup_effect = analysis
        .summary(StaticEffectNode::Callable(copied_initializer.callable()))
        .unwrap()
        .effects
        .iter()
        .find(|effect| effect.field == fields[2])
        .unwrap();
    assert_eq!(
        cleanup_effect.phase,
        StaticEffectPhase::InitializerAfterPublication
    );
}

#[test]
fn shared_release_includes_every_compatible_dynamic_finalizer() {
    let preliminary = lower(
        "class State {
           static base_destroyed: i64 = 1;
           static child_destroyed: i64 = 2;
           init() {}
         }
         class Base {
           init() {}
           destroy { var observed: i64 = State.base_destroyed; }
         }
         class Child extends Base {
           init() { super(); }
           destroy { var observed: i64 = State.child_destroyed; }
         }
         fn consume(owner: shared Base) -> i64 { return 0; }
         fn main() -> i64 { return consume(new Child()); }",
    );
    let fields = preliminary
        .static_fields()
        .map(|field| field.field)
        .collect::<Vec<_>>();
    let analysis = infer_static_effects(&preliminary);
    let effects = effect_fields(
        &analysis,
        StaticEffectNode::Callable(FunctionId::new(0).into()),
    );

    assert!(effects.contains(&fields[0]), "{effects:?}");
    assert!(effects.contains(&fields[1]), "{effects:?}");
}

#[test]
fn witness_selection_and_dump_are_stable() {
    let preliminary = lower(
        "fn left() -> i64 { return State.value; }
         fn right() -> i64 { return State.value; }
         fn choose() -> i64 { return left() + right(); }
         class State {
           static value: i64 = 1;
           static result: i64 = choose();
           init() {}
         }
         fn main() -> i64 { return 0; }",
    );
    let first = dump_static_effects(&infer_static_effects(&preliminary));
    for _ in 0..16 {
        assert_eq!(
            dump_static_effects(&infer_static_effects(&preliminary)),
            first
        );
    }
    let analysis = infer_static_effects(&preliminary);
    let value = preliminary.static_fields().next().unwrap().field;
    let choose = analysis
        .summary(StaticEffectNode::Callable(FunctionId::new(2).into()))
        .unwrap()
        .effects
        .iter()
        .find(|effect| effect.field == value)
        .unwrap();
    assert_eq!(
        choose.witness[0].target,
        StaticEffectNode::Callable(FunctionId::new(0).into())
    );
}

#[test]
fn string_language_item_initialization_is_in_the_effect_inventory() {
    let (_workspace, graph) = load_module_sources_with_standard_library(
        "app",
        &[(
            "app.ska",
            "from std::str import Str;
             class State { static text: Str = \"ready\"; init() {} }
             fn main() -> i64 { return 0; }",
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
    let preliminary = lower_preliminary_hir(&checked.hir.unwrap());
    let initializer = preliminary.static_initializers().next().unwrap();
    let analysis = infer_static_effects(&preliminary);
    let summary = analysis
        .summary(StaticEffectNode::Callable(initializer.callable()))
        .unwrap();

    assert!(summary.direct_effects.iter().any(|effect| {
        effect.field == initializer.field
            && effect.access == StaticAccessKind::Initialize
            && effect.phase == StaticEffectPhase::InitializerBeforePublication
    }));
    assert!(dump_static_effects(&analysis).contains("CopyConstructor"));
}

mod function_values;
mod generic_classes;
mod structural_indexing;
