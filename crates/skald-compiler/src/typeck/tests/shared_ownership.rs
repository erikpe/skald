use super::*;
use crate::{
    hir::{
        dump_hir, HirCallArgument, HirLocalInitializer, HirOwnerTransfer, HirReturnValue,
        HirSharedFieldWriteKind, HirSharedProducer, HirSharedSource, HirSharedTarget, HirStatement,
        Type,
    },
    identity::{ClassId, FunctionId, InitializerId, InterfaceId},
    mir::{dump_mir, lower_hir, verify_mir},
    typeck::INVALID_SHARED_CONVERSION,
};

#[test]
fn lowers_shared_targets_allocations_and_owner_provenance_into_hir() {
    let output = type_check_source(concat!(
        "interface Marker {}\n",
        "class Base { init() {} }\n",
        "class Dog extends Base implements Marker {\n",
        "  init() { super(); }\n",
        "}\n",
        "class Holder {\n",
        "  owner: shared Base;\n",
        "  marker: shared Marker;\n",
        "  init() { self.owner = new Dog(); self.marker = new Dog(); }\n",
        "}\n",
        "fn produce(value: shared Dog, marker: shared Marker, erased: shared Obj)",
        " -> shared Base {\n",
        "  var copied: shared Base = value;\n",
        "  var allocated: shared Base = new Dog();\n",
        "  return allocated;\n",
        "}\n",
        "fn main() -> i64 { return 0; }\n",
    ));
    assert_diagnostics(&output.diagnostics, &[]);
    let hir = output.hir.expect("valid shared semantics must produce HIR");

    let produce = hir.declarations.get(FunctionId::new(0)).unwrap();
    assert_eq!(
        produce.parameters[0].ty,
        Type::Shared(HirSharedTarget::Class(ClassId::new(1)))
    );
    assert_eq!(
        produce.parameters[1].ty,
        Type::Shared(HirSharedTarget::Interface(InterfaceId::new(0)))
    );
    assert_eq!(produce.parameters[2].ty, Type::Shared(HirSharedTarget::Obj));
    assert_eq!(
        produce.return_type,
        Type::Shared(HirSharedTarget::Class(ClassId::new(0)))
    );

    let holder = hir.classes.get(ClassId::new(2)).unwrap();
    assert_eq!(
        holder.fields[0].ty,
        Type::Shared(HirSharedTarget::Class(ClassId::new(0)))
    );
    assert_eq!(
        holder.fields[1].ty,
        Type::Shared(HirSharedTarget::Interface(InterfaceId::new(0)))
    );
    let holder_initializer = &hir
        .class_definitions
        .get(ClassId::new(2))
        .unwrap()
        .initializers[0];
    let HirStatement::SharedFieldWrite(field) = &holder_initializer.body.statements[0] else {
        panic!("expected typed shared field initialization");
    };
    assert_eq!(field.kind, HirSharedFieldWriteKind::Initialize);
    assert_eq!(field.value.operation, HirOwnerTransfer::Adopt);
    assert_eq!(field.value.target, HirSharedTarget::Class(ClassId::new(0)));

    let definition = hir.definitions.get(FunctionId::new(0)).unwrap();
    let HirStatement::Local(copied) = &definition.body.statements[0] else {
        panic!("expected copied shared local");
    };
    let HirLocalInitializer::Shared(copied) = &copied.initializer else {
        panic!("expected shared local initialization");
    };
    assert_eq!(copied.operation, HirOwnerTransfer::Copy);
    assert!(matches!(copied.source, HirSharedSource::Place(_)));

    let HirStatement::Local(allocated) = &definition.body.statements[1] else {
        panic!("expected allocated shared local");
    };
    let HirLocalInitializer::Shared(allocated) = &allocated.initializer else {
        panic!("expected shared local initialization");
    };
    assert_eq!(allocated.operation, HirOwnerTransfer::Adopt);
    let HirSharedSource::Produced(HirSharedProducer::Allocation(allocation)) = &allocated.source
    else {
        panic!("expected allocation producer");
    };
    assert_eq!(allocation.class, ClassId::new(1));
    let crate::hir::HirSharedAllocationMode::Initialize { initializer, .. } = allocation.mode
    else {
        panic!("expected ordinary allocation mode");
    };
    assert_eq!(initializer, InitializerId::new(ClassId::new(1), 0));

    let HirStatement::Return(result) = &definition.body.statements[2] else {
        panic!("expected shared return");
    };
    let Some(HirReturnValue::Shared(result)) = &result.value else {
        panic!("expected shared return value");
    };
    assert_eq!(result.operation, HirOwnerTransfer::Copy);

    let dump = dump_hir(&hir);
    assert_eq!(dump, dump_hir(&hir));
    assert!(dump.contains("shared class c0"));
    assert!(dump.contains("shared interface i0"));
    assert!(dump.contains("SharedTransfer Copy -> shared class c0"));
    assert!(dump.contains("SharedTransfer Adopt -> shared class c0"));
    assert!(dump.contains("SharedAllocation c1 initialize via c1:init0"));
    assert!(dump.contains("Shared c2:field0"));
    assert!(dump.contains("Shared c2:field1"));
    assert!(dump.contains("SharedField c2:field1"));
    assert!(dump.contains("SharedField c2:field0"));

    let mir = lower_hir(&hir);
    verify_mir(&mir).expect("shared up-view MIR must verify");
}

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
        "  inspect(owner);\n",
        "  inspect(holder.edge);\n",
        "  inspect(new Leaf());\n",
        "  inspect(produce());\n",
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
        "  return owner.leaf.read() + holder.edge.leaf.read() + new Container().leaf.read()",
        " + produce().leaf.read() + ((shared Container) produce()).leaf.read();\n",
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
        "  return inspect(holder.edge, replace(holder));\n",
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
        "  inspect(outer.edge.edge);\n",
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
        "  var before: i64 = forward(holder.edge);\n",
        "  touch(holder.edge, holder.edge);\n",
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
        "  var child: shared Leaf = holder.node.child();\n",
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
        "  return holder.value.read() + produce().read();\n",
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
        "  return inspect(holder.edge, ((Value) Value()).read());\n",
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

