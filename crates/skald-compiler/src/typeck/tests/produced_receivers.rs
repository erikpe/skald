use super::*;
use crate::{
    hir::{HirAccess, HirObjectProducer, HirObjectReceiver, HirViewSource},
    mir::{dump_mir, lower_hir, verify_mir},
    resolve::resolve_module_graph,
    test_support::{load_module_sources_with_standard_library, lower_hir_to_final_mir},
    typeck::{type_check, READ_ONLY_RECEIVER},
};

const PRODUCED_RECEIVERS: &str = concat!(
    "class Item {\n",
    "  value: i64;\n",
    "  init(value: i64) { self.value = value; }\n",
    "  static fn make_static(value: i64) -> Item { return Item(value); }\n",
    "  fn make_instance(value: i64) -> Item { return Item(value); }\n",
    "  fn read(extra: i64) -> i64 { return self.value + extra; }\n",
    "  mut fn replace(value: i64) -> unit { self.value = value; }\n",
    "}\n",
    "interface Producer { fn produce(value: i64) -> Item; }\n",
    "fn make_direct(value: i64) -> Item { return Item(value); }\n",
    "fn constructed() -> i64 { return Item(1).read(2); }\n",
    "fn direct() -> i64 { return make_direct(3).read(4); }\n",
    "fn static_result() -> i64 { return Item.make_static(5).read(6); }\n",
    "fn instance_result(ref item: Item) -> i64 { return item.make_instance(7).read(8); }\n",
    "fn interface_result(ref producer: Producer) -> i64 { return producer.produce(9).read(10); }\n",
    "fn grouped() -> i64 { return ((Item(11))).read(12); }\n",
    "fn main() -> i64 { return 0; }\n",
);

fn assert_produced_receiver(expression: &HirExpression) {
    let HirExpressionKind::MethodCall { receiver, .. } = &expression.kind else {
        panic!("expected method call");
    };
    let HirObjectReceiver::View {
        view,
        inspection_place,
    } = receiver
    else {
        panic!("expected general object-view receiver");
    };
    assert!(inspection_place.is_none());
    assert_eq!(view.access, HirAccess::ReadOnly);
    assert!(matches!(view.source, HirViewSource::Produced { .. }));
    assert!(matches!(
        view.source,
        HirViewSource::Produced {
            ref producer,
            ..
        } if matches!(
            &**producer,
            HirObjectProducer::Construct(_) | HirObjectProducer::Call(_)
        )
    ));
}

#[test]
fn every_exact_class_producer_uses_one_readonly_temporary_view() {
    let output = check_text(PRODUCED_RECEIVERS);
    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    let hir = output
        .hir
        .expect("valid produced receivers must produce HIR");

    for index in 1..=6 {
        assert_produced_receiver(returned_expression(
            hir.definitions.get(FunctionId::new(index)).unwrap(),
        ));
    }

    let hir_dump = dump_hir(&hir);
    assert_eq!(hir_dump.matches("ProducedMethodReceiver").count(), 6);
    let mir = lower_hir(&hir);
    verify_mir(&mir).expect("produced receiver MIR must verify");

    let repeated = check_text(PRODUCED_RECEIVERS).hir.unwrap();
    assert_eq!(dump_hir(&repeated), hir_dump);
    assert_eq!(dump_mir(&lower_hir(&repeated)), dump_mir(&mir));
}

#[test]
fn inherited_selection_projects_the_view_without_slicing_the_complete_object() {
    let output = check_text(concat!(
        "class Root {\n",
        "  value: i64;\n",
        "  init(value: i64) { self.value = value; }\n",
        "  fn read(extra: i64) -> i64 { return self.value + extra; }\n",
        "}\n",
        "class Leaf extends Root { init(value: i64) { super(value); } }\n",
        "fn make() -> Leaf { return Leaf(40); }\n",
        "fn inspect() -> i64 { return make().read(2); }\n",
        "fn main() -> i64 { return inspect(); }\n",
    ));
    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    let hir = output.hir.unwrap();
    let expression = returned_expression(hir.definitions.get(FunctionId::new(1)).unwrap());
    let HirExpressionKind::MethodCall { receiver, .. } = &expression.kind else {
        panic!("expected inherited produced call");
    };
    let HirObjectReceiver::View { view, .. } = receiver else {
        panic!("expected produced view");
    };
    let HirViewSource::Produced {
        producer,
        projections,
    } = &view.source
    else {
        panic!("expected produced provenance");
    };
    assert_eq!(producer.class(), crate::identity::ClassId::new(1));
    assert_eq!(
        projections,
        &[crate::object_path::ObjectProjection::Base(
            crate::identity::ClassId::new(0)
        )]
    );
    assert_eq!(
        view.target,
        crate::hir::HirViewTarget::Class(crate::identity::ClassId::new(0))
    );
    assert!(matches!(
        view.origin.as_ref(),
        crate::hir::HirObjectOrigin::Produced {
            dynamic_class,
            ..
        } if *dynamic_class == crate::identity::ClassId::new(1)
    ));
    verify_mir(&lower_hir(&hir)).expect("projected produced receiver must verify");
}

