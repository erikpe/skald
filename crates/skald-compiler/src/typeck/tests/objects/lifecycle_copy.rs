use super::*;

#[test]
fn lowers_valid_copy_lifecycle_declarations_and_bodies_into_hir() {
    let output = check_text(concat!(
        "class Value {\n",
        "  value: i64;\n",
        "  init(value: i64) { self.value = value; }\n",
        "  copy(ref other: Value) { self.value = other.value; }\n",
        "  assign(ref other: Value) { self.value = other.value; }\n",
        "}\n",
        "fn main() -> i64 { return 0; }\n",
    ));

    assert!(!output.has_errors(), "{:?}", output.diagnostics);
    let hir = output.hir.unwrap();
    let class = hir.class(ClassId::new(0)).unwrap();
    let copy_constructor = CopyConstructorId::new(class.id, 0);
    let copy_assignment = CopyAssignmentId::new(class.id, 0);
    assert!(matches!(
        &class.copy_constructor,
        HirCopyCapability::User(copy) if copy.operation == copy_constructor
    ));
    assert!(matches!(
        &class.copy_assignment,
        HirCopyCapability::User(copy) if copy.operation == copy_assignment
    ));
    assert_eq!(
        class
            .copy_constructor_declaration
            .as_ref()
            .unwrap()
            .parameters[0]
            .mode,
        crate::hir::HirParameterMode::ReadOnlyAlias
    );
    assert_eq!(
        class
            .copy_assignment_declaration
            .as_ref()
            .unwrap()
            .parameter
            .mode,
        crate::hir::HirParameterMode::ReadOnlyAlias
    );
    assert!(matches!(
        hir.member_definition(copy_constructor.into())
            .unwrap()
            .body
            .statements[0],
        HirStatement::FieldAssignment(_)
    ));
    assert!(matches!(
        hir.member_definition(copy_assignment.into())
            .unwrap()
            .body
            .statements[0],
        HirStatement::FieldAssignment(_)
    ));
}

#[test]
fn computes_ordered_copy_capabilities_for_empty_primitive_and_forward_nested_classes() {
    let output = check_text(concat!(
        "class Outer {\n",
        "  inner: Inner; count: i64;\n",
        "  init() { self.inner = Inner(0); self.count = 0; }\n",
        "}\n",
        "class Empty { init() {} }\n",
        "class Inner {\n",
        "  value: i64;\n",
        "  init(value: i64) { self.value = value; }\n",
        "  copy(ref other: Inner) { self.value = other.value; }\n",
        "  assign(ref other: Inner) { self.value = other.value; }\n",
        "}\n",
        "fn main() -> i64 { return 0; }\n",
    ));
    assert!(!output.has_errors(), "{:?}", output.diagnostics);
    let hir = output.hir.unwrap();

    let outer = hir.class(ClassId::new(0)).unwrap();
    let HirCopyCapability::Synthesized(construction) = &outer.copy_constructor else {
        panic!("expected synthesized outer copy construction");
    };
    assert_eq!(construction.class, outer.id);
    assert_eq!(construction.fields.len(), 2);
    assert_eq!(construction.fields[0].field(), outer.fields[0].id);
    assert_eq!(construction.fields[1].field(), outer.fields[1].id);
    assert!(matches!(
        construction.fields[0],
        HirSynthesizedFieldCopy::Class {
            operation: HirSelectedCopyOperation::User(_),
            ..
        }
    ));
    assert!(matches!(
        construction.fields[1],
        HirSynthesizedFieldCopy::Primitive { .. }
    ));

    let HirCopyCapability::Synthesized(assignment) = &outer.copy_assignment else {
        panic!("expected synthesized outer copy assignment");
    };
    assert!(matches!(
        assignment.fields[0],
        HirSynthesizedFieldCopy::Class {
            operation: HirSelectedCopyOperation::User(_),
            ..
        }
    ));
    let empty = hir.class(ClassId::new(1)).unwrap();
    let HirCopyCapability::Synthesized(empty_construction) = &empty.copy_constructor else {
        panic!("expected empty synthesized construction");
    };
    assert!(empty_construction.fields.is_empty());
}

