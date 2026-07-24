use super::*;
use crate::hir::{
    HirCheckedObjectViewKind, HirConstructionMode, HirObjectProducer, HirObjectSource,
};
use crate::typeck::{COPY_OPERATION_UNAVAILABLE, INVALID_COPY_CONSTRUCTION};

fn constructed_local(
    definition: &crate::hir::HirFunctionDefinition,
    statement: usize,
) -> (
    &crate::hir::HirObjectInitialization,
    &crate::hir::HirConstruction,
) {
    let HirStatement::Local(local) = &definition.body.statements[statement] else {
        panic!("expected local declaration");
    };
    let HirLocalInitializer::Object(initialization) = &local.initializer else {
        panic!("expected direct object construction");
    };
    let HirObjectProducer::Construct(construction) = &initialization.producer else {
        panic!("expected construction producer");
    };
    (initialization, construction)
}

#[test]
fn explicit_copy_selects_one_copy_operation_and_is_not_elided() {
    let output = check_text(concat!(
        "class Value {\n",
        "  value: i64;\n",
        "  init(value: i64) { self.value = value; }\n",
        "  copy(ref source: Value) { self.value = source.value + 1; }\n",
        "}\n",
        "fn main() -> i64 {\n",
        "  var source: Value = Value(40);\n",
        "  var result: Value = Value(copy source);\n",
        "  return result.value;\n",
        "}\n",
    ));

    assert!(!output.has_errors(), "{:?}", output.diagnostics);
    let hir = output.hir.unwrap();
    let main = hir.definitions.get(hir.entry_function).unwrap();
    let (initialization, construction) = constructed_local(main, 1);
    assert!(initialization.elided_copy.is_none());
    let HirConstructionMode::Copy { source, operation } = &construction.mode else {
        panic!("expected explicit copy construction");
    };
    assert_eq!(
        *operation,
        hir.class(ClassId::new(0))
            .unwrap()
            .copy_constructor
            .selected()
            .unwrap()
    );
    let HirObjectSource::Checked(source) = source.as_ref() else {
        panic!("target-directed copy must retain its checked source");
    };
    assert_eq!(source.kind, HirCheckedObjectViewKind::Static);

    let mir = lower_hir(&hir).unwrap();
    let main = mir.definitions.get(hir.entry_function).unwrap();
    let copy_count = main
        .body
        .blocks
        .iter()
        .flat_map(|block| &block.instructions)
        .filter(|instruction| matches!(instruction, MirInstruction::CopyConstruct(_)))
        .count();
    assert_eq!(copy_count, 1, "explicit construction invokes one copy");
    let dump = dump_hir(&hir);
    assert!(dump.contains("ExplicitCopyConstruct c0"));
}

