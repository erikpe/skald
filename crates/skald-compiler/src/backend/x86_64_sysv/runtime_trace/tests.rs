use std::path::Path;

use crate::{
    backend::{BackendInput, RuntimeTracePolicy, Target},
    identity::CallableId,
    resolve::resolve_module_graph,
    source::Span,
    test_support::{
        assembly_relocations, assert_system_assembler_accepts, load_module_sources,
        lower_hir_to_final_mir, lower_source_to_final_mir_with_sources,
        run_native_assembly_with_runtime_trace_probe, FinalMirWithSources,
    },
    typeck::type_check,
};

use super::*;
use crate::backend::x86_64_sysv::{
    emit,
    machine::{AssemblyProgram, AssemblyRuntimeTraceMetadata},
    symbol,
};

const CALLABLE_SOURCE: &str = concat!(
    "class Config { init() {} }\n",
    "class Widget {\n",
    "  value: i64;\n",
    "  static cache: i64 = 7;\n",
    "  init(value: i64, ref config: Config, mut ref scratch: Config) {\n",
    "    self.value = value;\n",
    "  }\n",
    "  init(values: i64[], maybe: i64?, owner: shared Config, optional: shared? Config) {\n",
    "    self.value = 0;\n",
    "  }\n",
    "  copy(ref other: Widget) { self.value = other.value; }\n",
    "  assign(ref other: Widget) { self.value = other.value; }\n",
    "  destroy {}\n",
    "  fn read() -> i64 { return self.value; }\n",
    "  static fn make() -> i64 { return 1; }\n",
    "}\n",
    "fn helper() -> i64 { return Widget.make(); }\n",
    "fn main() -> i64 { return helper(); }\n",
);

fn callable_fixture(path: impl AsRef<Path>) -> FinalMirWithSources {
    lower_source_to_final_mir_with_sources(path, CALLABLE_SOURCE)
}

fn function(fixture: &FinalMirWithSources, name: &str) -> CallableId {
    fixture
        .mir
        .declarations
        .iter()
        .find(|declaration| declaration.name == name)
        .map(|declaration| CallableId::Function(declaration.id))
        .unwrap_or_else(|| panic!("fixture function `{name}` must exist"))
}

fn requested_metadata(
    fixture: &FinalMirWithSources,
    requests: &[(CallableId, Span)],
) -> AssemblyRuntimeTraceMetadata {
    let metadata = Metadata::new(BackendInput::with_runtime_trace(
        &fixture.mir,
        &fixture.sources,
    ));
    for &(callable, span) in requests {
        metadata.request_location(callable, span).unwrap();
    }
    metadata.finish()
}

fn metadata_assembly(runtime_trace: AssemblyRuntimeTraceMetadata) -> String {
    emit::emit(&AssemblyProgram {
        functions: Vec::new(),
        static_slots: Vec::new(),
        dispatch_tables: Vec::new(),
        literal_backings: Vec::new(),
        panic_messages: Vec::new(),
        runtime_trace,
    })
}

#[test]
fn runtime_trace_metadata_formats_every_source_callable_semantically() {
    let fixture = callable_fixture("workspace/app.ska");
    let requests = fixture
        .mir
        .executable_definitions()
        .map(|definition| (definition.callable(), definition.span()))
        .collect::<Vec<_>>();
    let metadata = requested_metadata(&fixture, &requests);
    assert!(metadata
        .strings
        .iter()
        .all(|string| !string.bytes.windows(7).any(|window| window == b".init.i")));
    let assembly = metadata_assembly(metadata);

    for name in [
        "main::main",
        "main::helper",
        "main::Config.init()",
        "main::Widget.init(i64, ref main::Config, mut ref main::Config)",
        "main::Widget.init(i64[], i64?, shared main::Config, shared? main::Config)",
        "main::Widget.copy",
        "main::Widget.assign",
        "main::Widget.destroy",
        "main::Widget.read",
        "main::Widget.make",
        "main::Widget.cache::<static-init>",
    ] {
        assert!(assembly.contains(name), "missing semantic name `{name}`");
    }
    assert!(assembly.contains("workspace/app.ska"));
}