#[test]
fn receiver_production_control_effects_stabilize_earlier_scalar_values() {
    let output = check_text(concat!(
        "class Item {\n",
        "  value: i64;\n",
        "  init(value: i64) { self.value = value; }\n",
        "  fn read() -> i64 { return self.value; }\n",
        "}\n",
        "fn calculate(divisor: i64) -> i64 {\n",
        "  return 2 + Item(40 / divisor).read();\n",
        "}\n",
        "fn main() -> i64 { return calculate(1); }\n",
    ));
    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    let mir = lower_hir(&output.hir.unwrap());
    verify_mir(&mir).expect("control-affecting produced receiver must verify");
    let calculate = mir.definitions.get(FunctionId::new(0)).unwrap();
    assert!(calculate.storage.iter().any(|storage| {
        storage.kind == crate::mir::MirStorageKind::ScalarSpill && storage.name.starts_with("spill")
    }));
}

#[test]
fn string_and_closed_generic_results_are_ordinary_produced_receivers() {
    let (_workspace, graph) = load_module_sources_with_standard_library(
        "app",
        &[(
            "app.ska",
            concat!(
                "from std::str import Str;\n",
                "from std::vec import Vec;\n",
                "fn literal() -> u8 { return \"abc\".byte(1); }\n",
                "fn vector() -> u8 {\n",
                "  var values: Vec<Str> = Vec<Str>();\n",
                "  values.push(\"tail\");\n",
                "  return values.last().byte(0);\n",
                "}\n",
                "fn main() -> i64 { return 0; }\n",
            ),
        )],
    );
    let resolved = resolve_module_graph(&graph);
    assert!(!resolved.has_errors(), "{:?}", resolved.diagnostics);
    let checked = type_check(&resolved.program);
    assert!(!checked.has_errors(), "{:?}", checked.diagnostics);
    let hir = checked.hir.unwrap();
    let dump = dump_hir(&hir);
    assert!(dump.contains("StringLiteral"), "{dump}");
    assert!(
        dump.matches("ProducedMethodReceiver").count() >= 2,
        "{dump}"
    );
    verify_mir(&lower_hir_to_final_mir(&hir)).expect("literal and Vec<Str> receivers must verify");
}

#[test]
fn mutable_methods_reject_produced_receivers_with_the_existing_access_diagnostic() {
    let output = check_text(concat!(
        "class Item {\n",
        "  init() {}\n",
        "  mut fn replace() -> unit {}\n",
        "}\n",
        "fn main() -> i64 { Item().replace(); return 0; }\n",
    ));

    assert!(output.hir.is_none());
    assert_eq!(output.diagnostics.len(), 1, "{:?}", output.diagnostics);
    let diagnostic = output.diagnostics.iter().next().unwrap();
    assert_eq!(diagnostic.code, READ_ONLY_RECEIVER);
    assert!(diagnostic.message.contains("mutable method"));
    assert_eq!(diagnostic.labels.len(), 2);
}

#[test]
fn mutable_bound_requirements_reject_produced_closed_generic_receivers() {
    let source = concat!(
        "interface Editable { mut fn edit() -> unit; }\n",
        "class Item implements Editable {\n",
        "  init() {}\n",
        "  copy(ref source: Item) {}\n",
        "  assign(ref source: Item) {}\n",
        "  mut fn edit() -> unit {}\n",
        "}\n",
        "class Invoke<T> where T: Editable {\n",
        "  value: T;\n",
        "  init(value: T) { self.value = value; }\n",
        "  fn produce() -> T { return self.value; }\n",
        "  fn run() -> unit { self.produce().edit(); }\n",
        "}\n",
        "fn main() -> i64 { var invoke: Invoke<Item> = Invoke<Item>(Item()); return 0; }\n",
    );
    let resolved = crate::test_support::resolve_generic_source(source);
    let resolved_dump = crate::resolve::dump_resolved(&resolved);
    assert!(resolved_dump.contains("InterfaceCall"), "{resolved_dump}");
    let checked = type_check(&resolved);

    assert!(checked.hir.is_none());
    assert!(
        checked.diagnostics.iter().any(|diagnostic| {
            diagnostic.code == READ_ONLY_RECEIVER
                && diagnostic.message.contains("mutable interface requirement")
                && diagnostic.labels.len() == 2
        }),
        "{:?}",
        checked.diagnostics
    );
}
