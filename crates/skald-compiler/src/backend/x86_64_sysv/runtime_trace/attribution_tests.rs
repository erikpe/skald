use std::{fs, path::Path};

use crate::{
    backend::{RuntimeTracePolicy, Target},
    identity::CallableId,
    mir::MirInstruction,
    test_support::{
        lower_source_to_final_mir_with_sources, run_native_assembly_with_runtime_trace_probe,
        FinalMirWithSources,
    },
};

use super::{super::symbol, test_support::*};

fn instructions(
    fixture: &FinalMirWithSources,
    callable: CallableId,
) -> impl Iterator<Item = &MirInstruction> {
    fixture
        .mir
        .executable_definitions()
        .find(|definition| definition.callable() == callable)
        .unwrap()
        .body()
        .blocks
        .iter()
        .flat_map(|block| &block.instructions)
}

fn span_for_location_symbol(
    fixture: &FinalMirWithSources,
    callable: CallableId,
    location_symbol: &str,
) -> crate::source::Span {
    instructions(fixture, callable)
        .map(MirInstruction::span)
        .find(|span| trace_location_symbol(fixture, callable, *span) == location_symbol)
        .unwrap_or_else(|| panic!("no MIR instruction owns trace location `{location_symbol}`"))
}

fn location_before_call(function: &str, call: &str) -> String {
    let call_match = function
        .find(call)
        .unwrap_or_else(|| panic!("assembly function must contain `{call}`"));
    let call = function[..call_match]
        .rfind('\n')
        .map_or(0, |newline| newline + 1);
    assert!(
        function[call..].starts_with("    call "),
        "`{call}` must identify a direct call"
    );
    let prefix = "    lea r11, [rip + ";
    let load = function[..call]
        .rfind(prefix)
        .expect("source-attributed call must have a preceding location load");
    let symbol_start = load + prefix.len();
    let symbol_end = function[symbol_start..]
        .find("]\n")
        .map(|offset| symbol_start + offset)
        .expect("location load must end after its symbol");
    let symbol = function[symbol_start..symbol_end].to_owned();
    let load_end = symbol_end + "]\n".len();
    let store = &function[load_end..call];
    assert_eq!(
        store.lines().count(),
        1,
        "location load and direct generated call must be separated only by the trace-top store"
    );
    assert!(
        store.starts_with("    mov qword ptr [rbp - ") && store.ends_with(", r11\n"),
        "location load must be followed by the trace-top store"
    );
    symbol
}

fn native_result(fixture: &FinalMirWithSources) -> std::process::Output {
    let assembly = fixture
        .emit_assembly(Target::X86_64SysV, RuntimeTracePolicy::Enabled)
        .unwrap();
    run_native_assembly_with_runtime_trace_probe(&assembly)
}

#[test]
fn runtime_trace_attribution_routes_all_target_calls_through_the_audited_facade() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/backend/x86_64_sysv");
    let allowed = [root.join("emit.rs"), root.join("lower/call/emission.rs")];
    let mut pending = vec![root];
    let mut violations = Vec::new();

    while let Some(path) = pending.pop() {
        let mut entries = fs::read_dir(&path)
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .collect::<Vec<_>>();
        entries.sort();
        for entry in entries {
            if entry.is_dir() {
                pending.push(entry);
            } else if entry.extension().and_then(|extension| extension.to_str()) == Some("rs")
                && !allowed.contains(&entry)
                && !entry
                    .file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| {
                        name == "tests.rs"
                            || name == "test_support.rs"
                            || name.ends_with("_tests.rs")
                    })
                && !entry
                    .components()
                    .any(|component| component.as_os_str() == "tests")
            {
                let source = fs::read_to_string(&entry).unwrap();
                let constructs_raw_call = source.lines().any(|line| {
                    let line = line.replace("MirInstruction::Call", "");
                    line.contains("Instruction::Call(")
                        || line.contains("Instruction::CallIndirect(")
                });
                if constructs_raw_call {
                    violations.push(entry);
                }
            }
        }
    }

    assert!(
        violations.is_empty(),
        "raw call construction bypasses the attribution facade: {violations:?}"
    );
}

