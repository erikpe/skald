use super::*;

#[test]
fn checks_construction_fields_methods_and_all_callable_owners() {
    let output = check_text(concat!(
        "class Counter {\n",
        "  value: i64;\n",
        "  init(value: i64) { self.value = value; }\n",
        "  mut fn add(amount: i64) -> unit { self.value = self.value + amount; }\n",
        "  fn get() -> i64 { return self.value; }\n",
        "}\n",
        "fn main() -> i64 { var counter: Counter = Counter(40); counter.add(2); return counter.get(); }\n",
    ));

    assert!(!output.has_errors(), "{:?}", output.diagnostics);
    let hir = output.hir.unwrap();
    let class = hir.class(ClassId::new(0)).unwrap();
    assert_eq!(class.fields[0].id, FieldId::new(class.id, 0));
    assert_eq!(class.initializer.id, InitializerId::new(class.id, 0));
    assert_eq!(class.methods[0].id, MethodId::new(class.id, 0));
    assert_eq!(class.methods[0].receiver_access, HirAccess::Mutable);
    assert_eq!(class.methods[1].receiver_access, HirAccess::ReadOnly);

    let initializer = hir.member_definition(class.initializer.id.into()).unwrap();
    let HirStatement::FieldAssignment(assignment) = &initializer.body.statements[0] else {
        panic!("expected typed field initialization");
    };
    assert_eq!(assignment.place.field, class.fields[0].id);
    assert_eq!(
        assignment.place.receiver.root(),
        BindingId::Receiver(initializer.callable)
    );

    let main = hir.definitions.get(hir.entry_function).unwrap();
    assert_eq!(main.locals[0].ty, Type::Class(class.id));
    let HirStatement::Local(local) = &main.body.statements[0] else {
        panic!("expected object local");
    };
    let HirLocalInitializer::Object(initialization) = &local.initializer else {
        panic!("expected destination construction");
    };
    let crate::hir::HirObjectProducer::Construct(construction) = &initialization.producer else {
        panic!("expected constructor producer");
    };
    assert_eq!(construction.class, class.id);
    assert_eq!(construction.initializer, class.initializer.id);
}

#[test]
fn diagnoses_missing_duplicate_and_premature_field_initialization() {
    let output = check_text(concat!(
        "class Broken {\n",
        "  first: i64; second: i64; missing: i64;\n",
        "  init() {\n",
        "    self.first = self.second;\n",
        "    self.second = 1;\n",
        "    self.second = 2;\n",
        "  }\n",
        "}\n",
        "fn main() -> i64 { return 0; }\n",
    ));

    assert!(output.hir.is_none());
    let field_errors: Vec<_> = output
        .diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.code == FIELD_INITIALIZATION)
        .collect();
    assert_eq!(field_errors.len(), 4);
    assert!(field_errors
        .iter()
        .any(|error| error.message.contains("before initialization")));
    assert!(field_errors
        .iter()
        .any(|error| error.message.contains("more than once")));
    assert!(field_errors
        .iter()
        .any(|error| error.message.contains("not initialized")));
    assert_eq!(
        field_errors
            .iter()
            .filter(|error| error.message.contains("not initialized"))
            .count(),
        2
    );
}

#[test]
fn transitions_field_liveness_only_after_a_valid_assignment() {
    let output = check_text(concat!(
        "class Broken {\n",
        "  value: i64; copy: i64;\n",
        "  init() { self.value = true; self.copy = self.value; }\n",
        "}\n",
        "fn main() -> i64 { return 0; }\n",
    ));
    let diagnostics: Vec<_> = output.diagnostics.iter().collect();

    assert!(diagnostics.len() >= 2);
    assert_eq!(diagnostics[0].code, TYPE_MISMATCH);
    assert_eq!(diagnostics[1].code, FIELD_INITIALIZATION);
    assert_eq!(
        diagnostics[1].message,
        "field `value` is used before initialization"
    );
}

#[test]
fn enforces_initializer_shape_and_field_types() {
    let output = check_text(concat!(
        "class Broken {\n",
        "  value: i64;\n",
        "  init() { if (true) { self.value = 1; } }\n",
        "}\n",
        "class WrongType { value: i64; init() { self.value = true; } }\n",
        "fn main() -> i64 { return 0; }\n",
    ));

    assert!(output.hir.is_none());
    assert!(output
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == INVALID_INITIALIZER_BODY));
    assert!(output
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == FIELD_INITIALIZATION));
    assert!(output
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == TYPE_MISMATCH));
}