#[test]
fn runtime_trace_metadata_uses_provider_relative_module_paths() {
    let (_workspace, graph) = load_module_sources(
        "app::main",
        &[("app/main.ska", "fn main() -> i64 { return 0; }\n")],
    );
    let resolved = resolve_module_graph(&graph);
    assert!(!resolved.has_errors(), "{:?}", resolved.diagnostics);
    let sources = graph.into_sources();
    let checked = type_check(&resolved.program);
    assert!(!checked.has_errors(), "{:?}", checked.diagnostics);
    let mir = lower_hir_to_final_mir(&checked.hir.unwrap());
    let fixture = FinalMirWithSources { sources, mir };
    let callable = function(&fixture, "main");
    let span = fixture
        .mir
        .executable_definitions()
        .find(|definition| definition.callable() == callable)
        .unwrap()
        .span();
    let assembly = metadata_assembly(requested_metadata(&fixture, &[(callable, span)]));

    assert!(assembly.contains("app/main.ska"));
    assert!(!assembly.contains(_workspace.path().to_string_lossy().as_ref()));
}

#[test]
fn runtime_trace_metadata_resolves_unicode_columns_and_interns_requested_records() {
    let source = "// aé\nfn first() -> i64 { return 1; }\nfn main() -> i64 { return first(); }\n";
    let fixture = lower_source_to_final_mir_with_sources("app/main.ska", source);
    let callable = function(&fixture, "main");
    let source_file = fixture.sources.get(fixture.mir.span.source_id()).unwrap();
    let offset = source.find('é').unwrap() + 'é'.len_utf8();
    let span = source_file.span(offset, offset).unwrap();
    let metadata = requested_metadata(&fixture, &[(callable, span), (callable, span)]);

    assert_eq!(metadata.contexts.len(), 1);
    assert_eq!(metadata.locations.len(), 1);
    assert_eq!(metadata.locations[0].line, 1);
    assert_eq!(metadata.locations[0].column, 6);
    assert_eq!(metadata.strings.len(), 2, "name and path are interned once");
    assert!(metadata
        .strings
        .iter()
        .all(|string| string.bytes != b"main::first"));
}

#[test]
fn runtime_trace_metadata_is_request_order_independent_and_assembler_valid() {
    let fixture = callable_fixture("app/main.ska");
    let mut requests = fixture
        .mir
        .executable_definitions()
        .map(|definition| (definition.callable(), definition.span()))
        .collect::<Vec<_>>();
    let forward = metadata_assembly(requested_metadata(&fixture, &requests));
    requests.reverse();
    let reverse = metadata_assembly(requested_metadata(&fixture, &requests));

    assert_eq!(forward, reverse);
    assert!(forward.contains(".section .data.rel.ro.local,\"aw\",@progbits"));
    assert!(forward.contains(".size .Lska.trace.context."));
    assert!(forward.contains(", 32"));
    assert!(forward.contains(".size .Lska.trace.location."));
    assert!(forward.contains(", 24"));
    assert_system_assembler_accepts(&forward);
    let relocations = assembly_relocations(&forward);
    assert_eq!(
        relocations.matches("R_X86_64_64").count(),
        requests.len() * 3,
        "each context has two pointers and each location has one"
    );
}