#[test]
fn runtime_trace_attribution_updates_generated_source_boundaries_but_not_helpers() {
    let fixture = lower_source_to_final_mir_with_sources(
        "app/main.ska",
        concat!(
            "class Item { marker: i64; init() { self.marker = 0; } destroy {} }\n",
            "class Holder { values: Item[]; init() { self.values = Item[](1u); } }\n",
            "fn main() -> i64 {\n",
            "  var original: Holder = Holder();\n",
            "  var copied: Holder = original;\n",
            "  var owner: shared Item = new Item();\n",
            "  return 0;\n",
            "}\n",
        ),
    );
    let main = function(&fixture, "main");
    let assembly = fixture
        .emit_assembly(Target::X86_64SysV, RuntimeTracePolicy::Enabled)
        .unwrap();
    let main_symbol = symbol::callable(&fixture.mir, main);
    let main_function = assembly_function(&assembly, &main_symbol);

    for target in ["call ska_rt_alloc\n", "_clone\n", "_release\n"] {
        location_before_call(main_function, target);
    }

    let release = instructions(&fixture, main)
        .find_map(|instruction| match instruction {
            MirInstruction::SharedRelease(release) => Some(release.span),
            _ => None,
        })
        .expect("shared owner cleanup must release its handle");
    let release_location = trace_location_symbol(&fixture, main, release);
    let release_load = format!("    lea r11, [rip + {release_location}]\n");
    let last_owner = main_function
        .find("ownership_release_last")
        .expect("shared release must contain a last-owner edge");
    let location = main_function[last_owner..]
        .find(&release_load)
        .map(|offset| last_owner + offset)
        .expect("last-owner edge must record the source release");
    let target_load = main_function[location..]
        .find("    mov r11, qword ptr [rax + 8]\n")
        .map(|offset| location + offset)
        .expect("finalizer target selection must follow trace replacement");
    let indirect_call = main_function[target_load..]
        .find("    call r11\n")
        .map(|offset| target_load + offset)
        .expect("last-owner edge must call the generated finalizer");
    assert!(location < target_load && target_load < indirect_call);
    assert!(main_function[indirect_call..].contains("    call ska_rt_free\n"));

    for generated_symbol in assembly.lines().filter_map(|line| {
        line.strip_prefix(".type ")
            .and_then(|line| line.strip_suffix(", @function"))
            .filter(|symbol| {
                symbol.starts_with(".Lska_array_")
                    || symbol.starts_with(".Lska_shared_handle_")
                    || symbol.ends_with("finalize_complete")
            })
    }) {
        let generated = assembly_function(&assembly, generated_symbol);
        assert!(!generated.contains("ska_rt_trace_top"));
        assert!(!generated.contains("[rip + .Lska.trace.location."));
    }
}

#[test]
fn runtime_trace_attribution_updates_only_the_taken_ownership_overflow_edge() {
    let fixture = lower_source_to_final_mir_with_sources(
        "app/main.ska",
        concat!(
            "class Item { init() {} }\n",
            "fn main() -> i64 {\n",
            "  var first: shared Item = new Item();\n",
            "  var second: shared Item = first;\n",
            "  return 0;\n",
            "}\n",
        ),
    );
    let main = function(&fixture, "main");
    let copy_span = instructions(&fixture, main)
        .find_map(|instruction| match instruction {
            MirInstruction::SharedCopy(copy) => Some(copy.span),
            _ => None,
        })
        .expect("fixture must copy a shared owner");
    let location = trace_location_symbol(&fixture, main, copy_span);
    let assembly = fixture
        .emit_assembly(Target::X86_64SysV, RuntimeTracePolicy::Enabled)
        .unwrap();
    let main_symbol = symbol::callable(&fixture.mir, main);
    let function = assembly_function(&assembly, &main_symbol);
    let overflow = function.find("ownership_retain_overflow").unwrap();
    let end = replacement_end(&function[overflow..], &location);

    assert!(function[overflow + end..].starts_with("    call ska_rt_panic\n"));
    assert!(!function[..overflow].contains(&location));
}

#[test]
fn runtime_trace_attribution_native_nested_allocation_failure_uses_the_copy_operation() {
    let fixture = lower_source_to_final_mir_with_sources(
        "app/main.ska",
        concat!(
            "extern fn ska_test_fail_next_allocation() -> unit;\n",
            "class Holder { values: i64[]; init() { self.values = i64[](1u); } }\n",
            "fn main() -> i64 {\n",
            "  var original: Holder = Holder();\n",
            "  ska_test_fail_next_allocation();\n",
            "  var copied: Holder = original;\n",
            "  return 0;\n",
            "}\n",
        ),
    );
    let main = function(&fixture, "main");
    let assembly = fixture
        .emit_assembly(Target::X86_64SysV, RuntimeTracePolicy::Enabled)
        .unwrap();
    let main_symbol = symbol::callable(&fixture.mir, main);
    let main_function = assembly_function(&assembly, &main_symbol);
    let location = location_before_call(main_function, "_clone\n");
    let span = span_for_location_symbol(&fixture, main, &location);
    let expected = format!(
        "panic: memory allocation failed\nstacktrace:\n{}",
        trace_row(&fixture, main, span)
    );
    let result = run_native_assembly_with_runtime_trace_probe(&assembly);

    assert_eq!(result.status.code(), Some(1));
    assert_eq!(result.stderr, expected.as_bytes());
}

