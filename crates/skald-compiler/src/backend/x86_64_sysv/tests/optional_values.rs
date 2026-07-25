use super::super::layout;
use super::*;

#[test]
fn class_optional_layout_reserves_an_aligned_exact_payload() {
    let program = lower_text(
        "class Value { byte: u8; wide: i64; init() { self.byte = 1u8; self.wide = 2; } }\n\
         fn main() -> i64 { var value: Value? = none; return 0; }\n",
    );
    let layouts = layout::DataLayout::compute(&program).unwrap();
    let payload = layouts.class(ClassId::new(0)).unwrap().ty();
    let optional = layouts.optional_class(ClassId::new(0)).unwrap();

    assert_eq!(optional.payload_offset() % payload.alignment(), 0);
    assert!(optional.ty().size() >= optional.payload_offset() + payload.size());
    assert_eq!(optional.ty().alignment(), payload.alignment().max(8));
}

#[test]
fn primitive_optionals_execute_present_payloads_for_every_primitive() {
    let output = assembly(
        "fn main() -> i64 {\n\
           var signed: i64? = 40;\n\
           var unsigned: u64? = 7u;\n\
           var byte: u8? = 8u8;\n\
           var float: f64? = 1.5;\n\
           var flag: bool? = true;\n\
           var signed_value: i64 = signed!;\n\
           var unsigned_value: u64 = unsigned!;\n\
           var byte_value: u8 = byte!;\n\
           var float_value: f64 = float!;\n\
           var flag_value: bool = flag!;\n\
           if (flag_value) {\n\
             if (unsigned is some) {\n\
               if (byte is some) {\n\
                 if (float is some) { return signed_value + 2; }\n\
               }\n\
             }\n\
           }\n\
           return 0;\n\
         }\n",
    );

    assert!(output.contains("ud2"));
    assert_eq!(run_native_assembly(&output).code(), Some(42));
}

#[test]
fn primitive_optional_absence_presence_copy_assignment_and_joins_execute() {
    let source = "fn main() -> i64 {\n\
        var value: i64? = none;\n\
        var copy: i64? = value;\n\
        if (copy is none) { value = 19; } else { value = 1; }\n\
        copy = value;\n\
        if (copy is some) { return copy! + 4; }\n\
        return 0;\n\
    }\n";
    assert_eq!(run_native_assembly(&assembly(source)).code(), Some(23));
}

#[test]
fn absent_primitive_optional_unwrap_terminates() {
    let output = assembly(
        "fn main() -> i64 {\n\
           var value: i64? = none;\n\
           return value!;\n\
         }\n",
    );

    assert!(!run_native_assembly(&output).success());
}

#[test]
fn optional_fields_calls_results_and_stack_pressure_execute() {
    let source = "class Holder {\n\
        value: i64?;\n\
        init(value: i64?) { self.value = value; }\n\
        mut fn replace(value: i64?) -> i64? {\n\
            self.value = self.value;\n\
            self.value = value;\n\
            return self.value;\n\
        }\n\
    }\n\
    fn choose(a: i64?, b: i64?, c: i64?, d: i64?, e: i64?, f: i64?, g: i64?) -> i64? {\n\
        if (g is some) { return g; }\n\
        return a;\n\
    }\n\
    fn main() -> i64 {\n\
        var holder: Holder = Holder(none);\n\
        var result: i64? = holder.replace(choose(1, 2, 3, 4, 5, 6, 7));\n\
        var copy: Holder = holder;\n\
        copy = holder;\n\
        return copy.value!;\n\
    }\n";
    let output = assembly(source);

    assert!(output.contains("call .Lska_fn_"));
    assert!(output.contains("mov qword ptr [rsp]"));
    assert_eq!(run_native_assembly(&output).code(), Some(7), "{output}");
}

#[test]
fn optional_values_execute_through_virtual_and_interface_calls() {
    let source = "interface Maybe {\n\
        fn forward(value: i64?) -> i64?;\n\
    }\n\
    class Base {\n\
        init() {}\n\
        virtual fn forward(value: i64?) -> i64? { return value; }\n\
    }\n\
    class Derived extends Base implements Maybe {\n\
        init() { super(); }\n\
        override fn forward(value: i64?) -> i64? { return value; }\n\
    }\n\
    fn through_interface(ref source: Maybe, value: i64?) -> i64? {\n\
        return source.forward(value);\n\
    }\n\
    fn through_virtual(ref source: Base, value: i64?) -> i64? {\n\
        return source.forward(value);\n\
    }\n\
    fn main() -> i64 {\n\
        var item: Derived = Derived();\n\
        var left: i64? = through_interface(item, 20);\n\
        var right: i64? = through_virtual(item, 22);\n\
        return left! + right!;\n\
    }\n";

    assert_eq!(run_native_assembly(&assembly(source)).code(), Some(42));
}

#[test]
fn class_optional_lifecycle_executes_through_calls_fields_and_assignment() {
    let source = concat!(
        "extern fn ska_rt_println_i64(value: i64) -> unit;\n",
        "class Value {\n",
        "  marker: i64;\n",
        "  init(marker: i64) { self.marker = marker; }\n",
        "  destroy { ska_rt_println_i64(self.marker); }\n",
        "}\n",
        "class Holder {\n",
        "  value: Value?;\n",
        "  init(value: Value?) { self.value = value; }\n",
        "}\n",
        "fn forward(value: Value?) -> Value? { return value; }\n",
        "fn main() -> i64 {\n",
        "  var first: Value? = Value(42);\n",
        "  var second: Value? = first;\n",
        "  first = none;\n",
        "  first = forward(Value(42));\n",
        "  var holder: Holder = Holder(first);\n",
        "  if (holder.value is some) { return 42; }\n",
        "  return 0;\n",
        "}\n",
    );
    let mut output = assembly(source);
    output.push_str(println_i64_stub());
    let result = run_native_assembly_output(&output);

    assert_eq!(result.status.code(), Some(42));
    assert_eq!(result.stdout, b"42\n42\n42\n42\n42\n42\n42\n");
    assert!(result.stderr.is_empty());
}

#[test]
fn class_optional_parameters_and_results_execute_under_stack_pressure() {
    let source = "class Value { init() {} }\n\
        fn choose(a: Value?, b: Value?, c: Value?, d: Value?, e: Value?, f: Value?, g: Value?) -> Value? {\n\
          if (g is some) { return g; }\n\
          return a;\n\
        }\n\
        fn main() -> i64 {\n\
          var result: Value? = choose(none, none, none, none, none, none, Value());\n\
          if (result is some) { return 42; }\n\
          return 0;\n\
        }\n";
    let output = assembly(source);

    assert!(output.contains("mov qword ptr [rsp]"));
    assert_eq!(run_native_assembly(&output).code(), Some(42));
}