#[test]
fn shared_fields_are_non_containing_edges_with_complete_initialization_rules() {
    let valid = type_check_source(concat!(
        "class Left {\n",
        "  right: shared Right;\n",
        "  init(right: shared Right) { self.right = right; }\n",
        "  fn snapshot() -> shared Right { return self.right; }\n",
        "}\n",
        "class Right {\n",
        "  left: shared Left;\n",
        "  init(left: shared Left) { self.left = left; }\n",
        "}\n",
        "fn main() -> i64 { return 0; }\n",
    ));
    assert_diagnostics(&valid.diagnostics, &[]);

    let invalid = type_check_source(concat!(
        "class Item { init() {} }\n",
        "class Holder {\n",
        "  value: shared Item;\n",
        "  init() {}\n",
        "  fn invalid(value: shared Item) -> unit { self.value = value; }\n",
        "}\n",
        "fn main() -> i64 { return 0; }\n",
    ));
    assert!(invalid.diagnostics.iter().any(|diagnostic| diagnostic
        .message
        .contains("field `value` is not initialized")));
    assert!(invalid.diagnostics.iter().any(|diagnostic| diagnostic
        .message
        .contains("cannot assign through a read-only receiver")));
}

#[test]
fn shared_calls_record_copy_and_adopt_at_argument_and_result_boundaries() {
    let output = type_check_source(concat!(
        "class Base { init() {} }\n",
        "class Dog extends Base { init() { super(); } }\n",
        "fn make() -> shared Dog { return new Dog(); }\n",
        "fn consume(value: shared Base) -> i64 { return 0; }\n",
        "fn main() -> i64 {\n",
        "  var dog: shared Dog = make();\n",
        "  var first: i64 = consume(dog);\n",
        "  return consume(new Dog());\n",
        "}\n",
    ));
    assert_diagnostics(&output.diagnostics, &[]);
    let hir = output.hir.unwrap();
    let main = hir.definitions.get(FunctionId::new(2)).unwrap();

    let HirStatement::Local(dog) = &main.body.statements[0] else {
        panic!("expected shared call result local");
    };
    let HirLocalInitializer::Shared(dog) = &dog.initializer else {
        panic!("expected shared local");
    };
    assert_eq!(dog.operation, HirOwnerTransfer::Adopt);
    assert!(matches!(
        dog.source,
        HirSharedSource::Produced(HirSharedProducer::Call(_))
    ));

    let arguments = direct_call_arguments(&main.body.statements[1]);
    let HirCallArgument::Shared(argument) = &arguments[0] else {
        panic!("expected shared argument");
    };
    assert_eq!(argument.operation, HirOwnerTransfer::Copy);

    let HirStatement::Return(result) = &main.body.statements[2] else {
        panic!("expected scalar return");
    };
    let Some(HirReturnValue::Scalar(result)) = &result.value else {
        panic!("expected scalar return value");
    };
    let crate::hir::HirExpressionKind::DirectCall { arguments, .. } = &result.kind else {
        panic!("expected direct call");
    };
    let HirCallArgument::Shared(argument) = &arguments[0] else {
        panic!("expected shared argument");
    };
    assert_eq!(argument.operation, HirOwnerTransfer::Adopt);
}

