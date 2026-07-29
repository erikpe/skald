use std::os::unix::process::ExitStatusExt;

use super::*;

const TYPE_OPERATION_TYPES: &str = "\
interface Marker { fn mark() -> i64; }\n\
interface Extra { fn extra() -> i64; }\n\
class Root implements Marker {\n\
  init() {}\n\
  fn mark() -> i64 { return 3; }\n\
}\n\
class Middle extends Root { init() { super(); } }\n\
class Leaf extends Middle {\n\
  init() { super(); }\n\
  fn total(a: i64, b: i64, c: i64, d: i64, e: i64, f: i64,\n\
      g: i64, x: f64, y: f64) -> i64 {\n\
    return a + b + c + d + e + f + g;\n\
  }\n\
}\n\
class Both implements Marker, Extra {\n\
  init() {}\n\
  fn mark() -> i64 { return 5; }\n\
  fn extra() -> i64 { return 7; }\n\
}\n\
class Other { init() {} }\n";

#[test]
fn emits_unique_class_metadata_membership_checks_and_failure_reports() {
    let output = assembly(&format!(
        "{TYPE_OPERATION_TYPES}\
         fn inspect(ref any: Obj) -> bool {{ return any is Leaf; }}\n\
         fn cast_value(ref any: Obj) -> i64 {{\n\
           return ((Leaf) any).total(1, 2, 3, 4, 5, 6, 7, 1.0, 2.0);\n\
         }}\n\
         fn main() -> i64 {{ return 0; }}\n"
    ));

    for class in 0..5 {
        assert!(output.contains(&format!(".Lska_class_{class}_dispatch:")));
    }
    assert!(output.contains("cmp r11, rcx"));
    assert!(output.contains("call ska_rt_panic"));
    assert_system_assembler_accepts(&output);
}

#[test]
fn executes_class_interface_and_obj_tests_across_deep_and_multiple_views() {
    let source = format!(
        "{TYPE_OPERATION_TYPES}\
         fn class_code(ref any: Obj) -> i64 {{\n\
           if (any is Root) {{ return 1; }} else {{ return 2; }}\n\
         }}\n\
         fn interface_code(ref any: Obj) -> i64 {{\n\
           if (any is Marker) {{ return 4; }} else {{ return 8; }}\n\
         }}\n\
         fn extra_code(ref any: Obj) -> i64 {{\n\
           if (any is Extra) {{ return 16; }} else {{ return 32; }}\n\
         }}\n\
         fn from_marker(ref marker: Marker) -> i64 {{\n\
           if (marker is Leaf) {{ return 64; }} else {{ return 128; }}\n\
         }}\n\
         fn main() -> i64 {{\n\
           var leaf: Leaf = Leaf();\n\
           var both: Both = Both();\n\
           var other: Other = Other();\n\
           return class_code(leaf) + class_code(other)\n\
               + interface_code(leaf) + interface_code(other)\n\
               + extra_code(both) + extra_code(leaf) + from_marker(leaf);\n\
         }}\n"
    );

    let result = run_native_assembly_output(&assembly(&source));
    assert_eq!(result.status.code(), Some(127));
    assert!(result.stdout.is_empty());
    assert!(result.stderr.is_empty());
}

#[test]
fn materializes_class_and_interface_casts_through_nested_stack_pressure() {
    let source = format!(
        "{TYPE_OPERATION_TYPES}\
         fn cast_leaf(ref any: Obj, a: i64, b: i64, c: i64, d: i64,\n\
             e: i64, f: i64, g: i64, x: f64, y: f64) -> i64 {{\n\
           return ((Leaf) any).total(a, b, c, d, e, f, g, x, y);\n\
         }}\n\
         fn cast_marker(ref any: Obj) -> i64 {{\n\
           return ((Marker) any).mark();\n\
         }}\n\
         fn static_cast(ref leaf: Leaf) -> i64 {{\n\
           return ((Root) leaf).mark();\n\
         }}\n\
         fn main() -> i64 {{\n\
           var leaf: Leaf = Leaf();\n\
           return cast_leaf(leaf, 1, 2, 3, 4, 5, 6, 7, 1.0, 2.0)\n\
               + cast_marker(leaf) + static_cast(leaf);\n\
         }}\n"
    );

    let result = run_native_assembly_output(&assembly(&source));
    assert_eq!(result.status.code(), Some(34));
    assert!(result.stdout.is_empty());
    assert!(result.stderr.is_empty());
}

#[test]
fn successful_cast_preserves_temporary_cleanup_order() {
    let source = format!(
        "{TYPE_OPERATION_TYPES}\
         extern fn ska_rt_println_i64(value: i64) -> unit;\n\
         class Token {{\n\
           value: i64;\n\
           init(value: i64) {{ self.value = value; }}\n\
           destroy {{ ska_rt_println_i64(self.value); }}\n\
         }}\n\
         fn consume(value: Token) -> unit {{}}\n\
         fn cast_leaf(ref any: Obj) -> i64 {{\n\
           consume(Token(42));\n\
           return ((Leaf) any).total(1, 2, 3, 4, 5, 6, 7, 1.0, 2.0);\n\
         }}\n\
         fn main() -> i64 {{ var leaf: Leaf = Leaf(); return cast_leaf(leaf); }}\n"
    );
    let mut output = assembly(&source);
    output.push_str(println_i64_stub());

    let result = run_native_assembly_output(&output);
    assert_eq!(result.status.code(), Some(28));
    assert_eq!(result.stdout, b"42\n42\n");
    assert!(result.stderr.is_empty());
}

