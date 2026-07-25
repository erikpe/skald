use super::*;

#[test]
fn explicit_shared_dereference_supports_direct_access_and_type_tests() {
    let output = type_check_source(concat!(
        "interface Readable { fn read() -> i64; }\n",
        "class Child {\n",
        "  value: i64;\n",
        "  init(value: i64) { self.value = value; }\n",
        "  fn read() -> i64 { return self.value; }\n",
        "}\n",
        "class Root {\n",
        "  value: i64;\n",
        "  init(value: i64) { self.value = value; }\n",
        "  virtual fn read() -> i64 { return self.value; }\n",
        "}\n",
        "class Node extends Root implements Readable {\n",
        "  inline_child: Child;\n",
        "  next: shared Child;\n",
        "  init(value: i64) {\n",
        "    super(value);\n",
        "    self.inline_child = Child(value + 1);\n",
        "    self.next = new Child(value + 2);\n",
        "  }\n",
        "  override fn read() -> i64 { return self.value; }\n",
        "}\n",
        "fn make(value: i64) -> shared Node { return new Node(value); }\n",
        "fn make_readable(value: i64) -> shared Readable { return new Node(value); }\n",
        "fn main() -> i64 {\n",
        "  var owner: shared Node = new Node(1);\n",
        "  owner->value = 5;\n",
        "  var direct: i64 = owner->read();\n",
        "  var grouped: i64 = (*owner).read();\n",
        "  var prefixed_field: i64 = (*owner).value;\n",
        "  var inline_value: i64 = owner->inline_child.read();\n",
        "  var nested: i64 = owner->next->read();\n",
        "  var produced: i64 = make(6)->read();\n",
        "  var dynamic: i64 = make_readable(7)->read();\n",
        "  var matches: bool = *owner is Node;\n",
        "  return direct + grouped + prefixed_field + inline_value",
        " + nested + produced + dynamic;\n",
        "}\n",
    ));
    assert_diagnostics(&output.diagnostics, &[]);
    let hir = output
        .hir
        .expect("explicit direct shared-pointee uses must type check");
    let hir_dump = dump_hir(&hir);
    assert_eq!(hir_dump, dump_hir(&hir));
    assert!(hir_dump.contains("SharedPointee"));
    assert!(hir_dump.contains("AnchoredSharedPointee"));
    let mir = lower_hir(&hir);
    verify_mir(&mir).expect("explicit direct shared-pointee uses must verify");
    let mir_dump = dump_mir(&mir);
    assert_eq!(mir_dump, dump_mir(&mir));
    assert!(mir_dump.contains("shared-anchor"));
}

#[test]
fn explicit_shared_dereference_covers_alias_cast_and_inline_copy_consumers() {
    let output = type_check_source(concat!(
        "interface Readable { fn read() -> i64; }\n",
        "class Animal {\n",
        "  value: i64;\n",
        "  init(value: i64) { self.value = value; }\n",
        "  copy(ref source: Animal) { self.value = source.value; }\n",
        "  fn read() -> i64 { return self.value; }\n",
        "}\n",
        "class Dog extends Animal implements Readable {\n",
        "  extra: i64;\n",
        "  init(value: i64) { super(value); self.extra = value + 1; }\n",
        "  copy(ref source: Dog) { self.extra = source.extra; }\n",
        "  mut fn bump() -> unit { self.extra = self.extra + 1; }\n",
        "}\n",
        "class Holder {\n",
        "  animal: shared Animal;\n",
        "  copy: Dog;\n",
        "  init(owner: shared Dog) {\n",
        "    self.animal = owner;\n",
        "    self.copy = *owner;\n",
        "  }\n",
        "}\n",
        "fn inspect(ref value: Animal) -> i64 { return value.read(); }\n",
        "fn mutate(mut ref value: Dog) -> unit { value.bump(); }\n",
        "fn inspect_readable(ref value: Readable) -> i64 { return value.read(); }\n",
        "fn inspect_obj(ref value: Obj) -> unit {}\n",
        "fn consume(value: Dog) -> i64 { return value.extra; }\n",
        "fn produce() -> shared Animal { return new Dog(30); }\n",
        "fn copy_result(owner: shared Dog) -> Dog { return *owner; }\n",
        "fn main() -> i64 {\n",
        "  var dog: shared Dog = new Dog(10);\n",
        "  var animal: shared Animal = dog;\n",
        "  var readable: shared Readable = dog;\n",
        "  var object: shared Obj = dog;\n",
        "  var holder: Holder = Holder(dog);\n",
        "  var borrowed: i64 = inspect(*dog);\n",
        "  mutate(*dog);\n",
        "  mutate(*dog);\n",
        "  var interface_value: i64 = inspect_readable(*readable);\n",
        "  inspect_obj(*object);\n",
        "  var checked: Dog = (Dog) *animal;\n",
        "  var produced: Dog = (Dog) *produce();\n",
        "  var exact: Dog = *dog;\n",
        "  var sliced: Animal = *dog;\n",
        "  exact = *dog;\n",
        "  var consumed: i64 = consume(*dog);\n",
        "  var returned: Dog = copy_result(dog);\n",
        "  var direct_copy: Dog = Dog(copy *dog);\n",
        "  var allocated: shared Dog = new Dog(copy *animal);\n",
        "  return borrowed + interface_value + checked.extra + produced.extra + exact.extra",
        " + sliced.value + consumed + returned.extra + direct_copy.extra",
        " + allocated->extra + holder.copy.extra;\n",
        "}\n",
    ));
    assert!(!output.diagnostics.has_errors(), "{:?}", output.diagnostics);
    let hir = output
        .hir
        .expect("explicit shared-pointee consumers must type check");
    let hir_dump = dump_hir(&hir);
    assert_eq!(hir_dump, dump_hir(&hir));
    assert!(hir_dump.contains("SharedPointee"));
    assert!(hir_dump.contains("CheckedSource"));
    assert!(hir_dump.contains("CopyConstruction"));
    assert!(hir_dump.contains("SharedAllocation"));

    let mir = lower_hir(&hir);
    verify_mir(&mir).expect("explicit shared-pointee consumer lifetimes must verify");
    let mir_dump = dump_mir(&mir);
    assert_eq!(mir_dump, dump_mir(&mir));
    assert!(mir_dump.contains("shared-anchor"));
    assert!(mir_dump.contains("checked-cast"));
    assert!(mir_dump.contains("copy-construct"));
}

