use super::*;
use crate::{
    identity::{ArrayTypeId, ClassId, InterfaceId, StaticFieldId, StaticInitializerId},
    mir::PreliminaryMirSharedLifecycleTarget,
    resolve::resolve_module_graph,
    test_support::load_module_sources_with_standard_library,
    typeck::type_check,
};

const STORED_VALUE_MATRIX: &str = concat!(
    "class Item {\n",
    "  value: i64;\n",
    "  init(value: i64) { self.value = value; }\n",
    "  copy(ref other: Item) { self.value = other.value; }\n",
    "  destroy {}\n",
    "}\n",
    "fn make_item(value: i64) -> Item { return Item(value); }\n",
    "class State {\n",
    "  static signed: i64 = 1;\n",
    "  static unsigned: u64 = 2u;\n",
    "  static byte: u8 = 3u8;\n",
    "  static float: f64 = 4.0;\n",
    "  static flag: bool = true;\n",
    "  static direct: Item = Item(5);\n",
    "  static called: Item = make_item(6);\n",
    "  static copied: Item = (Item(7));\n",
    "  static maybe_signed: i64? = 8;\n",
    "  static no_signed: i64? = none;\n",
    "  static maybe_item: Item? = Item(9);\n",
    "  static no_item: Item? = none;\n",
    "  static owner: shared Item = new Item(10);\n",
    "  static maybe_owner: shared? Item = new Item(11);\n",
    "  static no_owner: shared? Item = none;\n",
    "  static values: i64[] = i64[]{12, 13};\n",
    "  static items: Item[] = Item[]{Item(14), Item(15)};\n",
    "  init() {}\n",
    "}\n",
    "fn main() -> i64 { return 0; }\n",
);

fn lower_preliminary(text: &str) -> PreliminaryMirProgram {
    let checked = type_check_source(text);
    assert!(checked.diagnostics.is_empty(), "{:?}", checked.diagnostics);
    lower_preliminary_hir(&checked.hir.unwrap())
}

fn operation_names(initializer: &PreliminaryMirStaticInitializer) -> Vec<&'static str> {
    initializer
        .body
        .blocks
        .iter()
        .flat_map(|block| &block.instructions)
        .filter_map(|instruction| match instruction {
            MirInstruction::Call(_) => Some("call"),
            MirInstruction::Cleanup(_) => Some("cleanup"),
            MirInstruction::Initialize(_) => Some("initialize"),
            MirInstruction::Store(_) => Some("store"),
            MirInstruction::CopyConstruct(_) => Some("copy-construct"),
            MirInstruction::EndFullExpression(_) => Some("end-full-expression"),
            MirInstruction::SharedAllocate(_) => Some("shared-allocate"),
            MirInstruction::SharedInitialize(_) => Some("shared-initialize"),
            MirInstruction::SharedPublish(_) => Some("shared-publish"),
            MirInstruction::SharedAdopt(_) => Some("shared-adopt"),
            MirInstruction::SharedCopy(_) => Some("shared-copy"),
            MirInstruction::SharedFieldCopy(_) => Some("shared-field-copy"),
            MirInstruction::SharedRelease(_) => Some("shared-release"),
            MirInstruction::SharedFieldInitialize(_) => Some("shared-field-initialize"),
            MirInstruction::StringInitialize(_) => Some("string-initialize"),
            MirInstruction::OptionalInitialize(_) => Some("optional-initialize"),
            MirInstruction::ClassOptionalInitialize(_) => Some("class-optional-initialize"),
            MirInstruction::ClassOptionalPublish(_) => Some("class-optional-publish"),
            MirInstruction::OptionalSharedInitialize(_) => Some("optional-shared-initialize"),
            MirInstruction::OptionalSharedCleanup(_) => Some("optional-shared-cleanup"),
            MirInstruction::Array(MirArrayInstruction::Adopt { .. }) => Some("array-adopt"),
            MirInstruction::Array(_) => Some("array-operation"),
            _ => None,
        })
        .collect()
}

fn has_static_destination(instruction: &MirInstruction, field: StaticFieldId) -> bool {
    let is_destination = |place: &MirPlace| {
        place.base == MirPlaceBase::StaticField(field) && place.projections.is_empty()
    };
    match instruction {
        MirInstruction::Store(operation) => is_destination(&operation.destination),
        MirInstruction::Call(operation) => {
            operation.destination.as_ref().is_some_and(is_destination)
        }
        MirInstruction::Initialize(operation) => is_destination(&operation.destination),
        MirInstruction::CopyConstruct(operation) => is_destination(&operation.destination),
        MirInstruction::OptionalInitialize(operation) => is_destination(&operation.destination),
        MirInstruction::ClassOptionalInitialize(operation) => {
            is_destination(&operation.destination)
        }
        MirInstruction::ClassOptionalPublish(operation) => is_destination(&operation.destination),
        MirInstruction::OptionalSharedInitialize(operation) => {
            is_destination(&operation.destination)
        }
        MirInstruction::SharedFieldInitialize(operation) => is_destination(&operation.destination),
        MirInstruction::Array(MirArrayInstruction::Adopt { destination, .. }) => {
            is_destination(destination)
        }
        _ => false,
    }
}