#[test]
fn checked_cast_places_execute_as_receivers_fields_and_alias_arguments() {
    let source = "\
         class Root { init() {} virtual fn code() -> i64 { return 1; } }\n\
         class Leaf extends Root {\n\
           value: i64;\n\
           init(value: i64) { super(); self.value = value; }\n\
           override fn code() -> i64 { return self.value; }\n\
         }\n\
         fn take(ref leaf: Leaf) -> i64 { return leaf.value; }\n\
         fn inspect(ref any: Obj) -> i64 {\n\
           return ((Leaf) any).code() + ((Leaf) any).value + take((Leaf) any);\n\
         }\n\
         fn main() -> i64 { var leaf: Leaf = Leaf(7); return inspect(leaf); }\n";

    let result = run_native_assembly_output(&assembly(source));
    assert_eq!(result.status.code(), Some(21));
    assert!(result.stdout.is_empty());
    assert!(result.stderr.is_empty());
}

#[test]
fn checked_cast_places_execute_through_owning_copy_operations() {
    let source = "\
         class Root {\n\
           value: i64;\n\
           init(value: i64) { self.value = value; }\n\
           copy(ref other: Root) { self.value = other.value + 10; }\n\
           assign(ref other: Root) { self.value = other.value + 20; }\n\
         }\n\
         class Leaf extends Root {\n\
           extra: i64;\n\
           init(value: i64, extra: i64) { super(value); self.extra = extra; }\n\
         }\n\
         class Holder {\n\
           item: Leaf;\n\
           init(ref source: Obj) { self.item = (Leaf) source; }\n\
           mut fn replace(ref source: Obj) -> unit { self.item = (Leaf) source; }\n\
           fn total() -> i64 { return self.item.value + self.item.extra; }\n\
         }\n\
         fn consume(value: Leaf) -> i64 { return value.value + value.extra; }\n\
         fn ordered(marker: i64, value: Leaf) -> i64 {\n\
           return marker + value.value + value.extra;\n\
         }\n\
         fn copied(ref source: Obj) -> Leaf { return (Leaf) source; }\n\
         fn exercise(destination: Leaf, ref source: Obj) -> i64 {\n\
           var local: Leaf = (Leaf) source;\n\
           var sliced: Root = (Root) source;\n\
           var produced: Root = (Root) Leaf(3, 4);\n\
           var returned: Leaf = copied(source);\n\
           var holder: Holder = Holder(source);\n\
           holder.replace(source);\n\
           destination = (Leaf) destination;\n\
           destination = (Leaf) source;\n\
           return local.value + local.extra + sliced.value + produced.value\n\
               + returned.value + returned.extra + holder.total()\n\
               + destination.value + destination.extra + consume((Leaf) source)\n\
               + ordered(5, (Leaf) source);\n\
         }\n\
         fn main() -> i64 {\n\
           var leaf: Leaf = Leaf(1, 2);\n\
           var destination: Leaf = Leaf(0, 0);\n\
           return exercise(destination, leaf);\n\
         }\n";

    let result = run_native_assembly_output(&assembly(source));
    assert_eq!(result.status.code(), Some(127));
    assert!(result.stdout.is_empty());
    assert!(result.stderr.is_empty());
}

#[test]
fn produced_cast_copy_sources_are_destroyed_exactly_once_after_copying() {
    let source = "\
         extern fn ska_rt_println_i64(value: i64) -> unit;\n\
         class Token {\n\
           value: i64;\n\
           init(value: i64) { self.value = value; }\n\
           copy(ref other: Token) { self.value = other.value; }\n\
           destroy { ska_rt_println_i64(self.value); }\n\
         }\n\
         fn main() -> i64 {\n\
           var copied: Token = (Token) Token(42);\n\
           return 0;\n\
         }\n";
    let mut output = assembly(source);
    output.push_str(println_i64_stub());

    let result = run_native_assembly_output(&output);
    assert_eq!(result.stdout, b"42\n42\n");
    assert_eq!(result.status.code(), Some(0));
    assert!(result.stderr.is_empty());
}

#[test]
fn failed_checked_cast_terminates_at_its_consumer() {
    let source = "\
         class Leaf { init() {} fn code() -> i64 { return 7; } }\n\
         class Other { init() {} }\n\
         fn inspect(ref any: Obj) -> i64 { return ((Leaf) any).code(); }\n\
         fn main() -> i64 { var other: Other = Other(); return inspect(other); }\n";

    let result = run_native_assembly_output(&assembly(source));
    assert!(!result.status.success());
    assert_eq!(result.status.signal(), Some(4));
    assert!(result.stdout.is_empty());
    assert!(result.stderr.is_empty());
}

#[test]
fn corrupt_type_operation_metadata_is_rejected_before_instruction_selection() {
    let mut program = lower_text(&format!(
        "{TYPE_OPERATION_TYPES}\
         fn inspect(ref any: Obj) -> bool {{ return any is Leaf; }}\n\
         fn main() -> i64 {{ return 0; }}\n"
    ));
    let inspect = program
        .definitions
        .get_mut_for_test(FunctionId::new(0))
        .expect("inspect definition");
    let target = inspect
        .body
        .blocks
        .iter_mut()
        .flat_map(|block| &mut block.instructions)
        .find_map(|instruction| match instruction {
            MirInstruction::Assign(assignment) => match &mut assignment.rvalue.kind {
                MirRvalueKind::TypeTest { target, .. } => Some(target),
                _ => None,
            },
            _ => None,
        })
        .expect("runtime type test");
    *target = MirViewTarget::Class(ClassId::new(99));

    let error = emit_assembly(Target::X86_64SysV, &program)
        .expect_err("corrupt type-operation metadata must not reach selection");
    assert!(error.message().contains("input MIR failed verification"));
    assert!(error.message().contains("type-test target is not declared"));
}
