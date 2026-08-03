use super::*;

const SOURCE: &str = concat!(
    "fn increment(mut ref value: i64) -> unit { value = value + 1; }\n",
    "class State {\n",
    "  static count: i64; static byte: u8; static ratio: f64; private static ready: bool;\n",
    "  init() {}\n",
    "  static fn run() -> i64 {\n",
    "  State.byte = 7u8; State.ratio = 2.5; State.ready = true;\n",
    "  State.count = (i64) State.byte + (i64) State.ratio;\n",
    "  increment(State.count);\n",
    "  if (State.ready) { return State.count + 32; } return 0;\n",
    "  }\n",
    "}\n",
    "fn main() -> i64 { return State.run(); }\n",
);

#[test]
fn emits_deterministic_aligned_zero_slots_and_rip_relative_addresses() {
    let program = lower_source_to_mir(SOURCE);
    let output = emit_assembly(Target::X86_64SysV, &program).unwrap();
    assert_eq!(output, emit_assembly(Target::X86_64SysV, &program).unwrap());
    for (index, size) in [8, 1, 8, 1].into_iter().enumerate() {
        let symbol = format!(".Lska.class.main.State.c0.static.s{index}");
        assert!(output.contains(&format!(".type {symbol}, @object")));
        assert!(output.contains(&format!("{symbol}:\n    .zero {size}")));
        assert!(
            output.contains(&format!("lea r11, [rip + {symbol}]"))
                || output.contains(&format!("lea rdi, [rip + {symbol}]"))
        );
    }
    assert!(output.contains("\n.bss\n"));
    assert!(output.contains("mov byte ptr [r11], al"));
    assert!(output.contains("movsd qword ptr [r11]"));
    assert_system_assembler_accepts(&output);
}

#[test]
fn primitive_static_reads_writes_and_aliases_execute_natively() {
    let output = lower_source_to_assembly(SOURCE, Target::X86_64SysV).unwrap();
    assert_eq!(run_native_assembly(&output).code(), Some(42));
}

#[test]
fn same_named_fields_in_distinct_classes_have_distinct_symbols() {
    let output = lower_source_to_assembly(
        concat!(
            "class Left { static value: i64; init() {} }\n",
            "class Right { static value: i64; init() {} }\n",
            "fn main() -> i64 { Left.value = 1; Right.value = 2; return Left.value + Right.value; }\n",
        ),
        Target::X86_64SysV,
    )
    .unwrap();

    assert!(output.contains(".Lska.class.main.Left.c0.static.s0:"));
    assert!(output.contains(".Lska.class.main.Right.c1.static.s0:"));
}
