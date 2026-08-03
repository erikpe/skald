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
        .contains("cannot have projections"));

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
