use super::*;

#[test]
fn executes_every_primitive_indexed_array_in_inline_and_shared_outer_storage() {
    let source = concat!(
        "fn main() -> i64 {\n",
        "  var empty: i64[] = i64[](0u; index => index + 100);\n",
        "  var signed: i64[] = i64[](3u; index => index + 1);\n",
        "  var unsigned: u64[] = u64[](3u; index => (u64) index + 2u);\n",
        "  var bytes: u8[] = u8[](3u; index => (u8) index + 2u8);\n",
        "  var floats: f64[] = f64[](3u; index => (f64) index + 0.5);\n",
        "  var flags: bool[] = bool[](3u; index => index == 1);\n",
        "  var shared: shared i64[] = new i64[](3u; index => index + 10);\n",
        "  if (empty.len() != 0u || !flags[1]) { return 99; }\n",
        "  return signed[2] + (i64) unsigned[2] + (i64) bytes[2]\n",
        "    + (i64) floats[2] + shared->[2];\n",
        "}\n",
    );
    let mut output = assembly(source);
    output.push_str(native_allocator());

    assert_eq!(run_native_assembly(&output).code(), Some(25), "{output}");
}

#[test]
fn primitive_indexed_assembly_is_deterministic_and_uses_existing_runtime_symbols() {
    let source = concat!(
        "fn main() -> i64 {\n",
        "  var values: i64[] = i64[](3u; index => index * index);\n",
        "  return values[2];\n",
        "}\n",
    );
    let first = assembly(source);
    let second = assembly(source);

    assert_eq!(first, second);
    assert!(first.contains("call ska_rt_abi_v9"), "{first}");
    assert!(first.contains("call ska_rt_alloc"), "{first}");
    assert!(first.contains("call ska_rt_free"), "{first}");
    assert!(
        !first.contains("indexed"),
        "source protocol must erase before assembly"
    );
}

#[test]
fn zero_length_skips_the_element_expression() {
    let source = concat!(
        "fn explode(index: i64) -> i64 { return 1 / index; }\n",
        "fn main() -> i64 {\n",
        "  var empty: i64[] = i64[](0u; index => explode(index));\n",
        "  return 7 + (i64) empty.len();\n",
        "}\n",
    );
    let mut output = assembly(source);
    output.push_str(native_allocator());

    assert_eq!(run_native_assembly(&output).code(), Some(7), "{output}");
}
