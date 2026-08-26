use super::*;
use crate::{
    hir::{
        HirAccess, HirIterationReceiverLifetime, HirIterationValueInitialization,
        HirOptionalPresenceTestPlan, HirOptionalUnwrapPlan, HirStatement, HirViewSource, Type,
    },
    resolve::{resolve_module_graph, ResolveOutput},
    test_support::{load_module_sources, CANONICAL_ITER_SOURCE},
    typeck::{type_check, COPY_OPERATION_UNAVAILABLE, GENERAL_ITERATION_UNSUPPORTED},
};

const COUNTER: &str = concat!(
    "from std::iter import Iterable;\n",
    "class Counter implements Iterable<i64, u64> {\n",
    "  init() {}\n",
    "  fn iter_state() -> u64 { return 0u; }\n",
    "  fn iter_next(mut ref state: u64) -> i64? { return none; }\n",
    "}\n",
);

fn resolve_iteration(source: &str) -> ResolveOutput {
    let (_workspace, graph) = load_module_sources(
        "app",
        &[("app.ska", source), ("std/iter.ska", CANONICAL_ITER_SOURCE)],
    );
    resolve_module_graph(&graph)
}

fn check_iteration(source: &str) -> crate::hir::HirProgram {
    let resolved = resolve_iteration(source);
    assert!(
        resolved.diagnostics.is_empty(),
        "{:?}",
        resolved.diagnostics
    );
    let checked = type_check(&resolved.program);
    assert!(checked.diagnostics.is_empty(), "{:?}", checked.diagnostics);
    checked.hir.expect("core iteration source must produce HIR")
}

fn first_for_in(hir: &crate::hir::HirProgram) -> &crate::hir::HirForIn {
    let definition = hir
        .definitions
        .iter()
        .find(|definition| {
            matches!(
                definition.body.statements.first(),
                Some(HirStatement::ForIn(_))
            )
        })
        .expect("fixture must contain an iteration function");
    let HirStatement::ForIn(statement) = &definition.body.statements[0] else {
        unreachable!()
    };
    statement
}

#[test]
fn primitive_iteration_retains_exact_dispatch_receiver_and_lifecycle_plans() {
    let hir = check_iteration(&format!(
        "{COUNTER}fn scan(values: Counter) -> unit {{ for (item: i64 in values) {{ var seen: i64 = item; }} }}\nfn main() -> i64 {{ return 0; }}\n"
    ));
    let loop_ = first_for_in(&hir);

    assert_eq!(loop_.protocol.item, Type::I64);
    assert_eq!(loop_.protocol.state, Type::U64);
    assert_eq!(
        loop_.protocol.iter_state.interface(),
        loop_.protocol.interface
    );
    assert_eq!(
        loop_.protocol.iter_next.interface(),
        loop_.protocol.interface
    );
    assert_eq!(
        loop_.receiver.lifetime,
        HirIterationReceiverLifetime::LoopDuration
    );
    assert_eq!(loop_.receiver.view.access, HirAccess::ReadOnly);
    assert!(matches!(
        loop_.receiver.view.source,
        HirViewSource::Place(_)
    ));
    assert_eq!(loop_.state.advance.state_alias.access, HirAccess::Mutable);
    assert_eq!(loop_.state.advance.state_alias.ty, Type::U64);
    assert_eq!(loop_.result.presence, HirOptionalPresenceTestPlan::OuterTag);
    assert_eq!(loop_.result.unwrap, HirOptionalUnwrapPlan::ExtractScalar);
    assert_eq!(loop_.result.payload, Type::I64);
    assert_eq!(loop_.item.access, HirAccess::ReadOnly);
    assert_eq!(loop_.item.binding, loop_.binding);
    assert_eq!(
        loop_.item.value.initialization,
        HirIterationValueInitialization::Trivial
    );

    let dump = dump_hir(&hir);
    assert_eq!(dump, dump_hir(&hir));
    assert!(dump.contains("ForIn"), "{dump}");
    assert!(dump.contains("Requirements iter_state="), "{dump}");
    assert!(dump.contains("lifetime=LoopDuration"), "{dump}");
    assert!(dump.contains("state-alias=mutable u64"), "{dump}");
    assert!(
        dump.contains("presence=OuterTag unwrap=ExtractScalar"),
        "{dump}"
    );
}