#[test]
fn explicit_dereference_rejects_owner_confusion_and_impossible_relations() {
    let owner_output = type_check_source(concat!(
        "class Leaf { init() {} }\n",
        "fn main() -> i64 {\n",
        "  var owner: shared Leaf = new Leaf();\n",
        "  var replacement: shared Leaf = *owner;\n",
        "  return 0;\n",
        "}\n",
    ));
    assert_diagnostics(&owner_output.diagnostics, &[INVALID_SHARED_CONVERSION]);

    let cast_output = type_check_source(concat!(
        "class Leaf { init() {} }\n",
        "class Other { init() {} }\n",
        "fn main() -> i64 {\n",
        "  var owner: shared Leaf = new Leaf();\n",
        "  var impossible: Other = (Other) *owner;\n",
        "  return 0;\n",
        "}\n",
    ));
    assert_diagnostics(&cast_output.diagnostics, &["TYP029"]);

    let view_output = type_check_source(concat!(
        "interface Readable { fn read() -> i64; }\n",
        "class Leaf implements Readable {\n",
        "  init() {}\n",
        "  copy(ref source: Leaf) {}\n",
        "  fn read() -> i64 { return 1; }\n",
        "}\n",
        "fn main() -> i64 {\n",
        "  var leaf: shared Leaf = new Leaf();\n",
        "  var readable: shared Readable = leaf;\n",
        "  var object: shared Obj = leaf;\n",
        "  var from_interface: Leaf = *readable;\n",
        "  var from_object: Leaf = *object;\n",
        "  return 0;\n",
        "}\n",
    ));
    assert_diagnostics(&view_output.diagnostics, &[]);
    let hir = view_output
        .hir
        .expect("interface and Obj pointees may be checked before an exact-class copy");
    let mir = lower_hir(&hir);
    verify_mir(&mir).expect("checked interface and Obj copy sources must verify");
}

#[test]
fn shared_pointee_boundary_preserves_view_targets_and_anchor_categories() {
    let output = type_check_source(concat!(
        "interface Readable { fn read() -> i64; }\n",
        "class Leaf implements Readable {\n",
        "  value: i64;\n",
        "  init(value: i64) { self.value = value; }\n",
        "  fn read() -> i64 { return self.value; }\n",
        "}\n",
        "class Holder {\n",
        "  leaf: shared Leaf;\n",
        "  object: shared Obj;\n",
        "  init() { self.leaf = new Leaf(2); self.object = new Leaf(3); }\n",
        "}\n",
        "fn inspect_leaf(ref value: Leaf) -> i64 { return value.read(); }\n",
        "fn inspect_object(ref value: Obj) -> unit {}\n",
        "fn inspect_readable(ref value: Readable) -> i64 { return value.read(); }\n",
        "fn produce() -> shared Obj { return new Leaf(4); }\n",
        "fn main() -> i64 {\n",
        "  var leaf: shared Leaf = new Leaf(1);\n",
        "  var object: shared Obj = leaf;\n",
        "  var readable: shared Readable = leaf;\n",
        "  var holder: Holder = Holder();\n",
        "  inspect_object(object);\n",
        "  inspect_object(holder.object);\n",
        "  var copied: Leaf = (Leaf) object;\n",
        "  var matches: bool = object is Leaf;\n",
        "  return leaf.read() + readable.read() + holder.leaf.read()",
        " + inspect_leaf(leaf) + inspect_readable(readable)",
        " + inspect_leaf((Leaf) produce()) + copied.read();\n",
        "}\n",
    ));
    assert_diagnostics(&output.diagnostics, &[]);
    let hir = output
        .hir
        .expect("all shared-pointee view targets must type check");
    let dump = dump_hir(&hir);
    assert!(dump.contains("Origin Shared"));
    assert!(dump.contains("Origin AnchoredShared"));
    assert!(dump.contains("SharedPointee"));
    assert!(dump.contains("AnchoredSharedPointee"));

    let mir = lower_hir(&hir);
    verify_mir(&mir).expect("all shared-pointee anchor categories must verify");
    let main = mir.definitions.get(mir.entry_function).unwrap();
    assert!(main
        .storage
        .iter()
        .any(|storage| storage.kind == crate::mir::MirStorageKind::SharedAnchor));
}