#[test]
fn target_directed_copy_supports_static_slicing_and_dynamic_downcasts() {
    let output = check_text(concat!(
        "class Animal { init() {} }\n",
        "class Dog extends Animal { init() { super(); } }\n",
        "fn from_dog(ref dog: Dog) -> i64 {\n",
        "  var animal: Animal = Animal(copy dog);\n",
        "  return 0;\n",
        "}\n",
        "fn from_animal(ref animal: Animal) -> i64 {\n",
        "  var dog: Dog = Dog(copy animal);\n",
        "  return 0;\n",
        "}\n",
        "fn main() -> i64 { return 0; }\n",
    ));

    assert!(!output.has_errors(), "{:?}", output.diagnostics);
    let hir = output.hir.unwrap();
    let from_dog = hir.definitions.get(FunctionId::new(0)).unwrap();
    let (_, construction) = constructed_local(from_dog, 0);
    let HirConstructionMode::Copy { source, .. } = &construction.mode else {
        panic!("expected copy construction");
    };
    let HirObjectSource::Checked(view) = source.as_ref() else {
        panic!("expected checked class view");
    };
    assert_eq!(view.kind, HirCheckedObjectViewKind::Static);
    assert_eq!(view.class, Some(ClassId::new(0)));

    let from_animal = hir.definitions.get(FunctionId::new(1)).unwrap();
    let (_, construction) = constructed_local(from_animal, 0);
    let HirConstructionMode::Copy { source, .. } = &construction.mode else {
        panic!("expected copy construction");
    };
    let HirObjectSource::Checked(view) = source.as_ref() else {
        panic!("expected checked class view");
    };
    assert_eq!(view.kind, HirCheckedObjectViewKind::RuntimeTerminate);
    assert_eq!(view.class, Some(ClassId::new(1)));

    let mut mir = lower_hir(&hir).expect("dynamic copy construction must lower");
    assert!(verify_mir(&mir).is_ok());
    let body = &mir.definitions.get(FunctionId::new(1)).unwrap().body;
    assert!(body.blocks.iter().any(|block| matches!(
        block.terminator,
        Some(crate::mir::MirTerminator::CheckedCast { .. })
    )));
    assert!(body.blocks.iter().any(|block| matches!(
        block.terminator,
        Some(crate::mir::MirTerminator::Terminate {
            reason: crate::mir::MirTerminationReason::ObjectCastFailure,
            ..
        })
    )));

    let definition = mir
        .definitions
        .get_mut_for_test(FunctionId::new(1))
        .unwrap();
    let copy = definition
        .body
        .blocks
        .iter_mut()
        .flat_map(|block| &mut block.instructions)
        .find_map(|instruction| match instruction {
            MirInstruction::CopyConstruct(copy) => Some(copy),
            _ => None,
        })
        .expect("explicit copy must lower to a copy instruction");
    copy.operation = crate::mir::MirSelectedCopyOperation::Synthesized(ClassId::new(0));
    assert!(
        verify_mir(&mir).is_err(),
        "the verifier must reject a corrupted explicit copy selection"
    );
}

#[test]
fn explicit_inner_cast_can_refine_a_copy_source() {
    let output = check_text(concat!(
        "class Animal { init() {} }\n",
        "class Dog extends Animal { init() { super(); } }\n",
        "fn from_animal(ref animal: Animal) -> i64 {\n",
        "  var dog: Dog = Dog(copy (Dog) animal);\n",
        "  return 0;\n",
        "}\n",
        "fn main() -> i64 { return 0; }\n",
    ));

    assert!(!output.has_errors(), "{:?}", output.diagnostics);
    let hir = output.hir.unwrap();
    let function = hir.definitions.get(FunctionId::new(0)).unwrap();
    let (_, construction) = constructed_local(function, 0);
    let HirConstructionMode::Copy { source, .. } = &construction.mode else {
        panic!("expected explicit copy mode");
    };
    let HirObjectSource::Checked(view) = source.as_ref() else {
        panic!("inner cast must remain a checked source");
    };
    assert_eq!(view.kind, HirCheckedObjectViewKind::RuntimeTerminate);
}

#[test]
fn rejects_statically_impossible_and_unavailable_explicit_copies() {
    let impossible = check_text(concat!(
        "class Animal { init() {} }\n",
        "class Dog extends Animal { init() { super(); } }\n",
        "fn main() -> i64 {\n",
        "  var animal: Animal = Animal();\n",
        "  var dog: Dog = Dog(copy animal);\n",
        "  return 0;\n",
        "}\n",
    ));
    assert!(impossible.hir.is_none());
    assert!(impossible
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == INVALID_COPY_CONSTRUCTION));

    let mut resolved = resolve_text(concat!(
        "class Locked {\n",
        "  init() {}\n",
        "}\n",
        "fn main() -> i64 {\n",
        "  var source: Locked = Locked();\n",
        "  var result: Locked = Locked(copy source);\n",
        "  return 0;\n",
        "}\n",
    ));
    resolved.classes.entries_mut_for_test()[0].copy_constructor =
        crate::resolve::ResolvedCopyOperation::Unavailable;
    let unavailable = crate::typeck::type_check(&resolved);
    assert!(unavailable.hir.is_none());
    assert!(unavailable
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == COPY_OPERATION_UNAVAILABLE));
}
