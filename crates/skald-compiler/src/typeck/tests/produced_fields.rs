use super::*;
use crate::{
    hir::{HirAccess, HirObjectOrigin, HirObjectReceiver, HirViewSource},
    mir::{dump_mir, verify_mir, MirStorageKind},
    object_path::ObjectProjection,
    test_support::lower_hir_to_final_mir,
    typeck::{INVALID_ALIAS_ARGUMENT, READ_ONLY_RECEIVER},
};

const PRODUCED_FIELD_CONSUMERS: &str = concat!(
    "class Leaf {\n",
    "  value: i64;\n",
    "  init(value: i64) { self.value = value; }\n",
    "  fn read() -> i64 { return self.value; }\n",
    "}\n",
    "class Holder {\n",
    "  leaf: Leaf; value: i64;\n",
    "  init(value: i64) { self.leaf = Leaf(value); self.value = value; }\n",
    "}\n",
    "class Outer {\n",
    "  holder: Holder;\n",
    "  init(value: i64) { self.holder = Holder(value); }\n",
    "}\n",
    "fn make(value: i64) -> Holder { return Holder(value); }\n",
    "fn inspect(ref leaf: Leaf) -> i64 { return leaf.read(); }\n",
    "fn consume(leaf: Leaf) -> i64 { return leaf.read(); }\n",
    "fn direct() -> i64 { return make(1).value; }\n",
    "fn nested() -> i64 { return Outer(2).holder.leaf.value; }\n",
    "fn method() -> i64 { return make(3).leaf.read(); }\n",
    "fn alias() -> i64 { return inspect(make(4).leaf); }\n",
    "fn checked() -> i64 { return ((Leaf) make(5).leaf).read(); }\n",
    "fn explicit_copy() -> i64 { var leaf: Leaf = Leaf(copy make(6).leaf); return leaf.read(); }\n",
    "fn owning_argument() -> i64 { return consume(make(7).leaf); }\n",
    "fn assignment_source() -> i64 {\n",
    "  var leaf: Leaf = Leaf(0); leaf = make(8).leaf; return leaf.read();\n",
    "}\n",
    "fn return_copy() -> Leaf { return make(9).leaf; }\n",
    "fn main() -> i64 { return direct() + nested() + method() + alias() + checked(); }\n",
);

fn produced_view(expression: &HirExpression) -> &crate::hir::HirObjectView {
    let HirExpressionKind::FieldRead(place) = &expression.kind else {
        panic!("expected produced primitive field read");
    };
    let HirObjectReceiver::View {
        view,
        inspection_place,
    } = &place.receiver
    else {
        panic!("expected produced object view");
    };
    assert!(inspection_place.is_none());
    assert_eq!(view.access, HirAccess::ReadOnly);
    view
}

#[test]
fn primitive_and_nested_reads_keep_one_readonly_produced_view() {
    let checked = check_text(PRODUCED_FIELD_CONSUMERS);
    assert!(!checked.has_errors(), "{:?}", checked.diagnostics);
    let hir = checked.hir.unwrap();

    let direct = produced_view(returned_expression(
        hir.definitions.get(FunctionId::new(3)).unwrap(),
    ));
    let HirViewSource::Produced {
        producer,
        projections,
    } = &direct.source
    else {
        panic!("direct field read must retain produced provenance");
    };
    assert_eq!(producer.class(), crate::identity::ClassId::new(1));
    assert!(projections.is_empty());
    assert!(matches!(
        direct.origin.as_ref(),
        HirObjectOrigin::Produced { dynamic_class, .. }
            if *dynamic_class == crate::identity::ClassId::new(1)
    ));

    let nested = produced_view(returned_expression(
        hir.definitions.get(FunctionId::new(4)).unwrap(),
    ));
    let HirViewSource::Produced { projections, .. } = &nested.source else {
        panic!("nested field read must retain produced provenance");
    };
    assert_eq!(
        projections,
        &[
            ObjectProjection::Field(crate::identity::FieldId::new(
                crate::identity::ClassId::new(2),
                0,
            )),
            ObjectProjection::Field(crate::identity::FieldId::new(
                crate::identity::ClassId::new(1),
                0,
            )),
        ]
    );
    assert!(matches!(
        nested.origin.as_ref(),
        HirObjectOrigin::Produced { dynamic_class, .. }
            if *dynamic_class == crate::identity::ClassId::new(0)
    ));

    let hir_dump = dump_hir(&hir);
    assert!(hir_dump.contains("FieldRead"), "{hir_dump}");
    assert!(hir_dump.contains("ProducedView"), "{hir_dump}");
    assert!(!hir_dump.contains("Receiver receiver"), "{hir_dump}");

    let mir = lower_hir_to_final_mir(&hir);
    verify_mir(&mir).expect("primitive and nested produced fields must verify");
    let mir_dump = dump_mir(&mir);
    assert_eq!(mir_dump, dump_mir(&lower_hir_to_final_mir(&hir)));
}

