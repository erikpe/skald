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

const PRODUCED_OWNING_FIELDS: &str = concat!(
    "class Item {\n",
    "  value: i64;\n",
    "  init(value: i64) { self.value = value; }\n",
    "  fn read() -> i64 { return self.value; }\n",
    "  mut fn bump() -> unit { self.value = self.value + 1; }\n",
    "}\n",
    "class Bundle {\n",
    "  primitive: i64?; item: Item?; values: i64[]; maybe_values: i64[]?; matrix: i64[][];\n",
    "  shared_values: shared i64[]; maybe_shared_values: (shared i64[])?;\n",
    "  owner: shared Item; maybe_owner: (shared Item)?;\n",
    "  box: shared Item?; maybe_box: (shared Item?)?; items: Item?[];\n",
    "  init(value: i64) {\n",
    "    self.primitive = some(value); self.item = some(Item(value));\n",
    "    self.values = i64[]{value, value + 1};\n",
    "    self.maybe_values = some(i64[]{value, value + 1});\n",
    "    self.matrix = i64[][]{i64[]{value, value + 1}};\n",
    "    self.shared_values = new i64[]{value, value + 1};\n",
    "    self.maybe_shared_values = some(new i64[]{value, value + 1});\n",
    "    self.owner = new Item(value); self.maybe_owner = some(new Item(value));\n",
    "    self.box = new Item?(Item(value));\n",
    "    self.maybe_box = some(new Item?(Item(value)));\n",
    "    self.items = Item?[]{some(Item(value)), none};\n",
    "  }\n",
    "}\n",
    "fn make(value: i64) -> Bundle { return Bundle(value); }\n",
    "fn inspect_array(ref values: i64[]) -> i64 { return values[0]; }\n",
    "fn inspect_item(ref item: Item) -> i64 { return item.read(); }\n",
    "fn consume_array(values: i64[]) -> i64 { return values[0]; }\n",
    "fn consume_optional(value: i64?) -> i64 { return value!; }\n",
    "fn consume_owner(owner: shared Item) -> i64 { return owner->read(); }\n",
    "fn primitive_optional() -> i64 { return consume_optional(make(1).primitive); }\n",
    "fn class_optional() -> i64 { return make(2).item!.read() + inspect_item(make(2).item!); }\n",
    "fn inline_array() -> i64 { return make(3).values[0] + inspect_array(make(4).values); }\n",
    "fn inline_array_copy() -> i64 {\n",
    "  var values: i64[] = make(5).values; values = make(5).values;\n",
    "  var slice: i64[] = make(5).values[:];\n",
    "  return consume_array(make(5).values) + values[0] + slice[0];\n",
    "}\n",
    "fn inline_array_result() -> i64[] { return make(5).values; }\n",
    "fn optional_inline_array() -> i64 { return make(5).maybe_values![0]; }\n",
    "fn optional_inline_array_result() -> i64[]? { return make(5).maybe_values; }\n",
    "fn nested_inline_array() -> i64 { return make(5).matrix[0][0]; }\n",
    "fn shared_array() -> i64 { return make(6).shared_values->[0]; }\n",
    "fn shared_array_result() -> shared i64[] { return make(6).shared_values; }\n",
    "fn optional_shared_array() -> i64 { return make(7).maybe_shared_values!->[0]; }\n",
    "fn optional_shared_array_result() -> (shared i64[])? { return make(7).maybe_shared_values; }\n",
    "fn shared_owner() -> i64 {\n",
    "  var owner: shared Item = make(8).owner; owner = make(8).owner;\n",
    "  return consume_owner(make(8).owner) + inspect_item(*make(8).owner) + owner->read();\n",
    "}\n",
    "fn shared_owner_result() -> shared Item { return make(8).owner; }\n",
    "fn shared_owner_cast() -> i64 { return ((Item) *make(8).owner).read(); }\n",
    "fn mutate_shared_pointees() -> unit {\n",
    "  make(8).owner->bump(); (*make(10).box)!.bump();\n",
    "}\n",
    "fn optional_shared_owner() -> i64 { return make(9).maybe_owner!->read(); }\n",
    "fn optional_shared_owner_result() -> (shared Item)? { return make(9).maybe_owner; }\n",
    "fn optional_box() -> i64 {\n",
    "  if ((*make(10).box) is some) { return (*make(10).box)!.read(); }\n",
    "  return 0;\n",
    "}\n",
    "fn optional_box_result() -> shared Item? { return make(10).box; }\n",
    "fn optional_box_owner() -> i64 { return (*make(11).maybe_box!)!.read(); }\n",
    "fn owner_assignments() -> i64 {\n",
    "  var shared_values: shared i64[] = make(19).shared_values;\n",
    "  shared_values = make(20).shared_values;\n",
    "  var maybe_values: (shared i64[])? = make(21).maybe_shared_values;\n",
    "  maybe_values = make(22).maybe_shared_values;\n",
    "  var owner: (shared Item)? = make(23).maybe_owner;\n",
    "  owner = make(24).maybe_owner;\n",
    "  var box: shared Item? = make(25).box; box = make(26).box;\n",
    "  var maybe_box: (shared Item?)? = make(27).maybe_box;\n",
    "  maybe_box = make(28).maybe_box;\n",
    "  return shared_values->[0] + maybe_values!->[0] + owner!->read()\n",
    "    + (*box)!.read() + (*maybe_box!)!.read();\n",
    "}\n",
    "fn inline_class_optional_array() -> i64 { return make(12).items[0]!.read(); }\n",
    "fn primitive_optional_result() -> i64? { return make(13).primitive; }\n",
    "fn class_optional_result() -> Item? { return make(14).item; }\n",
    "fn optional_assignments() -> i64 {\n",
    "  var primitive: i64? = make(15).primitive; primitive = make(16).primitive;\n",
    "  var item: Item? = make(17).item; item = make(18).item;\n",
    "  return primitive! + item!.read();\n",
    "}\n",
    "fn main() -> i64 { return 0; }\n",
);

#[test]
fn produced_owning_and_guarded_fields_reuse_existing_typed_consumers() {
    let checked = check_text(PRODUCED_OWNING_FIELDS);
    assert!(!checked.has_errors(), "{:?}", checked.diagnostics);
    let hir = checked.hir.unwrap();
    let hir_dump = dump_hir(&hir);
    assert_eq!(hir_dump, dump_hir(&hir));
    assert!(hir_dump.matches("ProducedView").count() >= 20, "{hir_dump}");

    let mir = lower_hir_to_final_mir(&hir);
    verify_mir(&mir).expect("produced owning and guarded fields must verify");
    let mir_dump = dump_mir(&mir);
    assert!(mir_dump.contains("shared-field-copy"), "{mir_dump}");
    assert!(mir_dump.contains("shared-anchor"), "{mir_dump}");
    assert!(mir_dump.contains("array-anchor"), "{mir_dump}");
    assert!(mir_dump.contains("array-allocation-failure"), "{mir_dump}");
    assert!(mir_dump.contains("end-optional-view"), "{mir_dump}");
    assert!(mir_dump.contains("end-optional-box-view"), "{mir_dump}");
    assert_eq!(mir_dump, dump_mir(&lower_hir_to_final_mir(&hir)));
}