#[test]
fn lowers_and_verifies_the_complete_stored_value_matrix() {
    let preliminary = lower_preliminary(STORED_VALUE_MATRIX);
    verify_preliminary_mir(&preliminary).unwrap();

    let initializers = preliminary.static_initializers().collect::<Vec<_>>();
    assert_eq!(initializers.len(), 17);
    assert_eq!(preliminary.static_fields().count(), 17);
    for (index, initializer) in initializers.iter().enumerate() {
        let field = StaticFieldId::new(ClassId::new(1), index);
        assert_eq!(initializer.id, StaticInitializerId::from(field));
        assert_eq!(initializer.field, field);
        assert!(initializer
            .block(initializer.publication.initialization_exit)
            .unwrap()
            .instructions
            .iter()
            .any(|instruction| has_static_destination(instruction, field)));
        assert!(matches!(
            initializer
                .block(initializer.publication.initialization_exit)
                .unwrap()
                .terminator,
            Some(MirTerminator::Goto { target, .. })
                if target == initializer.publication.cleanup_entry
        ));
    }

    let operations = initializers
        .iter()
        .map(|initializer| operation_names(initializer))
        .collect::<Vec<_>>();
    assert_eq!(
        operations,
        vec![
            vec!["store"],
            vec!["store"],
            vec!["store"],
            vec!["store"],
            vec!["store"],
            vec!["initialize"],
            vec!["call"],
            vec!["initialize", "copy-construct", "end-full-expression"],
            vec!["optional-initialize"],
            vec!["optional-initialize"],
            vec![
                "class-optional-initialize",
                "initialize",
                "class-optional-publish",
            ],
            vec!["class-optional-initialize"],
            vec![
                "shared-allocate",
                "shared-initialize",
                "shared-publish",
                "shared-adopt",
                "shared-field-initialize",
                "end-full-expression",
            ],
            vec![
                "shared-allocate",
                "shared-initialize",
                "shared-publish",
                "shared-adopt",
                "optional-shared-initialize",
                "end-full-expression",
            ],
            vec!["optional-shared-initialize"],
            vec![
                "array-operation",
                "array-operation",
                "array-operation",
                "array-operation",
                "array-adopt",
                "end-full-expression",
            ],
            vec![
                "array-operation",
                "initialize",
                "array-operation",
                "initialize",
                "array-operation",
                "array-operation",
                "array-adopt",
                "end-full-expression",
            ],
        ]
    );

    let dump = dump_preliminary_mir(&preliminary);
    assert_eq!(
        dump.matches("StaticInitializer c1:static").count(),
        17,
        "{dump}"
    );
    assert_eq!(dump.matches("Publication c1:static").count(), 17, "{dump}");
    assert!(dump.contains("DestructionPlan"), "{dump}");
    assert!(dump.contains("ArrayTypes"), "{dump}");
}

#[test]
fn lowers_named_static_sources_through_selected_copy_operations() {
    let preliminary = lower_preliminary(concat!(
        "class Item { value: i64; init(value: i64) { self.value = value; } ",
        "copy(ref other: Item) { self.value = other.value; } destroy {} }\n",
        "class State {\n",
        "  static owner: shared Item = new Item(1);\n",
        "  static owner_copy: shared Item = State.owner;\n",
        "  static maybe_owner: shared? Item = new Item(2);\n",
        "  static maybe_owner_copy: shared? Item = State.maybe_owner;\n",
        "  static values: i64[] = i64[]{3};\n",
        "  static values_copy: i64[] = State.values;\n",
        "  static maybe_number: i64? = 4;\n",
        "  static maybe_number_copy: i64? = State.maybe_number;\n",
        "  static maybe_item: Item? = Item(5);\n",
        "  static maybe_item_copy: Item? = State.maybe_item;\n",
        "  static item: Item = Item(6);\n",
        "  static item_copy: Item = State.item;\n",
        "  init() {}\n",
        "}\n",
        "fn main() -> i64 { return 0; }\n",
    ));
    verify_preliminary_mir(&preliminary).unwrap();
    let initializers = preliminary.static_initializers().collect::<Vec<_>>();

    assert!(operation_names(initializers[1]).contains(&"shared-field-copy"));
    assert!(operation_names(initializers[3]).contains(&"optional-shared-initialize"));
    assert!(operation_names(initializers[5]).contains(&"array-adopt"));
    assert!(operation_names(initializers[7]).contains(&"optional-initialize"));
    assert!(operation_names(initializers[9]).contains(&"class-optional-initialize"));
    assert!(operation_names(initializers[11]).contains(&"copy-construct"));

    let dump = dump_preliminary_mir(&preliminary);
    assert!(dump.contains("static(c1:static0)"), "{dump}");
    assert!(dump.contains("static(c1:static2)"), "{dump}");
    assert!(dump.contains("static(c1:static4)"), "{dump}");
    assert!(dump.contains("static(c1:static6)"), "{dump}");
    assert!(dump.contains("static(c1:static8)"), "{dump}");
    assert!(dump.contains("static(c1:static10)"), "{dump}");
}

