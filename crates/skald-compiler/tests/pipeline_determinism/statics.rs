use skald_compiler::{
    backend::{emit_assembly, BackendInput, Target},
    diagnostics::render_diagnostics,
    driver::EntrySelector,
    hir::dump_hir,
    lexer::lex,
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
    syntax::parse,
    typeck::type_check,
};

use crate::standard_library::canonical_standard_library_sources;

use super::{
    fixture::{link_directory, write_source, ModuleFixture},
    normalization::normalize_module_fixture_output,
    source::{
        lower_final_hir, single_source_full_phase_dump, single_source_planned_lifecycle_dump,
        single_source_type_error_dump, StandardLibraryInput,
    },
};

pub(crate) fn static_field_phase_dump() -> String {
    single_source_full_phase_dump(
        concat!(
            "fn increment(mut ref value: i64) -> unit { value = value + 1; }\n",
            "class Base { static count: i64; static maybe: i64?; static values: i64[]; init() {} }\n",
            "class Derived extends Base { static owner: shared? Item; init() { super(); } }\n",
            "class Item { value: i64; init(value: i64) { self.value = value; } }\n",
            "fn main() -> i64 { Derived.count = 40; increment(Base.count); ",
            "Derived.maybe = Base.count; Derived.values = i64[](1u); ",
            "Derived.values[0] = Derived.maybe!; Derived.owner = new Item(1); ",
            "return Base.values[0] + Derived.owner!->value; }\n",
        ),
        StandardLibraryInput::None,
    )
}

pub(crate) fn static_initializer_lifecycle_phase_dump() -> String {
    single_source_planned_lifecycle_dump(
        concat!(
            "class Item { value: i64; init(value: i64) { self.value = value; } }\n",
            "class State {\n",
            "  static count: i64 = combine(20, 22);\n",
            "  static item: Item = Item(State.count);\n",
            "  static owner: shared Item = new Item(1);\n",
            "  static owner_copy: shared Item = State.owner;\n",
            "  static values: i64[] = i64[]{1, 2};\n",
            "  init() {}\n",
            "}\n",
            "fn combine(left: i64, right: i64) -> i64 { return left + right; }\n",
            "fn main() -> i64 {\n",
            "  State.count = 42; State.item = Item(42);\n",
            "  State.owner = new Item(1); State.owner_copy = State.owner;\n",
            "  State.values = i64[]{1, 2}; return 0;\n",
            "}\n",
        ),
        StandardLibraryInput::None,
    )
}

pub(crate) fn static_lifetime_cycle_diagnostic_dump() -> String {
    let text = concat!(
        "fn read_left() -> i64 { return State.left; }\n",
        "fn read_right() -> i64 { return State.right; }\n",
        "class State {\n",
        "  static left: i64 = read_right();\n",
        "  static right: i64 = read_left();\n",
        "  init() {}\n",
        "}\n",
        "fn main() -> i64 { return State.left; }\n",
    );
    let mut sources = SourceDatabase::new();
    let source_id = sources.add("static-lifetime-diagnostics.ska", text);
    let source = sources.get(source_id).unwrap();
    let lexed = lex(source);
    assert!(lexed.diagnostics.is_empty());
    let parsed = parse(source, &lexed.tokens);
    assert!(parsed.diagnostics.is_empty());
    let resolved = resolve(&parsed.ast);
    assert!(resolved.diagnostics.is_empty());
    let checked = type_check(&resolved.program);
    assert!(checked.diagnostics.is_empty(), "{:?}", checked.diagnostics);
    let preliminary = lower_preliminary_hir(&checked.hir.unwrap());
    let preliminary = verify_preliminary_mir(preliminary).unwrap();
    let diagnostics = plan_static_lifetimes(preliminary)
        .unwrap_err()
        .into_diagnostics();

    render_diagnostics(&sources, &diagnostics)
}

pub(crate) fn static_field_diagnostic_dump() -> String {
    single_source_type_error_dump(
        "static-field-diagnostics.ska",
        concat!(
            "class Item { init() {} }\n",
            "class Invalid {\n",
            "  private static seed: i64 = 40;\n",
            "  static answer: i64 = add(Invalid.seed, 2);\n",
            "  static item: Item;\n",
            "  static owner: shared Item;\n",
            "  init() {}\n",
            "}\n",
            "fn add(left: i64, right: i64) -> i64 { return left + right; }\n",
            "fn main() -> i64 { return 0; }\n",
        ),
        StandardLibraryInput::None,
    )
}

