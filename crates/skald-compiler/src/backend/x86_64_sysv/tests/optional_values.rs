use super::*;

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
