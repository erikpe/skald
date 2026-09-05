use super::*;

#[test]
fn shared_backed_alias_arguments_classify_stable_and_anchored_lifetimes() {
    let output = type_check_source(concat!(
        "class Root { init() {} }\n",
        "class Leaf extends Root { init() { super(); } }\n",
        "class Holder {\n",
        "  edge: shared Leaf;\n",
        "  init() { self.edge = new Leaf(); }\n",
        "}\n",
        "fn inspect(ref value: Root) -> unit {}\n",
        "fn produce() -> shared Leaf { return new Leaf(); }\n",
        "fn main() -> i64 {\n",
        "  var owner: shared Leaf = new Leaf();\n",
        "  var holder: Holder = Holder();\n",
        "  inspect(*owner);\n",
        "  inspect(*holder.edge);\n",
        "  inspect(*new Leaf());\n",
        "  inspect(*produce());\n",
        "  return 0;\n",
        "}\n",
    ));
    assert_diagnostics(&output.diagnostics, &[]);
    let hir = output.hir.expect("shared-backed aliases must type check");
    let dump = dump_hir(&hir);
    assert_eq!(dump.matches("SharedPointee").count(), 4);
    assert_eq!(dump.matches("AnchoredSharedPointee").count(), 3);

    let mir = lower_hir(&hir);
    verify_mir(&mir).expect("shared call anchors must verify");
    let main = mir.definitions.get(mir.entry_function).unwrap();
    assert_eq!(
        main.storage
            .iter()
            .filter(|storage| storage.kind == crate::mir::MirStorageKind::SharedAnchor)
            .count(),
        3
    );
}
#[test]
fn shared_backed_receivers_cover_inline_payload_subobjects() {
    let output = type_check_source(concat!(
        "class Leaf { init() {} fn read() -> i64 { return 1; } }\n",
        "class Container { leaf: Leaf; init() { self.leaf = Leaf(); } }\n",
        "class Holder {\n",
        "  edge: shared Container;\n",
        "  init() { self.edge = new Container(); }\n",
        "}\n",
        "fn produce() -> shared Container { return new Container(); }\n",
        "fn main() -> i64 {\n",
        "  var owner: shared Container = new Container();\n",
        "  var holder: Holder = Holder();\n",
        "  return owner->leaf.read() + holder.edge->leaf.read() + new Container()->leaf.read()",
        " + produce()->leaf.read() + ((shared Container) produce())->leaf.read();\n",
        "}\n",
    ));
    assert_diagnostics(&output.diagnostics, &[]);
    let hir = output.hir.expect("shared-backed receivers must type check");
    let mir = lower_hir(&hir);
    verify_mir(&mir).expect("shared receiver anchors must verify");
    let main = mir.definitions.get(mir.entry_function).unwrap();
    assert_eq!(
        main.storage
            .iter()
            .filter(|storage| storage.kind == crate::mir::MirStorageKind::SharedAnchor)
            .count(),
        4
    );
}

#[test]
fn inline_field_copy_sources_preserve_shared_pointee_lifetimes() {
    let output = type_check_source(concat!(
        "class Leaf {\n",
        "  value: i64;\n",
        "  init(value: i64) { self.value = value; }\n",
        "  copy(ref source: Leaf) { self.value = source.value; }\n",
        "}\n",
        "class Container {\n",
        "  leaf: Leaf;\n",
        "  init(value: i64) { self.leaf = Leaf(value); }\n",
        "}\n",
        "class Holder {\n",
        "  edge: shared Container;\n",
        "  init(value: i64) { self.edge = new Container(value); }\n",
        "}\n",
        "fn produce(value: i64) -> shared Container {\n",
        "  return new Container(value);\n",
        "}\n",
        "fn consume(value: Leaf) -> i64 { return value.value; }\n",
        "fn main() -> i64 {\n",
        "  var owner: shared Container = new Container(3);\n",
        "  var holder: Holder = Holder(5);\n",
        "  var stable: Leaf = owner->leaf;\n",
        "  var anchored: Leaf = holder.edge->leaf;\n",
        "  return stable.value + anchored.value + consume(produce(7)->leaf);\n",
        "}\n",
    ));
    assert_diagnostics(&output.diagnostics, &[]);
    let hir = output
        .hir
        .expect("shared-pointee inline fields must be valid owning copy sources");
    let mir = lower_hir(&hir);
    verify_mir(&mir).expect("shared-pointee inline-field copies must verify");
    let main = mir.definitions.get(mir.entry_function).unwrap();
    assert_eq!(
        main.storage
            .iter()
            .filter(|storage| storage.kind == crate::mir::MirStorageKind::SharedAnchor)
            .count(),
        2
    );
    assert_eq!(
        main.storage
            .iter()
            .filter(|storage| {
                matches!(storage.kind, crate::mir::MirStorageKind::CheckedView(_))
            })
            .count(),
        3
    );
}