#[test]
fn shared_backed_checked_places_cover_borrowing_mutation_and_inline_copy_consumers() {
    let output = type_check_source(concat!(
        "class Root {\n",
        "  value: i64;\n",
        "  init(value: i64) { self.value = value; }\n",
        "}\n",
        "class Leaf extends Root {\n",
        "  extra: i64;\n",
        "  init(value: i64) { super(value); self.extra = value + 1; }\n",
        "  fn read() -> i64 { return self.value + self.extra; }\n",
        "}\n",
        "class Holder {\n",
        "  edge: shared Obj;\n",
        "  init() { self.edge = new Leaf(2); }\n",
        "}\n",
        "fn inspect(ref value: Leaf) -> i64 { return value.read(); }\n",
        "fn consume(value: Leaf) -> i64 { return value.read(); }\n",
        "fn produce() -> shared Obj { return new Leaf(3); }\n",
        "fn copy_result(value: shared Obj) -> Leaf { return (Leaf) value; }\n",
        "fn main() -> i64 {\n",
        "  var owner: shared Obj = new Leaf(1);\n",
        "  var holder: Holder = Holder();\n",
        "  var first: i64 = ((Leaf) owner).read();\n",
        "  var second: i64 = inspect((Leaf) holder.edge);\n",
        "  var third: i64 = inspect((Leaf) produce());\n",
        "  var fourth: i64 = ((Leaf) new Leaf(4)).read();\n",
        "  ((Leaf) holder.edge).extra = 5;\n",
        "  var copied: Leaf = (Leaf) holder.edge;\n",
        "  var sliced: Root = (Leaf) holder.edge;\n",
        "  copied = (Leaf) holder.edge;\n",
        "  var consumed: i64 = consume((Leaf) holder.edge);\n",
        "  var returned: Leaf = copy_result(owner);\n",
        "  return first + second + third + fourth + consumed",
        " + copied.extra + sliced.value + returned.extra;\n",
        "}\n",
    ));
    assert_diagnostics(&output.diagnostics, &[]);
    let hir = output
        .hir
        .expect("shared-backed checked-place consumers must type check");
    let dump = dump_hir(&hir);
    assert_eq!(dump, dump_hir(&hir));
    assert!(dump.contains("CheckedViewArgument runtime-terminate"));
    assert!(dump.contains("CheckedSource runtime-terminate"));
    assert!(dump.contains("SliceSource"));
    assert!(dump.contains("AnchoredSharedPointee"));

    let mir = lower_hir(&hir);
    verify_mir(&mir).expect("shared-backed checked-place lifetimes must verify");
    let mir_dump = dump_mir(&mir);
    assert_eq!(mir_dump, dump_mir(&mir));
    assert!(mir_dump.contains("shared-anchor"));
    assert!(mir_dump.contains("checked-cast"));
    assert!(mir_dump.contains("end-checked-view"));
    assert!(mir_dump.contains("shared-release"));
    let main = mir.definitions.get(mir.entry_function).unwrap();
    assert!(main
        .storage
        .iter()
        .any(|storage| storage.kind == crate::mir::MirStorageKind::SharedAnchor));
    assert!(main.body.blocks.iter().any(|block| {
        matches!(
            block.terminator,
            Some(crate::mir::MirTerminator::CheckedCast { .. })
        )
    }));
    assert!(main
        .body
        .blocks
        .iter()
        .flat_map(|block| &block.instructions)
        .any(|instruction| matches!(instruction, crate::mir::MirInstruction::BindCheckedView(_))));
}
#[test]
fn produced_shared_allocations_retain_exact_dynamic_knowledge_for_place_casts() {
    let output = type_check_source(concat!(
        "class Root { init() {} }\n",
        "class Leaf extends Root {\n",
        "  init() { super(); }\n",
        "  fn read() -> i64 { return 7; }\n",
        "}\n",
        "fn main() -> i64 {\n",
        "  return ((Leaf) new Leaf()).read()",
        " + ((Leaf) ((shared Obj) new Leaf())).read();\n",
        "}\n",
    ));
    assert_diagnostics(&output.diagnostics, &[]);
    let hir = output
        .hir
        .expect("exact produced shared place casts must type check");
    let mir = lower_hir(&hir);
    verify_mir(&mir).expect("exact produced shared place casts must verify");
    let main = mir.definitions.get(mir.entry_function).unwrap();
    assert!(!main.body.blocks.iter().any(|block| {
        matches!(
            block.terminator,
            Some(crate::mir::MirTerminator::CheckedCast { .. })
        )
    }));
    assert_eq!(
        main.body
            .blocks
            .iter()
            .flat_map(|block| &block.instructions)
            .filter(|instruction| matches!(
                instruction,
                crate::mir::MirInstruction::BindCheckedView(_)
            ))
            .count(),
        2
    );
}

