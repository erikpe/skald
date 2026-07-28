use super::super::{frame, layout};
use super::*;

const NESTED_DESTRUCTION_SOURCE: &str = concat!(
    "extern fn ska_rt_println_i64(value: i64) -> unit;\n",
    "class Root {\n",
    "  marker: i64; owner: Owner;\n",
    "  init(left: i64, right: i64, owner_marker: i64, marker: i64) {\n",
    "    self.marker = marker; self.owner = Owner(left, right, owner_marker);\n",
    "  }\n",
    "  destroy { ska_rt_println_i64(self.marker); }\n",
    "}\n",
    "class Owner {\n",
    "  prefix: u8; left: Leaf; ratio: f64; empty: Empty; right: Leaf; suffix: bool; marker: i64;\n",
    "  init(left: i64, right: i64, marker: i64) {\n",
    "    self.prefix = 1u8; self.left = Leaf(left); self.ratio = 2.5; self.empty = Empty();\n",
    "    self.right = Leaf(right); self.suffix = true; self.marker = marker;\n",
    "  }\n",
    "  destroy { ska_rt_println_i64(self.marker); }\n",
    "}\n",
    "class Leaf {\n",
    "  small: u8; value: i64; enabled: bool;\n",
    "  init(value: i64) { self.small = 1u8; self.value = value; self.enabled = true; }\n",
    "  destroy { ska_rt_println_i64(self.value); }\n",
    "}\n",
    "class Empty { init() {} }\n",
    "fn main() -> i64 { var root: Root = Root(11, 12, 10, 100); return 37; }\n",
);

#[test]
fn lowers_body_then_recursive_fields_through_existing_place_and_call_machinery() {
    let first = assembly(NESTED_DESTRUCTION_SOURCE);
    let second = assembly(NESTED_DESTRUCTION_SOURCE);

    assert_eq!(first, second);
    assert!(first.contains(".Lska_class_0_destroy_0:"));
    assert!(first.contains(".Lska_class_1_destroy_0:"));
    assert!(first.contains(".Lska_class_2_destroy_0:"));
    assert!(first.contains(concat!(
        "    lea rdi, [rbp - 96]\n",
        "    lea rsi, [rbp - 96]\n",
        "    lea rdx, [rip + .Lska_class_0_dispatch]\n",
        "    call .Lska_class_0_destroy_0\n",
        "    lea rdi, [rbp - 88]\n",
        "    lea rsi, [rbp - 88]\n",
        "    lea rdx, [rip + .Lska_class_1_dispatch]\n",
        "    call .Lska_class_1_destroy_0\n",
        "    lea rdi, [rbp - 40]\n",
        "    lea rsi, [rbp - 40]\n",
        "    lea rdx, [rip + .Lska_class_2_dispatch]\n",
        "    call .Lska_class_2_destroy_0\n",
        "    lea rdi, [rbp - 80]\n",
        "    lea rsi, [rbp - 80]\n",
        "    lea rdx, [rip + .Lska_class_2_dispatch]\n",
        "    call .Lska_class_2_destroy_0\n",
    )));
    assert_system_assembler_accepts(&first);
}

#[test]
fn cleanup_preserves_the_precomputed_return_value_without_aggregate_runtime_operations() {
    let program = lower_text(NESTED_DESTRUCTION_SOURCE);
    let data_layout = layout::DataLayout::compute(&program).unwrap();
    let main = program.definitions.get(program.entry_function).unwrap();
    let frame = frame::FrameLayout::plan(main.into(), &data_layout).unwrap();
    let return_value = main
        .body
        .blocks
        .iter()
        .find_map(|block| match &block.terminator {
            Some(MirTerminator::Return {
                value: Some(value), ..
            }) => Some(*value),
            _ => None,
        })
        .unwrap();
    let output = emit_assembly(Target::X86_64SysV, &program).unwrap();
    let cleanup = output
        .find("call .Lska_class_0_destroy_0")
        .expect("root cleanup must call its user body");
    let expected_reload = format!(
        "mov rax, qword ptr [rbp - {}]",
        frame.value(return_value).unsigned_abs()
    );
    let reload = output[cleanup..]
        .find(&expected_reload)
        .expect("return value must reload from its frame home after cleanup");

    assert!(reload > 0);
    for excluded in ["memcpy", "malloc", "free"] {
        assert!(!output.contains(excluded));
    }
}