#[test]
fn type_checks_class_field_copy_operations_only_inside_copy_bodies() {
    let output = check_text(concat!(
        "class Child { value: i64; init(value: i64) { self.value = value; } }\n",
        "class Parent {\n",
        "  child: Child; value: i64;\n",
        "  init(value: i64) { self.child = Child(value); self.value = value; }\n",
        "  copy(ref other: Parent) { self.child = other.child; self.value = other.value; }\n",
        "  assign(ref other: Parent) { self.child = other.child; self.value = other.value; }\n",
        "}\n",
        "fn main() -> i64 { return 0; }\n",
    ));
    assert!(!output.has_errors(), "{:?}", output.diagnostics);
    let hir = output.hir.unwrap();
    let parent = hir.class(ClassId::new(1)).unwrap();
    let constructor = hir
        .member_definition(
            parent
                .copy_constructor_declaration
                .as_ref()
                .unwrap()
                .id
                .into(),
        )
        .unwrap();
    let assignment = hir
        .member_definition(
            parent
                .copy_assignment_declaration
                .as_ref()
                .unwrap()
                .id
                .into(),
        )
        .unwrap();
    let HirStatement::FieldCopyConstruction(copy_child) = &constructor.body.statements[0] else {
        panic!("expected class-field copy construction");
    };
    assert_eq!(copy_child.source.class(), ClassId::new(0));
    let crate::hir::HirObjectSource::Place(copy_source) = &copy_child.source else {
        panic!("copy-constructor field source should remain an exact place");
    };
    assert_eq!(copy_source.access, HirAccess::ReadOnly);
    assert_eq!(
        copy_child.operation,
        HirSelectedCopyOperation::Synthesized(ClassId::new(0))
    );
    let HirStatement::FieldCopyAssignment(assign_child) = &assignment.body.statements[0] else {
        panic!("expected class-field copy assignment");
    };
    assert_eq!(assign_child.place.field, parent.fields[0].id);
    assert_eq!(assign_child.source.class(), ClassId::new(0));

    let dump = dump_hir(&hir);
    let copy_lines: Vec<_> = dump
        .lines()
        .map(str::trim)
        .filter(|line| {
            line.starts_with("User c1:")
                || line.starts_with("MemberDefinition c1:copy0")
                || line.starts_with("MemberDefinition c1:assign0")
                || line.starts_with("FieldCopy")
        })
        .map(|line| line.split(" @").next().unwrap())
        .collect();
    assert_eq!(
        copy_lines,
        [
            "User c1:copy0",
            "User c1:assign0",
            "MemberDefinition c1:copy0",
            "FieldCopyConstruction",
            "MemberDefinition c1:assign0",
            "FieldCopyAssignment",
        ]
    );
}

#[test]
fn selects_place_to_place_copy_construction_and_assignment_in_hir() {
    let output = check_text(concat!(
        "class Value {\n",
        "  value: i64;\n",
        "  init(value: i64) { self.value = value; }\n",
        "  copy(ref other: Value) { self.value = other.value; }\n",
        "  assign(ref other: Value) { self.value = other.value; }\n",
        "}\n",
        "class Holder {\n",
        "  left: Value; right: Value;\n",
        "  init(value: i64) { self.left = Value(value); self.right = Value(value); }\n",
        "  mut fn exercise(ref source: Value) -> unit {\n",
        "    var whole: Holder = self;\n",
        "    var field: Value = self.left;\n",
        "    var alias: Value = source;\n",
        "    self.left = source;\n",
        "    self.right = self.left;\n",
        "  }\n",
        "}\n",
        "fn main() -> i64 {\n",
        "  var first: Value = Value(1);\n",
        "  var second: Value = first;\n",
        "  second = first;\n",
        "  second = second;\n",
        "  var holder: Holder = Holder(2);\n",
        "  (holder.left) = second;\n",
        "  return 0;\n",
        "}\n",
    ));

    assert!(!output.has_errors(), "{:?}", output.diagnostics);
    let hir = output.hir.unwrap();
    let value = hir.class(ClassId::new(0)).unwrap();
    let holder = hir.class(ClassId::new(1)).unwrap();
    let main = hir.definitions.get(hir.entry_function).unwrap();

    let HirStatement::Local(second) = &main.body.statements[1] else {
        panic!("expected copied local");
    };
    let HirLocalInitializer::Copy(copy) = &second.initializer else {
        panic!("expected explicit copy construction");
    };
    assert_eq!(copy.destination.root(), BindingId::Local(second.local));
    assert_eq!(
        source_place(&copy.source).root(),
        BindingId::Local(main.locals[0].id)
    );
    assert_eq!(
        copy.operation,
        HirSelectedCopyOperation::User(CopyConstructorId::new(value.id, 0))
    );

    let HirStatement::CopyAssignment(assignment) = &main.body.statements[2] else {
        panic!("expected explicit local copy assignment");
    };
    assert_eq!(
        assignment.destination.root(),
        BindingId::Local(second.local)
    );
    assert_eq!(
        source_place(&assignment.source).root(),
        BindingId::Local(main.locals[0].id)
    );
    assert_eq!(
        assignment.operation,
        HirSelectedCopyOperation::User(CopyAssignmentId::new(value.id, 0))
    );

    let HirStatement::CopyAssignment(self_assignment) = &main.body.statements[3] else {
        panic!("expected self-assignment to remain explicit");
    };
    assert_eq!(
        self_assignment.destination.root(),
        source_place(&self_assignment.source).root()
    );
    assert_eq!(
        self_assignment.destination.projections(),
        source_place(&self_assignment.source).projections()
    );

    let exercise = hir
        .member_definition(holder.methods[0].id.into())
        .expect("exercise method must have a definition");
    let HirStatement::Local(whole) = &exercise.body.statements[0] else {
        panic!("expected receiver copy");
    };
    let HirLocalInitializer::Copy(whole) = &whole.initializer else {
        panic!("expected receiver copy construction");
    };
    assert_eq!(
        source_place(&whole.source).root(),
        BindingId::Receiver(exercise.callable)
    );
    assert_eq!(
        whole.operation,
        HirSelectedCopyOperation::Synthesized(holder.id)
    );

    let HirStatement::Local(alias_copy) = &exercise.body.statements[2] else {
        panic!("expected alias copy");
    };
    let HirLocalInitializer::Copy(alias_copy) = &alias_copy.initializer else {
        panic!("expected alias copy construction");
    };
    assert_eq!(source_place(&alias_copy.source).access, HirAccess::ReadOnly);

    for statement in &exercise.body.statements[3..] {
        let HirStatement::CopyAssignment(assignment) = statement else {
            panic!("expected projected object assignment");
        };
        assert_eq!(
            assignment.destination.root(),
            BindingId::Receiver(exercise.callable)
        );
        assert_eq!(assignment.destination.projections().len(), 1);
    }

    let dump = dump_hir(&hir);
    let copy_dump = dump
        .lines()
        .map(str::trim)
        .filter(|line| {
            line.starts_with("CopyConstruction")
                || line.starts_with("CopyAssignmentStatement")
                || line.starts_with("Operation ")
        })
        .collect::<Vec<_>>();
    assert!(copy_dump[0].starts_with("CopyConstruction @"));
    assert_eq!(copy_dump[1], "Operation Synthesized c1");
}