#[test]
fn shared_polymorphic_consumers_keep_static_targets_and_header_origins() {
    let output = type_check_source(concat!(
        "interface Readable { fn read() -> i64; }\n",
        "class Root implements Readable {\n",
        "  value: i64;\n",
        "  init(value: i64) { self.value = value; }\n",
        "  virtual fn read() -> i64 { return self.value; }\n",
        "}\n",
        "class Leaf extends Root {\n",
        "  init(value: i64) { super(value); }\n",
        "  override fn read() -> i64 { return self.value + 1; }\n",
        "  fn leaf_only() -> i64 { return 9; }\n",
        "}\n",
        "fn main() -> i64 {\n",
        "  var leaf: shared Leaf = new Leaf(4);\n",
        "  var root: shared Root = leaf;\n",
        "  var readable: shared Readable = leaf;\n",
        "  var matches: bool = root is Leaf;\n",
        "  return root.read() + readable.read();\n",
        "}\n",
    ));
    assert_diagnostics(&output.diagnostics, &[]);
    let hir = output.hir.unwrap();
    let dump = dump_hir(&hir);
    assert!(dump.contains("Origin Shared"));
    assert!(dump.contains("SharedPointee"));
    let mir = lower_hir(&hir);
    verify_mir(&mir).expect("stable shared views must verify");
}

#[test]
fn shared_casts_classify_static_runtime_and_produced_owner_transfer() {
    let output = type_check_source(concat!(
        "interface Tagged {}\n",
        "class Root { init() {} }\n",
        "class Leaf extends Root implements Tagged { init() { super(); } }\n",
        "fn cast(value: shared Obj) -> shared Leaf {\n",
        "  var leaf: shared Leaf = (shared Leaf) value;\n",
        "  return leaf;\n",
        "}\n",
        "fn main() -> i64 {\n",
        "  var tagged: shared Tagged = (shared Tagged) new Leaf();\n",
        "  return 0;\n",
        "}\n",
    ));
    assert_diagnostics(&output.diagnostics, &[]);
    let hir = output.hir.expect("valid shared casts must produce HIR");
    let dump = dump_hir(&hir);
    assert!(dump.contains("SharedCast runtime-terminate -> shared class c1"));
    assert!(dump.contains("SharedCast static -> shared interface i0"));
    assert!(dump.contains("SharedTransfer Adopt"));
    let mir = lower_hir(&hir);
    verify_mir(&mir).expect("shared cast MIR must verify");
}

#[test]
fn shared_casts_reject_nonowners_and_statically_impossible_targets() {
    let output = type_check_source(concat!(
        "class Left { init() {} }\n",
        "class Right { init() {} }\n",
        "fn from_alias(ref value: Left) -> unit {\n",
        "  var invalid: shared Left = (shared Left) value;\n",
        "}\n",
        "fn main() -> i64 {\n",
        "  var inline: Left = Left();\n",
        "  var invalid_inline: shared Left = (shared Left) inline;\n",
        "  var impossible: shared Right = (shared Right) new Left();\n",
        "  return 0;\n",
        "}\n",
    ));
    let cast_errors: Vec<_> = output
        .diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.code == crate::typeck::program::INVALID_OBJECT_CAST)
        .collect();
    assert_eq!(cast_errors.len(), 3, "{:?}", output.diagnostics);
    assert!(cast_errors.iter().any(|diagnostic| diagnostic
        .message
        .contains("requires an existing or produced shared owner")));
    assert!(cast_errors
        .iter()
        .any(|diagnostic| diagnostic.message.contains("never succeed")));
}