#[test]
fn floating_return_values_reload_into_xmm0_after_cleanup_calls() {
    let source = concat!(
        "class Resource { init() {} destroy {} }\n",
        "fn compute() -> f64 { var resource: Resource = Resource(); return 2.5; }\n",
        "fn main() -> i64 { return 0; }\n",
    );
    let program = lower_text(source);
    let data_layout = layout::DataLayout::compute(&program).unwrap();
    let compute = program.definitions.get(FunctionId::new(0)).unwrap();
    let frame = frame::FrameLayout::plan(compute.into(), &data_layout).unwrap();
    let return_value = match &compute.body.blocks[0].terminator {
        Some(MirTerminator::Return {
            value: Some(value), ..
        }) => *value,
        _ => panic!("compute must return its floating value"),
    };
    let output = emit_assembly(Target::X86_64SysV, &program).unwrap();
    let function = output
        .split(".Lska_fn_1:")
        .next()
        .expect("compute assembly must precede main");
    let cleanup = function
        .find("call .Lska_class_0_destroy_0")
        .expect("compute must clean its local resource");
    let expected_reload = format!(
        "movsd xmm0, qword ptr [rbp - {}]",
        frame.value(return_value).unsigned_abs()
    );

    assert!(function[cleanup..].contains(&expected_reload));
}

#[test]
fn malformed_cleanup_is_rejected_before_instruction_selection() {
    let mut program = lower_text(concat!(
        "class Resource { init() {} destroy {} }\n",
        "fn main() -> i64 { var resource: Resource = Resource(); return 0; }\n",
    ));
    let main = program
        .definitions
        .get_mut_for_test(program.entry_function)
        .unwrap();
    let cleanup = main.body.blocks[0]
        .instructions
        .iter_mut()
        .find_map(|instruction| match instruction {
            MirInstruction::Cleanup(cleanup) => Some(cleanup),
            _ => None,
        })
        .unwrap();
    cleanup.target = ClassId::new(99);

    let error = emit_assembly(Target::X86_64SysV, &program).unwrap_err();
    assert!(error.message().contains("input MIR failed verification"));
    assert!(error
        .message()
        .contains("cleanup target c99 is not declared"));
}

#[test]
fn projected_receiver_cleanup_reuses_indirect_place_addressing() {
    let mut program = lower_text(concat!(
        "class Owner { prefix: u8; leaf: Leaf; init() { self.prefix = 1u8; self.leaf = Leaf(); } mut fn release() -> unit {} }\n",
        "class Leaf { value: i64; init() { self.value = 0; } destroy {} }\n",
        "fn main() -> i64 { return 0; }\n",
    ));
    let owner = ClassId::new(0);
    let leaf = ClassId::new(1);
    let method = MethodId::new(owner, 0);
    let leaf_field = FieldId::new(owner, 1);
    let definition = program
        .member_definitions
        .get_mut_for_test(method.into())
        .unwrap();
    definition.body.blocks[0]
        .instructions
        .push(MirInstruction::Cleanup(MirCleanup {
            destination: MirPlace::base(definition.receiver.unwrap()).project_field(leaf_field),
            target: leaf,
            span: definition.span,
        }));

    let output = emit_assembly(Target::X86_64SysV, &program).unwrap();
    assert!(output.contains(concat!(
        ".Lska_class_0_method_0_block_0:\n",
        "    mov rdi, qword ptr [rbp - 8]\n",
        "    lea rdi, [rdi + 8]\n",
        "    mov rsi, qword ptr [rbp - 8]\n",
        "    lea rsi, [rsi + 8]\n",
        "    lea rdx, [rip + .Lska_class_1_dispatch]\n",
        "    call .Lska_class_1_destroy_0\n",
    )));
}