#[test]
fn receiver_and_argument_anchors_precede_later_replacement_and_release_after_call() {
    let output = type_check_source(concat!(
        "class Leaf { value: i64; init(value: i64) { self.value = value; } }\n",
        "class Holder {\n",
        "  edge: shared Leaf;\n",
        "  init() { self.edge = new Leaf(1); }\n",
        "}\n",
        "fn inspect(ref value: Leaf, later: i64) -> i64 { return value.value + later; }\n",
        "fn replace(mut ref holder: Holder) -> i64 {\n",
        "  holder.edge = new Leaf(2);\n",
        "  return 4;\n",
        "}\n",
        "fn main() -> i64 {\n",
        "  var holder: Holder = Holder();\n",
        "  return inspect(*holder.edge, replace(holder));\n",
        "}\n",
    ));
    assert_diagnostics(&output.diagnostics, &[]);
    let hir = output.hir.expect("replacement-safe call must type check");
    let mir = lower_hir(&hir);
    verify_mir(&mir).expect("replacement-safe anchor MIR must verify");
    let definition = mir.definitions.get(mir.entry_function).unwrap();
    let instructions = &definition.body.blocks[0].instructions;
    let anchor = instructions
        .iter()
        .position(|instruction| {
            matches!(instruction, crate::mir::MirInstruction::SharedFieldCopy(_))
        })
        .unwrap();
    let replace = instructions
        .iter()
        .position(|instruction| {
            matches!(
                instruction,
                crate::mir::MirInstruction::Call(call)
                    if call.target == crate::mir::MirCallTarget::Direct(FunctionId::new(1))
            )
        })
        .unwrap();
    let inspect = instructions
        .iter()
        .position(|instruction| {
            matches!(
                instruction,
                crate::mir::MirInstruction::Call(call)
                    if call.target == crate::mir::MirCallTarget::Direct(FunctionId::new(0))
            )
        })
        .unwrap();
    let release = instructions
        .iter()
        .position(|instruction| {
            matches!(
                instruction,
                crate::mir::MirInstruction::SharedRelease(release)
                    if definition.storage[release.owner.index()].kind
                        == crate::mir::MirStorageKind::SharedAnchor
            )
        })
        .unwrap();
    assert!(anchor < replace && replace < inspect && inspect < release);
}

#[test]
fn nested_shared_fields_anchor_each_owner_edge_without_graph_search() {
    let output = type_check_source(concat!(
        "class Leaf { init() {} }\n",
        "class Middle { edge: shared Leaf; init() { self.edge = new Leaf(); } }\n",
        "class Outer { edge: shared Middle; init() { self.edge = new Middle(); } }\n",
        "fn inspect(ref value: Leaf) -> unit {}\n",
        "fn main() -> i64 {\n",
        "  var outer: Outer = Outer();\n",
        "  inspect(*outer.edge->edge);\n",
        "  return 0;\n",
        "}\n",
    ));
    assert_diagnostics(&output.diagnostics, &[]);
    let hir = output
        .hir
        .expect("nested shared field borrow must type check");
    let mir = lower_hir(&hir);
    verify_mir(&mir).expect("nested shared anchors must verify");
    let main = mir.definitions.get(mir.entry_function).unwrap();
    assert_eq!(
        main.storage
            .iter()
            .filter(|storage| storage.kind == crate::mir::MirStorageKind::SharedAnchor)
            .count(),
        2
    );
}

#[test]
fn shared_anchors_support_forwarding_and_deliberately_overlapping_mutable_aliases() {
    let output = type_check_source(concat!(
        "class Leaf { value: i64; init() { self.value = 0; } }\n",
        "class Holder { edge: shared Leaf; init() { self.edge = new Leaf(); } }\n",
        "fn inspect(ref value: Leaf) -> i64 { return value.value; }\n",
        "fn forward(ref value: Leaf) -> i64 { return inspect(value); }\n",
        "fn touch(mut ref left: Leaf, mut ref right: Leaf) -> unit {\n",
        "  left.value = 1;\n",
        "  right.value = 2;\n",
        "}\n",
        "fn main() -> i64 {\n",
        "  var holder: Holder = Holder();\n",
        "  var before: i64 = forward(*holder.edge);\n",
        "  touch(*holder.edge, *holder.edge);\n",
        "  return before;\n",
        "}\n",
    ));
    assert_diagnostics(&output.diagnostics, &[]);
    let hir = output
        .hir
        .expect("shared alias forwarding and overlap must type check");
    let mir = lower_hir(&hir);
    verify_mir(&mir).expect("shared alias forwarding and overlap must verify");
    let main = mir.definitions.get(mir.entry_function).unwrap();
    assert_eq!(
        main.storage
            .iter()
            .filter(|storage| storage.kind == crate::mir::MirStorageKind::SharedAnchor)
            .count(),
        3
    );
}

