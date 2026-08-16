use super::*;
use crate::{backend::x86_64_sysv::layout::DataLayout, driver::compile_source_to_assembly};

#[test]
fn final_markers_do_not_change_class_layout() {
    let mutable = lower_source_to_mir(concat!(
        "class Value {\n",
        "  byte: u8; payload: i64;\n",
        "  init(byte: u8, payload: i64) { self.byte = byte; self.payload = payload; }\n",
        "}\n",
        "fn main() -> i64 { return 0; }\n",
    ));
    let final_fields = lower_source_to_mir(concat!(
        "class Value {\n",
        "  final byte: u8; final payload: i64;\n",
        "  init(byte: u8, payload: i64) { self.byte = byte; self.payload = payload; }\n",
        "}\n",
        "fn main() -> i64 { return 0; }\n",
    ));

    let mutable_layout = DataLayout::compute(&mutable).unwrap();
    let final_layout = DataLayout::compute(&final_fields).unwrap();
    let class = ClassId::new(0);
    assert_eq!(
        mutable_layout.class(class).unwrap().ty(),
        final_layout.class(class).unwrap().ty()
    );
    for index in 0..2 {
        let field = FieldId::new(class, index);
        assert_eq!(mutable_layout.field(field), final_layout.field(field));
    }
}

#[test]
fn executes_final_construction_synthesized_copy_construction_and_reads() {
    let artifact = compile_source_to_assembly(
        "final-construction.ska",
        concat!(
            "class Value {\n",
            "  final value: i64;\n",
            "  init(value: i64) { self.value = value; }\n",
            "  fn get() -> i64 { return self.value; }\n",
            "}\n",
            "fn main() -> i64 {\n",
            "  var first: Value = Value(21); var second: Value = first;\n",
            "  return first.get() + second.value;\n",
            "}\n",
        ),
        Target::X86_64SysV,
    )
    .expect("final instance construction and reads must compile");

    assert_eq!(run_native_assembly(&artifact.assembly).code(), Some(42));
}

#[test]
fn lowers_every_final_storage_family_through_the_backend() {
    let artifact = compile_source_to_assembly(
        "final-storage-matrix.ska",
        concat!(
            "class Item { value: i64; init(value: i64) { self.value = value; } }\n",
            "fn identity(value: i64) -> i64 { return value; }\n",
            "class Values {\n",
            "  final primitive: i64; final object: Item; final maybe: i64?;\n",
            "  final owner: shared Item; final values: i64[]; final callback: fn(i64) -> i64;\n",
            "  init() { self.primitive = 1; self.object = Item(2); self.maybe = 3;\n",
            "    self.owner = new Item(4); self.values = i64[]{5}; self.callback = identity; }\n",
            "  fn score() -> i64 { return self.primitive + self.object.value + self.maybe!\n",
            "    + self.owner->value + self.values[0] + self.callback(6); }\n",
            "}\n",
            "fn main() -> i64 { var first: Values = Values(); var second: Values = first;\n",
            "  return first.score() + second.score(); }\n",
        ),
        Target::X86_64SysV,
    )
    .expect("the complete final storage matrix must lower");

    assert!(artifact
        .assembly
        .contains(".Lska.class.main.Values.c1.method.score.m0"));
}

#[test]
fn executes_synthesized_and_user_final_assignment_including_self_assignment() {
    let synthesized = compile_source_to_assembly(
        "final-synthesized-assignment.ska",
        concat!(
            "class Inner { final value: i64; init(value: i64) { self.value = value; } }\n",
            "class Outer { inner: Inner; init(value: i64) { self.inner = Inner(value); } }\n",
            "fn main() -> i64 {\n",
            "  var left: Outer = Outer(1); var right: Outer = Outer(21);\n",
            "  left = right; left = left; return left.inner.value * 2;\n",
            "}\n",
        ),
        Target::X86_64SysV,
    )
    .expect("synthesized final assignment must compile");
    assert_eq!(run_native_assembly(&synthesized.assembly).code(), Some(42));

    let user = compile_source_to_assembly(
        "final-user-assignment.ska",
        concat!(
            "class Value { final value: i64; init(value: i64) { self.value = value; }\n",
            "  assign(ref other: Value) {\n",
            "    if (other.value > 0) { self.value = other.value + 1; }\n",
            "    else { self.value = 0; }\n",
            "  }\n",
            "}\n",
            "fn main() -> i64 { var left: Value = Value(1); var right: Value = Value(20);\n",
            "  left = right; return left.value * 2; }\n",
        ),
        Target::X86_64SysV,
    )
    .expect("user final assignment must compile");
    assert_eq!(run_native_assembly(&user.assembly).code(), Some(42));
}