#[test]
fn requires_one_explicit_initializer_even_for_empty_classes() {
    let output = check_text("class Empty {} fn main() -> i64 { return 0; }");

    assert!(output.hir.is_none());
    let diagnostic = output.diagnostics.iter().next().unwrap();
    assert_eq!(diagnostic.code, INVALID_OBJECT_DECLARATION);
    assert!(diagnostic.message.contains("explicit initializer"));
}

#[test]
fn constructs_class_fields_and_exposes_them_only_after_successful_initialization() {
    let output = check_text(concat!(
        "class Seed { value: i64; init(value: i64) { self.value = value; } }\n",
        "class Child { value: i64; init(ref seed: Seed) { self.value = seed.value; } fn get() -> i64 { return self.value; } }\n",
        "fn inspect(ref child: Child) -> i64 { return child.get(); }\n",
        "class Parent {\n",
        "  child: Child; tag: i64; total: i64;\n",
        "  init(ref seed: Seed) {\n",
        "    self.tag = 1;\n",
        "    self.child = Child(seed);\n",
        "    self.total = inspect(self.child) + self.child.get();\n",
        "  }\n",
        "  fn get() -> i64 { return self.total + self.tag; }\n",
        "}\n",
        "fn main() -> i64 {\n",
        "  var seed: Seed = Seed(20);\n",
        "  var parent: Parent = Parent(seed);\n",
        "  return parent.get();\n",
        "}\n",
    ));

    assert!(!output.has_errors(), "{:?}", output.diagnostics);
    let hir = output.hir.unwrap();
    let parent = hir.class(ClassId::new(2)).unwrap();
    let initializer = hir.member_definition(parent.initializer.id.into()).unwrap();
    assert!(matches!(
        initializer.body.statements.as_slice(),
        [
            HirStatement::FieldAssignment(_),
            HirStatement::FieldConstruction(_),
            HirStatement::FieldAssignment(_),
        ]
    ));
    let HirStatement::FieldConstruction(statement) = &initializer.body.statements[1] else {
        unreachable!();
    };
    assert_eq!(statement.place.field, FieldId::new(parent.id, 0));
    assert_eq!(statement.construction.class, ClassId::new(1));
    assert_eq!(
        statement.construction.initializer,
        InitializerId::new(ClassId::new(1), 0)
    );
    let HirCallArgument::Place(seed) = &statement.construction.arguments[0] else {
        panic!("expected alias constructor argument");
    };
    assert_eq!(seed.access, HirAccess::ReadOnly);

    let dump = dump_hir(&hir);
    let field_construction: Vec<_> = dump
        .lines()
        .filter(|line| {
            line.contains("FieldConstruction") || line.contains("Construct c1 via c1:init0")
        })
        .map(str::trim)
        .collect();
    assert_eq!(
        field_construction,
        [
            "FieldConstruction @345..370",
            "Construct c1 via c1:init0 @358..369",
        ]
    );

    let mir = lower_hir(&hir);
    assert!(verify_mir(&mir).is_ok());
    let parent_initializer = mir.member_definition(parent.initializer.id.into()).unwrap();
    let construction = parent_initializer.body.blocks[0]
        .instructions
        .iter()
        .find_map(|instruction| match instruction {
            MirInstruction::Initialize(initialize)
                if initialize.target == InitializerId::new(ClassId::new(1), 0) =>
            {
                Some(initialize)
            }
            _ => None,
        })
        .expect("field construction should lower to destination initialization");
    assert_eq!(
        construction.destination.projections,
        [MirPlaceProjection::Field(FieldId::new(parent.id, 0))]
    );
}

#[test]
fn diagnoses_invalid_class_field_construction_forms_without_object_rvalues() {
    let scalar = check_text(concat!(
        "class Child { init() {} }\n",
        "class Parent { child: Child; init() { self.child = 1; } }\n",
        "fn main() -> i64 { return 0; }\n",
    ));
    assert!(scalar.diagnostics.iter().any(|diagnostic| {
        diagnostic.code == INVALID_CONSTRUCTION
            && diagnostic.message.contains("requires direct construction")
    }));

    let grouped = check_text(concat!(
        "class Child { init() {} }\n",
        "class Parent { child: Child; init() { self.child = (Child()); } }\n",
        "fn main() -> i64 { return 0; }\n",
    ));
    assert!(grouped.diagnostics.iter().any(|diagnostic| {
        diagnostic.code == INVALID_CONSTRUCTION
            && diagnostic.labels[0].message.contains("ungrouped")
    }));

    let wrong = check_text(concat!(
        "class Child { init() {} }\n",
        "class Other { init() {} }\n",
        "class Parent { child: Child; init() { self.child = Other(); } }\n",
        "fn main() -> i64 { return 0; }\n",
    ));
    assert!(wrong.diagnostics.iter().any(|diagnostic| {
        diagnostic.code == INVALID_CONSTRUCTION
            && diagnostic.message.contains("does not match class field")
    }));

    let primitive = check_text(concat!(
        "class Child { init() {} }\n",
        "class Parent { value: i64; init() { self.value = Child(); } }\n",
        "fn main() -> i64 { return 0; }\n",
    ));
    assert!(primitive.diagnostics.iter().any(|diagnostic| {
        diagnostic.code == INVALID_CONSTRUCTION && diagnostic.message.contains("primitive field")
    }));
}

