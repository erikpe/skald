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
    assert_eq!(class.initializers[0].id, InitializerId::new(class.id, 0));
    assert_eq!(class.methods[0].id, MethodId::new(class.id, 0));
    assert_eq!(
        class.methods[0].kind.receiver_access(),
        Some(HirAccess::Mutable)
    );
    assert_eq!(
        class.methods[1].kind.receiver_access(),
        Some(HirAccess::ReadOnly)
    );

    let initializer = hir
        .member_definition(class.initializers[0].id.into())
        .unwrap();
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
    assert_eq!(
        construction.initializer().unwrap(),
        class.initializers[0].id
    );
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
    let initializer = hir
        .member_definition(parent.initializers[0].id.into())
        .unwrap();
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
        statement.construction.initializer().unwrap(),
        InitializerId::new(ClassId::new(1), 0)
    );
    let (_, seed) = class_alias_view(&statement.construction.arguments().unwrap()[0]);
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
    let parent_initializer = mir
        .member_definition(parent.initializers[0].id.into())
        .unwrap();
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
        .any(|diagnostic| diagnostic.code == NO_MATCHING_INITIALIZER));
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
    assert!(
        output
            .diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.code == NO_MATCHING_INITIALIZER)
            .count()
            >= 2
    );
}

#[test]
fn selects_exact_primitive_initializer_overloads_into_hir() {
    let output = check_text(concat!(
        "class Choice {\n",
        "  selected: i64;\n",
        "  init(value: i64) { self.selected = 1; }\n",
        "  init(value: bool) { self.selected = 2; }\n",
        "  init(value: i64, flag: bool) { self.selected = 3; }\n",
        "}\n",
        "fn main() -> i64 {\n",
        "  var integer: Choice = Choice(7);\n",
        "  var boolean: Choice = Choice(true);\n",
        "  var pair: Choice = Choice(7, true);\n",
        "  return 0;\n",
        "}\n",
    ));

    assert!(!output.has_errors(), "{:?}", output.diagnostics);
    let hir = output.hir.unwrap();
    let main = hir.definitions.get(hir.entry_function).unwrap();
    let selected: Vec<_> = main
        .body
        .statements
        .iter()
        .filter_map(|statement| {
            let HirStatement::Local(local) = statement else {
                return None;
            };
            let HirLocalInitializer::Object(initialization) = &local.initializer else {
                panic!("expected object construction");
            };
            let crate::hir::HirObjectProducer::Construct(construction) = &initialization.producer
            else {
                panic!("expected constructor producer");
            };
            Some(construction.initializer().unwrap())
        })
        .collect();
    assert_eq!(
        selected,
        [
            InitializerId::new(ClassId::new(0), 0),
            InitializerId::new(ClassId::new(0), 1),
            InitializerId::new(ClassId::new(0), 2),
        ]
    );

    let mir = lower_hir(&hir);
    assert!(verify_mir(&mir).is_ok());
    let main = mir.definitions.get(mir.entry_function).unwrap();
    let lowered: Vec<_> = main
        .body
        .blocks
        .iter()
        .flat_map(|block| &block.instructions)
        .filter_map(|instruction| match instruction {
            MirInstruction::Initialize(initialize)
                if initialize.target.class() == ClassId::new(0) =>
            {
                Some(initialize.target)
            }
            _ => None,
        })
        .collect();
    assert_eq!(lowered, selected);
}

#[test]
fn selects_the_unique_most_specific_object_parameter_types() {
    let output = check_text(concat!(
        "interface Named {}\n",
        "class Animal { init() {} }\n",
        "class Dog extends Animal implements Named { init() { super(); } }\n",
        "class Choice {\n",
        "  init(ref value: Obj) {}\n",
        "  init(ref value: Named) {}\n",
        "  init(ref value: Animal) {}\n",
        "  init(ref value: Dog) {}\n",
        "}\n",
        "fn from_obj(ref value: Obj) -> Choice { return Choice(value); }\n",
        "fn main() -> i64 {\n",
        "  var dog: Dog = Dog();\n",
        "  var animal: Animal = Animal();\n",
        "  var exact: Choice = Choice(dog);\n",
        "  var base: Choice = Choice(animal);\n",
        "  return 0;\n",
        "}\n",
    ));

    assert!(!output.has_errors(), "{:?}", output.diagnostics);
    let hir = output.hir.unwrap();
    let main = hir.definitions.get(hir.entry_function).unwrap();
    let selected: Vec<_> = main
        .body
        .statements
        .iter()
        .filter_map(|statement| {
            let HirStatement::Local(local) = statement else {
                return None;
            };
            let HirLocalInitializer::Object(initialization) = &local.initializer else {
                return None;
            };
            let crate::hir::HirObjectProducer::Construct(construction) = &initialization.producer
            else {
                return None;
            };
            (construction.class == ClassId::new(2)).then_some(construction.initializer().unwrap())
        })
        .collect();
    assert_eq!(
        selected,
        [
            InitializerId::new(ClassId::new(2), 3),
            InitializerId::new(ClassId::new(2), 2),
        ]
    );
}