#[test]
fn cleanup_of_initializer_temporaries_starts_after_publication() {
    let preliminary = lower_preliminary(concat!(
        "class Item { init() {} copy(ref other: Item) {} destroy {} }\n",
        "class State { static item: Item = (Item()); init() {} }\n",
        "fn main() -> i64 { return 0; }\n",
    ));
    let initializer = preliminary.static_initializers().next().unwrap();
    let initialization = initializer
        .block(initializer.publication.initialization_exit)
        .unwrap();
    let cleanup = initializer
        .block(initializer.publication.cleanup_entry)
        .unwrap();

    let copy_index = initialization
        .instructions
        .iter()
        .position(|instruction| {
            matches!(instruction, MirInstruction::CopyConstruct(operation)
            if operation.destination == MirPlace::static_field(initializer.field))
        })
        .unwrap();
    assert!(copy_index > 0);
    assert!(!initialization
        .instructions
        .iter()
        .any(|instruction| matches!(instruction, MirInstruction::EndFullExpression(_))));
    assert!(cleanup.instructions.iter().any(
        |instruction| matches!(instruction, MirInstruction::EndFullExpression(end)
            if !end.temporaries.is_empty())
    ));
    assert!(!cleanup
        .instructions
        .iter()
        .any(|instruction| has_static_destination(instruction, initializer.field)));
    verify_preliminary_mir(&preliminary).unwrap();
}

#[test]
fn lowers_string_static_initialization_with_ordinary_temporary_cleanup() {
    let (_workspace, graph) = load_module_sources_with_standard_library(
        "app",
        &[(
            "app.ska",
            concat!(
                "from std::str import Str;\n",
                "class State { static text: Str = \"ready\"; init() {} }\n",
                "fn main() -> i64 { return 0; }\n",
            ),
        )],
    );
    let resolved = resolve_module_graph(&graph);
    assert!(!resolved.has_errors(), "{:?}", resolved.diagnostics);
    let checked = type_check(&resolved.program);
    assert!(checked.diagnostics.is_empty(), "{:?}", checked.diagnostics);
    let preliminary = lower_preliminary_hir(&checked.hir.unwrap());
    verify_preliminary_mir(&preliminary).unwrap();

    let initializer = preliminary.static_initializers().next().unwrap();
    let operations = operation_names(initializer);
    assert_eq!(
        operations,
        ["string-initialize", "copy-construct", "end-full-expression"]
    );
    assert!(initializer
        .body
        .blocks
        .iter()
        .flat_map(|block| &block.instructions)
        .any(|instruction| matches!(instruction, MirInstruction::StringInitialize(_))));
    assert!(
        operations.contains(&"end-full-expression"),
        "{operations:?}"
    );

    let dump = dump_preliminary_mir(&preliminary);
    assert!(dump.contains("string-initialize"), "{dump}");
    assert!(dump.contains("copy-construct static("), "{dump}");
}

#[test]
fn retains_implicit_shared_release_in_transitively_called_ordinary_body() {
    let preliminary = lower_preliminary(concat!(
        "class Item { init() {} destroy {} }\n",
        "fn consume(owner: shared Item) -> i64 { return 1; }\n",
        "class State { static count: i64 = consume(new Item()); init() {} }\n",
        "fn main() -> i64 { return 0; }\n",
    ));
    verify_preliminary_mir(&preliminary).unwrap();

    let initializer = preliminary.static_initializers().next().unwrap();
    assert!(operation_names(initializer).contains(&"call"));
    assert!(preliminary
        .executable_definitions()
        .filter(|definition| !matches!(definition, MirDefinitionRef::StaticInitializer(_)))
        .flat_map(|definition| &definition.body().blocks)
        .flat_map(|block| &block.instructions)
        .any(|instruction| matches!(instruction, MirInstruction::SharedRelease(_))));
}

