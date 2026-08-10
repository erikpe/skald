use super::*;
use crate::driver::compile_source_to_assembly;

fn compile(source: &str) -> String {
    compile_source_to_assembly("static-shutdown.ska", source, Target::X86_64SysV)
        .expect("static shutdown source must compile")
        .assembly
}

#[test]
fn lowers_exact_reverse_shutdown_and_preserves_the_entry_result() {
    let source = concat!(
        "class Item {\n",
        "  value: i64; init() { self.value = 0; } init(value: i64) { self.value = value; }\n",
        "}\n",
        "class State {\n",
        "  static first: Item = Item(1); static primitive: i64 = 2;\n",
        "  static last: Item = Item(3); init() {}\n",
        "}\n",
        "fn main() -> i64 { return 42; }\n",
    );
    let assembly = compile(source);
    let finalizer = function_assembly(&assembly, ".Lska.static.finalize");
    let last = "lea rdi, [rip + .Lska.class.main.State.c1.static.s2]";
    let first = "lea rdi, [rip + .Lska.class.main.State.c1.static.s0]";

    assert!(finalizer.find(last).unwrap() < finalizer.find(first).unwrap());
    assert!(!finalizer.contains("static.s1"));
    assert_eq!(
        finalizer
            .matches("call .Lska.class.main.Item.c0.finalize_complete")
            .count(),
        2
    );

    let wrapper = function_assembly(&assembly, "main");
    assert!(wrapper.contains(concat!(
        "    call .Lska.fn.main.main.f0\n",
        "    sub rsp, 16\n",
        "    mov qword ptr [rbp - 8], rax\n",
        "    call .Lska.static.finalize\n",
        "    mov rax, qword ptr [rbp - 8]\n",
    )));
    assert_system_assembler_accepts(&assembly);
    assert_eq!(run_native_assembly(&assembly).code(), Some(42));
}

#[test]
fn initializer_free_owning_slots_destroy_their_current_contents() {
    let source = concat!(
        "class Item {\n",
        "  value: i64; init() { self.value = 0; } init(value: i64) { self.value = value; }\n",
        "}\n",
        "class State {\n",
        "  static maybe: Item?; static owner: shared? Item; static items: Item[]; init() {}\n",
        "}\n",
        "fn main() -> i64 {\n",
        "  State.maybe = Item(1); State.owner = new Item(2); State.items = Item[](1u);\n",
        "  return 42;\n",
        "}\n",
    );
    let mut assembly = compile(source);
    let finalizer = function_assembly(&assembly, ".Lska.static.finalize");

    assert!(finalizer.contains("call .Lska_array_0_release"));
    assert!(finalizer.contains("call .Lska_shared_handle_release"));
    assert!(finalizer.contains("finalize_optional_complete"));
    assert!(finalizer.contains("call .Lska.class.main.Item.c0.finalize_complete"));
    assembly.push_str(native_allocator());

    assert_system_assembler_accepts(&assembly);
    assert_eq!(run_native_assembly(&assembly).code(), Some(42));
}

#[test]
fn static_destructors_run_after_entry_with_dependencies_still_live() {
    let source = concat!(
        "extern fn test_record_i64(value: i64) -> unit;\n",
        "class Item {\n",
        "  init() {}\n",
        "  destroy { test_record_i64(State.value); }\n",
        "}\n",
        "class State { static item: Item?; static value: i64; init() {} }\n",
        "fn main() -> i64 { State.value = 42; State.item = Item(); return 37; }\n",
    );
    let mut assembly = compile(source);
    assembly.push_str(record_i64_stub());
    let result = run_native_assembly_output(&assembly);

    assert_eq!(result.status.code(), Some(37));
    // Assignment cleans its temporary after publication; shutdown then
    // destroys the retained current payload while `State.value` is still live.
    assert_eq!(result.stdout, b"42\n42\n");
    assert!(result.stderr.is_empty());
}

#[test]
fn nested_optional_static_payload_is_destroyed_at_shutdown() {
    let source = concat!(
        "extern fn test_record_i64(value: i64) -> unit;\n",
        "class Item { marker: i64; init(marker: i64) { self.marker = marker; } destroy { test_record_i64(self.marker); } }\n",
        "class State { static item: Item?? = some(some(Item(42))); init() {} }\n",
        "fn main() -> i64 { if (State.item is some) { return 42; } return 0; }\n",
    );
    let mut output = compile(source);
    output.push_str(record_i64_stub());
    let result = run_native_assembly_output(&output);
    assert_eq!(result.status.code(), Some(42), "{output}");
    assert_eq!(result.stdout, b"42\n");
}

#[test]
fn static_arrays_destroy_elements_in_reverse_index_order() {
    let source = concat!(
        "extern fn test_next() -> i64; extern fn test_destroy(value: i64) -> unit;\n",
        "class Item {\n",
        "  value: i64; init() { self.value = test_next(); }\n",
        "  destroy { test_destroy(self.value); }\n",
        "}\n",
        "class State { static items: Item[]; init() {} }\n",
        "fn main() -> i64 { State.items = Item[](2u); return 42; }\n",
    );
    let mut assembly = compile(source);
    assembly.push_str(native_allocator());
    assembly.push_str(reverse_array_probe());
    let result = run_native_assembly_output(&assembly);

    assert_eq!(result.status.code(), Some(42));
    assert_eq!(result.stdout, b"2\n1\n");
    assert!(result.stderr.is_empty());
}

