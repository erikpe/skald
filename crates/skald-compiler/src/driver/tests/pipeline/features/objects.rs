use crate::{backend::Target, driver::compile_source_to_assembly};

#[test]
fn unused_destructor_bodies_are_pruned_from_published_assembly() {
    let artifact = compile_source_to_assembly(
        "destructor.ska",
        concat!(
            "class Resource { value: i64; init() { self.value = 0; } destroy { self.value = 1; } }\n",
            "fn main() -> i64 { return 0; }\n",
        ),
        Target::X86_64SysV,
    )
    .expect("valid destructor definitions must lower deterministically");

    assert!(artifact.report.diagnostics.is_empty());
    assert!(!artifact
        .assembly
        .contains(".Lska.class.main.Resource.c0.destroy.d0"));
}

#[test]
fn unused_copy_lifecycle_bodies_are_pruned_from_published_assembly() {
    let artifact = compile_source_to_assembly(
        "copy-lifecycle.ska",
        concat!(
            "class Value {\n",
            "  value: i64;\n",
            "  init(value: i64) { self.value = value; }\n",
            "  copy(ref other: Value) { self.value = other.value; }\n",
            "  assign(ref other: Value) { self.value = other.value; }\n",
            "}\n",
            "fn main() -> i64 { return 0; }\n",
        ),
        Target::X86_64SysV,
    )
    .expect("copy lifecycle bodies must lower as MIR member definitions");

    assert!(artifact.report.diagnostics.is_empty());
    assert!(!artifact
        .assembly
        .contains(".Lska.class.main.Value.c0.init.i0"));
    assert!(!artifact
        .assembly
        .contains(".Lska.class.main.Value.c0.copy.k0"));
    assert!(!artifact
        .assembly
        .contains(".Lska.class.main.Value.c0.assign.a0"));
}

#[test]
fn static_inheritance_composes_through_the_complete_pipeline() {
    let artifact = compile_source_to_assembly(
        "inheritance.ska",
        concat!(
            "class Base { value: i64; init(value: i64) { self.value = value; } }\n",
            "class Derived extends Base { init(value: i64) { super(value); } }\n",
            "fn main() -> i64 { var value: Derived = Derived(7); return value.value; }\n",
        ),
        Target::X86_64SysV,
    )
    .unwrap();

    assert!(artifact.report.diagnostics.is_empty());
    assert!(artifact
        .assembly
        .contains("call .Lska.class.main.Base.c0.init.i0"));
}

#[test]
fn composes_the_complete_object_frontend_and_backend_pipeline() {
    let artifact = compile_source_to_assembly(
        "object.ska",
        concat!(
            "class Box { value: i64; init(value: i64) { self.value = value; } ",
            "fn get() -> i64 { return self.value; } } ",
            "fn main() -> i64 { var value: Box = Box(42); return value.get(); }",
        ),
        Target::X86_64SysV,
    )
    .unwrap();

    assert!(artifact.report.diagnostics.is_empty());
    assert!(artifact
        .assembly
        .contains("call .Lska.class.main.Box.c0.init.i0"));
    assert!(artifact
        .assembly
        .contains("call .Lska.class.main.Box.c0.method.get.m0"));
}

#[test]
fn local_copy_operations_reach_the_backend() {
    let artifact = compile_source_to_assembly(
        "copy.ska",
        concat!(
            "class Value { init() {} }\n",
            "fn main() -> i64 {\n",
            "  var source: Value = Value();\n",
            "  var copy: Value = source;\n",
            "  copy = source;\n",
            "  return 0;\n",
            "}\n",
        ),
        Target::X86_64SysV,
    )
    .expect("local copy operations must lower through the backend");
    assert!(artifact.report.diagnostics.is_empty());
    assert!(!artifact.assembly.contains("memcpy"));
}

#[test]
fn verified_cell_writes_reach_backend_lowering() {
    let artifact = compile_source_to_assembly(
        "cell-write.ska",
        concat!(
            "class Cache {\n",
            "  private cell value: i64;\n",
            "  init() { self.value = 0; }\n",
            "  fn remember(value: i64) -> unit { self.value = value; }\n",
            "}\n",
            "fn main() -> i64 { return 0; }\n",
        ),
        Target::X86_64SysV,
    )
    .unwrap();
    assert!(artifact.assembly.contains(".globl main"));
}

#[test]
fn executes_final_instance_construction_and_reads() {
    let artifact = compile_source_to_assembly(
        "final-instance.ska",
        concat!(
            "class Values {\n",
            "  final value: i64;\n",
            "  init(value: i64) { self.value = value; }\n",
            "  fn get() -> i64 { return self.value; }\n",
            "}\n",
            "fn main() -> i64 { var value: Values = Values(7); return value.get(); }\n",
        ),
        Target::X86_64SysV,
    )
    .unwrap();

    assert!(artifact.report.diagnostics.is_empty());
    assert!(artifact.assembly.contains(".globl main"));
}

#[test]
fn emits_complete_assignment_of_direct_and_transitively_final_values() {
    let cases = [
        concat!(
            "class Value { final value: i64; init(value: i64) { self.value = value; } }\n",
            "fn main() -> i64 { var left: Value = Value(1); var right: Value = Value(2); ",
            "left = right; return left.value; }\n",
        ),
        concat!(
            "class Inner { final value: i64; init(value: i64) { self.value = value; } }\n",
            "class Outer { inner: Inner; init(value: i64) { self.inner = Inner(value); } }\n",
            "fn main() -> i64 { var left: Outer = Outer(1); var right: Outer = Outer(2); ",
            "left = right; return left.inner.value; }\n",
        ),
        concat!(
            "class Base { final value: i64; init(value: i64) { self.value = value; } }\n",
            "class Derived extends Base { init(value: i64) { super(value); } }\n",
            "fn main() -> i64 { var left: Derived = Derived(1); var right: Derived = Derived(2); ",
            "left = right; return left.value; }\n",
        ),
    ];

    for source in cases {
        let artifact =
            compile_source_to_assembly("final-assignment.ska", source, Target::X86_64SysV).unwrap();
        assert!(artifact.report.diagnostics.is_empty());
        assert!(artifact.assembly.contains(".globl main"));
    }
}
