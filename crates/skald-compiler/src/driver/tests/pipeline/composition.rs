use crate::{backend::Target, driver::compile_source_to_assembly};

#[test]
fn function_values_compile_through_the_public_driver() {
    let artifact = compile_source_to_assembly(
        "function-values.ska",
        concat!(
            "fn identity(value: i64) -> i64 { return value; }\n",
            "fn main() -> i64 { var callback: fn(i64) -> i64 = identity; return callback(42); }\n",
        ),
        Target::X86_64SysV,
    )
    .expect("function-value source must compile through native realization");

    assert!(artifact.report.diagnostics.is_empty());
    assert!(artifact
        .assembly
        .contains("lea rax, [rip + .Lska.fn.main.identity.f0]"));
    assert!(artifact.assembly.contains("call r11"));
}

#[test]
fn composes_the_complete_frontend_and_backend_pipeline() {
    let artifact = compile_source_to_assembly(
        "complete.ska",
        "fn double(x: i64) -> i64 { return x * 2; }\n\
         fn main() -> i64 { return double(21); }",
        Target::X86_64SysV,
    )
    .unwrap();

    assert!(artifact.report.diagnostics.is_empty());
    assert!(artifact.assembly.contains("call .Lska.fn.main.double.f0"));
    assert!(artifact.assembly.contains(".globl main"));
}

#[test]
fn typed_alias_syntax_reaches_the_backend_pipeline() {
    let artifact = compile_source_to_assembly(
        "alias-syntax.ska",
        concat!(
            "class Dog { init() {} }\n",
            "fn inspect(ref dog: Dog) -> unit {}\n",
            "fn main() -> i64 { var dog: Dog = Dog(); inspect(dog); return 0; }\n",
        ),
        Target::X86_64SysV,
    )
    .unwrap();

    assert!(artifact.report.diagnostics.is_empty());
    assert!(artifact.assembly.contains(".Lska.fn.main.inspect.f0:"));
}
