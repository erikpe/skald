use super::*;
use crate::{backend::x86_64_sysv::layout::DataLayout, driver::compile_source_to_assembly};

fn compile(source: &str) -> String {
    compile_source_to_assembly("static-initialization.ska", source, Target::X86_64SysV)
        .expect("initialized static source must compile")
        .assembly
}

#[test]
fn lowers_the_complete_initialized_static_storage_matrix() {
    let source = concat!(
        "class Item {\n",
        "  value: i64; init(value: i64) { self.value = value; }\n",
        "  copy(ref other: Item) { self.value = other.value; }\n",
        "}\n",
        "class State {\n",
        "  static signed: i64 = 1; static unsigned: u64 = 2u;\n",
        "  static byte: u8 = 3u8; static ratio: f64 = 4.0; static ready: bool = true;\n",
        "  static item: Item = Item(5); static maybe_number: i64? = 6;\n",
        "  static maybe_item: Item? = Item(7);\n",
        "  static owner: shared Item = new Item(8);\n",
        "  static maybe_owner: shared? Item = new Item(9);\n",
        "  static values: i64[] = i64[]{10, 11};\n",
        "  init() {}\n",
        "}\n",
        "fn main() -> i64 { return 0; }\n",
    );
    let program = lower_source_to_final_mir(source);
    let layout = DataLayout::compute(&program).unwrap();
    let assembly = emit_assembly(Target::X86_64SysV, &program).unwrap();

    for field in &program.class(ClassId::new(1)).unwrap().static_fields {
        let symbol = format!(".Lska.class.main.State.c1.static.s{}", field.id.index());
        let expected_size = layout.ty(field.ty).unwrap().size();
        assert!(
            assembly.contains(&format!("{symbol}:\n    .zero {expected_size}")),
            "missing initialized slot {symbol} with size {expected_size}"
        );
        assert!(assembly.contains(&format!("{symbol}.initialize:")));
    }
    assert!(assembly.contains(".Lska.static.initialize:"));
    assert!(!assembly.contains(".globl .Lska.static"));
    assert!(!assembly.contains(".globl .Lska.class.main.State.c1.static"));
    assert_system_assembler_accepts(&assembly);
}

#[test]
fn initialized_owning_values_execute_before_entry() {
    let source = concat!(
        "class Item {\n",
        "  value: i64; init(value: i64) { self.value = value; }\n",
        "  copy(ref other: Item) { self.value = other.value; }\n",
        "}\n",
        "class State {\n",
        "  static number: i64 = 1; static item: Item = Item(2);\n",
        "  static maybe_number: i64? = 3; static maybe_item: Item? = Item(4);\n",
        "  static owner: shared Item = new Item(5);\n",
        "  static maybe_owner: shared? Item = new Item(6);\n",
        "  static values: i64[] = i64[]{10, 11}; init() {}\n",
        "}\n",
        "fn main() -> i64 {\n",
        "  var item: Item = State.item; var maybe_item: Item = State.maybe_item!;\n",
        "  var owner: shared Item = State.owner;\n",
        "  var maybe_owner: shared Item = State.maybe_owner!;\n",
        "  return State.number + item.value + State.maybe_number! +\n",
        "    maybe_item.value + owner->value + maybe_owner->value +\n",
        "    State.values[0] + State.values[1];\n",
        "}\n",
    );
    let mut assembly = compile(source);
    assembly.push_str(native_allocator());

    assert_system_assembler_accepts(&assembly);
    assert_eq!(run_native_assembly(&assembly).code(), Some(42));
}

#[test]
fn coordinator_uses_dependency_order_and_wrapper_runs_it_before_entry() {
    let source = concat!(
        "fn read_base() -> i64 { return State.base + 1; }\n",
        "class State {\n",
        "  static dependent: i64 = read_base(); static zero: i64; static base: i64 = 40;\n",
        "  init() {}\n",
        "}\n",
        "fn main() -> i64 { return State.dependent + 1; }\n",
    );
    let assembly = compile(source);
    let coordinator = function_assembly(&assembly, ".Lska.static.initialize");
    let base_call = "call .Lska.class.main.State.c0.static.s2.initialize";
    let dependent_call = "call .Lska.class.main.State.c0.static.s0.initialize";

    assert!(coordinator.find(base_call).unwrap() < coordinator.find(dependent_call).unwrap());
    assert!(!coordinator.contains("static.s1.initialize"));
    assert!(assembly.contains(
        "    call ska_rt_abi_v8\n    call .Lska.static.initialize\n    call .Lska.fn.main.main.f1"
    ));
    assert_eq!(run_native_assembly(&assembly).code(), Some(42));
}

