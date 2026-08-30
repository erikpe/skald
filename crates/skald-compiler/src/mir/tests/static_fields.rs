use super::*;
use crate::identity::StaticFieldId;

const SOURCE: &str = concat!(
    "fn increment(mut ref value: i64) -> unit { value = value + 1; }\n",
    "class State {\n",
    "  static count: i64; static byte: u8; static ratio: f64; static ready: bool;\n",
    "  init() {}\n",
    "}\n",
    "fn main() -> i64 { State.count = 40; increment(State.count); return State.count + 1; }\n",
);

#[test]
fn lowers_dense_declarations_and_identity_based_always_live_places() {
    let program = lower_text(SOURCE);
    verify_mir(&program).unwrap();
    let class = program.class(ClassId::new(0)).unwrap();
    assert_eq!(class.static_fields.len(), 4);
    assert_eq!(class.static_fields[0].ty, MirType::I64);
    assert_eq!(class.static_fields[2].ty, MirType::F64);

    let main = program.definitions.get(program.entry_function).unwrap();
    let static_roots = main
        .body
        .blocks
        .iter()
        .flat_map(|block| &block.instructions)
        .filter_map(|instruction| match instruction {
            MirInstruction::Store(store) => match store.destination.base {
                MirPlaceBase::StaticField(field) => Some(field),
                _ => None,
            },
            MirInstruction::Assign(assignment) => match &assignment.rvalue.kind {
                MirRvalueKind::Load(place) => match place.base {
                    MirPlaceBase::StaticField(field) => Some(field),
                    _ => None,
                },
                _ => None,
            },
            _ => None,
        })
        .collect::<Vec<_>>();
    assert!(static_roots
        .iter()
        .all(|field| *field == StaticFieldId::new(class.id, 0)));
    assert!(static_roots.len() >= 2);
    assert!(MirPlaceBase::StaticField(StaticFieldId::new(class.id, 0))
        .local_storage()
        .is_none());
    assert!(dump_mir(&program).contains("StaticField c0:static0 \"count\" : i64"));
    assert!(dump_mir(&program).contains("static(c0:static0)"));
}

