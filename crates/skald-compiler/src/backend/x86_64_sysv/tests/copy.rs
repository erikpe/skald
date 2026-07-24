use super::super::{frame, layout};
use super::*;

const COPY_SOURCE: &str = concat!(
    "class Value {\n",
    "  small: u8; value: i64; ratio: f64; enabled: bool;\n",
    "  init(value: i64) { self.small = 1u8; self.value = value; self.ratio = 2.5; self.enabled = true; }\n",
    "  init(ref other: Value) { self.small = other.small; self.value = other.value; self.ratio = other.ratio; self.enabled = other.enabled; }\n",
    "  assign(ref other: Value) { self.small = other.small; self.value = other.value; self.ratio = other.ratio; self.enabled = other.enabled; }\n",
    "}\n",
    "class Pair { left: Value; right: Value; init(value: i64) { self.left = Value(value); self.right = Value(value); } }\n",
    "fn main() -> i64 {\n",
    "  var source: Pair = Pair(7);\n",
    "  var destination: Pair = source;\n",
    "  destination = destination;\n",
    "  return destination.right.value;\n",
    "}\n",
);

#[test]
fn lowers_user_and_synthesized_copy_in_mir_defined_field_order() {
    let output = assembly(COPY_SOURCE);

    assert!(output.contains(".Lska_class_0_init_0:"));
    assert!(output.contains(".Lska_class_0_copy_0:"));
    assert!(!output.contains(".Lska_class_0_init_1"));
    assert_eq!(output.matches("call .Lska_class_0_copy_0").count(), 2);
    assert_eq!(output.matches("call .Lska_class_0_assign_0").count(), 2);
    assert!(output.contains("movzx"));
    assert!(output.contains("movsd"));
    assert!(!output.contains("memcpy"));
    assert_system_assembler_accepts(&output);
}

#[test]
fn preserves_live_aliases_and_scalar_homes_across_copy_calls() {
    let output = assembly(concat!(
        "class Value {\n",
        "  value: i64;\n",
        "  init(value: i64) { self.value = value; }\n",
        "  init(ref other: Value) { self.value = other.value + 5; }\n",
        "}\n",
        "fn preserve(ref source: Value, marker: i64) -> i64 {\n",
        "  var copy: Value = source;\n",
        "  return marker + copy.value;\n",
        "}\n",
        "fn main() -> i64 { var source: Value = Value(2); return preserve(source, 40); }\n",
    ));

    assert_eq!(run_native_assembly(&output).code(), Some(47));
}

#[test]
fn malformed_copy_mir_is_rejected_before_instruction_selection() {
    let mut program = lower_text(COPY_SOURCE);
    let function = program
        .definitions
        .get_mut_for_test(program.entry_function)
        .unwrap();
    let copy = function.body.blocks[0]
        .instructions
        .iter_mut()
        .find_map(|instruction| match instruction {
            MirInstruction::CopyConstruct(copy) => Some(copy),
            _ => None,
        })
        .unwrap();
    copy.operation = MirSelectedCopyOperation::User(CopyConstructorId::new(copy.class, 1));

    let error = emit_assembly(Target::X86_64SysV, &program).unwrap_err();
    assert!(error.message().contains("input MIR failed verification"));
    assert!(error
        .message()
        .contains("does not match the class capability"));
}

#[test]
fn allocates_and_destroys_bounded_full_expression_temporaries() {
    let mut program = lower_text(concat!(
        "extern fn ska_rt_println_i64(value: i64) -> unit;\n",
        "class Value { value: i64; init(value: i64) { self.value = value; } destroy { ska_rt_println_i64(self.value); } }\n",
        "fn main() -> i64 { var source: Value = Value(42); return 0; }\n",
    ));
    let class = ClassId::new(0);
    let function = program
        .definitions
        .get_mut_for_test(program.entry_function)
        .unwrap();
    let span = function.span;
    let temporary = StorageId::new(function.function, function.storage.len());
    function.storage.push(MirStorage {
        id: temporary,
        source: None,
        name: "temporary0".to_owned(),
        kind: MirStorageKind::Temporary,
        ty: MirType::Class(class),
        span,
    });
    let initialize = function.body.blocks[0]
        .instructions
        .iter()
        .position(|instruction| matches!(instruction, MirInstruction::Initialize(_)))
        .unwrap();
    function.body.blocks[0].instructions.insert(
        initialize + 1,
        MirInstruction::CopyConstruct(MirCopyConstruction {
            destination: MirPlace::base(temporary),
            source: MirPlace::base(function.storage[0].id),
            class,
            operation: MirSelectedCopyOperation::Synthesized(class),
            span,
        }),
    );
    function.body.blocks[0].instructions.insert(
        initialize + 2,
        MirInstruction::EndFullExpression(MirEndFullExpression {
            temporaries: vec![MirCleanup {
                destination: MirPlace::base(temporary),
                target: class,
                span,
            }],
            span,
        }),
    );
    verify_mir(&program).unwrap();

    let data = layout::DataLayout::compute(&program).unwrap();
    let main = program.definitions.get(program.entry_function).unwrap();
    let planned = frame::FrameLayout::plan(main.into(), &data).unwrap();
    assert_ne!(
        planned.storage(main.storage[0].id),
        planned.storage(temporary)
    );

    let mut output = emit_assembly(Target::X86_64SysV, &program).unwrap();
    output.push_str(println_i64_stub());
    let result = run_native_assembly_output(&output);
    assert!(
        result.status.success(),
        "temporary copy program failed with {:?}: {}",
        result.status,
        String::from_utf8_lossy(&result.stderr)
    );
    assert_eq!(result.stdout, b"42\n42\n");
    assert!(result.stderr.is_empty());
}
