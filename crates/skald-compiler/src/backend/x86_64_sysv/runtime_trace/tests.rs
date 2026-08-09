use std::path::Path;

use crate::{
    backend::{BackendInput, RuntimeTracePolicy, Target},
    identity::CallableId,
    resolve::resolve_module_graph,
    source::Span,
    test_support::{
        assembly_relocations, assert_system_assembler_accepts, load_module_sources,
        lower_hir_to_final_mir, lower_source_to_final_mir_with_sources, FinalMirWithSources,
    },
    typeck::type_check,
};

use super::*;
use crate::backend::x86_64_sysv::{
    emit,
    machine::{AssemblyProgram, AssemblyRuntimeTraceMetadata},
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
    let mut metadata = Metadata::new(BackendInput::with_runtime_trace(
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
    let mut metadata = Metadata::new(BackendInput::without_runtime_trace(&fixture.mir));

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
    assert_eq!(enabled, omitted, "no request means no trace-only emission");
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
    let mut metadata = Metadata::new(BackendInput::with_runtime_trace(
        &fixture.mir,
        &fixture.sources,
    ));

    let error = metadata.request_location(callable, span).unwrap_err();
    assert_eq!(error.callable(), Some(callable));
    assert!(error.message().contains("different source"));
}
