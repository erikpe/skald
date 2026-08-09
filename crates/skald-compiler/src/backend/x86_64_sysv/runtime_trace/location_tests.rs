use crate::{
    backend::{RuntimeTracePolicy, Target},
    identity::CallableId,
    mir::{MirCallTarget, MirInstruction, MirMethodCallTarget, MirTerminator},
    resolve::resolve_module_graph,
    source::Span,
    test_support::{
        load_module_sources_with_standard_library, lower_hir_to_final_mir,
        lower_source_to_final_mir_with_sources, run_native_assembly_with_runtime_trace_probe,
        FinalMirWithSources,
    },
    typeck::type_check,
};

use super::super::symbol;

fn function(fixture: &FinalMirWithSources, name: &str) -> CallableId {
    fixture
        .mir
        .declarations
        .iter()
        .find(|declaration| declaration.name == name)
        .map(|declaration| CallableId::Function(declaration.id))
        .unwrap_or_else(|| panic!("fixture function `{name}` must exist"))
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

fn trace_location_symbol(
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

fn callable_by_trace_name(fixture: &FinalMirWithSources, name: &str) -> CallableId {
    fixture
        .mir
        .executable_definitions()
        .map(|definition| definition.callable())
        .find(|callable| super::names::callable(&fixture.mir, *callable).unwrap() == name)
        .unwrap_or_else(|| panic!("fixture trace callable `{name}` must exist"))
}

fn first_call_span(fixture: &FinalMirWithSources, callable: CallableId) -> Span {
    let definition = fixture
        .mir
        .executable_definitions()
        .find(|definition| definition.callable() == callable)
        .unwrap();
    definition
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

fn first_termination_span(fixture: &FinalMirWithSources, callable: CallableId) -> Span {
    let definition = fixture
        .mir
        .executable_definitions()
        .find(|definition| definition.callable() == callable)
        .unwrap();
    definition
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

fn first_cleanup_span(fixture: &FinalMirWithSources, callable: CallableId) -> Span {
    let definition = fixture
        .mir
        .executable_definitions()
        .find(|definition| definition.callable() == callable)
        .unwrap();
    definition
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

fn trace_row(fixture: &FinalMirWithSources, callable: CallableId, span: Span) -> String {
    let source = fixture.sources.get(span.source_id()).unwrap();
    let location = source.location(span.range().start()).unwrap();
    format!(
        "  at {} (app/main.ska:{}:{})\n",
        super::names::callable(&fixture.mir, callable).unwrap(),
        location.line,
        location.column
    )
}

fn module_fixture(source: &str) -> FinalMirWithSources {
    let (_workspace, graph) =
        load_module_sources_with_standard_library("app::main", &[("app/main.ska", source)]);
    let resolved = resolve_module_graph(&graph);
    assert!(!resolved.has_errors(), "{:?}", resolved.diagnostics);
    let sources = graph.into_sources();
    let checked = type_check(&resolved.program);
    assert!(!checked.has_errors(), "{:?}", checked.diagnostics);
    FinalMirWithSources {
        sources,
        mir: lower_hir_to_final_mir(&checked.hir.unwrap()),
    }
}

fn replacement_end(function: &str, location_symbol: &str) -> usize {
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

#[test]
fn runtime_trace_location_precedes_every_explicit_call_target_after_marshalling() {
    let fixture = lower_source_to_final_mir_with_sources(
        "app/main.ska",
        concat!(
            "extern fn external(value: i64) -> i64;\n",
            "interface Reader { fn read(value: i64) -> i64; }\n",
            "class Base { init() {} virtual fn value(input: i64) -> i64 { return input; } }\n",
            "class Worker extends Base implements Reader {\n",
            "  init() { super(); }\n",
            "  override fn value(input: i64) -> i64 { return input + 1; }\n",
            "  fn read(value: i64) -> i64 { return value + 2; }\n",
            "  static fn twice(value: i64) -> i64 { return value * 2; }\n",
            "}\n",
            "fn direct(value: i64) -> i64 { return value + 3; }\n",
            "fn exercise(ref base: Base, ref reader: Reader) -> i64 {\n",
            "  var first: i64 = direct(1);\n",
            "  var second: i64 = Worker.twice(first);\n",
            "  var third: i64 = base.value(second);\n",
            "  var fourth: i64 = reader.read(third);\n",
            "  return external(fourth);\n",
            "}\n",
            "fn main() -> i64 { var worker: Worker = Worker(); return exercise(worker, worker); }\n",
        ),
    );
    let caller = function(&fixture, "exercise");
    let definition = fixture
        .mir
        .executable_definitions()
        .find(|definition| definition.callable() == caller)
        .unwrap();
    let calls = definition
        .body()
        .blocks
        .iter()
        .flat_map(|block| &block.instructions)
        .filter_map(|instruction| match instruction {
            MirInstruction::Call(call) => Some(call),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(calls.len(), 5);

    let assembly = fixture
        .emit_assembly(Target::X86_64SysV, RuntimeTracePolicy::Enabled)
        .unwrap();
    let caller_symbol = symbol::callable(&fixture.mir, caller);
    let function = assembly_function(&assembly, &caller_symbol);
    let mut previous_call = 0;
    for call in calls {
        let location = trace_location_symbol(&fixture, caller, call.span);
        let replacement_end = replacement_end(function, &location);
        assert!(replacement_end >= previous_call);
        let call_position = match call.target {
            MirCallTarget::Direct(target) => {
                let target = symbol::callable(&fixture.mir, target.into());
                let expected = format!("    call {target}\n");
                assert_eq!(
                    &function[replacement_end..replacement_end + expected.len()],
                    expected
                );
                replacement_end
            }
            MirCallTarget::Static(target) => {
                let target = symbol::callable(&fixture.mir, target.into());
                let expected = format!("    call {target}\n");
                assert_eq!(
                    &function[replacement_end..replacement_end + expected.len()],
                    expected
                );
                replacement_end
            }
            MirCallTarget::Method(MirMethodCallTarget::Direct(target)) => {
                let target = symbol::callable(&fixture.mir, target.into());
                let expected = format!("    call {target}\n");
                assert_eq!(
                    &function[replacement_end..replacement_end + expected.len()],
                    expected
                );
                replacement_end
            }
            MirCallTarget::Method(MirMethodCallTarget::Virtual { .. })
            | MirCallTarget::Interface(_) => {
                let relative = function[replacement_end..]
                    .find("    call r11\n")
                    .expect("indirect source call must use r11");
                let target_selection = &function[replacement_end..replacement_end + relative];
                assert!(target_selection.contains("r11"));
                replacement_end + relative
            }
        };
        previous_call = call_position + 1;
    }
}

#[test]
fn runtime_trace_location_is_failure_only_and_immediately_precedes_reporters() {
    let fixture = lower_source_to_final_mir_with_sources(
        "app/main.ska",
        concat!(
            "class Leaf { init() {} fn code() -> i64 { return 7; } }\n",
            "class Other { init() {} }\n",
            "fn checked_cast(ref value: Obj) -> i64 { return ((Leaf) value).code(); }\n",
            "fn divide(value: i64) -> i64 { return 84 / value; }\n",
            "fn remainder(value: i64) -> i64 { return 84 % value; }\n",
            "fn shift(value: u64, count: u64) -> u64 { return value << count; }\n",
            "fn unwrap(value: i64?) -> i64 { return value!; }\n",
            "fn cast(value: f64) -> i64 { return (i64) value; }\n",
            "fn bounds(values: i64[], index: i64) -> i64 { return values[index]; }\n",
            "fn main() -> i64 { var other: Other = Other(); return checked_cast(other); }\n",
        ),
    );
    let assembly = fixture
        .emit_assembly(Target::X86_64SysV, RuntimeTracePolicy::Enabled)
        .unwrap();

    for name in [
        "checked_cast",
        "divide",
        "remainder",
        "shift",
        "unwrap",
        "cast",
        "bounds",
    ] {
        let callable = function(&fixture, name);
        let definition = fixture
            .mir
            .executable_definitions()
            .find(|definition| definition.callable() == callable)
            .unwrap();
        let termination = definition
            .body()
            .blocks
            .iter()
            .find_map(|block| match block.terminator.as_ref() {
                Some(terminator @ MirTerminator::Terminate { .. }) => Some(terminator),
                _ => None,
            })
            .unwrap_or_else(|| panic!("`{name}` must have a reporter failure block"));
        let location = trace_location_symbol(&fixture, callable, termination.span());
        let callable_symbol = symbol::callable(&fixture.mir, callable);
        let function = assembly_function(&assembly, &callable_symbol);
        let end = replacement_end(function, &location);

        assert!(function[end..].starts_with("    call ska_rt_panic\n"));
        let failure_label = function[..end].rfind(".block_").unwrap();
        assert!(function[failure_label..end].contains(".Lska.trace.location."));
        assert!(!function[..failure_label].contains(&location));
    }
}

#[test]
fn runtime_trace_location_does_not_instrument_hard_traps_or_omitted_output() {
    let fixture = lower_source_to_final_mir_with_sources(
        "app/main.ska",
        concat!(
            "class Item { value: shared? Item; init() { self.value = none; } }\n",
            "fn main() -> i64 { var item: Item = Item(); return 0; }\n",
        ),
    );
    let enabled = fixture
        .emit_assembly(Target::X86_64SysV, RuntimeTracePolicy::Enabled)
        .unwrap();
    let omitted = fixture
        .emit_assembly(Target::X86_64SysV, RuntimeTracePolicy::Omitted)
        .unwrap();

    for function in enabled.split(".size ") {
        for trap in function.match_indices("    ud2\n").map(|(index, _)| index) {
            let preceding = &function[..trap];
            let mut instructions = preceding.lines().rev();
            let previous = instructions.next().unwrap_or_default();
            let before_previous = instructions.next().unwrap_or_default();
            assert!(
                !(previous.starts_with("    mov qword ptr [rbp - ")
                    && previous.ends_with(", r11")
                    && before_previous.starts_with("    lea r11, [rip + .Lska.trace.location.")),
                "hard traps must not receive a trace-location replacement"
            );
        }
    }
    assert!(!omitted.contains(".Lska.trace.location."));
    assert!(!omitted.contains("ska_rt_trace_top"));
}

#[test]
fn runtime_trace_location_covers_user_lifecycle_calls_from_their_source_operations() {
    let fixture = lower_source_to_final_mir_with_sources(
        "app/main.ska",
        concat!(
            "class Item {\n",
            "  marker: i64;\n",
            "  init(marker: i64) { self.marker = marker; }\n",
            "  copy(ref other: Item) { self.marker = other.marker; }\n",
            "  assign(ref other: Item) { self.marker = other.marker; }\n",
            "  destroy {}\n",
            "}\n",
            "fn main() -> i64 {\n",
            "  var first: Item = Item(1);\n",
            "  var second: Item = first;\n",
            "  second = first;\n",
            "  return 0;\n",
            "}\n",
        ),
    );
    let main = function(&fixture, "main");
    let assembly = fixture
        .emit_assembly(Target::X86_64SysV, RuntimeTracePolicy::Enabled)
        .unwrap();
    let main_symbol = symbol::callable(&fixture.mir, main);
    let function = assembly_function(&assembly, &main_symbol);
    let lifecycle = fixture
        .mir
        .executable_definitions()
        .map(|definition| definition.callable())
        .filter(|callable| {
            matches!(
                callable,
                CallableId::Initializer(_)
                    | CallableId::CopyConstructor(_)
                    | CallableId::CopyAssignment(_)
                    | CallableId::Destructor(_)
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(lifecycle.len(), 4);

    for target in lifecycle {
        let target = symbol::callable(&fixture.mir, target);
        let call = format!("    call {target}\n");
        let mut remaining = function;
        let mut count = 0;
        while let Some(position) = remaining.find(&call) {
            let preceding = &remaining[..position];
            let mut lines = preceding.lines().rev();
            assert!(lines
                .next()
                .unwrap_or_default()
                .starts_with("    mov qword ptr [rbp - "));
            assert!(lines
                .next()
                .unwrap_or_default()
                .starts_with("    lea r11, [rip + .Lska.trace.location."));
            count += 1;
            remaining = &remaining[position + call.len()..];
        }
        assert!(count > 0, "main must call lifecycle target `{target}`");
    }
}

#[test]
fn runtime_trace_location_native_chain_distinguishes_all_skald_dispatch_sites() {
    let fixture = lower_source_to_final_mir_with_sources(
        "app/main.ska",
        concat!(
            "interface Runner { fn run() -> i64; }\n",
            "class Root { init() {} virtual fn fail() -> i64 { return 1; } }\n",
            "class Worker extends Root implements Runner {\n",
            "  init() { super(); }\n",
            "  override fn fail() -> i64 { var zero: i64 = 0; return 1 / zero; }\n",
            "  fn run() -> i64 { return self.fail(); }\n",
            "  static fn start(ref runner: Runner) -> i64 { return runner.run(); }\n",
            "}\n",
            "fn direct(ref worker: Worker) -> i64 { return Worker.start(worker); }\n",
            "fn main() -> i64 { var worker: Worker = Worker(); return direct(worker); }\n",
        ),
    );
    let fail = callable_by_trace_name(&fixture, "main::Worker.fail");
    let run = callable_by_trace_name(&fixture, "main::Worker.run");
    let start = callable_by_trace_name(&fixture, "main::Worker.start");
    let direct = callable_by_trace_name(&fixture, "main::direct");
    let main = callable_by_trace_name(&fixture, "main::main");
    let expected = format!(
        "panic: integer division by zero\nstacktrace:\n{}{}{}{}{}",
        trace_row(&fixture, fail, first_termination_span(&fixture, fail)),
        trace_row(&fixture, run, first_call_span(&fixture, run)),
        trace_row(&fixture, start, first_call_span(&fixture, start)),
        trace_row(&fixture, direct, first_call_span(&fixture, direct)),
        trace_row(&fixture, main, first_call_span(&fixture, main)),
    );
    let assembly = fixture
        .emit_assembly(Target::X86_64SysV, RuntimeTracePolicy::Enabled)
        .unwrap();
    let result = run_native_assembly_with_runtime_trace_probe(&assembly);

    assert_eq!(result.status.code(), Some(1));
    assert_eq!(result.stderr, expected.as_bytes());
}

#[test]
fn runtime_trace_location_native_external_failure_stays_at_the_call_site() {
    let fixture = lower_source_to_final_mir_with_sources(
        "app/main.ska",
        concat!(
            "extern fn ska_test_external_panic() -> unit;\n",
            "fn main() -> i64 { ska_test_external_panic(); return 0; }\n",
        ),
    );
    let main = function(&fixture, "main");
    let expected = format!(
        "panic: external failure\nstacktrace:\n{}",
        trace_row(&fixture, main, first_call_span(&fixture, main))
    );
    let assembly = fixture
        .emit_assembly(Target::X86_64SysV, RuntimeTracePolicy::Enabled)
        .unwrap();
    let result = run_native_assembly_with_runtime_trace_probe(&assembly);

    assert_eq!(result.status.code(), Some(1));
    assert_eq!(result.stderr, expected.as_bytes());
}

#[test]
fn runtime_trace_location_native_destructor_failure_retains_the_cleanup_site() {
    let fixture = lower_source_to_final_mir_with_sources(
        "app/main.ska",
        concat!(
            "class Item {\n",
            "  init() {}\n",
            "  destroy { var zero: i64 = 0; var failure: i64 = 1 / zero; }\n",
            "}\n",
            "fn main() -> i64 { var item: Item = Item(); return 0; }\n",
        ),
    );
    let destructor = callable_by_trace_name(&fixture, "main::Item.destroy");
    let main = function(&fixture, "main");
    let expected = format!(
        "panic: integer division by zero\nstacktrace:\n{}{}",
        trace_row(
            &fixture,
            destructor,
            first_termination_span(&fixture, destructor)
        ),
        trace_row(&fixture, main, first_cleanup_span(&fixture, main)),
    );
    let assembly = fixture
        .emit_assembly(Target::X86_64SysV, RuntimeTracePolicy::Enabled)
        .unwrap();
    let result = run_native_assembly_with_runtime_trace_probe(&assembly);

    assert_eq!(result.status.code(), Some(1));
    assert_eq!(result.stderr, expected.as_bytes());
}

#[test]
fn runtime_trace_location_native_explicit_panic_omits_the_intrinsic_frame() {
    let fixture = module_fixture(concat!(
        "from std::error import panic;\n",
        "fn main() -> i64 { panic(\"explicit failure\"); return 0; }\n",
    ));
    let main = function(&fixture, "main");
    let expected = format!(
        "panic: explicit failure\nstacktrace:\n{}",
        trace_row(&fixture, main, first_termination_span(&fixture, main))
    );
    let assembly = fixture
        .emit_assembly(Target::X86_64SysV, RuntimeTracePolicy::Enabled)
        .unwrap();
    let main_symbol = symbol::callable(&fixture.mir, main);
    let function = assembly_function(&assembly, &main_symbol);
    let location = trace_location_symbol(&fixture, main, first_termination_span(&fixture, main));
    let end = replacement_end(function, &location);
    assert!(function[end..].starts_with("    call ska_rt_panic\n"));
    assert!(!assembly.contains("std::error::panic"));
    let result = run_native_assembly_with_runtime_trace_probe(&assembly);

    assert_eq!(result.status.code(), Some(1));
    assert_eq!(result.stderr, expected.as_bytes());
    assert!(!String::from_utf8_lossy(&result.stderr).contains("std::error::panic"));
}