#[test]
fn runtime_trace_metadata_omission_never_requests_or_emits_trace_data() {
    let fixture = callable_fixture("secret/path.ska");
    let callable = function(&fixture, "main");
    let span = fixture
        .mir
        .executable_definitions()
        .find(|definition| definition.callable() == callable)
        .unwrap()
        .span();
    let metadata = Metadata::new(BackendInput::without_runtime_trace(&fixture.mir));

    assert_eq!(metadata.request_location(callable, span).unwrap(), None);
    let assembly = metadata_assembly(metadata.finish());
    assert!(!assembly.contains("secret/path.ska"));
    assert!(!assembly.contains(".Lska.trace."));
    assert!(!assembly.contains(".data.rel.ro"));

    let enabled = fixture
        .emit_assembly(Target::X86_64SysV, RuntimeTracePolicy::Enabled)
        .unwrap();
    let omitted = fixture
        .emit_assembly(Target::X86_64SysV, RuntimeTracePolicy::Omitted)
        .unwrap();
    assert_ne!(
        enabled, omitted,
        "enabled lowering requests activation locations"
    );
    assert!(enabled.contains("ska_rt_trace_top@tpoff"));
    assert!(!omitted.contains(".Lska.trace."));
}

#[test]
fn runtime_trace_metadata_escapes_paths_into_one_safe_line() {
    assert_eq!(
        escape_path_bytes(b"dir\\part\nnext\rrow\ttab\x01.ska"),
        b"dir\\\\part\\nnext\\rrow\\ttab\\x01.ska"
    );
    assert_eq!(
        escape_path_bytes("mód.ska".as_bytes()),
        "mód.ska".as_bytes()
    );
    assert_eq!(escape_path_bytes(b"bad\xff.ska"), b"bad\\xff.ska");
}

#[test]
fn runtime_trace_metadata_rejects_invalid_source_ownership() {
    let mut fixture = callable_fixture("app.ska");
    let callable = function(&fixture, "main");
    let other = fixture.sources.add("other.ska", "x");
    let span = fixture.sources.get(other).unwrap().span(0, 0).unwrap();
    let metadata = Metadata::new(BackendInput::with_runtime_trace(
        &fixture.mir,
        &fixture.sources,
    ));

    let error = metadata.request_location(callable, span).unwrap_err();
    assert_eq!(error.callable(), Some(callable));
    assert!(error.message().contains("different source"));
}

fn assembly_function<'assembly>(assembly: &'assembly str, symbol: &str) -> &'assembly str {
    let start_marker = format!("{symbol}:\n");
    let start = assembly
        .find(&start_marker)
        .unwrap_or_else(|| panic!("assembly must define `{symbol}`"));
    let end_marker = format!(".size {symbol}, .-{symbol}");
    let end = assembly[start..]
        .find(&end_marker)
        .map(|offset| start + offset + end_marker.len())
        .expect("assembly function must have a size directive");
    &assembly[start..end]
}

fn trace_frame_fixture() -> FinalMirWithSources {
    lower_source_to_final_mir_with_sources(
        "app/main.ska",
        concat!(
            "fn increment(value: i64) -> i64 { return value + 1; }\n",
            "fn main() -> i64 { return increment(41); }\n",
        ),
    )
}

#[test]
fn runtime_trace_frame_emits_the_frozen_push_and_pop_sequences() {
    let fixture = trace_frame_fixture();
    let increment = function(&fixture, "increment");
    let symbol = symbol::callable(&fixture.mir, increment);
    let assembly = fixture
        .emit_assembly(Target::X86_64SysV, RuntimeTracePolicy::Enabled)
        .unwrap();
    let function = assembly_function(&assembly, &symbol);
    let lines = function.lines().collect::<Vec<_>>();
    let push = lines
        .iter()
        .position(|line| *line == "    mov r11, qword ptr fs:ska_rt_trace_top@tpoff")
        .expect("traced source body must load the prior TLS top");

    assert!(lines[..push].contains(&"    sub rsp, 48"));
    assert!(
        lines[..push].iter().any(|line| line.contains(", rdi")),
        "parameter spill must precede trace publication"
    );
    assert!(lines[push + 1].starts_with("    mov qword ptr [rbp - "));
    assert!(lines[push + 1].ends_with(", r11"));
    assert!(lines[push + 2].starts_with("    lea r11, [rip + .Lska.trace.location."));
    assert!(lines[push + 3].starts_with("    mov qword ptr [rbp - "));
    assert!(lines[push + 3].ends_with(", r11"));
    let previous = lines[push + 1]
        .strip_prefix("    mov qword ptr ")
        .and_then(|line| line.strip_suffix(", r11"))
        .unwrap();
    assert_eq!(lines[push + 4], format!("    lea r11, {previous}"));
    assert_eq!(
        lines[push + 5],
        "    mov qword ptr fs:ska_rt_trace_top@tpoff, r11"
    );

    let pop = lines
        .iter()
        .rposition(|line| line.starts_with("    mov r11, qword ptr [rbp - "))
        .expect("normal return must restore the prior trace top");
    assert_eq!(
        lines[pop + 1],
        "    mov qword ptr fs:ska_rt_trace_top@tpoff, r11"
    );
    assert!(lines[pop + 2].starts_with("    mov rax, qword ptr [rbp - "));
}