#[test]
fn runtime_trace_attribution_native_generated_copy_and_finalizer_chains_omit_helpers() {
    let copy_fixture = lower_source_to_final_mir_with_sources(
        "app/main.ska",
        concat!(
            "class Leaf { init() {} }\n",
            "class Item {\n",
            "  owner: shared Leaf;\n",
            "  init() { self.owner = new Leaf(); }\n",
            "  copy(ref other: Item) { self.owner = other.owner; }\n",
            "}\n",
            "class Container { items: Item[]; init() { self.items = Item[](1u); } }\n",
            "fn main() -> i64 {\n",
            "  var original: Container = Container();\n",
            "  var copied: Container = original;\n",
            "  return 0;\n",
            "}\n",
        ),
    );
    let copy = callable_by_trace_name(&copy_fixture, "main::Item.copy");
    let main = function(&copy_fixture, "main");
    let mut assembly = copy_fixture
        .emit_assembly(Target::X86_64SysV, RuntimeTracePolicy::Enabled)
        .unwrap();
    let main_function = assembly_function(&assembly, &symbol::callable(&copy_fixture.mir, main));
    let location = location_before_call(main_function, "_clone\n");
    let caller_span = span_for_location_symbol(&copy_fixture, main, &location);
    let copy_symbol = symbol::callable(&copy_fixture.mir, copy);
    let copy_start = assembly.find(&format!("{copy_symbol}:\n")).unwrap();
    let overflow = assembly[copy_start..]
        .find("retain_overflow_")
        .map(|offset| copy_start + offset)
        .expect("source copy body must contain a retain overflow edge");
    let count_load = "    mov rcx, qword ptr [rax]\n";
    let load = assembly[copy_start..overflow]
        .rfind(count_load)
        .map(|offset| copy_start + offset)
        .unwrap();
    assembly.replace_range(
        load..load + count_load.len(),
        "    mov rcx, 0xfffffffffffffffe\n",
    );
    let copy_function = assembly_function(&assembly, &copy_symbol);
    let copy_overflow = copy_function.find("ownership_").unwrap();
    let copy_location_load = copy_function[copy_overflow..]
        .find("    lea r11, [rip + .Lska.trace.location.")
        .map(|offset| copy_overflow + offset)
        .unwrap();
    let symbol_start = copy_location_load + "    lea r11, [rip + ".len();
    let symbol_end = copy_function[symbol_start..]
        .find("]\n")
        .map(|offset| symbol_start + offset)
        .unwrap();
    let copy_span = span_for_location_symbol(
        &copy_fixture,
        copy,
        &copy_function[symbol_start..symbol_end],
    );
    let expected = format!(
        "panic: ownership count overflow\nstacktrace:\n{}{}",
        trace_row(&copy_fixture, copy, copy_span),
        trace_row(&copy_fixture, main, caller_span),
    );
    let result = run_native_assembly_with_runtime_trace_probe(&assembly);
    assert_eq!(result.status.code(), Some(1));
    assert_eq!(result.stderr, expected.as_bytes());

    let finalizer_fixture = lower_source_to_final_mir_with_sources(
        "app/main.ska",
        concat!(
            "class Item {\n",
            "  init() {}\n",
            "  destroy { var zero: i64 = 0; var failure: i64 = 1 / zero; }\n",
            "}\n",
            "fn main() -> i64 { var owner: shared Item = new Item(); return 0; }\n",
        ),
    );
    let destructor = callable_by_trace_name(&finalizer_fixture, "main::Item.destroy");
    let main = function(&finalizer_fixture, "main");
    let release_span = instructions(&finalizer_fixture, main)
        .find_map(|instruction| match instruction {
            MirInstruction::SharedRelease(release) => Some(release.span),
            _ => None,
        })
        .unwrap();
    let expected = format!(
        "panic: integer division by zero\nstacktrace:\n{}{}",
        trace_row(
            &finalizer_fixture,
            destructor,
            first_termination_span(&finalizer_fixture, destructor)
        ),
        trace_row(&finalizer_fixture, main, release_span),
    );
    let result = native_result(&finalizer_fixture);
    assert_eq!(result.status.code(), Some(1));
    assert_eq!(result.stderr, expected.as_bytes());
}

