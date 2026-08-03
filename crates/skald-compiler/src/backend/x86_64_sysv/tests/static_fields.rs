use super::*;
use crate::backend::x86_64_sysv::layout::DataLayout;

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

#[test]
fn primitive_optional_statics_begin_absent_and_support_aliases_and_replacement() {
    let source = concat!(
        "fn replace(mut ref value: i64?, next: i64?) -> unit { value = next; }\n",
        "class State {\n",
        "  static signed: i64?; static unsigned: u64?; static byte: u8?;\n",
        "  static float: f64?; private static ready: bool?;\n",
        "  init() {}\n",
        "  static fn run() -> i64 {\n",
        "    if (State.signed is some || State.unsigned is some || State.byte is some ||\n",
        "        State.float is some || State.ready is some) { return 1; }\n",
        "    replace(State.signed, 40); State.ready = true;\n",
        "    while (State.signed is some) { State.signed = State.signed! + 1; break; }\n",
        "    if (State.ready is some) { State.ready = none; return State.signed! + 1; }\n",
        "    return 2;\n",
        "  }\n",
        "}\n",
        "fn main() -> i64 { return State.run(); }\n",
    );
    let output = lower_source_to_assembly(source, Target::X86_64SysV).unwrap();

    assert_system_assembler_accepts(&output);
    assert_eq!(run_native_assembly(&output).code(), Some(42));
}

#[test]
fn optional_static_layout_reuses_inline_layout_without_changing_instances() {
    let source = concat!(
        "class Item { byte: u8; value: i64; init() { self.byte = 0u8; self.value = 0; } }\n",
        "class State { static number: i64?; static item: Item?; init() {} }\n",
        "fn main() -> i64 { return 0; }\n",
    );
    let program = lower_source_to_mir(source);
    let layout = DataLayout::compute(&program).unwrap();
    let primitive = layout
        .ty(MirType::OptionalPrimitive(MirPrimitiveType::I64))
        .unwrap();
    let class_optional = layout.optional_class(ClassId::new(0)).unwrap();

    assert_eq!(layout.class(ClassId::new(1)).unwrap().ty().size(), 1);
    assert_eq!(class_optional.payload_offset(), 8);

    let assembly = emit_assembly(Target::X86_64SysV, &program).unwrap();
    assert!(assembly.contains(&format!(
        ".Lska.class.main.State.c1.static.s0:\n    .zero {}",
        primitive.size()
    )));
    assert!(assembly.contains(&format!(
        ".Lska.class.main.State.c1.static.s1:\n    .zero {}",
        class_optional.ty().size()
    )));
    assert_system_assembler_accepts(&assembly);
}

#[test]
fn inherited_optional_selection_uses_the_declaring_class_slot() {
    let source = concat!(
        "class Base { static value: i64?; init() {} }\n",
        "class Derived extends Base { init() { super(); } }\n",
        "fn main() -> i64 { Derived.value = 41; return Base.value! + 1; }\n",
    );
    let assembly = lower_source_to_assembly(source, Target::X86_64SysV).unwrap();

    assert!(assembly.contains(".Lska.class.main.Base.c0.static.s0:"));
    assert!(!assembly.contains(".Lska.class.main.Derived.c1.static"));
    assert_eq!(run_native_assembly(&assembly).code(), Some(42));
}

#[test]
fn absent_static_unwrap_and_guarded_reentrant_clear_terminate() {
    let absent = lower_source_to_assembly(
        "class State { static value: i64?; init() {} }\n\
         fn main() -> i64 { return State.value!; }\n",
        Target::X86_64SysV,
    )
    .unwrap();
    assert!(!run_native_assembly(&absent).success());

    let guarded = lower_source_to_assembly(
        concat!(
            "class Item { value: i64; init(value: i64) { self.value = value; } }\n",
            "class State { static item: Item?; init() {} }\n",
            "fn clear() -> i64 { State.item = none; return 0; }\n",
            "fn consume(ref item: Item, ignored: i64) -> i64 { return item.value + ignored; }\n",
            "fn main() -> i64 { State.item = Item(42); return consume(State.item!, clear()); }\n",
        ),
        Target::X86_64SysV,
    )
    .unwrap();
    assert!(!run_native_assembly(&guarded).success());
}

#[test]
fn class_optional_static_replacement_destroys_old_payload_but_not_final_payload() {
    let source = concat!(
        "extern fn test_record_i64(value: i64) -> unit;\n",
        "class Item {\n",
        "  value: i64; init(value: i64) { self.value = value; }\n",
        "  destroy { test_record_i64(self.value); }\n",
        "}\n",
        "class State { static item: Item?; static copied: Item?; init() {} }\n",
        "fn read(ref value: Item?) -> i64 { if (value is some) { return value!.value; } return 0; }\n",
        "fn main() -> i64 {\n",
        "  if (State.item is some) { return 1; }\n",
        "  State.item = Item(7);\n",
        "  State.item = Item(42);\n",
        "  State.copied = State.item;\n",
        "  return read(State.copied);\n",
        "}\n",
    );
    let mut output = lower_source_to_assembly(source, Target::X86_64SysV).unwrap();
    output.push_str(record_seven_stub());
    let result = run_native_assembly_output(&output);

    assert_eq!(result.status.code(), Some(42));
    assert_eq!(result.stdout, b"7\n42\n");
    assert!(result.stderr.is_empty());
}

fn record_seven_stub() -> &'static str {
    concat!(
        ".bss\n",
        ".align 8\n",
        ".Lrecord_optional_count:\n",
        "    .zero 8\n",
        ".section .rodata\n",
        ".Lrecord_optional_output:\n",
        "    .ascii \"7\\n42\\n\"\n",
        ".text\n",
        ".globl test_record_i64\n",
        ".type test_record_i64, @function\n",
        "test_record_i64:\n",
        "    mov r10, qword ptr [rip + .Lrecord_optional_count]\n",
        "    cmp r10, 0\n",
        "    jne .Lrecord_optional_second\n",
        "    cmp rdi, 7\n",
        "    jne .Lrecord_optional_bad_value\n",
        "    mov rsi, 0\n",
        "    mov rdx, 2\n",
        "    jmp .Lrecord_optional_write\n",
        ".Lrecord_optional_second:\n",
        "    cmp r10, 1\n",
        "    jne .Lrecord_optional_bad_value\n",
        "    cmp rdi, 42\n",
        "    jne .Lrecord_optional_bad_value\n",
        "    mov rsi, 2\n",
        "    mov rdx, 3\n",
        ".Lrecord_optional_write:\n",
        "    add r10, 1\n",
        "    mov qword ptr [rip + .Lrecord_optional_count], r10\n",
        "    mov rax, 1\n",
        "    mov rdi, 1\n",
        "    lea r11, [rip + .Lrecord_optional_output]\n",
        "    add rsi, r11\n",
        "    syscall\n",
        "    ret\n",
        ".Lrecord_optional_bad_value:\n",
        "    mov rax, 60\n",
        "    mov rdi, 99\n",
        "    syscall\n",
        ".size test_record_i64, .-test_record_i64\n",
    )
}