#[test]
fn runtime_trace_frame_adds_one_aligned_record_only_to_source_bodies() {
    let fixture = lower_source_to_final_mir_with_sources(
        "app/main.ska",
        concat!(
            "class Item { value: i64; init(value: i64) { self.value = value; } }\n",
            "fn main() -> i64 { var owner: shared Item = new Item(7); return owner->value; }\n",
        ),
    );
    let enabled = fixture
        .emit_assembly(Target::X86_64SysV, RuntimeTracePolicy::Enabled)
        .unwrap();
    let omitted = fixture
        .emit_assembly(Target::X86_64SysV, RuntimeTracePolicy::Omitted)
        .unwrap();
    let definition_count = fixture.mir.executable_definitions().count();

    assert_eq!(
        enabled
            .matches("mov r11, qword ptr fs:ska_rt_trace_top@tpoff")
            .count(),
        definition_count,
        "only source definitions publish trace frames"
    );
    assert_eq!(
        enabled.matches(".size .Lska.trace.context.").count(),
        definition_count,
        "each source definition owns exactly one trace context"
    );
    assert!(
        enabled.matches(".size .Lska.trace.location.").count() > definition_count,
        "source operations may add locations without adding trace frames"
    );
    assert!(!assembly_function(&enabled, "main").contains("ska_rt_trace_top"));
    assert!(
        enabled.contains("ska_rt_alloc"),
        "fixture must generate runtime/helper calls"
    );
    assert!(!omitted.contains("ska_rt_trace_top"));
    assert!(!omitted.contains(".Lska.trace."));
}

#[test]
fn runtime_trace_frame_uses_local_exec_tls_relocations_and_caller_saved_scratch() {
    let fixture = trace_frame_fixture();
    let assembly = fixture
        .emit_assembly(Target::X86_64SysV, RuntimeTracePolicy::Enabled)
        .unwrap();
    assert_system_assembler_accepts(&assembly);
    let relocations = assembly_relocations(&assembly);

    assert_eq!(relocations.matches("R_X86_64_TPOFF32").count(), 6);
    assert!(relocations.contains("ska_rt_trace_top"));
    for callee_saved in ["rbx", "r12", "r13", "r14", "r15"] {
        assert!(!assembly.contains(callee_saved));
    }
}

