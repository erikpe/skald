use super::*;
use crate::{
    identity::FieldId,
    object_path::ObjectProjection,
    typeck::{type_check, READ_ONLY_RECEIVER},
};

fn returned_field(
    output: &ResolveOutput,
    function: usize,
) -> &crate::resolve::ResolvedFieldAccessExpr {
    let definition = output
        .program
        .definitions
        .get(FunctionId::new(function))
        .expect("expected resolved function body");
    let ResolvedExpression::FieldAccess(access) = return_value(&definition.body.statements[0])
    else {
        panic!("expected produced field read in function {function}");
    };
    access
}

#[test]
fn exact_class_producer_families_resolve_once_as_field_receivers() {
    let source = concat!(
        "class Item {\n",
        "  value: i64;\n",
        "  init(value: i64) { self.value = value; }\n",
        "  static fn make_static(value: i64) -> Item { return Item(value); }\n",
        "  fn make_instance(value: i64) -> Item { return Item(value); }\n",
        "}\n",
        "interface Producer { fn produce(value: i64) -> Item; }\n",
        "fn make_direct(value: i64) -> Item { return Item(value); }\n",
        "fn constructed() -> i64 { return Item(1).value; }\n",
        "fn direct() -> i64 { return make_direct(2).value; }\n",
        "fn static_result() -> i64 { return Item.make_static(3).value; }\n",
        "fn instance_result(ref item: Item) -> i64 { return item.make_instance(4).value; }\n",
        "fn interface_result(ref producer: Producer) -> i64 { return producer.produce(5).value; }\n",
        "fn grouped() -> i64 { return ((Item(6))).value; }\n",
        "fn main() -> i64 { return 0; }\n",
    );
    let output = resolve_text(source);
    assert!(!output.has_errors(), "{:?}", output.diagnostics);

    for (function, expected_producer) in [
        (1, "construct"),
        (2, "direct"),
        (3, "static"),
        (4, "method"),
        (5, "interface"),
        (6, "construct"),
    ] {
        let access = returned_field(&output, function);
        assert_eq!(access.field, FieldId::new(ClassId::new(0), 0));
        let ResolvedObjectReceiver::Produced {
            producer,
            exact_class,
            projections,
            class,
            ..
        } = &access.receiver
        else {
            panic!("expected produced receiver in function {function}");
        };
        assert_eq!(*exact_class, ClassId::new(0));
        assert_eq!(*class, ClassId::new(0));
        assert!(projections.is_empty());
        let actual_producer = match &**producer {
            ResolvedExpression::Construct(_) => "construct",
            ResolvedExpression::DirectCall(_) => "direct",
            ResolvedExpression::StaticCall(_) => "static",
            ResolvedExpression::MethodCall(_) => "method",
            ResolvedExpression::InterfaceCall(_) => "interface",
            other => panic!("unexpected produced field source: {other:?}"),
        };
        assert_eq!(actual_producer, expected_producer);
    }

    let dump = dump_resolved(&output.program);
    assert_eq!(dump.matches("FieldAccess c0:field0").count(), 6, "{dump}");
    assert_eq!(
        dump.matches("ProducedReceiver class c0 complete c0")
            .count(),
        6
    );
    assert_eq!(dump, dump_resolved(&resolve_text(source).program));
}

