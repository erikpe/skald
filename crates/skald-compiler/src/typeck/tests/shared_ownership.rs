use super::*;
use crate::{
    hir::{
        dump_hir, HirCallArgument, HirLocalInitializer, HirOwnerTransfer, HirReturnValue,
        HirSharedFieldWriteKind, HirSharedProducer, HirSharedSource, HirSharedTarget, HirStatement,
        Type,
    },
    identity::{ClassId, FunctionId, InitializerId, InterfaceId},
    mir::{lower_hir, verify_mir},
    typeck::{INVALID_SHARED_CONVERSION, UNSUPPORTED_SHARED_OPERATION},
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
    assert_eq!(
        allocation.initializer,
        InitializerId::new(ClassId::new(1), 0)
    );

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
    assert!(dump.contains("SharedAllocation c1 via c1:init0"));
    assert!(dump.contains("Shared c2:field0"));
    assert!(dump.contains("Shared c2:field1"));
    assert!(dump.contains("SharedField c2:field1"));
    assert!(dump.contains("SharedField c2:field0"));

    let mir = lower_hir(&hir).expect("shared up-views must lower after SO9");
    verify_mir(&mir).expect("shared up-view MIR must verify");
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
    let mir = lower_hir(&hir).expect("stable shared views must lower");
    verify_mir(&mir).expect("stable shared views must verify");
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
fn copy_allocation_remains_a_structured_typed_hir_exclusion() {
    let output = type_check_source(concat!(
        "class Item { init() {} }\n",
        "fn main() -> i64 {\n",
        "  var source: Item = Item();\n",
        "  var owner: shared Item = new Item(copy source);\n",
        "  return 0;\n",
        "}\n",
    ));

    assert_diagnostics(&output.diagnostics, &[UNSUPPORTED_SHARED_OPERATION]);
    assert!(output.hir.is_none());
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
