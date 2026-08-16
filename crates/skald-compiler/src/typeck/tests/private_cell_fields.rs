use super::*;
use crate::{
    hir::{HirFieldWriteAuthorization, HirStatement},
    identity::{ClassId, FieldId},
    mir::{lower_hir, verify_mir},
    resolve::ResolvedCopyOperation,
    typeck::{COPY_OPERATION_UNAVAILABLE, READ_ONLY_RECEIVER, TYPE_MISMATCH},
};

#[test]
fn carries_cell_metadata_through_typed_generic_ir() {
    let hir = check_generic_source(concat!(
        "class Box<T> {\n",
        "  private cell value: T;\n",
        "  init(value: T) { self.value = value; }\n",
        "  fn get() -> T { return self.value; }\n",
        "}\n",
        "fn main() -> i64 { var box: Box<i64> = Box<i64>(7); return box.get(); }\n",
    ));

    let class = hir.classes.iter().next().expect("specialized class");
    assert!(class.fields[0].cell_span.is_some());
    assert!(dump_hir(&hir).contains("Field c0:field0 cell \"value\""));

    let mir = lower_hir(&hir);
    assert!(mir.class(ClassId::new(0)).unwrap().fields[0]
        .cell_span
        .is_some());
    verify_mir(&mir).unwrap();
}

#[test]
fn authorizes_a_whole_cell_field_without_upgrading_receiver_access() {
    let output = check_text(concat!(
        "class Cache {\n",
        "  private cell value: i64;\n",
        "  init() { self.value = 0; }\n",
        "  fn write() -> unit { self.value = 1; }\n",
        "}\n",
        "fn main() -> i64 { return 0; }\n",
    ));

    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    let hir = output.hir.unwrap();
    let cache = hir.class(ClassId::new(0)).unwrap();
    let method = &hir.class_definitions.get(cache.id).unwrap().methods[0];
    let HirStatement::FieldAssignment(assignment) = &method.body.statements[0] else {
        panic!("expected scalar field assignment");
    };
    assert_eq!(
        assignment.place.receiver.access(),
        crate::hir::HirAccess::ReadOnly
    );
    assert_eq!(
        assignment.place.write_authorization,
        Some(HirFieldWriteAuthorization::DeclaringClassCell)
    );
    assert_eq!(assignment.place.field, FieldId::new(cache.id, 0));
    let dump = dump_hir(&hir);
    assert!(
        dump.contains("WriteAuthorization DeclaringClassCell c0:field0"),
        "{dump}"
    );
}

#[test]
fn one_authorization_decision_covers_every_field_assignment_family() {
    let output = check_text(concat!(
        "class Item { value: i64; init(value: i64) { self.value = value; } }\n",
        "class Cache {\n",
        "  private cell scalar: i64;\n",
        "  private cell object: Item;\n",
        "  private cell maybe: i64?;\n",
        "  private cell owner: shared Item;\n",
        "  private cell values: i64[];\n",
        "  init() {\n",
        "    self.scalar = 0; self.object = Item(0); self.maybe = none;\n",
        "    self.owner = new Item(0); self.values = i64[]{};\n",
        "  }\n",
        "  fn replace(ref object: Item, owner: shared Item) -> unit {\n",
        "    self.scalar = 1; self.object = object; self.maybe = 2;\n",
        "    self.owner = owner; self.values = i64[]{3};\n",
        "  }\n",
        "}\n",
        "fn main() -> i64 { return 0; }\n",
    ));

    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    let hir = output.hir.unwrap();
    let writes = crate::hir::collect_cell_writes(&hir);
    assert_eq!(writes.len(), 5, "{}", dump_hir(&hir));
    assert_eq!(
        writes
            .iter()
            .map(|write| write.field.index())
            .collect::<Vec<_>>(),
        vec![0, 1, 2, 3, 4]
    );
    assert_eq!(
        dump_hir(&hir)
            .matches("WriteAuthorization DeclaringClassCell")
            .count(),
        5
    );
}

#[test]
fn supports_read_only_self_alias_checked_and_canonical_base_receivers() {
    let output = check_text(concat!(
        "class Base {\n",
        "  private cell value: i64;\n",
        "  init() { self.value = 0; }\n",
        "  fn through_self() -> unit { self.value = 1; }\n",
        "  static fn through_alias(ref target: Base) -> unit { (target).value = 2; }\n",
        "  static fn through_view(ref target: Obj) -> unit { ((Base) target).value = 3; }\n",
        "  static fn through_base(ref target: Derived) -> unit { target.value = 4; }\n",
        "}\n",
        "class Derived extends Base { init() { super(); } }\n",
        "fn main() -> i64 { return 0; }\n",
    ));

    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    let hir = output.hir.unwrap();
    assert_eq!(crate::hir::collect_cell_writes(&hir).len(), 4);
}