#[test]
fn static_shared_release_runs_the_last_owner_finalizer() {
    let source = concat!(
        "extern fn test_record_i64(value: i64) -> unit;\n",
        "class Item {\n",
        "  value: i64; init(value: i64) { self.value = value; }\n",
        "  destroy { test_record_i64(self.value); }\n",
        "}\n",
        "class State { static owner: shared? Item; init() {} }\n",
        "fn main() -> i64 { State.owner = new Item(42); return 37; }\n",
    );
    let mut assembly = compile(source);
    assembly.push_str(native_allocator());
    assembly.push_str(record_i64_stub());
    let result = run_native_assembly_output(&assembly);

    assert_eq!(result.status.code(), Some(37));
    assert_eq!(result.stdout, b"42\n");
    assert!(result.stderr.is_empty());
}

#[test]
fn static_complete_finalization_includes_derived_and_base_destructors() {
    let source = concat!(
        "class Base { init() {} destroy {} }\n",
        "class Derived extends Base { init() { super(); } destroy {} }\n",
        "class State { static item: Derived?; init() {} }\n",
        "fn main() -> i64 { State.item = Derived(); return 42; }\n",
    );
    let assembly = compile(source);
    let program_finalizer = function_assembly(&assembly, ".Lska.static.finalize");
    let complete = function_assembly(&assembly, ".Lska.class.main.Derived.c1.finalize_complete");
    let derived = "call .Lska.class.main.Derived.c1.destroy.d0";
    let base = "call .Lska.class.main.Base.c0.destroy.d0";

    assert!(program_finalizer.contains("call .Lska.class.main.Derived.c1.finalize_complete"));
    assert!(complete.find(derived).unwrap() < complete.find(base).unwrap());
    assert_system_assembler_accepts(&assembly);
    assert_eq!(run_native_assembly(&assembly).code(), Some(42));
}

#[test]
fn entry_panic_does_not_attempt_static_unwinding() {
    let source = concat!(
        "extern fn test_forbidden() -> unit;\n",
        "class Item { init() {} destroy { test_forbidden(); } }\n",
        "class State { static item: shared? Item; static values: i64[]; init() {} }\n",
        "fn main() -> i64 {\n",
        "  State.item = new Item(); State.values = i64[](1u); return State.values[1];\n",
        "}\n",
    );
    let mut assembly = compile(source);
    assembly.push_str(native_allocator());
    assembly.push_str(native_panic_reporter());
    assembly.push_str(forbidden_destructor_probe());
    let result = run_native_assembly_output(&assembly);

    assert_eq!(result.status.code(), Some(1));
    assert!(!result.stderr.is_empty());
}

#[test]
fn destructor_panic_stops_before_remaining_static_cleanup() {
    let source = concat!(
        "extern fn test_forbidden() -> unit;\n",
        "class Survivor { init() {} destroy { test_forbidden(); } }\n",
        "class Failing {\n",
        "  init() {}\n",
        "  destroy { var values: i64[] = i64[](1u); var ignored: i64 = values[1]; }\n",
        "}\n",
        "class State {\n",
        "  static survivor: shared? Survivor; static failing: shared? Failing; init() {}\n",
        "}\n",
        "fn main() -> i64 {\n",
        "  State.survivor = new Survivor(); State.failing = new Failing(); return 42;\n",
        "}\n",
    );
    let mut assembly = compile(source);
    assembly.push_str(native_allocator());
    assembly.push_str(native_panic_reporter());
    assembly.push_str(forbidden_destructor_probe());
    let result = run_native_assembly_output(&assembly);

    assert_eq!(result.status.code(), Some(1));
    assert!(!result.stderr.is_empty());
}

fn reverse_array_probe() -> &'static str {
    concat!(
        ".bss\n",
        ".p2align 3\n",
        ".Lstatic_array_constructed: .zero 8\n",
        ".Lstatic_array_destroyed: .zero 8\n",
        ".section .rodata\n",
        ".Lstatic_array_output: .ascii \"2\\n1\\n\"\n",
        ".text\n",
        ".globl test_next\n",
        ".type test_next, @function\n",
        "test_next:\n",
        "    mov rax, qword ptr [rip + .Lstatic_array_constructed]\n",
        "    add rax, 1\n",
        "    mov qword ptr [rip + .Lstatic_array_constructed], rax\n",
        "    ret\n",
        ".size test_next, .-test_next\n",
        ".globl test_destroy\n",
        ".type test_destroy, @function\n",
        "test_destroy:\n",
        "    mov r10, qword ptr [rip + .Lstatic_array_destroyed]\n",
        "    mov r11, 2\n",
        "    sub r11, r10\n",
        "    cmp rdi, r11\n",
        "    jne .Lstatic_array_bad_order\n",
        "    add r10, 1\n",
        "    mov qword ptr [rip + .Lstatic_array_destroyed], r10\n",
        "    mov rax, 1\n",
        "    mov rdi, 1\n",
        "    lea rsi, [rip + .Lstatic_array_output]\n",
        "    lea rsi, [rsi + r10 * 2 - 2]\n",
        "    mov rdx, 2\n",
        "    syscall\n",
        "    ret\n",
        ".Lstatic_array_bad_order:\n",
        "    mov rax, 60\n",
        "    mov rdi, 99\n",
        "    syscall\n",
        ".size test_destroy, .-test_destroy\n",
    )
}

fn forbidden_destructor_probe() -> &'static str {
    concat!(
        ".text\n",
        ".globl test_forbidden\n",
        ".type test_forbidden, @function\n",
        "test_forbidden:\n",
        "    mov rax, 60\n",
        "    mov rdi, 99\n",
        "    syscall\n",
        ".size test_forbidden, .-test_forbidden\n",
    )
}