#[test]
fn records_named_and_produced_shared_local_assignment() {
    let output = type_check_source(concat!(
        "class Item { init() {} }\n",
        "fn main() -> i64 {\n",
        "  var source: shared Item = new Item();\n",
        "  var destination: shared Item = source;\n",
        "  destination = source;\n",
        "  destination = new Item();\n",
        "  return 0;\n",
        "}\n",
    ));
    assert_diagnostics(&output.diagnostics, &[]);
    let hir = output.hir.unwrap();
    let main = hir.definitions.get(FunctionId::new(0)).unwrap();

    let HirStatement::SharedAssignment(named) = &main.body.statements[2] else {
        panic!("expected named shared assignment");
    };
    assert_eq!(named.value.operation, HirOwnerTransfer::Copy);
    assert!(matches!(named.value.source, HirSharedSource::Place(_)));

    let HirStatement::SharedAssignment(produced) = &main.body.statements[3] else {
        panic!("expected produced shared assignment");
    };
    assert_eq!(produced.value.operation, HirOwnerTransfer::Adopt);
    assert!(matches!(
        produced.value.source,
        HirSharedSource::Produced(HirSharedProducer::Allocation(_))
    ));

    let dump = dump_hir(&hir);
    assert!(dump.contains("SharedAssignment f0:l1"));
    assert!(dump.contains("SharedTransfer Copy -> shared class c0"));
    assert!(dump.contains("SharedTransfer Adopt -> shared class c0"));
}

#[test]
fn rejects_implicit_inline_downcast_and_external_shared_conversions() {
    let output = type_check_source(concat!(
        "class Base { init() {} }\n",
        "class Dog extends Base { init() { super(); } }\n",
        "extern fn foreign(value: shared Base) -> i64;\n",
        "fn from_alias(ref source: Base) -> i64 {\n",
        "  var invalid_alias_owner: shared Base = source;\n",
        "  return 0;\n",
        "}\n",
        "fn main() -> i64 {\n",
        "  var inline: Base = Base();\n",
        "  var invalid_owner: shared Base = inline;\n",
        "  var base: shared Base = new Base();\n",
        "  var invalid_downcast: shared Dog = base;\n",
        "  var dog: shared Dog = new Dog();\n",
        "  dog = base;\n",
        "  return 0;\n",
        "}\n",
    ));

    let codes: Vec<_> = output
        .diagnostics
        .iter()
        .map(|diagnostic| diagnostic.code)
        .collect();
    assert!(codes.contains(&crate::typeck::INVALID_EXTERNAL_DECLARATION));
    assert_eq!(
        codes
            .iter()
            .filter(|code| **code == INVALID_SHARED_CONVERSION)
            .count(),
        4
    );
    assert!(output.hir.is_none());
}