#[test]
fn runtime_trace_frame_omission_preserves_the_pre_trace_function_shape() {
    let fixture = trace_frame_fixture();
    let increment = function(&fixture, "increment");
    let symbol = symbol::callable(&fixture.mir, increment);
    let omitted = fixture
        .emit_assembly(Target::X86_64SysV, RuntimeTracePolicy::Omitted)
        .unwrap();
    let function = assembly_function(&omitted, &symbol);

    assert_eq!(
        function,
        format!(
            "{symbol}:\n\
             \x20   push rbp\n\
             \x20   mov rbp, rsp\n\
             \x20   sub rsp, 32\n\
             \x20   mov qword ptr [rbp - 8], rdi\n\
             {symbol}.block_0:\n\
             \x20   mov rax, qword ptr [rbp - 8]\n\
             \x20   mov qword ptr [rbp - 16], rax\n\
             \x20   mov rax, 1\n\
             \x20   mov qword ptr [rbp - 24], rax\n\
             \x20   mov rax, qword ptr [rbp - 16]\n\
             \x20   mov rcx, qword ptr [rbp - 24]\n\
             \x20   add rax, rcx\n\
             \x20   mov qword ptr [rbp - 32], rax\n\
             \x20   mov rax, qword ptr [rbp - 32]\n\
             \x20   jmp {symbol}.epilogue\n\
             {symbol}.epilogue:\n\
             \x20   leave\n\
             \x20   ret\n\
             .size {symbol}, .-{symbol}"
        )
    );
    assert!(!function.contains("r11"));
    assert!(!function.contains("ska_rt_trace_top"));
    assert!(!omitted.contains(".Lska.trace."));
}

#[test]
fn runtime_trace_frame_recursive_panic_reports_newest_first_with_real_runtime() {
    let fixture = lower_source_to_final_mir_with_sources(
        "app/main.ska",
        concat!(
            "fn recurse(depth: i64) -> i64 { if (depth == 0) { return 1 / depth; } return recurse(depth - 1); }\n",
            "fn main() -> i64 { return recurse(3); }\n",
        ),
    );
    let assembly = fixture
        .emit_assembly(Target::X86_64SysV, RuntimeTracePolicy::Enabled)
        .unwrap();
    let result = run_native_assembly_with_runtime_trace_probe(&assembly);

    assert_eq!(result.status.code(), Some(1));
    assert!(result.stdout.is_empty());
    assert_eq!(
        result.stderr,
        concat!(
            "panic: integer division by zero\n",
            "stacktrace:\n",
            "  at main::recurse (app/main.ska:1:58)\n",
            "  at main::recurse (app/main.ska:1:78)\n",
            "  at main::recurse (app/main.ska:1:78)\n",
            "  at main::recurse (app/main.ska:1:78)\n",
            "  at main::main (app/main.ska:2:27)\n",
        )
        .as_bytes()
    );
}

#[test]
fn runtime_trace_frame_mixed_returns_preserve_results_and_restore_null() {
    let fixture = lower_source_to_final_mir_with_sources(
        "app/main.ska",
        concat!(
            "extern fn ska_test_trace_depth() -> i64;\n",
            "class Value { marker: i64; init(marker: i64) { self.marker = marker; } }\n",
            "fn scalar() -> i64 { return 4; }\n",
            "fn floating() -> f64 { return 2.5; }\n",
            "fn nothing() -> unit {}\n",
            "fn object() -> Value { return Value(5); }\n",
            "fn owner() -> shared Value { return new Value(6); }\n",
            "fn optional_owner() -> shared? Value { return owner(); }\n",
            "fn recurse(depth: i64) -> i64 { if (depth == 0) { return 1; } return recurse(depth - 1) + 1; }\n",
            "fn main() -> i64 {\n",
            "  var value: Value = object();\n",
            "  var shared_value: shared Value = owner();\n",
            "  var maybe: shared? Value = optional_owner();\n",
            "  nothing();\n",
            "  if (floating() == 2.5) {\n",
            "    return scalar() + value.marker + shared_value->marker + maybe!->marker + recurse(3) + ska_test_trace_depth();\n",
            "  }\n",
            "  return 1;\n",
            "}\n",
        ),
    );
    let assembly = fixture
        .emit_assembly(Target::X86_64SysV, RuntimeTracePolicy::Enabled)
        .unwrap();
    let result = run_native_assembly_with_runtime_trace_probe(&assembly);

    assert_eq!(result.status.code(), Some(26), "{assembly}");
    assert!(result.stdout.is_empty());
    assert!(result.stderr.is_empty());
}