#[test]
fn cell_permission_ends_before_nested_mutation_and_mutable_forwarding() {
    let output = check_text(concat!(
        "class Item {\n",
        "  value: i64; init() { self.value = 0; }\n",
        "  mut fn change() -> unit { self.value = 1; }\n",
        "}\n",
        "fn change(mut ref item: Item) -> unit { item.change(); }\n",
        "class Holder {\n",
        "  private cell item: Item; private cell values: i64[]; private cell maybe: Item?;\n",
        "  init() { self.item = Item(); self.values = i64[]{0}; self.maybe = Item(); }\n",
        "  fn invalid() -> unit {\n",
        "    self.item.value = 2; self.item.change(); change(self.item);\n",
        "    self.values[0] = 3; self.maybe!.value = 4;\n",
        "  }\n",
        "}\n",
        "fn main() -> i64 { return 0; }\n",
    ));

    assert!(output.hir.is_none());
    assert!(
        output
            .diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.code == READ_ONLY_RECEIVER)
            .count()
            >= 4,
        "{:?}",
        output.diagnostics
    );
}

#[test]
fn cell_fields_retain_every_capability_through_a_mutable_root() {
    let output = check_text(concat!(
        "class Item {\n",
        "  value: i64; init() { self.value = 0; }\n",
        "  mut fn change() -> unit { self.value = 1; }\n",
        "}\n",
        "class Holder {\n",
        "  private cell item: Item; private cell values: i64[];\n",
        "  init() { self.item = Item(); self.values = i64[]{0}; }\n",
        "  mut fn replace() -> unit {\n",
        "    self.item = Item(); self.item.value = 2; self.item.change(); self.values[0] = 3;\n",
        "  }\n",
        "}\n",
        "fn main() -> i64 { return 0; }\n",
    ));

    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    let hir = output.hir.unwrap();
    assert!(crate::hir::collect_cell_writes(&hir).is_empty());
    verify_mir(&lower_hir(&hir)).unwrap();
}

#[test]
fn generic_cell_replacement_reuses_inferred_assignment_requirements() {
    let hir = check_generic_source(concat!(
        "class Item { value: i64; init(value: i64) { self.value = value; } }\n",
        "class Slot<T> {\n",
        "  private cell value: T;\n",
        "  init(ref value: T) { self.value = value; }\n",
        "  fn replace(ref value: T) -> unit { self.value = value; }\n",
        "}\n",
        "fn main() -> i64 { var slot: Slot<Item> = Slot<Item>(Item(1)); return 0; }\n",
    ));

    let writes = crate::hir::collect_cell_writes(&hir);
    assert_eq!(writes.len(), 1, "{}", dump_hir(&hir));
    assert!(dump_hir(&hir).contains("FieldCopyAssignment"));
}

#[test]
fn cell_replacement_reports_the_ordinary_unavailable_copy_capability() {
    let mut program = resolve_text(concat!(
        "class Item { value: i64; init(value: i64) { self.value = value; } }\n",
        "class Cache {\n",
        "  private cell item: Item;\n",
        "  init() { self.item = Item(0); }\n",
        "  fn replace(ref item: Item) -> unit { self.item = item; }\n",
        "}\n",
        "fn main() -> i64 { return 0; }\n",
    ));
    program.classes.entries_mut_for_test()[0].copy_assignment = ResolvedCopyOperation::Unavailable;

    let output = crate::typeck::type_check(&program);
    assert!(output.hir.is_none());
    assert!(output.diagnostics.iter().any(|diagnostic| {
        diagnostic.code == COPY_OPERATION_UNAVAILABLE
            && diagnostic.message.contains("copy assignment")
    }));
}

#[test]
fn cell_replacement_preserves_ordinary_type_mismatch_diagnostics() {
    let output = check_text(concat!(
        "class Cache {\n",
        "  private cell value: i64;\n",
        "  init() { self.value = 0; }\n",
        "  fn replace() -> unit { self.value = true; }\n",
        "}\n",
        "fn main() -> i64 { return 0; }\n",
    ));

    assert!(output.hir.is_none());
    assert!(output
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == TYPE_MISMATCH));
    assert!(!output
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == READ_ONLY_RECEIVER));
}