#[test]
fn shared_call_result_is_secured_before_receiver_anchor_cleanup() {
    let output = type_check_source(concat!(
        "class Leaf { init() {} }\n",
        "class Node {\n",
        "  edge: shared Leaf;\n",
        "  init() { self.edge = new Leaf(); }\n",
        "  fn child() -> shared Leaf { return self.edge; }\n",
        "}\n",
        "class Holder { node: shared Node; init() { self.node = new Node(); } }\n",
        "fn main() -> i64 {\n",
        "  var holder: Holder = Holder();\n",
        "  var child: shared Leaf = holder.node->child();\n",
        "  return 0;\n",
        "}\n",
    ));
    assert_diagnostics(&output.diagnostics, &[]);
    let hir = output
        .hir
        .expect("anchored shared-result call must type check");
    let mir = lower_hir(&hir);
    verify_mir(&mir).expect("anchored shared-result call must verify");
    let main = mir.definitions.get(mir.entry_function).unwrap();
    let instructions = &main.body.blocks[0].instructions;
    let call = instructions
        .iter()
        .position(|instruction| {
            matches!(
                instruction,
                crate::mir::MirInstruction::Call(call) if call.shared_result.is_some()
            )
        })
        .unwrap();
    let release = instructions
        .iter()
        .position(|instruction| {
            matches!(
                instruction,
                crate::mir::MirInstruction::SharedRelease(release)
                    if main.storage[release.owner.index()].kind
                        == crate::mir::MirStorageKind::SharedAnchor
            )
        })
        .unwrap();
    assert!(call < release);
}

#[test]
fn shared_interface_fields_and_producers_use_call_anchors() {
    let output = type_check_source(concat!(
        "interface Readable { fn read() -> i64; }\n",
        "class Leaf implements Readable {\n",
        "  init() {}\n",
        "  fn read() -> i64 { return 3; }\n",
        "}\n",
        "class Holder {\n",
        "  value: shared Readable;\n",
        "  init() { self.value = new Leaf(); }\n",
        "}\n",
        "fn produce() -> shared Readable { return new Leaf(); }\n",
        "fn main() -> i64 {\n",
        "  var holder: Holder = Holder();\n",
        "  return holder.value->read() + produce()->read();\n",
        "}\n",
    ));
    assert_diagnostics(&output.diagnostics, &[]);
    let hir = output
        .hir
        .expect("shared interface receivers must type check");
    let mir = lower_hir(&hir);
    verify_mir(&mir).expect("shared interface anchors must verify");
    let main = mir.definitions.get(mir.entry_function).unwrap();
    assert_eq!(
        main.storage
            .iter()
            .filter(|storage| storage.kind == crate::mir::MirStorageKind::SharedAnchor)
            .count(),
        2
    );
}

#[test]
fn anchors_and_inline_temporaries_cleanup_in_reverse_completion_order() {
    let output = type_check_source(concat!(
        "class Leaf { init() {} }\n",
        "class Value { init() {} fn read() -> i64 { return 1; } }\n",
        "class Holder { edge: shared Leaf; init() { self.edge = new Leaf(); } }\n",
        "fn inspect(ref leaf: Leaf, value: i64) -> i64 { return value; }\n",
        "fn main() -> i64 {\n",
        "  var holder: Holder = Holder();\n",
        "  return inspect(*holder.edge, ((Value) Value()).read());\n",
        "}\n",
    ));
    assert_diagnostics(&output.diagnostics, &[]);
    let hir = output.hir.expect("mixed temporary call must type check");
    let mir = lower_hir(&hir);
    verify_mir(&mir).expect("mixed temporary cleanup must verify");
    let main = mir.definitions.get(mir.entry_function).unwrap();
    let instructions = &main.body.blocks[0].instructions;
    let inline_cleanup = instructions
        .iter()
        .position(|instruction| {
            matches!(
                instruction,
                crate::mir::MirInstruction::EndFullExpression(end)
                    if !end.temporaries.is_empty()
            )
        })
        .unwrap();
    let anchor_release = instructions
        .iter()
        .position(|instruction| {
            matches!(
                instruction,
                crate::mir::MirInstruction::SharedRelease(release)
                    if main.storage[release.owner.index()].kind
                        == crate::mir::MirStorageKind::SharedAnchor
            )
        })
        .unwrap();
    assert!(inline_cleanup < anchor_release);
}
