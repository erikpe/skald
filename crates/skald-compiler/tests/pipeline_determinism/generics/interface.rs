use std::path::PathBuf;

use skald_compiler::{
    backend::{emit_assembly, BackendInput, Target},
    diagnostics::render_diagnostics,
    driver::EntrySelector,
    hir::dump_hir,
    lexer::{dump_tokens, lex},
    mir::{dump_mir, dump_preliminary_mir, lower_preliminary_hir, verify_preliminary_mir},
    module::{
        dump_module_graph, load_module_graph, normalize_provider_roots, ProviderRootConfiguration,
    },
    passes::{
        run_mir_pipeline,
        static_lifecycle::{
            dump_planned_mir, plan_static_lifetimes, synthesize_static_lifecycle,
            verify_planned_mir,
        },
    },
    resolve::{dump_resolved, resolve, resolve_module_graph},
    source::SourceDatabase,
    syntax::{dump_ast, parse},
    typeck::type_check,
};

use super::super::{
    fixture::{link_directory, write_source, ModuleFixture},
    normalization::normalize_fixture_paths,
};

pub(crate) fn generic_interface_module_phase_dump(variant: usize) -> String {
    let fixture = ModuleFixture::new("generic-interface-products", variant);
    let modules = fixture.path().join("modules");
    let modules_alias = fixture.path().join("modules-alias");
    link_directory(&modules, &modules_alias);
    let sources = [
        (
            modules.join("app.ska"),
            "import api;\n\
             import model;\n\
             from api import Value as ImportedValue;\n\
             from model import Both as RenamedBoth;\n\
             class Value { init() {} }\n\
             fn inspect(ref value: Obj) -> i64 {\n\
               var exact: bool = value is ImportedValue<i64>;\n\
               var other: bool = value is api::Value<u64>;\n\
               return ((ImportedValue<i64>) value).value();\n\
             }\n\
             fn nested(ref value: api::Value<model::Box<i64>>) -> unit {}\n\
             fn cycles(ref left: api::Left<i64>, ref right: api::Right<i64>) -> unit {}\n\
             fn main() -> i64 {\n\
               var value: RenamedBoth = RenamedBoth(42);\n\
               var reader: model::Reader<RenamedBoth> = model::Reader<RenamedBoth>();\n\
               return reader.read(value) + value.name() - inspect(value) - 7;\n\
             }\n",
        ),
        (
            modules.join("api.ska"),
            "public interface Value<T> { fn value() -> T; }\n\
             public interface Named<T> { fn name() -> i64; }\n\
             public interface Left<T> { fn cross(ref value: Right<T>) -> T; }\n\
             public interface Right<T> { fn cross(ref value: Left<T>) -> T; }\n",
        ),
        (
            modules.join("model.ska"),
            "import api;\n\
             public class Box<T> { value: T; init(value: T) { self.value = value; } }\n\
             public class Both implements api::Value<i64>, api::Named<i64>, api::Named<u64> {\n\
               amount: i64;\n\
               init(amount: i64) { self.amount = amount; }\n\
               fn value() -> i64 { return self.amount; }\n\
               fn name() -> i64 { return 7; }\n\
             }\n\
             public class Reader<Source> where Source: api::Value<i64> {\n\
               init() {}\n\
               fn read(ref source: Source) -> i64 { return source.value(); }\n\
             }\n",
        ),
    ];
    for index in if variant == 0 { [0, 1, 2] } else { [2, 0, 1] } {
        write_source(&sources[index].0, sources[index].1);
    }

    let configurations = if variant == 0 {
        vec![
            ProviderRootConfiguration::module_root(PathBuf::from("modules-alias")),
            ProviderRootConfiguration::module_root(PathBuf::from("modules")),
        ]
    } else {
        vec![
            ProviderRootConfiguration::module_root(PathBuf::from("modules")),
            ProviderRootConfiguration::module_root(PathBuf::from(
                "modules-alias/..//modules-alias",
            )),
        ]
    };
    let providers = normalize_provider_roots(fixture.path(), &configurations).unwrap();
    let graph = load_module_graph(
        &EntrySelector::Module("app".parse().unwrap()),
        fixture.path(),
        &providers,
    )
    .unwrap();
    let resolved = resolve_module_graph(&graph);
    assert!(
        resolved.diagnostics.is_empty(),
        "{:?}",
        resolved.diagnostics
    );
    let checked = type_check(&resolved.program);
    assert!(checked.diagnostics.is_empty(), "{:?}", checked.diagnostics);
    let hir = checked.hir.unwrap();
    let preliminary = lower_preliminary_hir(&hir);
    let preliminary_dump = dump_preliminary_mir(&preliminary);
    let preliminary = verify_preliminary_mir(preliminary).unwrap();
    let planned = plan_static_lifetimes(preliminary).unwrap();
    let planned_dump = dump_planned_mir(&planned);
    let mir = run_mir_pipeline(synthesize_static_lifecycle(
        verify_planned_mir(planned).unwrap(),
    ))
    .unwrap();
    let assembly = emit_assembly(
        Target::X86_64SysV,
        BackendInput::with_runtime_trace(&mir, graph.sources()),
    )
    .unwrap();

    normalize_fixture_paths(
        fixture.path(),
        format!(
            "GRAPH\n{}RESOLVED\n{}HIR\n{}PRELIMINARY MIR\n{}PLANNED MIR\n{}FINAL MIR\n{}ASSEMBLY\n{}",
            dump_module_graph(&graph),
            dump_resolved(&resolved.program),
            dump_hir(&hir),
            preliminary_dump,
            planned_dump,
            dump_mir(&mir),
            assembly,
        ),
    )
}

pub(crate) fn generic_interface_diagnostic_dump() -> String {
    let mut sources = SourceDatabase::new();
    let source_id = sources.add(
        "generic-interface-specialization.ska",
        "interface Chain<T> { fn next() -> Chain<T>; }\n\
         interface Expand<T> { fn next() -> Expand<T[]>; }\n\
         fn first(ref chain: Chain<(shared Item)?>, ref failed: Expand<i64>) -> unit {}\n\
         fn second(ref chain: Chain<shared? Item>, ref failed: Expand<i64>) -> unit {}\n\
         class Item {}\n\
         fn main() -> i64 { return 0; }\n",
    );
    let source = sources.get(source_id).unwrap();
    let lexed = lex(source);
    assert!(lexed.diagnostics.is_empty());
    let parsed = parse(source, &lexed.tokens);
    assert!(parsed.diagnostics.is_empty());
    let resolved = resolve(&parsed.ast);

    format!(
        "TOKENS\n{}AST\n{}RESOLVED\n{}DIAGNOSTICS\n{}",
        dump_tokens(source, &lexed.tokens),
        dump_ast(&parsed.ast),
        dump_resolved(&resolved.program),
        render_diagnostics(&sources, &resolved.diagnostics),
    )
}