pub(crate) fn static_field_module_phase_dump(variant: usize) -> String {
    let fixture = ModuleFixture::new("static-field-modules", variant);
    let modules = fixture.path().join("modules");
    let modules_alias = fixture.path().join("modules-alias");
    let sources = [
        (
            modules.join("app.ska"),
            concat!(
                "import state;\n",
                "fn main() -> i64 { state::Derived.count = 42; ",
                "return state::Base.count; }\n",
            ),
        ),
        (
            modules.join("state.ska"),
            concat!(
                "import helper;\n",
                "public class Base { static count: i64; init() {} }\n",
                "public class Derived extends Base { init() { super(); } }\n",
                "public fn helper_value() -> i64 { return helper::value(); }\n",
            ),
        ),
        (
            modules.join("helper.ska"),
            concat!(
                "import state;\n",
                "public fn value() -> i64 { return state::Base.count; }\n",
            ),
        ),
    ];
    for index in if variant == 0 { [0, 1, 2] } else { [2, 1, 0] } {
        write_source(&sources[index].0, sources[index].1);
    }
    link_directory(&modules, &modules_alias);
    let configurations = if variant == 0 {
        vec![
            ProviderRootConfiguration::module_root(modules_alias),
            ProviderRootConfiguration::module_root(modules),
        ]
    } else {
        vec![
            ProviderRootConfiguration::module_root(modules),
            ProviderRootConfiguration::module_root(modules_alias),
        ]
    };
    let providers = normalize_provider_roots(fixture.path(), &configurations).unwrap();
    let entry = if variant == 0 {
        EntrySelector::Module("app".parse().unwrap())
    } else {
        EntrySelector::File(fixture.path().join("modules-alias/app.ska"))
    };
    let graph = load_module_graph(&entry, fixture.path(), &providers).unwrap();
    let resolved = resolve_module_graph(&graph);
    assert!(resolved.diagnostics.is_empty());
    let checked = type_check(&resolved.program);
    assert!(checked.diagnostics.is_empty());
    let hir = checked.hir.unwrap();
    let mir = lower_final_hir(&hir);
    let assembly = emit_assembly(
        Target::X86_64SysV,
        BackendInput::without_runtime_trace(&mir),
    )
    .unwrap();

    // Module loading records the temporary provider root and source identities
    // derived from it; their displayed spellings and spans vary with that root.
    normalize_module_fixture_output(
        fixture.path(),
        format!(
            "GRAPH\n{}RESOLVED\n{}HIR\n{}MIR\n{}ASSEMBLY\n{}",
            dump_module_graph(&graph),
            dump_resolved(&resolved.program),
            dump_hir(&hir),
            dump_mir(&mir),
            assembly,
        ),
    )
}

pub(crate) fn imported_unused_static_phase_dump() -> String {
    let fixture = ModuleFixture::new("imported-unused-static-products", 0);
    let application = fixture.path().join("application");
    let standard_library = fixture.path().join("standard-library");
    let mut sources = vec![
        (
            application.join("app.ska"),
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../tests/golden/static_fields/cases/imported_unused_static/modules/app.ska"
            )),
        ),
        (
            application.join("dormant.ska"),
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../tests/golden/static_fields/cases/imported_unused_static/modules/dormant.ska"
            )),
        ),
    ];
    sources.extend(
        canonical_standard_library_sources(&[])
            .into_iter()
            .map(|(relative, source)| (standard_library.join(relative), source)),
    );
    for (path, source) in sources {
        write_source(&path, source);
    }
    let providers = normalize_provider_roots(
        fixture.path(),
        &[
            ProviderRootConfiguration::module_root(application),
            ProviderRootConfiguration::standard_library(standard_library),
        ],
    )
    .unwrap();
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
    let final_mir = run_mir_pipeline(synthesize_static_lifecycle(
        verify_planned_mir(planned).unwrap(),
    ))
    .unwrap();
    let final_dump = dump_mir(&final_mir);
    let assembly = emit_assembly(
        Target::X86_64SysV,
        BackendInput::without_runtime_trace(&final_mir),
    )
    .unwrap();

    assert!(preliminary_dump.contains("marker"));
    assert!(planned_dump.contains("marker"));
    assert!(final_dump.contains("\"marker\""));
    assert!(!final_dump.contains("Dormant.marker"));
    assert!(!assembly.contains("marker"));

    // The temporary application and standard-library roots flow into the
    // module graph and the source identities printed by later phase dumps.
    normalize_module_fixture_output(
        fixture.path(),
        format!(
            "GRAPH\n{}RESOLVED\n{}HIR\n{}PRELIMINARY MIR\n{}PLANNED MIR\n{}FINAL MIR\n{}ASSEMBLY\n{}",
            dump_module_graph(&graph),
            dump_resolved(&resolved.program),
            dump_hir(&hir),
            preliminary_dump,
            planned_dump,
            final_dump,
            assembly,
        ),
    )
}