#[test]
fn exact_interface_views_and_bound_specializations_keep_selected_dispatch() {
    let viewed = check_iteration(&format!(
        "{COUNTER}fn scan(ref values: Iterable<i64, u64>) -> unit {{ for (item in values) {{}} }}\nfn main() -> i64 {{ return 0; }}\n"
    ));
    let viewed_loop = first_for_in(&viewed);
    assert!(matches!(
        viewed_loop.receiver.view.source,
        HirViewSource::Forwarded { .. }
    ));
    assert_eq!(
        viewed_loop.receiver.iterable,
        Type::Interface(viewed_loop.protocol.interface)
    );

    let bound = check_iteration(concat!(
        "from std::iter import Iterable;\n",
        "class Concrete implements Iterable<i64, u64> {\n",
        "  init() {}\n",
        "  fn iter_state() -> u64 { return 0u; }\n",
        "  fn iter_next(mut ref state: u64) -> i64? { return none; }\n",
        "}\n",
        "class Scanner<T> where T: Iterable<i64, u64> {\n",
        "  init() {}\n",
        "  fn scan(ref values: T) -> unit { for (item in values) {} }\n",
        "}\n",
        "fn use(ref scanner: Scanner<Concrete>) -> unit {}\n",
        "fn main() -> i64 { return 0; }\n",
    ));
    let loop_ = bound
        .class_definitions
        .iter()
        .flat_map(|class| &class.methods)
        .find_map(|method| match method.body.statements.first() {
            Some(HirStatement::ForIn(loop_)) => Some(loop_),
            _ => None,
        })
        .expect("specialized bound body must retain structured iteration");
    assert_eq!(loop_.protocol.item, Type::I64);
    assert_eq!(loop_.protocol.state, Type::U64);
    assert_eq!(
        loop_.protocol.iter_next.interface(),
        loop_.protocol.interface
    );
}

#[test]
fn trivial_exact_class_items_select_copy_and_one_layer_optional_plans() {
    let hir = check_iteration(concat!(
        "from std::iter import Iterable;\n",
        "class Item { value: i64; init(value: i64) { self.value = value; } }\n",
        "class Items implements Iterable<Item, u64> {\n",
        "  init() {}\n",
        "  fn iter_state() -> u64 { return 0u; }\n",
        "  fn iter_next(mut ref state: u64) -> Item? { return none; }\n",
        "}\n",
        "fn scan(values: Items) -> unit { for (item in values) {} }\n",
        "fn main() -> i64 { return 0; }\n",
    ));
    let loop_ = first_for_in(&hir);
    let Type::Class(item) = loop_.protocol.item else {
        panic!("expected exact class item")
    };
    assert!(matches!(
        loop_.item.value.initialization,
        HirIterationValueInitialization::CopyClass { class, .. } if class == item
    ));
    assert_eq!(loop_.result.payload, Type::Class(item));
    assert_eq!(
        loop_.result.unwrap,
        HirOptionalUnwrapPlan::CheckedInlineClass(item)
    );
}

#[test]
fn unsupported_state_and_item_capabilities_are_source_diagnostics() {
    let state = resolve_iteration(concat!(
        "from std::iter import Iterable;\n",
        "class State { init() {} }\n",
        "class Values implements Iterable<i64, State> {\n",
        "  init() {}\n",
        "  fn iter_state() -> State { return State(); }\n",
        "  fn iter_next(mut ref state: State) -> i64? { return none; }\n",
        "}\n",
        "fn scan(values: Values) -> unit { for (item in values) {} }\n",
        "fn main() -> i64 { return 0; }\n",
    ));
    assert!(state.diagnostics.is_empty(), "{:?}", state.diagnostics);
    let checked = type_check(&state.program);
    assert!(checked.hir.is_none());
    assert!(checked
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == GENERAL_ITERATION_UNSUPPORTED));

    let mut unavailable = resolve_iteration(concat!(
        "from std::iter import Iterable;\n",
        "class Item { init() {} }\n",
        "class Values implements Iterable<Item, u64> {\n",
        "  init() {}\n",
        "  fn iter_state() -> u64 { return 0u; }\n",
        "  fn iter_next(mut ref state: u64) -> Item? { return none; }\n",
        "}\n",
        "fn scan(values: Values) -> unit { for (item in values) {} }\n",
        "fn main() -> i64 { return 0; }\n",
    ));
    assert!(
        unavailable.diagnostics.is_empty(),
        "{:?}",
        unavailable.diagnostics
    );
    let item = unavailable
        .program
        .classes
        .iter()
        .find(|class| class.name == "Item")
        .unwrap()
        .id;
    unavailable.program.classes.entries_mut_for_test()[item.index()].copy_constructor =
        crate::resolve::ResolvedCopyOperation::Unavailable;
    let checked = type_check(&unavailable.program);
    assert!(checked.hir.is_none());
    assert!(checked
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == COPY_OPERATION_UNAVAILABLE));
}