#[test]
fn initializer_side_effects_and_post_publication_cleanup_finish_in_order() {
    let source = concat!(
        "extern fn test_step(value: i64) -> i64;\n",
        "class Item {\n",
        "  value: i64; init(value: i64) { self.value = value; }\n",
        "  copy(ref other: Item) { self.value = other.value; }\n",
        "  destroy { var ignored: i64 = test_step(self.value); }\n",
        "}\n",
        "class State {\n",
        "  static item: Item = (Item(1)); static next: i64 = test_step(2); init() {}\n",
        "}\n",
        "fn main() -> i64 { return State.next + 40; }\n",
    );
    let mut assembly = compile(source);
    assembly.push_str(ordered_step_stub());

    assert_eq!(run_native_assembly(&assembly).code(), Some(42));
}

#[test]
fn ordinary_static_access_has_no_lifecycle_guard() {
    let source = concat!(
        "class State { static value: i64 = 42; init() {} }\n",
        "fn main() -> i64 { return State.value; }\n",
    );
    let assembly = compile(source);
    let entry = function_assembly(&assembly, ".Lska.fn.main.main.f0");

    assert!(entry.contains("lea r11, [rip + .Lska.class.main.State.c0.static.s0]"));
    assert!(!entry.contains("cmp"));
    assert!(!entry.contains("ska.static.initialize"));
    assert!(!assembly.contains("static.state"));
}

#[test]
fn initialized_static_assembly_is_deterministic() {
    let source = concat!(
        "class Item { value: i64; init(value: i64) { self.value = value; } }\n",
        "fn read(item: shared Item) -> i64 { return item->value; }\n",
        "class State { static owner: shared Item = new Item(42); init() {} }\n",
        "fn main() -> i64 { return read(State.owner); }\n",
    );

    assert_eq!(compile(source), compile(source));
}

#[test]
fn malformed_lifecycle_coordinator_is_a_structured_backend_error() {
    let mut program = lower_source_to_final_mir(concat!(
        "class State { static value: i64 = 42; init() {} }\n",
        "fn main() -> i64 { return State.value; }\n",
    ));
    program
        .static_lifecycle
        .as_mut()
        .unwrap()
        .activation_mut_for_test()
        .clear();

    let error = emit_assembly(Target::X86_64SysV, &program).unwrap_err();
    assert_eq!(error.target(), Target::X86_64SysV);
    assert!(error.message().contains("input MIR failed verification"));
    assert!(error.message().contains("activation regions"));
}

fn ordered_step_stub() -> &'static str {
    concat!(
        ".bss\n",
        ".p2align 3\n",
        ".Lstatic_initializer_step:\n",
        "    .zero 8\n",
        ".text\n",
        ".globl test_step\n",
        ".type test_step, @function\n",
        "test_step:\n",
        "    mov rax, qword ptr [rip + .Lstatic_initializer_step]\n",
        "    cmp rax, 2\n",
        "    je .Lstatic_initializer_shutdown_step\n",
        "    mov r10, rax\n",
        "    add r10, 1\n",
        "    cmp rdi, r10\n",
        "    jne .Lstatic_initializer_bad_step\n",
        "    jmp .Lstatic_initializer_record_step\n",
        ".Lstatic_initializer_shutdown_step:\n",
        "    cmp rdi, 1\n",
        "    jne .Lstatic_initializer_bad_step\n",
        ".Lstatic_initializer_record_step:\n",
        "    add rax, 1\n",
        "    mov qword ptr [rip + .Lstatic_initializer_step], rax\n",
        "    mov rax, rdi\n",
        "    ret\n",
        ".Lstatic_initializer_bad_step:\n",
        "    mov rax, 60\n",
        "    mov rdi, 99\n",
        "    syscall\n",
        ".size test_step, .-test_step\n",
    )
}