#[test]
fn rejects_premature_subobject_use_duplicate_construction_and_invalid_destinations() {
    let premature = check_text(concat!(
        "class Child { value: i64; init(value: i64) { self.value = value; } fn get() -> i64 { return self.value; } }\n",
        "fn inspect(ref child: Child) -> i64 { return child.get(); }\n",
        "class Parent { child: Child; first: i64; second: i64; init() {\n",
        "  self.first = self.child.get();\n",
        "  self.second = inspect(self.child);\n",
        "  self.child = Child(1);\n",
        "  self.child = Child(2);\n",
        "} }\n",
        "fn main() -> i64 { return 0; }\n",
    ));
    assert_eq!(
        premature
            .diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.code == FIELD_INITIALIZATION)
            .count(),
        5
    );
    assert_eq!(
        premature
            .diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.message.contains("used before initialization"))
            .count(),
        2
    );
    assert!(premature
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.message.contains("more than once")));

    let destinations = check_text(concat!(
        "class Child { init() {} }\n",
        "class Box { child: Child; init() { self.child = Child(); } }\n",
        "class Parent { box: Box; init(mut ref other: Parent) {\n",
        "  self.box.child = Child();\n",
        "  other.box = Box();\n",
        "  self.box = Box();\n",
        "} mut fn replace() -> unit { self.box = Box(); } }\n",
        "fn main() -> i64 { return 0; }\n",
    ));
    assert_eq!(
        destinations
            .diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.code == INVALID_INITIALIZER_BODY)
            .count(),
        2
    );
}

#[test]
fn failed_constructor_arguments_do_not_make_a_class_field_live() {
    let output = check_text(concat!(
        "class Child { value: i64; init(value: i64) { self.value = value; } fn get() -> i64 { return self.value; } }\n",
        "class Parent { child: Child; result: i64; init() {\n",
        "  self.child = Child(true);\n",
        "  self.result = self.child.get();\n",
        "} }\n",
        "fn main() -> i64 { return 0; }\n",
    ));

    assert!(output
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == TYPE_MISMATCH));
    assert!(output.diagnostics.iter().any(|diagnostic| {
        diagnostic.code == FIELD_INITIALIZATION
            && diagnostic.message.contains("used before initialization")
    }));
    assert!(output.diagnostics.iter().any(|diagnostic| {
        diagnostic.code == FIELD_INITIALIZATION
            && diagnostic.message == "field `child` is not initialized"
    }));
}

#[test]
fn validates_exact_direct_construction_and_constructor_arguments() {
    let output = check_text(concat!(
        "class Left { init(value: i64) {} }\n",
        "class Right { init() {} }\n",
        "fn main() -> i64 {\n",
        "  var wrong: Left = Right();\n",
        "  var grouped: Left = (Left(1));\n",
        "  var arity: Left = Left();\n",
        "  var typed: Left = Left(true);\n",
        "  return 0;\n",
        "}\n",
    ));

    assert!(output.hir.is_none());
    assert!(output
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == INVALID_CONSTRUCTION));
    assert!(output
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == WRONG_ARGUMENT_COUNT));
    assert!(output
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == TYPE_MISMATCH));
}

#[test]
fn rejects_object_values_outside_copy_initialization() {
    let output = check_text(concat!(
        "class Value { init() {} }\n",
        "fn main() -> i64 {\n",
        "  var first: Value = Value();\n",
        "  var copy: Value = first;\n",
        "  var scalar: i64 = Value();\n",
        "  return first;\n",
        "}\n",
    ));

    assert!(output.hir.is_none());
    assert_eq!(
        output
            .diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.code == INVALID_OBJECT_CONTEXT)
            .count(),
        1
    );
    assert!(output
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == INVALID_CONSTRUCTION));
}
