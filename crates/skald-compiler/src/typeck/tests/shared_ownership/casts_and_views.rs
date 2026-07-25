use super::*;

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