#[test]
fn retains_closed_world_dispatch_destruction_and_array_lifecycle_metadata() {
    let preliminary = lower_preliminary(concat!(
        "interface View { fn read() -> i64; }\n",
        "class Base implements View { init() {} virtual fn read() -> i64 { return 1; } destroy {} }\n",
        "class Child extends Base { init() { super(); } override fn read() -> i64 { return 2; } destroy {} }\n",
        "class Other { init() {} destroy {} }\n",
        "class State { static owner: shared View = new Child(); static values: Child[] = Child[]{Child()}; init() {} }\n",
        "fn invoke(ref value: View) -> i64 { return value.read(); }\n",
        "fn main() -> i64 { return 0; }\n",
    ));
    verify_preliminary_mir(&preliminary).unwrap();

    let dump = dump_preliminary_mir(&preliminary);
    assert!(dump.contains("VirtualFamilies"), "{dump}");
    assert!(dump.contains("Conformance i0"), "{dump}");
    assert!(dump.matches("DestructionPlan").count() >= 3, "{dump}");
    assert!(dump.contains("ArrayTypes"), "{dump}");
    assert_eq!(
        preliminary.executable_definitions().count(),
        preliminary.program().executable_definitions().count() + 2
    );

    assert_eq!(
        preliminary.shared_lifecycle_targets(MirSharedTarget::Class(ClassId::new(0))),
        vec![
            PreliminaryMirSharedLifecycleTarget::Class(ClassId::new(0)),
            PreliminaryMirSharedLifecycleTarget::Class(ClassId::new(1)),
        ]
    );
    assert_eq!(
        preliminary.shared_lifecycle_targets(MirSharedTarget::Interface(InterfaceId::new(0))),
        vec![
            PreliminaryMirSharedLifecycleTarget::Class(ClassId::new(0)),
            PreliminaryMirSharedLifecycleTarget::Class(ClassId::new(1)),
        ]
    );
    assert_eq!(
        preliminary.shared_lifecycle_targets(MirSharedTarget::Array(ArrayTypeId::new(0))),
        vec![PreliminaryMirSharedLifecycleTarget::Array(
            ArrayTypeId::new(0)
        )]
    );
    assert_eq!(
        preliminary
            .shared_lifecycle_targets(MirSharedTarget::Obj)
            .len(),
        4
    );
    assert!(preliminary.class(ClassId::new(1)).is_some());
    assert!(preliminary.array_type(ArrayTypeId::new(0)).is_some());
}

#[test]
fn rejects_malformed_preliminary_products() {
    let valid = lower_preliminary(
        "class State { static value: i64 = 1; init() {} } fn main() -> i64 { return 0; }",
    );

    let mut missing_body = valid.clone();
    missing_body.static_initializers_mut_for_test().clear();
    assert_verification_contains(
        &missing_body,
        "explicit static field has no initializer body",
    );

    let mut unexpected_body = valid.clone();
    unexpected_body.static_fields_mut_for_test()[0].initializer = None;
    assert_verification_contains(
        &unexpected_body,
        "initializer body has no explicit static field",
    );

    let mut wrong_type = valid.clone();
    wrong_type.static_initializers_mut_for_test()[0].destination_type = MirType::Bool;
    assert_verification_contains(
        &wrong_type,
        "static initializer destination type differs from its field",
    );

    let mut no_completion = valid.clone();
    no_completion.static_initializers_mut_for_test()[0]
        .body
        .blocks[0]
        .instructions
        .retain(|instruction| !matches!(instruction, MirInstruction::Store(_)));
    assert_verification_contains(
        &no_completion,
        "static initializer does not complete its destination on every publication path",
    );

    let mut invalid_publication = valid.clone();
    let cleanup = invalid_publication.static_initializers_mut_for_test()[0]
        .publication
        .cleanup_entry;
    invalid_publication.static_initializers_mut_for_test()[0]
        .publication
        .initialization_exit = cleanup;
    assert_verification_contains(
        &invalid_publication,
        "static publication must be one direct edge to cleanup",
    );

    let mut early_return = valid.clone();
    let span = early_return.static_initializers_mut_for_test()[0].span;
    early_return.static_initializers_mut_for_test()[0]
        .body
        .blocks[0]
        .terminator = Some(MirTerminator::Return { value: None, span });
    assert_verification_contains(
        &early_return,
        "static initializer returns before publication",
    );
}

fn assert_verification_contains(program: &PreliminaryMirProgram, expected: &str) {
    let errors = verify_preliminary_mir(program).unwrap_err().to_string();
    assert!(
        errors.contains(expected),
        "expected `{expected}` in:\n{errors}"
    );
}

#[test]
fn only_lifecycle_free_preliminary_mir_converts_to_backend_input() {
    let explicit = lower_preliminary(
        "class State { static value: i64 = 1; init() {} } fn main() -> i64 { return 0; }",
    );
    assert!(explicit.try_into_final().is_err());

    let zero_default = lower_preliminary(
        "class State { static value: i64; init() {} } fn main() -> i64 { return 0; }",
    );
    let final_program = zero_default.try_into_final().unwrap();
    verify_mir(&final_program).unwrap();
}
