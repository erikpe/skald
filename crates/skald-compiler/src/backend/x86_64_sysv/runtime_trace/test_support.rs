use crate::{
    identity::CallableId,
    mir::{MirInstruction, MirTerminator},
    resolve::resolve_module_graph,
    source::Span,
    test_support::{
        load_module_sources_with_standard_library, lower_hir_to_final_mir, FinalMirWithSources,
    },
    typeck::type_check,
};

use super::super::symbol;

pub(super) fn function(fixture: &FinalMirWithSources, name: &str) -> CallableId {
    fixture
        .mir
        .declarations
        .iter()
        .find(|declaration| declaration.name == name)
        .map(|declaration| CallableId::Function(declaration.id))
        .unwrap_or_else(|| panic!("fixture function `{name}` must exist"))
}

pub(super) fn assembly_function<'assembly>(
    assembly: &'assembly str,
    symbol: &str,
) -> &'assembly str {
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

pub(super) fn trace_location_symbol(
    fixture: &FinalMirWithSources,
    callable: CallableId,
    span: Span,
) -> String {
    let source = fixture
        .sources
        .get(span.source_id())
        .expect("MIR span source must be retained");
    let location = source
        .location(span.range().start())
        .expect("verified MIR span must resolve to a source location");
    symbol::trace_location(
        &fixture.mir,
        callable,
        u64::try_from(location.line).unwrap(),
        u64::try_from(location.column).unwrap(),
    )
}

pub(super) fn callable_by_trace_name(fixture: &FinalMirWithSources, name: &str) -> CallableId {
    fixture
        .mir
        .executable_definitions()
        .map(|definition| definition.callable())
        .find(|callable| super::names::callable(&fixture.mir, *callable).unwrap() == name)
        .unwrap_or_else(|| panic!("fixture trace callable `{name}` must exist"))
}

pub(super) fn first_call_span(fixture: &FinalMirWithSources, callable: CallableId) -> Span {
    definition(fixture, callable)
        .body()
        .blocks
        .iter()
        .flat_map(|block| &block.instructions)
        .find_map(|instruction| match instruction {
            MirInstruction::Call(call) => Some(call.span),
            _ => None,
        })
        .expect("fixture callable must contain a source call")
}

pub(super) fn first_termination_span(fixture: &FinalMirWithSources, callable: CallableId) -> Span {
    definition(fixture, callable)
        .body()
        .blocks
        .iter()
        .find_map(|block| match block.terminator.as_ref() {
            Some(MirTerminator::Panic { span, .. } | MirTerminator::Terminate { span, .. }) => {
                Some(*span)
            }
            _ => None,
        })
        .expect("fixture callable must contain a reporting terminator")
}

pub(super) fn first_cleanup_span(fixture: &FinalMirWithSources, callable: CallableId) -> Span {
    definition(fixture, callable)
        .body()
        .blocks
        .iter()
        .flat_map(|block| &block.instructions)
        .find_map(|instruction| match instruction {
            MirInstruction::Cleanup(cleanup) => Some(cleanup.span),
            _ => None,
        })
        .expect("fixture callable must contain source-attributed cleanup")
}

pub(super) fn trace_row(fixture: &FinalMirWithSources, callable: CallableId, span: Span) -> String {
    let source = fixture.sources.get(span.source_id()).unwrap();
    let location = source.location(span.range().start()).unwrap();
    let module = super::names::module_for_callable(&fixture.mir, callable).unwrap();
    let path = fixture
        .mir
        .modules
        .get(module)
        .unwrap()
        .source_location()
        .trace_source_path();
    format!(
        "  at {} ({}:{}:{})\n",
        super::names::callable(&fixture.mir, callable).unwrap(),
        path.display(),
        location.line,
        location.column
    )
}

pub(super) fn module_fixture(source: &str) -> FinalMirWithSources {
    let (_workspace, graph) =
        load_module_sources_with_standard_library("app::main", &[("app/main.ska", source)]);
    let resolved = resolve_module_graph(&graph);
    assert!(!resolved.has_errors(), "{:?}", resolved.diagnostics);
    let sources = graph.into_sources();
    let checked = type_check(&resolved.program);
    assert!(!checked.has_errors(), "{:?}", checked.diagnostics);
    FinalMirWithSources {
        sources,
        mir: crate::passes::run_mir_pipeline(lower_hir_to_final_mir(&checked.hir.unwrap()))
            .unwrap(),
    }
}

pub(super) fn replacement_end(function: &str, location_symbol: &str) -> usize {
    let load = format!("    lea r11, [rip + {location_symbol}]\n");
    let start = function
        .find(&load)
        .unwrap_or_else(|| panic!("function must load trace location `{location_symbol}`"));
    let store_start = start + load.len();
    let store_end = function[store_start..]
        .find('\n')
        .map(|offset| store_start + offset + 1)
        .expect("location store must end with a line feed");
    assert!(function[store_start..store_end].starts_with("    mov qword ptr [rbp - "));
    assert!(function[store_start..store_end].ends_with(", r11\n"));
    store_end
}

fn definition(
    fixture: &FinalMirWithSources,
    callable: CallableId,
) -> crate::mir::MirDefinitionRef<'_> {
    fixture
        .mir
        .executable_definitions()
        .find(|definition| definition.callable() == callable)
        .unwrap()
}