#[test]
fn inline_class_fields_feed_every_readonly_object_consumer() {
    let checked = check_text(PRODUCED_FIELD_CONSUMERS);
    assert!(!checked.has_errors(), "{:?}", checked.diagnostics);
    let hir = checked.hir.unwrap();
    let dump = dump_hir(&hir);

    assert!(dump.matches("ProducedView").count() >= 8, "{dump}");
    assert!(dump.contains("CheckedSource static"), "{dump}");
    assert!(dump.contains("ExplicitCopyConstruct"), "{dump}");
    assert!(dump.contains("CopyAssignment"), "{dump}");
    assert!(dump.contains("ObjectResult"), "{dump}");

    let mir = lower_hir_to_final_mir(&hir);
    verify_mir(&mir).expect("all inline produced-field consumers must verify");
    let temporary_count = mir
        .definitions
        .iter()
        .flat_map(|definition| &definition.storage)
        .filter(|storage| storage.kind == MirStorageKind::Temporary)
        .count();
    assert!(temporary_count >= 9);
}

#[test]
fn produced_field_effects_spill_earlier_scalars_and_mutation_stays_rejected() {
    let checked = check_text(concat!(
        "class Leaf { value: i64; init(value: i64) { self.value = value; } mut fn edit() -> unit {} }\n",
        "class Holder { leaf: Leaf; value: i64; init(value: i64) { self.leaf = Leaf(value); self.value = value; } }\n",
        "fn mutate(mut ref leaf: Leaf) -> unit {}\n",
        "fn calculate(divisor: i64) -> i64 { return 2 + Holder(40 / divisor).value; }\n",
        "fn invalid() -> unit {\n",
        "  Holder(1).value = 2;\n",
        "  Holder(2).leaf = Leaf(3);\n",
        "  Holder(4).leaf.edit();\n",
        "  mutate(Holder(5).leaf);\n",
        "}\n",
        "fn main() -> i64 { return calculate(1); }\n",
    ));
    assert!(checked.hir.is_none());
    assert_eq!(
        checked
            .diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.code == READ_ONLY_RECEIVER)
            .count(),
        3,
        "{:?}",
        checked.diagnostics
    );
    assert!(checked
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == INVALID_ALIAS_ARGUMENT));

    let valid = check_text(concat!(
        "class Holder { value: i64; init(value: i64) { self.value = value; } }\n",
        "fn calculate(divisor: i64) -> i64 { return 2 + Holder(40 / divisor).value; }\n",
        "fn main() -> i64 { return calculate(1); }\n",
    ));
    assert!(!valid.has_errors(), "{:?}", valid.diagnostics);
    let mir = lower_hir_to_final_mir(&valid.hir.unwrap());
    verify_mir(&mir).expect("control-affecting produced field must verify");
    let calculate = mir.definitions.get(FunctionId::new(0)).unwrap();
    assert!(calculate.storage.iter().any(|storage| {
        storage.kind == MirStorageKind::ScalarSpill && storage.name.starts_with("spill")
    }));
}

#[test]
fn produced_generic_fields_preserve_virtual_interface_and_structural_consumers() {
    let hir = check_generic_source(concat!(
        "interface Reader { fn read() -> i64; }\n",
        "class Base implements Reader {\n",
        "  value: i64;\n",
        "  init(value: i64) { self.value = value; }\n",
        "  virtual fn read() -> i64 { return self.value; }\n",
        "}\n",
        "class Leaf extends Base {\n",
        "  init(value: i64) { super(value); }\n",
        "  override fn read() -> i64 { return self.value; }\n",
        "}\n",
        "class Box<T> { value: T; init(value: T) { self.value = value; } }\n",
        "class Sequence {\n",
        "  init() {}\n",
        "  fn index_get(key: i64) -> Box<Leaf> { return Box<Leaf>(Leaf(key)); }\n",
        "}\n",
        "fn inspect_base(ref value: Base) -> i64 { return value.read(); }\n",
        "fn inspect(ref value: Reader) -> i64 { return value.read(); }\n",
        "fn main() -> i64 {\n",
        "  return inspect_base(Box<Leaf>(Leaf(20)).value) + inspect(Sequence()[22].value);\n",
        "}\n",
    ));

    let dump = dump_hir(&hir);
    assert!(dump.contains("MethodCall Virtual"), "{dump}");
    assert!(dump.contains("InterfaceCall"), "{dump}");
    assert!(dump.matches("ProducedView").count() >= 2, "{dump}");
    assert!(dump.contains("ObjectCall method"), "{dump}");

    let mir = lower_hir_to_final_mir(&hir);
    verify_mir(&mir).expect("generic and dispatched produced-field consumers must verify");
    assert_eq!(dump_mir(&mir), dump_mir(&lower_hir_to_final_mir(&hir)));
}