#[test]
fn copy_allocation_records_the_selected_operation_and_checked_source() {
    let output = type_check_source(concat!(
        "class Item {\n",
        "  value: i64;\n",
        "  init(value: i64) { self.value = value; }\n",
        "  copy(ref source: Item) { self.value = source.value; }\n",
        "}\n",
        "fn main() -> i64 {\n",
        "  var source: Item = Item(7);\n",
        "  var owner: shared Item = new Item(copy source);\n",
        "  return 0;\n",
        "}\n",
    ));

    assert_diagnostics(&output.diagnostics, &[]);
    let hir = output.hir.expect("valid copy allocation must produce HIR");
    let main = hir.definitions.get(hir.entry_function).unwrap();
    let HirStatement::Local(owner) = &main.body.statements[1] else {
        panic!("expected shared owner local");
    };
    let HirLocalInitializer::Shared(owner) = &owner.initializer else {
        panic!("expected shared owner initialization");
    };
    let HirSharedSource::Produced(HirSharedProducer::Allocation(allocation)) = &owner.source else {
        panic!("expected copy allocation producer");
    };
    let crate::hir::HirSharedAllocationMode::Copy { source, operation } = &allocation.mode else {
        panic!("expected explicit copy-allocation mode");
    };
    assert!(matches!(
        source.as_ref(),
        crate::hir::HirObjectSource::Checked(_)
    ));
    assert_eq!(
        *operation,
        hir.class(allocation.class)
            .unwrap()
            .copy_constructor
            .selected()
            .unwrap()
    );
    let mir = lower_hir(&hir);
    verify_mir(&mir).expect("copy-allocation MIR must verify");
}

#[test]
fn copy_allocation_accepts_alias_produced_and_shared_checked_sources() {
    let output = type_check_source(concat!(
        "class Animal {\n",
        "  value: i64;\n",
        "  init(value: i64) { self.value = value; }\n",
        "  copy(ref source: Animal) { self.value = source.value; }\n",
        "}\n",
        "class Dog extends Animal {\n",
        "  extra: i64;\n",
        "  init(value: i64, extra: i64) { super(value); self.extra = extra; }\n",
        "  copy(ref source: Dog) { self.extra = source.extra; }\n",
        "}\n",
        "fn from_alias(ref dog: Dog) -> shared Animal {\n",
        "  return new Animal(copy dog);\n",
        "}\n",
        "fn from_shared(animal: shared Animal) -> shared Dog {\n",
        "  return new Dog(copy animal);\n",
        "}\n",
        "fn from_inline_producer() -> shared Animal {\n",
        "  return new Animal(copy Dog(3, 4));\n",
        "}\n",
        "fn from_shared_producer() -> shared Animal {\n",
        "  return new Animal(copy new Dog(5, 6));\n",
        "}\n",
        "fn main() -> i64 { return 0; }\n",
    ));

    assert_diagnostics(&output.diagnostics, &[]);
    let hir = output
        .hir
        .expect("all supported copy sources must type-check");
    let mir = lower_hir(&hir);
    verify_mir(&mir).expect("all supported copy sources must verify");
    let copy_allocations = mir
        .definitions
        .iter()
        .flat_map(|definition| &definition.body.blocks)
        .flat_map(|block| &block.instructions)
        .filter(|instruction| {
            matches!(
                instruction,
                crate::mir::MirInstruction::SharedAllocate(crate::mir::MirSharedAllocate {
                    mode: crate::mir::MirSharedAllocationMode::Copy { .. },
                    ..
                })
            )
        })
        .count();
    assert_eq!(copy_allocations, 4);
    assert!(mir
        .definitions
        .get(FunctionId::new(1))
        .unwrap()
        .body
        .blocks
        .iter()
        .any(|block| matches!(
            block.terminator,
            Some(crate::mir::MirTerminator::CheckedCast { .. })
        )));
}

fn direct_call_arguments(statement: &HirStatement) -> &[HirCallArgument] {
    let HirStatement::Local(local) = statement else {
        panic!("expected scalar local");
    };
    let HirLocalInitializer::Value(value) = &local.initializer else {
        panic!("expected scalar local value");
    };
    let crate::hir::HirExpressionKind::DirectCall { arguments, .. } = &value.kind else {
        panic!("expected direct call");
    };
    arguments
}

fn assert_diagnostics(diagnostics: &crate::diagnostics::Diagnostics, expected: &[&'static str]) {
    let actual: Vec<_> = diagnostics
        .iter()
        .map(|diagnostic| diagnostic.code)
        .collect();
    assert_eq!(actual, expected);
}
