use super::*;
use crate::{
    hir::{
        HirAccess, HirIterationReceiverCarrier, HirIterationReceiverLifetime,
        HirIterationValueInitialization, HirOptionalPresenceTestPlan, HirOptionalUnwrapPlan,
        HirStatement, HirViewSource, Type,
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

fn receiver_view(loop_: &crate::hir::HirForIn) -> &crate::hir::HirObjectView {
    match &loop_.receiver.carrier {
        HirIterationReceiverCarrier::View(view) => view,
        HirIterationReceiverCarrier::Checked(view) => &view.view,
    }
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
    assert_eq!(loop_.receiver.carrier.access(), HirAccess::ReadOnly);
    assert!(matches!(
        receiver_view(loop_).source,
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
        receiver_view(viewed_loop).source,
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

    let mir = crate::mir::lower_hir(&hir);
    crate::mir::verify_mir(&mir).expect("mixed nested loop MIR must verify");
}

#[test]
fn loop_duration_receivers_cover_produced_shared_optional_and_array_sources() {
    let hir = check_iteration(concat!(
        "from std::iter import Iterable;\n",
        "class Counter implements Iterable<i64, u64> {\n",
        "  value: i64; init(value: i64) { self.value = value; }\n",
        "  fn iter_state() -> u64 { return 0u; }\n",
        "  fn iter_next(mut ref state: u64) -> i64? { return none; }\n",
        "}\n",
        "fn receivers() -> unit {\n",
        "  for (item in Counter(1)) {}\n",
        "  var owner: shared Counter = new Counter(2);\n",
        "  for (item in *owner) { owner = new Counter(3); break; }\n",
        "  for (item in *(new Counter(8))) { var independent: i64 = item; }\n",
        "  var maybe: Counter? = Counter(4);\n",
        "  for (item in maybe!) { var independent: i64 = item; }\n",
        "  var box: shared Counter? = new Counter?(Counter(11));\n",
        "  for (item in (*box)!) { box = new Counter?(Counter(12)); break; }\n",
        "  var values: Counter[] = Counter[]{Counter(5)};\n",
        "  for (item in values[0]) { values = Counter[]{Counter(6)}; break; }\n",
        "  var shared_values: shared Counter[] = new Counter[]{Counter(9)};\n",
        "  for (item in (*shared_values)[0]) { var independent: i64 = item; }\n",
        "  var maybe_values: Counter[]? = Counter[]{Counter(10)};\n",
        "  for (item in maybe_values![0]) { var independent: i64 = item; }\n",
        "}\n",
        "fn main() -> i64 { return 0; }\n",
    ));

    let receivers = hir
        .definitions
        .iter()
        .find(|definition| {
            definition
                .body
                .statements
                .iter()
                .any(|statement| matches!(statement, HirStatement::ForIn(_)))
        })
        .expect("receiver fixture must retain its loops");
    let sources: Vec<_> = receivers
        .body
        .statements
        .iter()
        .filter_map(|statement| match statement {
            HirStatement::ForIn(loop_) => Some(&receiver_view(loop_).source),
            _ => None,
        })
        .collect();
    assert!(matches!(sources[0], HirViewSource::Produced { .. }));
    assert!(matches!(sources[1], HirViewSource::AnchoredShared { .. }));
    assert!(matches!(sources[2], HirViewSource::AnchoredShared { .. }));
    assert!(matches!(sources[3], HirViewSource::OptionalPayload { .. }));
    assert!(matches!(
        sources[4],
        HirViewSource::OptionalBoxPayload { .. }
    ));
    assert!(matches!(sources[5], HirViewSource::ArrayElement(_)));
    assert!(matches!(sources[6], HirViewSource::ArrayElement(_)));
    assert!(matches!(sources[7], HirViewSource::ArrayElement(_)));

    let mir = crate::mir::lower_hir(&hir);
    crate::mir::verify_mir(&mir).expect("all retained receiver families must verify");
}

#[test]
fn checked_cast_iteration_retains_cast_and_owner_carriers() {
    let hir = check_iteration(concat!(
        "from std::iter import Iterable;\n",
        "class Base { init() {} }\n",
        "class Derived extends Base implements Iterable<i64, u64> {\n",
        "  init() { super(); }\n",
        "  fn iter_state() -> u64 { return 0u; }\n",
        "  fn iter_next(mut ref state: u64) -> i64? { return none; }\n",
        "}\n",
        "fn scan(owner: shared Base) -> unit {\n",
        "  for (item in (Derived) *owner) { var independent: i64 = item; }\n",
        "}\n",
        "fn main() -> i64 { return 0; }\n",
    ));
    let loop_ = first_for_in(&hir);
    let HirIterationReceiverCarrier::Checked(checked) = &loop_.receiver.carrier else {
        panic!("cast receiver must retain its checked carrier")
    };
    assert!(matches!(
        checked.view.source,
        HirViewSource::AnchoredShared { .. }
    ));
    let mir = crate::mir::lower_hir(&hir);
    crate::mir::verify_mir(&mir).expect("loop-duration checked cast must verify");
    let dump = crate::mir::dump_mir(&mir);
    assert!(dump.contains("checked-cast"), "{dump}");
    assert!(dump.contains("terminate object-cast-failure"), "{dump}");
    assert!(dump.contains("end-checked-view"), "{dump}");
    assert!(dump.contains("shared-release"), "{dump}");
}

#[test]
fn inherited_iterable_view_preserves_complete_exact_origin() {
    let hir = check_iteration(concat!(
        "from std::iter import Iterable;\n",
        "class Base implements Iterable<i64, u64> {\n",
        "  init() {}\n",
        "  fn iter_state() -> u64 { return 0u; }\n",
        "  fn iter_next(mut ref state: u64) -> i64? { return none; }\n",
        "}\n",
        "class Derived extends Base { init() { super(); } }\n",
        "fn scan(value: Derived) -> unit { for (item in value) {} }\n",
        "fn main() -> i64 { return 0; }\n",
    ));
    let loop_ = first_for_in(&hir);
    let view = receiver_view(loop_);
    assert!(matches!(
        view.origin.as_ref(),
        crate::hir::HirObjectOrigin::Exact { dynamic_class, .. }
            if Type::Class(*dynamic_class) == loop_.receiver.iterable
    ));
    assert_eq!(
        view.target,
        crate::hir::HirViewTarget::Interface(loop_.protocol.interface)
    );
    let mir = crate::mir::lower_hir(&hir);
    crate::mir::verify_mir(&mir).expect("inherited iterable dispatch must verify");
}

#[test]
fn rejects_body_write_that_invalidates_guarded_optional_receiver() {
    let resolved = resolve_iteration(concat!(
        "from std::iter import Iterable;\n",
        "class Counter implements Iterable<i64, u64> {\n",
        "  init() {}\n",
        "  fn iter_state() -> u64 { return 0u; }\n",
        "  fn iter_next(mut ref state: u64) -> i64? { return none; }\n",
        "}\n",
        "fn scan() -> unit {\n",
        "  var maybe: Counter? = Counter();\n",
        "  for (item in maybe!) { maybe = Counter(); }\n",
        "}\n",
        "fn main() -> i64 { return 0; }\n",
    ));
    assert!(
        resolved.diagnostics.is_empty(),
        "{:?}",
        resolved.diagnostics
    );
    let checked = type_check(&resolved.program);
    assert!(checked.hir.is_none());
    assert!(checked.diagnostics.iter().any(|diagnostic| {
        diagnostic.code == GENERAL_ITERATION_UNSUPPORTED
            && diagnostic.message.contains("guarded optional")
    }));
}

#[test]
fn core_iteration_lowers_to_verified_deterministic_ordinary_mir() {
    let hir = check_iteration(&format!(
        "{COUNTER}fn scan(values: Counter) -> unit {{ for (item in values) {{}} }}\nfn main() -> i64 {{ return 0; }}\n"
    ));
    let iteration = first_for_in(&hir);
    let mir = crate::mir::lower_hir(&hir);
    crate::mir::verify_mir(&mir).expect("core iteration MIR must verify");
    let crate::identity::CallableId::Function(function) = iteration.loop_id.callable() else {
        panic!("fixture iteration must belong to a function")
    };
    let definition = mir.definitions.get(function).unwrap();
    let calls = definition
        .body
        .blocks
        .iter()
        .flat_map(|block| &block.instructions)
        .filter_map(|instruction| match instruction {
            crate::mir::MirInstruction::Call(call) => Some(&call.target),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(
        calls
            .iter()
            .filter(|target| matches!(
                target,
                crate::mir::MirCallTarget::Interface(target)
                    if target.requirement == iteration.protocol.iter_state
            ))
            .count(),
        1
    );
    assert_eq!(
        calls
            .iter()
            .filter(|target| matches!(
                target,
                crate::mir::MirCallTarget::Interface(target)
                    if target.requirement == iteration.protocol.iter_next
            ))
            .count(),
        1
    );
    let dump = crate::mir::dump_mir(&mir);
    assert_eq!(dump, crate::mir::dump_mir(&crate::mir::lower_hir(&hir)));
    assert!(dump.contains("call interface"), "{dump}");
    assert!(dump.contains("optional-presence"), "{dump}");
    assert!(!dump.contains("ForIn"), "{dump}");
}

#[test]
fn class_items_and_mixed_loop_exits_lower_to_verified_mir() {
    let hir = check_iteration(concat!(
        "from std::iter import Iterable;\n",
        "class Item { value: i64; init(value: i64) { self.value = value; } }\n",
        "class Items implements Iterable<Item, u64> {\n",
        "  init() {}\n",
        "  fn iter_state() -> u64 { return 0u; }\n",
        "  fn iter_next(mut ref state: u64) -> Item? { return none; }\n",
        "}\n",
        "fn scan(values: Items) -> i64 {\n",
        "  for (item in values) {\n",
        "    while (false) { continue; }\n",
        "    if (item.value == 1) { continue; }\n",
        "    if (item.value == 2) { break; }\n",
        "    return item.value;\n",
        "  }\n",
        "  return 0;\n",
        "}\n",
        "fn main() -> i64 { return 0; }\n",
    ));
    let mir = crate::mir::lower_hir(&hir);
    crate::mir::verify_mir(&mir).expect("class-item iteration MIR must verify");
    let dump = crate::mir::dump_mir(&mir);
    assert!(dump.contains("begin-optional-view"), "{dump}");
    assert!(dump.contains("copy-construct"), "{dump}");
}

#[test]
fn produced_receiver_cleanup_follows_state_on_every_outer_exit() {
    let hir = check_iteration(concat!(
        "from std::iter import Iterable;\n",
        "class Counter implements Iterable<i64, u64> {\n",
        "  init() {}\n",
        "  fn iter_state() -> u64 { return 0u; }\n",
        "  fn iter_next(mut ref state: u64) -> i64? { return none; }\n",
        "}\n",
        "fn scan() -> i64 {\n",
        "  for (item in Counter()) {\n",
        "    if (item == 1) { break; }\n",
        "    if (item == 2) { return item; }\n",
        "  }\n",
        "  return 0;\n",
        "}\n",
        "fn main() -> i64 { return 0; }\n",
    ));
    let loop_ = first_for_in(&hir);
    let crate::identity::CallableId::Function(function) = loop_.loop_id.callable() else {
        unreachable!()
    };
    let mir = crate::mir::lower_hir(&hir);
    crate::mir::verify_mir(&mir).expect("produced receiver exit matrix must verify");
    let definition = mir.definitions.get(function).unwrap();
    let mir_dump = crate::mir::dump_mir(&mir);
    let receiver = definition
        .storage
        .iter()
        .find(|storage| storage.name.starts_with("temporary"))
        .expect("produced receiver needs one owning temporary")
        .id;
    let state = definition
        .storage
        .iter()
        .find(|storage| storage.name.starts_with("iteration-state"))
        .expect("iteration needs retained state storage")
        .id;
    let mut receiver_cleanups = 0;
    for block in &definition.body.blocks {
        for (index, instruction) in block.instructions.iter().enumerate() {
            let crate::mir::MirInstruction::Cleanup(cleanup) = instruction else {
                continue;
            };
            if cleanup.destination.base.local_storage() != Some(receiver) {
                continue;
            }
            receiver_cleanups += 1;
            assert!(
                block.instructions[..index].iter().any(|instruction| {
                    matches!(
                        instruction,
                        crate::mir::MirInstruction::StorageDead(dead) if dead.storage == state
                    )
                }),
                "{mir_dump}"
            );
            assert!(block.instructions[index + 1..].iter().any(|instruction| {
                matches!(
                    instruction,
                    crate::mir::MirInstruction::StorageDead(dead) if dead.storage == receiver
                )
            }));
        }
    }
    assert_eq!(
        receiver_cleanups, 3,
        "exhaustion, break, and return each need cleanup"
    );
}
