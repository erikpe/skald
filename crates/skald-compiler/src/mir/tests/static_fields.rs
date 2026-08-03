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
        MirType::OptionalPrimitive(MirPrimitiveType::I64)
    );
    assert_eq!(
        state.static_fields[1].ty,
        MirType::OptionalClass(ClassId::new(0))
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
        MirType::OptionalShared(MirSharedTarget::Class(ClassId::new(0)))
    );
    let dump = dump_mir(&program);
    assert!(dump.contains("static(c1:static0)"), "{dump}");
    assert!(
        dump.contains("optional-shared-assign static(c1:static0)"),
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
    mistyped.classes.entries_mut_for_test()[1].static_fields[0].ty =
        MirType::OptionalShared(MirSharedTarget::Obj);
    assert!(verify_mir(&mistyped)
        .unwrap_err()
        .to_string()
        .contains("wrong exact target type"));
}