#[test]
fn diagnoses_object_assignment_outside_the_supported_destination_and_source_boundary() {
    let output = check_text(concat!(
        "class Value { init() {} }\n",
        "class Other { init() {} }\n",
        "class Box {\n",
        "  child: Value;\n",
        "  init() { self.child = Value(); }\n",
        "  mut fn replace(ref other: Box) -> unit { self = other; }\n",
        "}\n",
        "fn misuse(ref readonly: Value, mut ref alias: Box) -> unit {\n",
        "  var value: Value = Value();\n",
        "  var other: Other = Other();\n",
        "  readonly = value;\n",
        "  alias.child = value;\n",
        "  value = Value();\n",
        "  value = other;\n",
        "}\n",
        "fn main() -> i64 { return 0; }\n",
    ));

    assert!(output.hir.is_none());
    assert!(output.diagnostics.iter().any(|diagnostic| {
        diagnostic.code == INVALID_OBJECT_CONTEXT
            && diagnostic.message.contains("complete method receiver")
    }));
    assert_eq!(
        output
            .diagnostics
            .iter()
            .filter(|diagnostic| {
                diagnostic.code == INVALID_OBJECT_CONTEXT
                    && diagnostic.message.contains("alias-rooted object")
            })
            .count(),
        2
    );
    assert!(output.diagnostics.iter().any(|diagnostic| {
        diagnostic.code == INVALID_OBJECT_CONTEXT && diagnostic.message.contains("same class")
    }));
}

#[test]
fn rejects_invalid_copy_body_liveness_access_and_returns() {
    let output = check_text(concat!(
        "class Broken {\n",
        "  first: i64; second: i64;\n",
        "  init() { self.first = 0; self.second = 0; }\n",
        "  copy(ref other: Broken) {\n",
        "    self.first = self.second;\n",
        "    self.first = other.first;\n",
        "    self.first = other.first;\n",
        "    return;\n",
        "  }\n",
        "  assign(ref other: Broken) {\n",
        "    var escaped: Broken = other;\n",
        "    other.first = 1;\n",
        "    return 2;\n",
        "  }\n",
        "}\n",
        "fn main() -> i64 { return 0; }\n",
    ));

    assert!(output.hir.is_none());
    assert!(output
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == INVALID_INITIALIZER_BODY));
    assert!(output.diagnostics.iter().any(|diagnostic| {
        diagnostic.code == FIELD_INITIALIZATION
            && diagnostic.message.contains("used before initialization")
    }));
    assert!(output.diagnostics.iter().any(|diagnostic| {
        diagnostic.code == FIELD_INITIALIZATION && diagnostic.message.contains("more than once")
    }));
    assert!(output
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == READ_ONLY_RECEIVER));
    assert!(output
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == INVALID_RETURN));
}

#[test]
fn accepts_object_parameters_and_results_at_the_type_boundary() {
    let resolved = resolve_text(concat!(
        "class Value {\n",
        "  field: i64;\n",
        "  init(value: i64) { self.field = value; }\n",
        "  fn copy() -> Value { return self; }\n",
        "}\n",
        "fn copy(value: Value) -> Value { return value; }\n",
        "fn main() -> i64 { return 0; }\n",
    ));

    let output = type_check(&resolved);

    assert!(output.diagnostics.is_empty());
    let hir = output.hir.unwrap();
    assert_eq!(
        hir.class(ClassId::new(0)).unwrap().methods[0].return_type,
        Type::Class(ClassId::new(0))
    );
    assert_eq!(
        hir.declarations
            .get(FunctionId::new(0))
            .unwrap()
            .return_type,
        Type::Class(ClassId::new(0))
    );
}