#[test]
fn item_is_immutable_and_loop_effects_keep_termination_fallthrough() {
    let invalid = resolve_iteration(&format!(
        "{COUNTER}fn touch(mut ref value: i64) -> unit {{}}\nfn scan(values: Counter) -> unit {{ for (item in values) {{ item = 1; touch(item); }} }}\nfn main() -> i64 {{ return 0; }}\n"
    ));
    assert!(invalid.diagnostics.is_empty(), "{:?}", invalid.diagnostics);
    let checked = type_check(&invalid.program);
    assert!(checked.hir.is_none());
    assert!(checked
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == crate::typeck::READ_ONLY_RECEIVER));
    assert!(checked
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == crate::typeck::INSUFFICIENT_ALIAS_ACCESS));

    let hir = check_iteration(&format!(
        "{COUNTER}fn scan(values: Counter) -> i64 {{ for (item in values) {{ return item; }} return 0; }}\nfn main() -> i64 {{ return 0; }}\n"
    ));
    let loop_ = first_for_in(&hir);
    assert!(loop_.effects.can_fall_through());
    assert!(loop_.effects.can_exit_function());
    assert!(!loop_.effects.can_break_to(loop_.loop_id));
    assert!(!loop_.effects.can_continue_to(loop_.loop_id));

    // The recursive HIR consumer must descend through the new statement.
    assert!(crate::hir::collect_cell_writes(&hir).is_empty());
}

#[test]
fn manually_rebuilt_hir_enforces_identity_invariants_and_dumps_deterministically() {
    let hir = check_iteration(&format!(
        "{COUNTER}fn scan(values: Counter) -> unit {{ for (item in values) {{}} }}\nfn main() -> i64 {{ return 0; }}\n"
    ));
    let original = first_for_in(&hir).clone();
    let rebuilt = crate::hir::HirForIn::new(
        original.loop_id,
        original.binding,
        original.protocol,
        original.receiver.clone(),
        original.state.clone(),
        original.result,
        original.item,
        original.body.clone(),
        original.spans,
    );
    assert_eq!(rebuilt, original);
    assert_eq!(dump_hir(&hir), dump_hir(&hir));

    let mut invalid_state = original.state.clone();
    invalid_state.advance.target.requirement = original.protocol.iter_state;
    let rejected = std::panic::catch_unwind(|| {
        crate::hir::HirForIn::new(
            original.loop_id,
            original.binding,
            original.protocol,
            original.receiver,
            invalid_state,
            original.result,
            original.item,
            original.body,
            original.spans,
        )
    });
    assert!(
        rejected.is_err(),
        "mismatched iter_next identity must be rejected"
    );
}

#[test]
fn mixed_nested_loops_preserve_only_outer_effects() {
    let hir = check_iteration(&format!(
        "{COUNTER}fn scan(values: Counter) -> unit {{\n  while (true) {{\n    for (item in values) {{ while (true) {{ break; }} continue; }}\n    break;\n  }}\n}}\nfn main() -> i64 {{ return 0; }}\n"
    ));
    let scan = hir
        .definitions
        .iter()
        .find(|definition| {
            matches!(
                definition.body.statements.first(),
                Some(HirStatement::While(_))
            )
        })
        .unwrap();
    let HirStatement::While(outer) = &scan.body.statements[0] else {
        unreachable!()
    };
    let HirStatement::ForIn(iteration) = &outer.body.statements[0] else {
        panic!("expected nested general iteration")
    };
    let HirStatement::While(inner) = &iteration.body.statements[0] else {
        panic!("expected innermost while")
    };
    assert!(inner.effects.can_fall_through());
    assert!(!inner.effects.can_break_to(inner.loop_id));
    assert!(iteration.effects.can_fall_through());
    assert!(!iteration.effects.can_continue_to(iteration.loop_id));
    assert!(outer.effects.can_fall_through());
    assert!(!outer.effects.can_break_to(outer.loop_id));
}

#[test]
fn mir_lowering_remains_intentionally_gated_until_it4() {
    let hir = check_iteration(&format!(
        "{COUNTER}fn scan(values: Counter) -> unit {{ for (item in values) {{}} }}\nfn main() -> i64 {{ return 0; }}\n"
    ));
    let rejected = std::panic::catch_unwind(|| crate::mir::lower_hir(&hir));
    assert!(rejected.is_err());
}