#[test]
fn inherited_nested_and_closed_generic_fields_keep_canonical_provenance() {
    let output = resolve_text(concat!(
        "class Leaf { value: i64; init(value: i64) { self.value = value; } }\n",
        "class Branch { leaf: Leaf; init(leaf: Leaf) { self.leaf = leaf; } }\n",
        "class Base { branch: Branch; init(branch: Branch) { self.branch = branch; } }\n",
        "class Derived extends Base { init(value: i64) { super(Branch(Leaf(value))); } }\n",
        "class Box<T> { value: T; init(value: T) { self.value = value; } }\n",
        "fn make(value: i64) -> Derived { return Derived(value); }\n",
        "fn inherited() -> i64 { return make(1).branch.leaf.value; }\n",
        "fn generic() -> i64 { return Box<Leaf>(Leaf(2)).value.value; }\n",
        "fn main() -> i64 { return 0; }\n",
    ));
    assert!(!output.has_errors(), "{:?}", output.diagnostics);

    let inherited = returned_field(&output, 1);
    let ResolvedObjectReceiver::Produced {
        exact_class,
        projections,
        class,
        ..
    } = &inherited.receiver
    else {
        panic!("expected inherited produced receiver");
    };
    assert_eq!(*exact_class, ClassId::new(3));
    assert_eq!(*class, ClassId::new(0));
    assert_eq!(
        projections,
        &[
            ObjectProjection::Base(ClassId::new(2)),
            ObjectProjection::Field(FieldId::new(ClassId::new(2), 0)),
            ObjectProjection::Field(FieldId::new(ClassId::new(1), 0)),
        ]
    );

    let generic = returned_field(&output, 2);
    let ResolvedObjectReceiver::Produced {
        exact_class,
        projections,
        class,
        ..
    } = &generic.receiver
    else {
        panic!("expected specialized produced receiver");
    };
    assert_eq!(*class, ClassId::new(0));
    assert_ne!(*exact_class, *class);
    assert_eq!(
        projections,
        &[ObjectProjection::Field(FieldId::new(*exact_class, 0))]
    );

    let dump = dump_resolved(&output.program);
    assert!(dump.contains("BaseProjection c2"), "{dump}");
    assert!(dump.contains("FieldProjection c2:field0"), "{dump}");
    assert!(dump.contains("FieldProjection c1:field0"), "{dump}");
    assert_eq!(dump, dump_resolved(&output.program));
}

#[test]
fn structural_getters_private_lookup_and_writes_preserve_existing_rules() {
    let structural = resolve_text(concat!(
        "class Item { value: i64; init(value: i64) { self.value = value; } }\n",
        "interface Source { fn index_get(index: i64) -> Item; }\n",
        "class Values implements Source {\n",
        "  fn index_get(index: i64) -> Item { return Item(index); }\n",
        "}\n",
        "fn direct(ref values: Values) -> i64 { return values[1].value; }\n",
        "fn through_interface(ref values: Source) -> i64 { return values[2].value; }\n",
        "fn main() -> i64 { return 0; }\n",
    ));
    assert!(!structural.has_errors(), "{:?}", structural.diagnostics);
    assert!(matches!(
        returned_field(&structural, 0).receiver,
        ResolvedObjectReceiver::Produced { ref producer, .. }
            if matches!(&**producer, ResolvedExpression::MethodCall(_))
    ));
    assert!(matches!(
        returned_field(&structural, 1).receiver,
        ResolvedObjectReceiver::Produced { ref producer, .. }
            if matches!(&**producer, ResolvedExpression::InterfaceCall(_))
    ));

    let private = resolve_text(concat!(
        "class Secret {\n",
        "  private value: i64;\n",
        "  init(value: i64) { self.value = value; }\n",
        "  fn own() -> i64 { return Secret(1).value; }\n",
        "}\n",
        "fn forbidden() -> i64 { return Secret(2).value; }\n",
        "fn main() -> i64 { return 0; }\n",
    ));
    assert_eq!(
        private
            .diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.code == PRIVATE_MEMBER_ACCESS)
            .count(),
        1,
        "{:?}",
        private.diagnostics
    );
    assert!(!private
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == INVALID_MEMBER_SELECTION));
    assert_eq!(
        dump_resolved(&private.program)
            .matches("ProducedReceiver class c0 complete c0")
            .count(),
        1
    );

    let write = resolve_text(concat!(
        "class Item { value: i64; init(value: i64) { self.value = value; } }\n",
        "fn write() -> unit { Item(1).value = 2; }\n",
        "fn main() -> i64 { return 0; }\n",
    ));
    assert!(!write.has_errors(), "{:?}", write.diagnostics);
    let definition = write.program.definitions.get(FunctionId::new(0)).unwrap();
    let ResolvedStatement::FieldAssignment(assignment) = &definition.body.statements[0] else {
        panic!("write-shaped produced field must reach typed access checking");
    };
    assert!(matches!(
        assignment.receiver,
        ResolvedObjectReceiver::Produced { .. }
    ));
    let checked = type_check(&write.program);
    assert!(checked
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == READ_ONLY_RECEIVER));
}
