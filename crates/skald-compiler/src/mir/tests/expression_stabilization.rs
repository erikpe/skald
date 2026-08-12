use super::*;

#[test]
fn scalar_values_survive_multiple_checked_subexpressions_without_arrays() {
    let mir = lower_text(concat!(
        "fn inspect(value: i64) -> i64 { return value; }\n",
        "fn main() -> i64 {\n",
        "  var boxed: shared? i64? = new i64?(40);\n",
        "  var addend: i64? = 2;\n",
        "  return (*(boxed!))! + inspect(addend!);\n",
        "}\n",
    ));
    verify_mir(&mir).expect("checked subexpressions must produce dominating scalar values");

    let function = mir.definitions.get(mir.entry_function).unwrap();
    let spill = function
        .storage
        .iter()
        .find(|storage| storage.kind == MirStorageKind::ScalarSpill)
        .expect("the left result must have stable storage before the later checked argument");
    assert_eq!(spill.ty, MirType::I64);

    let dump = dump_mir(&mir);
    let store = dump
        .find(&format!("store {}", spill.id))
        .expect("the left result must be stored before lowering the right call");
    let right_unwrap = dump
        .rfind("optional-unwrap")
        .expect("the right call argument must retain its checked unwrap");
    let reload = dump
        .rfind(&format!("load {}", spill.id))
        .expect("the left result must be reloaded after the right call");
    let addition = dump
        .rfind("add.i64")
        .expect("the continuation must combine both results");

    assert!(store < right_unwrap);
    assert!(right_unwrap < reload);
    assert!(reload < addition);
    assert_eq!(dump, dump_mir(&mir));
}

#[test]
fn scalar_values_survive_checked_object_array_copy_arguments() {
    let mir = lower_text(concat!(
        "class Item {\n",
        "  value: i64;\n",
        "  init(value: i64) { self.value = value; }\n",
        "}\n",
        "fn inspect(prefix: i64, value: Item) -> i64 {\n",
        "  return prefix + value.value;\n",
        "}\n",
        "fn main() -> i64 {\n",
        "  var values: Item[] = Item[]{Item(3)};\n",
        "  return 1 + inspect(2, values[0]);\n",
        "}\n",
    ));
    verify_mir(&mir).expect("checked object-array arguments must stabilize earlier scalars");

    let function = mir.definitions.get(mir.entry_function).unwrap();
    assert_eq!(
        function
            .storage
            .iter()
            .filter(|storage| storage.kind == MirStorageKind::ScalarSpill)
            .count(),
        2,
        "both the enclosing left operand and the earlier scalar argument need stable homes"
    );
}

#[test]
fn scalar_values_survive_named_array_argument_copy_loops() {
    let mir = lower_text(concat!(
        "fn inspect(prefix: i64, values: i64[]) -> i64 {\n",
        "  return prefix + values[0];\n",
        "}\n",
        "fn main() -> i64 {\n",
        "  var values: i64[] = i64[]{39};\n",
        "  return 1 + inspect(2, values);\n",
        "}\n",
    ));
    verify_mir(&mir).expect("named array argument copy loops must stabilize earlier scalars");

    let function = mir.definitions.get(mir.entry_function).unwrap();
    assert_eq!(
        function
            .storage
            .iter()
            .filter(|storage| {
                storage.kind == MirStorageKind::ScalarSpill && storage.name.starts_with("spill")
            })
            .count(),
        2,
        "both the enclosing left operand and the earlier scalar argument need stable homes"
    );
}