#[test]
fn reports_deterministic_missing_and_ambiguous_initializer_candidates() {
    let output = check_text(concat!(
        "interface Left {}\n",
        "interface Right {}\n",
        "class Both implements Left, Right { init() {} }\n",
        "class Choice {\n",
        "  init(ref value: Left) {}\n",
        "  init(ref value: Right) {}\n",
        "  init(value: i64) {}\n",
        "}\n",
        "fn main() -> i64 {\n",
        "  var both: Both = Both();\n",
        "  var ambiguous: Choice = Choice(both);\n",
        "  var missing: Choice = Choice(true);\n",
        "  return 0;\n",
        "}\n",
    ));

    let diagnostics: Vec<_> = output.diagnostics.iter().collect();
    assert_eq!(
        diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.code == AMBIGUOUS_INITIALIZER)
            .count(),
        1
    );
    assert_eq!(
        diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.code == NO_MATCHING_INITIALIZER)
            .count(),
        1
    );
    let ambiguous = diagnostics
        .iter()
        .find(|diagnostic| diagnostic.code == AMBIGUOUS_INITIALIZER)
        .unwrap();
    assert_eq!(ambiguous.labels.len(), 3);
    assert!(ambiguous.labels[1].message.contains("init(ref Left)"));
    assert!(ambiguous.labels[2].message.contains("init(ref Right)"));
    let missing = diagnostics
        .iter()
        .find(|diagnostic| diagnostic.code == NO_MATCHING_INITIALIZER)
        .unwrap();
    assert_eq!(missing.labels.len(), 4);
    assert!(missing.labels[0].message.contains("supplied (bool)"));
}

#[test]
fn alias_access_filters_candidates_before_type_specificity() {
    let output = check_text(concat!(
        "class Animal { init() {} }\n",
        "class Dog extends Animal { init() { super(); } }\n",
        "class Choice {\n",
        "  init(ref value: Animal) {}\n",
        "  init(mut ref value: Dog) {}\n",
        "}\n",
        "fn readonly(ref value: Dog) -> Choice { return Choice(value); }\n",
        "fn main() -> i64 {\n",
        "  var dog: Dog = Dog();\n",
        "  var mutable: Choice = Choice(dog);\n",
        "  var produced: Choice = Choice(Dog());\n",
        "  return 0;\n",
        "}\n",
    ));

    assert!(!output.has_errors(), "{:?}", output.diagnostics);
    let hir = output.hir.unwrap();
    let readonly = hir
        .definitions
        .iter()
        .find(|definition| definition.function == FunctionId::new(0))
        .unwrap();
    let HirStatement::Return(result) = &readonly.body.statements[0] else {
        panic!("expected return");
    };
    let Some(crate::hir::HirReturnValue::Object(crate::hir::HirObjectReturn::Construct {
        construction,
        ..
    })) = &result.value
    else {
        panic!("expected returned construction");
    };
    assert_eq!(
        construction.initializer().unwrap(),
        InitializerId::new(ClassId::new(2), 0)
    );

    let main = hir.definitions.get(hir.entry_function).unwrap();
    let HirStatement::Local(local) = &main.body.statements[1] else {
        panic!("expected choice local");
    };
    let HirLocalInitializer::Object(initialization) = &local.initializer else {
        panic!("expected choice construction");
    };
    let crate::hir::HirObjectProducer::Construct(construction) = &initialization.producer else {
        panic!("expected constructor producer");
    };
    assert_eq!(
        construction.initializer().unwrap(),
        InitializerId::new(ClassId::new(2), 1)
    );

    let HirStatement::Local(local) = &main.body.statements[2] else {
        panic!("expected produced choice local");
    };
    let HirLocalInitializer::Object(initialization) = &local.initializer else {
        panic!("expected produced choice construction");
    };
    let crate::hir::HirObjectProducer::Construct(construction) = &initialization.producer else {
        panic!("expected constructor producer");
    };
    assert_eq!(
        construction.initializer().unwrap(),
        InitializerId::new(ClassId::new(2), 0),
        "a produced source must select the read-only alias candidate"
    );
    assert!(matches!(
        construction.arguments().unwrap(),
        [crate::hir::HirCallArgument::View(
            crate::hir::HirObjectView {
                source: crate::hir::HirViewSource::Produced(_),
                access: HirAccess::ReadOnly,
                ..
            }
        )]
    ));
}

#[test]
fn value_copy_applicability_competes_with_alias_views_for_existing_and_produced_sources() {
    let output = check_text(concat!(
        "class Source { init() {} }\n",
        "class Derived extends Source { init() { super(); } }\n",
        "class Choice {\n",
        "  init(ref value: Obj) {}\n",
        "  init(value: Source) {}\n",
        "}\n",
        "fn main() -> i64 {\n",
        "  var source: Derived = Derived();\n",
        "  var existing: Choice = Choice(source);\n",
        "  var produced: Choice = Choice(Derived());\n",
        "  return 0;\n",
        "}\n",
    ));

    assert!(!output.has_errors(), "{:?}", output.diagnostics);
    let hir = output.hir.unwrap();
    let main = hir.definitions.get(hir.entry_function).unwrap();
    let constructions: Vec<_> = main
        .body
        .statements
        .iter()
        .filter_map(|statement| {
            let HirStatement::Local(local) = statement else {
                return None;
            };
            let HirLocalInitializer::Object(initialization) = &local.initializer else {
                return None;
            };
            let crate::hir::HirObjectProducer::Construct(construction) = &initialization.producer
            else {
                return None;
            };
            (construction.class == ClassId::new(2)).then_some(construction)
        })
        .collect();

    assert_eq!(constructions.len(), 2);
    for construction in &constructions {
        assert_eq!(
            construction.initializer().unwrap(),
            InitializerId::new(ClassId::new(2), 1)
        );
        assert!(matches!(
            construction.arguments().unwrap(),
            [crate::hir::HirCallArgument::Copy(_)]
        ));
    }
    let crate::hir::HirCallArgument::Copy(produced) = &constructions[1].arguments().unwrap()[0]
    else {
        unreachable!();
    };
    let crate::hir::HirObjectSource::Slice(slice) = &produced.source else {
        panic!("derived value argument should slice to the selected value parameter type");
    };
    assert!(matches!(
        slice.source.as_ref(),
        crate::hir::HirObjectSource::Produced(_)
    ));
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
