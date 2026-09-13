use crate::{
    backend::Target,
    driver::{compile_source_to_assembly, CompilationError},
};

#[test]
fn emits_final_static_fields_through_verified_lifecycle_and_backend_paths() {
    let artifact = compile_source_to_assembly(
        "final-fields.ska",
        concat!(
            "class Values {\n",
            "  final static version: u64 = 1u;\n",
            "  init() {}\n",
            "}\n",
            "fn main() -> i64 { var version: u64 = Values.version; return 0; }\n",
        ),
        Target::X86_64SysV,
    )
    .unwrap();

    assert!(artifact.report.diagnostics.is_empty());
    assert!(artifact.assembly.contains(".globl main"));
}

#[test]
fn primitive_static_programs_cross_the_complete_driver_pipeline() {
    let artifact = compile_source_to_assembly(
        "static-field.ska",
        concat!(
            "class State { static count: i64; init() {} }\n",
            "fn main() -> i64 { State.count = 1; return State.count; }\n",
        ),
        Target::X86_64SysV,
    )
    .expect("primitive static fields must lower through x86-64");
    assert!(artifact
        .assembly
        .contains(".Lska.class.main.State.c0.static.s0"));
    assert!(artifact.assembly.contains("\n.bss\n"));
}

#[test]
fn synthesized_static_initializers_cross_the_complete_driver_pipeline() {
    let artifact = compile_source_to_assembly(
        "static-initializer.ska",
        concat!(
            "class State { static count: i64 = 42; init() {} }\n",
            "fn main() -> i64 { return State.count; }\n",
        ),
        Target::X86_64SysV,
    )
    .expect("verified static initializer must lower through x86-64 startup");

    assert!(artifact.assembly.contains(".Lska.static.initialize:"));
    assert!(artifact.assembly.contains(
        "    call ska_rt_abi_v9\n    call .Lska.static.initialize\n    call .Lska.fn.main.main.f0"
    ));
}

#[test]
fn static_lifetime_cycles_are_reported_as_source_diagnostics_before_synthesis() {
    let CompilationError::Diagnostics(report) = compile_source_to_assembly(
        "static-cycle.ska",
        concat!(
            "fn read_left() -> i64 { return State.left; }\n",
            "fn read_right() -> i64 { return State.right; }\n",
            "class State {\n",
            "  static left: i64 = read_right();\n",
            "  static right: i64 = read_left();\n",
            "  init() {}\n",
            "}\n",
            "fn main() -> i64 { return State.left; }\n",
        ),
        Target::X86_64SysV,
    )
    .unwrap_err() else {
        panic!("static lifetime cycles must be ordinary source diagnostics");
    };

    assert_eq!(report.diagnostics.len(), 1);
    let diagnostic = report.diagnostics.iter().next().unwrap();
    assert_eq!(
        diagnostic.code,
        crate::passes::static_lifecycle::STATIC_LIFECYCLE_DEPENDENCY_CYCLE
    );
    assert!(diagnostic
        .labels
        .iter()
        .any(|label| label.message.contains("DirectCall")));
}