#[test]
fn runtime_trace_attribution_native_ownership_overflow_uses_the_retain_operation() {
    let fixture = lower_source_to_final_mir_with_sources(
        "app/main.ska",
        concat!(
            "class Item { init() {} }\n",
            "fn main() -> i64 {\n",
            "  var first: shared Item = new Item();\n",
            "  var second: shared Item = first;\n",
            "  return 0;\n",
            "}\n",
        ),
    );
    let main = function(&fixture, "main");
    let span = instructions(&fixture, main)
        .find_map(|instruction| match instruction {
            MirInstruction::SharedCopy(copy) => Some(copy.span),
            _ => None,
        })
        .unwrap();
    let mut assembly = fixture
        .emit_assembly(Target::X86_64SysV, RuntimeTracePolicy::Enabled)
        .unwrap();
    let overflow = assembly.find("ownership_retain_overflow").unwrap();
    let count_load = "    mov rcx, qword ptr [rax]\n";
    let load = assembly[..overflow].rfind(count_load).unwrap();
    assembly.replace_range(
        load..load + count_load.len(),
        "    mov rcx, 0xfffffffffffffffe\n",
    );
    let expected = format!(
        "panic: ownership count overflow\nstacktrace:\n{}",
        trace_row(&fixture, main, span)
    );
    let result = run_native_assembly_with_runtime_trace_probe(&assembly);

    assert_eq!(result.status.code(), Some(1));
    assert_eq!(result.stderr, expected.as_bytes());
}

#[test]
fn runtime_trace_attribution_native_standard_library_and_static_lifecycle_visibility() {
    let standard = module_fixture(concat!(
        "from std::test import assert_true;\n",
        "fn main() -> i64 { assert_true(false); return 0; }\n",
    ));
    let assertion = callable_by_trace_name(&standard, "std::test::assert_true");
    let main = function(&standard, "main");
    let expected = format!(
        "panic: Assert failed: expected true\nstacktrace:\n{}{}",
        trace_row(
            &standard,
            assertion,
            first_termination_span(&standard, assertion)
        ),
        trace_row(&standard, main, first_call_span(&standard, main)),
    );
    let result = native_result(&standard);
    assert_eq!(result.status.code(), Some(1));
    assert_eq!(result.stderr, expected.as_bytes());

    let static_initialization = lower_source_to_final_mir_with_sources(
        "app/main.ska",
        concat!(
            "fn fail() -> i64 { var zero: i64 = 0; return 1 / zero; }\n",
            "class State { static value: i64 = fail(); init() {} }\n",
            "fn main() -> i64 { return 0; }\n",
        ),
    );
    let fail = function(&static_initialization, "fail");
    let initializer =
        callable_by_trace_name(&static_initialization, "main::State.value::<static-init>");
    let expected = format!(
        "panic: integer division by zero\nstacktrace:\n{}{}",
        trace_row(
            &static_initialization,
            fail,
            first_termination_span(&static_initialization, fail)
        ),
        trace_row(
            &static_initialization,
            initializer,
            first_call_span(&static_initialization, initializer)
        ),
    );
    let result = native_result(&static_initialization);
    assert_eq!(result.status.code(), Some(1));
    assert_eq!(result.stderr, expected.as_bytes());

    let static_shutdown = lower_source_to_final_mir_with_sources(
        "app/main.ska",
        concat!(
            "class Item {\n",
            "  init() {}\n",
            "  destroy { var zero: i64 = 0; var failure: i64 = 1 / zero; }\n",
            "}\n",
            "class State { static item: Item = Item(); init() {} }\n",
            "fn main() -> i64 { return 0; }\n",
        ),
    );
    let destructor = callable_by_trace_name(&static_shutdown, "main::Item.destroy");
    let expected = format!(
        "panic: integer division by zero\nstacktrace:\n{}",
        trace_row(
            &static_shutdown,
            destructor,
            first_termination_span(&static_shutdown, destructor)
        ),
    );
    let result = native_result(&static_shutdown);
    assert_eq!(result.status.code(), Some(1));
    assert_eq!(result.stderr, expected.as_bytes());
}
