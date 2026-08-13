use super::*;
use crate::{
    backend::x86_64_sysv::{layout::DataLayout, symbol},
    test_support::lower_generic_source_to_final_mir,
};

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
fn closed_generic_static_slots_have_distinct_assembler_safe_symbols() {
    let program = lower_generic_source_to_final_mir(
        "class Cache<T> { static value: T?; init() {} }
         fn main() -> i64 {
           if (Cache<i64>::value is some) { return 1; }
           if (Cache<bool>::value is some) { return 2; }
           return 0;
         }",
    );
    let fields = program
        .classes
        .iter()
        .filter(|class| class.name.starts_with("Cache<"))
        .map(|class| class.static_fields[0].id)
        .collect::<Vec<_>>();
    let symbols = fields
        .iter()
        .map(|field| symbol::static_field(&program, *field))
        .collect::<Vec<_>>();
    let assembly = emit_assembly(Target::X86_64SysV, &program).unwrap();

    assert_eq!(fields.len(), 2);
    assert!(!program.classes.iter().any(|class| class.name == "Cache"));
    assert_ne!(fields[0], fields[1]);
    assert_ne!(symbols[0], symbols[1]);
    assert!(symbols.iter().all(|symbol| symbol
        .chars()
        .all(|character| !matches!(character, '<' | '>' | '?' | ' '))));
    for symbol in symbols {
        assert!(assembly.contains(&format!("{symbol}:")), "{assembly}");
    }
    let wrapper = function_assembly(&assembly, "main");
    assert!(
        wrapper.contains("mov qword ptr [rbp - 8], rax"),
        "{wrapper}"
    );
    assert!(wrapper.contains("call .Lska.static.finalize"), "{wrapper}");
    assert!(
        wrapper.contains("mov rax, qword ptr [rbp - 8]"),
        "{wrapper}"
    );
    assert_system_assembler_accepts(&assembly);
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
        .ty(MirType::Optional(
            program.optional_for_payload(MirType::I64).unwrap(),
        ))
        .unwrap();
    let class_optional = layout
        .optional_type(
            program
                .optional_for_payload(MirType::Class(ClassId::new(0)))
                .unwrap(),
        )
        .unwrap();

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
fn class_optional_static_replacement_and_shutdown_destroy_current_payloads() {
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
    output.push_str(record_seven_then_42_stub());
    let result = run_native_assembly_output(&output);

    assert_eq!(result.status.code(), Some(42));
    // Replacement destroys 7, named copying cleans its source temporary, and
    // shutdown destroys the copied slot before the original slot.
    assert_eq!(result.stdout, b"7\n42\n42\n42\n");
    assert!(result.stderr.is_empty());
}

fn record_seven_then_42_stub() -> &'static str {
    concat!(
        ".section .rodata\n",
        ".Lrecord_optional_output:\n",
        "    .ascii \"7\\n42\\n\"\n",
        ".text\n",
        ".globl test_record_i64\n",
        ".type test_record_i64, @function\n",
        "test_record_i64:\n",
        "    cmp rdi, 7\n",
        "    jne .Lrecord_optional_42\n",
        "    mov rsi, 0\n",
        "    mov rdx, 2\n",
        "    jmp .Lrecord_optional_write\n",
        ".Lrecord_optional_42:\n",
        "    cmp rdi, 42\n",
        "    jne .Lrecord_optional_bad_value\n",
        "    mov rsi, 2\n",
        "    mov rdx, 3\n",
        ".Lrecord_optional_write:\n",
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

#[test]
fn optional_shared_statics_preserve_replacement_counts_and_release_final_owner() {
    let source = concat!(
        "extern fn test_record_i64(value: i64) -> unit;\n",
        "class Item {\n",
        "  value: i64; init(value: i64) { self.value = value; }\n",
        "  fn read() -> i64 { return self.value; }\n",
        "  destroy { test_record_i64(self.value); }\n",
        "}\n",
        "class State { static current: shared? Item; static copy: shared? Item; init() {} }\n",
        "fn make(value: i64) -> shared? Item { return new Item(value); }\n",
        "fn main() -> i64 {\n",
        "  if (State.current is some || State.copy is some) { return 1; }\n",
        "  State.current = new Item(7);\n",
        "  State.copy = State.current;\n",
        "  State.current = State.current;\n",
        "  State.current = none;\n",
        "  State.copy = make(42);\n",
        "  return State.copy!->read();\n",
        "}\n",
    );
    let mut assembly = lower_source_to_assembly(source, Target::X86_64SysV).unwrap();
    assert_eq!(
        assembly,
        lower_source_to_assembly(source, Target::X86_64SysV).unwrap()
    );
    assert!(assembly.contains(".Lska.class.main.State.c1.static.s0:\n    .zero 8"));
    assembly.push_str(native_allocator());
    assembly.push_str(record_seven_then_42_shutdown_stub());
    let result = run_native_assembly_output(&assembly);

    assert_eq!(result.status.code(), Some(42));
    assert_eq!(result.stdout, b"7\n42\n");
    assert!(result.stderr.is_empty());
}

#[test]
fn optional_shared_static_targets_support_views_casts_and_arrays() {
    let source = concat!(
        "interface Readable { fn read() -> i64; }\n",
        "class Item implements Readable {\n",
        "  value: i64; init(value: i64) { self.value = value; }\n",
        "  fn read() -> i64 { return self.value; }\n",
        "}\n",
        "class State {\n",
        "  private static concrete: shared? Item;\n",
        "  static readable: shared? Readable; static object: shared? Obj;\n",
        "  static values: shared? i64[]; init() {}\n",
        "  static fn prepare() -> unit {\n",
        "    State.concrete = new Item(40);\n",
        "    State.readable = (shared Readable) State.concrete!;\n",
        "  }\n",
        "}\n",
        "fn main() -> i64 {\n",
        "  if (State.readable is some || State.object is some || State.values is some) { return 1; }\n",
        "  State.prepare();\n",
        "  State.object = State.readable;\n",
        "  State.values = new i64[](2u);\n",
        "  var recovered: shared Item = (shared Item) State.object!;\n",
        "  return recovered->read() + (i64) State.values!->len();\n",
        "}\n",
    );
    let mut assembly = lower_source_to_assembly(source, Target::X86_64SysV).unwrap();
    assembly.push_str(native_allocator());

    assert_system_assembler_accepts(&assembly);
    assert_eq!(run_native_assembly(&assembly).code(), Some(42));
}

#[test]
fn inherited_optional_shared_selection_uses_one_base_owned_slot() {
    let source = concat!(
        "class Item { value: i64; init(value: i64) { self.value = value; } }\n",
        "class Base { static owner: shared? Item; init() {} }\n",
        "class Derived extends Base { init() { super(); } }\n",
        "fn main() -> i64 { Derived.owner = new Item(42); return Base.owner!->value; }\n",
    );
    let mut assembly = lower_source_to_assembly(source, Target::X86_64SysV).unwrap();
    assembly.push_str(native_allocator());

    assert!(assembly.contains(".Lska.class.main.Base.c1.static.s0:"));
    assert!(!assembly.contains(".Lska.class.main.Derived.c2.static"));
    assert_eq!(run_native_assembly(&assembly).code(), Some(42));
}

#[test]
fn inherited_inline_array_selection_uses_one_base_owned_descriptor() {
    let source = concat!(
        "class Base { static values: i64[]; init() {} }\n",
        "class Derived extends Base { init() { super(); } }\n",
        "fn main() -> i64 {\n",
        "  Derived.values = i64[](1u); Derived.values[0] = 42;\n",
        "  return Base.values[0];\n",
        "}\n",
    );
    let mut assembly = lower_source_to_assembly(source, Target::X86_64SysV).unwrap();
    assembly.push_str(native_allocator());

    assert!(assembly.contains(".Lska.class.main.Base.c0.static.s0:"));
    assert!(!assembly.contains(".Lska.class.main.Derived.c1.static"));
    assert_eq!(run_native_assembly(&assembly).code(), Some(42));
}

#[test]
fn final_static_owner_may_participate_in_an_unreleased_strong_cycle() {
    let source = concat!(
        "class Node {\n",
        "  next: shared? Node; init() { self.next = none; }\n",
        "  mut fn link(value: shared? Node) -> unit { self.next = value; }\n",
        "}\n",
        "class State { static root: shared? Node; init() {} }\n",
        "fn main() -> i64 {\n",
        "  State.root = new Node();\n",
        "  State.root!->link(State.root);\n",
        "  return 42;\n",
        "}\n",
    );
    let mut assembly = lower_source_to_assembly(source, Target::X86_64SysV).unwrap();
    assembly.push_str(native_allocator());

    assert_eq!(run_native_assembly(&assembly).code(), Some(42));
}

#[test]
fn inline_array_statics_support_replacement_projection_aliases_and_detached_backings() {
    let source = concat!(
        "fn reset() -> i64 { State.values = i64[](1u); State.values[0] = 2; return 0; }\n",
        "fn detached(ref value: i64, ignored: i64) -> i64 { return value + ignored; }\n",
        "fn mutate(mut ref values: i64[]) -> unit { values[0] = 40; }\n",
        "class Item { init(value: i64) {} }\n",
        "class State {\n",
        "  static values: i64[]; static items: Item[]; static maybe: i64?[];\n",
        "  static owners: (shared Item)[]; static nested: i64[][]; init() {}\n",
        "}\n",
        "fn main() -> i64 {\n",
        "  if (State.values.len() != 0u || State.items.len() != 0u || State.nested.len() != 0u) { return 1; }\n",
        "  State.values = i64[](3u); mutate(State.values);\n",
        "  State.values[1] = 1; State.values[2] = 2;\n",
        "  State.values[1:3] = State.values[0:2];\n",
        "  State.values[2] = 2;\n",
        "  var copied: i64[] = State.values[1:3];\n",
        "  var old: i64 = detached(State.values[0], reset());\n",
        "  return old + copied[1];\n",
        "}\n",
    );
    let mut assembly = lower_source_to_assembly(source, Target::X86_64SysV).unwrap();
    assert_eq!(
        assembly,
        lower_source_to_assembly(source, Target::X86_64SysV).unwrap()
    );
    assert!(assembly.contains(".Lska.class.main.State.c1.static.s0:\n    .zero 8"));
    assembly.push_str(native_allocator());

    assert_system_assembler_accepts(&assembly);
    assert_eq!(run_native_assembly(&assembly).code(), Some(42));
}

#[test]
fn static_array_replacement_and_shutdown_release_each_backing() {
    let source = concat!(
        "extern fn test_record_i64(value: i64) -> unit;\n",
        "class Item {\n",
        "  value: i64; init() { self.value = 7; }\n",
        "  destroy { test_record_i64(self.value); }\n",
        "}\n",
        "class State { static items: Item[]; init() {} }\n",
        "fn main() -> i64 {\n",
        "  State.items = Item[](1u);\n",
        "  State.items = Item[]();\n",
        "  State.items = Item[](1u);\n",
        "  return 42;\n",
        "}\n",
    );
    let mut assembly = lower_source_to_assembly(source, Target::X86_64SysV).unwrap();
    assembly.push_str(native_allocator());
    assembly.push_str(record_two_sevens_stub());
    let result = run_native_assembly_output(&assembly);

    assert_eq!(result.status.code(), Some(42));
    assert_eq!(result.stdout, b"7\n7\n");
    assert!(result.stderr.is_empty());
}

#[test]
fn static_array_checked_failures_terminate_before_addressing() {
    let index = concat!(
        "class State { static values: i64[]; init() {} }\n",
        "fn main() -> i64 { State.values = i64[](1u); return State.values[1]; }\n",
    );
    let mut index_assembly = lower_source_to_assembly(index, Target::X86_64SysV).unwrap();
    index_assembly.push_str(native_allocator());
    assert!(!run_native_assembly(&index_assembly).success());

    let slice = concat!(
        "class State { static values: i64[]; init() {} }\n",
        "fn main() -> i64 { State.values = i64[](1u); var copy: i64[] = State.values[1:0]; return 0; }\n",
    );
    let mut slice_assembly = lower_source_to_assembly(slice, Target::X86_64SysV).unwrap();
    slice_assembly.push_str(native_allocator());
    assert!(!run_native_assembly(&slice_assembly).success());
}

fn record_seven_then_42_shutdown_stub() -> &'static str {
    concat!(
        ".bss\n",
        ".align 8\n",
        ".Lrecord_shared_count:\n",
        "    .zero 8\n",
        ".section .rodata\n",
        ".Lrecord_shared_output:\n",
        "    .ascii \"7\\n42\\n\"\n",
        ".text\n",
        ".globl test_record_i64\n",
        ".type test_record_i64, @function\n",
        "test_record_i64:\n",
        "    mov r10, qword ptr [rip + .Lrecord_shared_count]\n",
        "    cmp r10, 0\n",
        "    jne .Lrecord_shared_second\n",
        "    cmp rdi, 7\n",
        "    jne .Lrecord_shared_bad_value\n",
        "    mov rsi, 0\n",
        "    mov rdx, 2\n",
        "    jmp .Lrecord_shared_write\n",
        ".Lrecord_shared_second:\n",
        "    cmp r10, 1\n",
        "    jne .Lrecord_shared_bad_value\n",
        "    cmp rdi, 42\n",
        "    jne .Lrecord_shared_bad_value\n",
        "    mov rsi, 2\n",
        "    mov rdx, 3\n",
        ".Lrecord_shared_write:\n",
        "    add r10, 1\n",
        "    mov qword ptr [rip + .Lrecord_shared_count], r10\n",
        "    mov rax, 1\n",
        "    mov rdi, 1\n",
        "    lea r11, [rip + .Lrecord_shared_output]\n",
        "    add rsi, r11\n",
        "    syscall\n",
        "    ret\n",
        ".Lrecord_shared_bad_value:\n",
        "    mov rax, 60\n",
        "    mov rdi, 99\n",
        "    syscall\n",
        ".size test_record_i64, .-test_record_i64\n",
    )
}

fn record_two_sevens_stub() -> &'static str {
    concat!(
        ".bss\n",
        ".align 8\n",
        ".Lrecord_array_count:\n",
        "    .zero 8\n",
        ".section .rodata\n",
        ".Lrecord_array_output:\n",
        "    .ascii \"7\\n\"\n",
        ".text\n",
        ".globl test_record_i64\n",
        ".type test_record_i64, @function\n",
        "test_record_i64:\n",
        "    cmp rdi, 7\n",
        "    jne .Lrecord_array_bad_value\n",
        "    mov r10, qword ptr [rip + .Lrecord_array_count]\n",
        "    cmp r10, 2\n",
        "    jae .Lrecord_array_bad_value\n",
        "    add r10, 1\n",
        "    mov qword ptr [rip + .Lrecord_array_count], r10\n",
        "    mov rax, 1\n",
        "    mov rdi, 1\n",
        "    lea rsi, [rip + .Lrecord_array_output]\n",
        "    mov rdx, 2\n",
        "    syscall\n",
        "    ret\n",
        ".Lrecord_array_bad_value:\n",
        "    mov rax, 60\n",
        "    mov rdi, 99\n",
        "    syscall\n",
        ".size test_record_i64, .-test_record_i64\n",
    )
}
