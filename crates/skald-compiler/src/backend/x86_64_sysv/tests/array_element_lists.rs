use super::*;

#[test]
fn primitive_element_lists_execute_for_every_scalar_and_outer_ownership() {
    for (ty, first, second) in [
        ("i64", "-7", "42"),
        ("u64", "1u", "9u"),
        ("u8", "2u8", "8u8"),
        ("f64", "1.25", "2.5"),
        ("bool", "false", "true"),
    ] {
        let source = format!(
            concat!(
                "fn main() -> i64 {{\n",
                "  var inline: {ty}[] = {ty}[]{{{first}, {second}}};\n",
                "  var shared: shared {ty}[] = new {ty}[]{{{first}, {second}}};\n",
                "  if (inline[0] != {first}) {{ return 1; }}\n",
                "  if (inline[1] != {second}) {{ return 2; }}\n",
                "  if (shared->[0] != {first}) {{ return 3; }}\n",
                "  if (shared->[1] != {second}) {{ return 4; }}\n",
                "  return 42;\n",
                "}}\n",
            ),
            ty = ty,
            first = first,
            second = second,
        );
        let mut output = assembly(&source);
        assert!(output.contains("call ska_rt_abi_v8"));
        assert!(!output.contains("ska_rt_array_element_list"));
        output.push_str(native_allocator());
        assert_eq!(
            run_native_assembly(&output).code(),
            Some(42),
            "{ty} element lists failed\n{output}"
        );
    }
}

#[test]
fn primitive_element_lists_execute_in_nested_expression_contexts() {
    let source = concat!(
        "class Holder {\n",
        "  values: i64[];\n",
        "  init(values: i64[]) { self.values = values; }\n",
        "  mut fn replace() -> unit { self.values = i64[]{30, 12}; }\n",
        "  fn first() -> i64 { return self.values[0]; }\n",
        "}\n",
        "fn returned() -> i64[] { return i64[]{40, 2}; }\n",
        "fn first(values: i64[]) -> i64 { return values[0]; }\n",
        "fn main() -> i64 {\n",
        "  var empty: i64[] = i64[]{};\n",
        "  if (empty.len() != 0u) { return 1; }\n",
        "  if (first(i64[]{42}) != 42) { return 2; }\n",
        "  if (i64[]{42}[0] != 42) { return 3; }\n",
        "  if (returned()[0] != 40) { return 4; }\n",
        "  var holder: Holder = Holder(i64[]{10, 20});\n",
        "  holder.replace();\n",
        "  if (holder.first() != 30) { return 5; }\n",
        "  return 42;\n",
        "}\n",
    );
    let mut output = assembly(source);
    output.push_str(native_allocator());
    assert_eq!(run_native_assembly(&output).code(), Some(42), "{output}");
}

#[test]
fn primitive_element_lists_preserve_control_effects_between_positions() {
    let source = concat!(
        "class Trace { static count: i64; init() {} }\n",
        "fn mark(value: bool) -> bool { Trace.count = Trace.count + 1; return value; }\n",
        "fn main() -> i64 {\n",
        "  var values: bool[] = bool[]{false && mark(true), true || mark(false), mark(true) && mark(true)};\n",
        "  if (values[0] || !values[1] || !values[2]) { return 1; }\n",
        "  if (Trace.count != 2) { return 2; }\n",
        "  return 42;\n",
        "}\n",
    );
    let mut output = assembly(source);
    output.push_str(native_allocator());
    assert_eq!(run_native_assembly(&output).code(), Some(42), "{output}");
}

#[test]
fn primitive_element_list_expressions_run_once_in_left_to_right_order() {
    let source = concat!(
        "class Trace { static order: i64; init() {} }\n",
        "fn record(value: i64) -> i64 { Trace.order = Trace.order * 10 + value; return value; }\n",
        "fn main() -> i64 {\n",
        "  var values: i64[] = i64[]{record(1), record(2), record(3)};\n",
        "  if (values[0] != 1 || values[1] != 2 || values[2] != 3) { return 1; }\n",
        "  if (Trace.order != 123) { return 2; }\n",
        "  return 42;\n",
        "}\n",
    );
    let mut output = assembly(source);
    output.push_str(native_allocator());
    assert_eq!(run_native_assembly(&output).code(), Some(42), "{output}");
}