#[test]
fn exact_class_static_replacement_lowers_to_verified_copy_assignment() {
    let program = lower_source_to_final_mir(concat!(
        "class Item { value: i64; init(value: i64) { self.value = value; } ",
        "  assign(ref other: Item) { self.value = other.value; } destroy {} }\n",
        "class State { static item: Item = Item(1); init() {} }\n",
        "fn main() -> i64 { var replacement: Item = Item(2); ",
        "  State.item = replacement; State.item = Item(3); State.item = State.item; ",
        "  return State.item.value; }\n",
    ));

    verify_mir(&program).unwrap();
    let main = program.definitions.get(program.entry_function).unwrap();
    let assignments = main
        .body
        .blocks
        .iter()
        .flat_map(|block| &block.instructions)
        .filter_map(|instruction| match instruction {
            MirInstruction::CopyAssign(assignment)
                if matches!(assignment.destination.base, MirPlaceBase::StaticField(_)) =>
            {
                Some(assignment)
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(assignments.len(), 3);
    assert!(assignments.iter().all(|assignment| {
        assignment.destination == MirPlace::static_field(StaticFieldId::new(ClassId::new(1), 0))
            && assignment.class == ClassId::new(0)
            && assignment.operation
                == MirSelectedCopyOperation::User(crate::identity::CopyAssignmentId::new(
                    ClassId::new(0),
                    0,
                ))
    }));

    let dump = dump_mir(&program);
    assert_eq!(
        dump.matches("copy-assign static(c1:static0)").count(),
        3,
        "{dump}"
    );
}

#[test]
fn rejects_missing_mismatched_and_projected_static_roots() {
    let mut missing = lower_text(SOURCE);
    let main = missing
        .definitions
        .get_mut_for_test(missing.entry_function)
        .unwrap();
    let store = main.body.blocks[0]
        .instructions
        .iter_mut()
        .find_map(|instruction| match instruction {
            MirInstruction::Store(store)
                if matches!(store.destination.base, MirPlaceBase::StaticField(_)) =>
            {
                Some(store)
            }
            _ => None,
        })
        .unwrap();
    store.destination.base = MirPlaceBase::StaticField(StaticFieldId::new(ClassId::new(0), 99));
    assert!(verify_mir(&missing)
        .unwrap_err()
        .to_string()
        .contains("is not declared"));

    let mut projected = lower_text(SOURCE);
    let main = projected
        .definitions
        .get_mut_for_test(projected.entry_function)
        .unwrap();
    let store = main.body.blocks[0]
        .instructions
        .iter_mut()
        .find_map(|instruction| match instruction {
            MirInstruction::Store(store)
                if matches!(store.destination.base, MirPlaceBase::StaticField(_)) =>
            {
                Some(store)
            }
            _ => None,
        })
        .unwrap();
    store
        .destination
        .projections
        .push(MirPlaceProjection::Base(ClassId::new(0)));
    assert!(verify_mir(&projected)
        .unwrap_err()
        .to_string()
        .contains("has a non-class base"));

    let mut malformed = lower_text(SOURCE);
    malformed.classes.entries_mut_for_test()[0].static_fields[0].ty =
        MirType::Class(ClassId::new(0));
    assert!(verify_mir(&malformed)
        .unwrap_err()
        .to_string()
        .contains("unsupported MIR type"));

    let mut duplicate = lower_text(SOURCE);
    duplicate.classes.entries_mut_for_test()[0].static_fields[1].id =
        StaticFieldId::new(ClassId::new(0), 0);
    assert!(verify_mir(&duplicate)
        .unwrap_err()
        .to_string()
        .contains("static-field table index 1"));
}

#[test]
fn inline_optional_statics_are_always_live_typed_roots() {
    let program = lower_text(concat!(
        "class Item { value: i64; init(value: i64) { self.value = value; } }\n",
        "class State { static count: i64?; static item: Item?; init() {} }\n",
        "fn main() -> i64 {\n",
        "  var local: i64? = none;\n",
        "  if (State.count is some || State.item is some) { return 1; }\n",
        "  State.count = 41; State.item = Item(1);\n",
        "  if (State.item is some) { return State.count! + State.item!.value; }\n",
        "  return 0;\n",
        "}\n",
    ));

    verify_mir(&program).unwrap();
    let state = program.class(ClassId::new(1)).unwrap();
    assert_eq!(
        state.static_fields[0].ty,
        MirType::Optional(program.optional_for_payload(MirType::I64).unwrap())
    );
    assert_eq!(
        state.static_fields[1].ty,
        MirType::Optional(
            program
                .optional_for_payload(MirType::Class(ClassId::new(0)))
                .unwrap()
        )
    );

    let dump = dump_mir(&program);
    assert!(dump.contains("static(c1:static0)"), "{dump}");
    assert!(
        dump.contains("static(c1:static1).optional-payload(c0)"),
        "{dump}"
    );
    assert!(dump.contains("storage-live"), "{dump}");
    assert!(
        !dump.contains("OptionalInitialize static(c1:static0)"),
        "{dump}"
    );
    assert!(
        !dump.contains("ClassOptionalInitialize static(c1:static1)"),
        "{dump}"
    );
}

#[test]
fn verifier_rejects_malformed_static_optional_projection_types() {
    let mut program = lower_text(concat!(
        "class Item { value: i64; init(value: i64) { self.value = value; } }\n",
        "class State { static item: Item?; init() {} }\n",
        "fn main() -> i64 { State.item = Item(42); return State.item!.value; }\n",
    ));
    let main = program
        .definitions
        .get_mut_for_test(program.entry_function)
        .unwrap();
    let projected =
        main.body
            .blocks
            .iter_mut()
            .flat_map(|block| &mut block.instructions)
            .find_map(|instruction| match instruction {
                MirInstruction::Assign(assignment)
                    if matches!(assignment.rvalue.kind, MirRvalueKind::Load(_)) =>
                {
                    let MirRvalueKind::Load(place) = &mut assignment.rvalue.kind else {
                        unreachable!()
                    };
                    place.projections.iter_mut().find(|projection| {
                        matches!(projection, MirPlaceProjection::OptionalPayload(_))
                    })
                }
                _ => None,
            })
            .expect("fixture must load the static optional payload");
    *projected = MirPlaceProjection::OptionalPayload(ClassId::new(1));

    assert!(verify_mir(&program)
        .unwrap_err()
        .to_string()
        .contains("incompatible base type"));
}

#[test]
fn optional_shared_statics_are_initialized_program_owned_containers() {
    let program = lower_text(concat!(
        "class Item { init() {} }\n",
        "class State { static owner: shared? Item; init() {} }\n",
        "fn main() -> i64 {\n",
        "  var local: i64 = 0;\n",
        "  if (State.owner is some) { return 1; }\n",
        "  State.owner = new Item();\n",
        "  State.owner = State.owner;\n",
        "  return 42;\n",
        "}\n",
    ));

    verify_mir(&program).unwrap();
    let field = &program.class(ClassId::new(1)).unwrap().static_fields[0];
    assert_eq!(
        field.ty,
        MirType::Optional(
            program
                .optional_for_payload(MirType::Shared(MirSharedTarget::Class(ClassId::new(0))))
                .unwrap()
        )
    );
    let dump = dump_mir(&program);
    assert!(dump.contains("static(c1:static0)"), "{dump}");
    assert!(
        dump.contains("optional-shared-assign o0 static(c1:static0)"),
        "{dump}"
    );
    assert!(
        !dump.contains("optional-shared-cleanup static(c1:static0)"),
        "{dump}"
    );
    assert!(dump.contains("storage-live"), "{dump}");
}

#[test]
fn verifier_rejects_nonoptional_and_mistyped_shared_static_roots() {
    let source = concat!(
        "class Item { init() {} }\n",
        "class State { static owner: shared? Item; init() {} }\n",
        "fn main() -> i64 { State.owner = new Item(); return 0; }\n",
    );

    let mut nonoptional = lower_text(source);
    nonoptional.classes.entries_mut_for_test()[1].static_fields[0].ty =
        MirType::Shared(MirSharedTarget::Class(ClassId::new(0)));
    assert!(verify_mir(&nonoptional)
        .unwrap_err()
        .to_string()
        .contains("unsupported MIR type"));

    let mut mistyped = lower_text(source);
    let optional = mistyped
        .optional_for_payload(MirType::Shared(MirSharedTarget::Class(ClassId::new(0))))
        .unwrap();
    mistyped.optional_types.entries_mut_for_test()[optional.index()].payload =
        MirType::Shared(MirSharedTarget::Obj);
    mistyped.optional_types.entries_mut_for_test()[optional.index()].storage =
        crate::mir::MirOptionalStorage::SharedOwner(MirSharedTarget::Obj);
    assert!(verify_mir(&mistyped)
        .unwrap_err()
        .to_string()
        .contains("wrong exact target type"));
}

#[test]
fn inline_array_statics_lower_as_anchored_always_live_roots() {
    let source = concat!(
        "fn inspect(ref values: i64[]) -> u64 { return values.len(); }\n",
        "class State { static values: i64[]; static nested: i64[][]; init() {} }\n",
        "fn main() -> i64 {\n",
        "  State.values = i64[](2u); State.values[0] = 42;\n",
        "  var slice: i64[] = State.values[:];\n",
        "  return State.values[0] + (i64) inspect(State.values) - (i64) slice.len();\n",
        "}\n",
    );
    let program = lower_text(source);

    verify_mir(&program).unwrap();
    let field = &program.class(ClassId::new(0)).unwrap().static_fields[0];
    let MirType::Array(array) = field.ty else {
        panic!("static array declaration must retain its exact array identity")
    };
    let dump = dump_mir(&program);
    assert!(dump.contains("array-replace static(c0:static0)"), "{dump}");
    assert!(dump.contains("base: StaticField(StaticFieldId"), "{dump}");
    assert!(dump.contains("InlineBacking"), "{dump}");
    assert!(!dump.contains("array-release static(c0:static0)"), "{dump}");

    let mut malformed = program.clone();
    malformed.classes.entries_mut_for_test()[0].static_fields[0].ty =
        MirType::Array(crate::identity::ArrayTypeId::new(array.index() + 99));
    assert!(verify_mir(&malformed)
        .unwrap_err()
        .to_string()
        .contains("undeclared array type"));
}
