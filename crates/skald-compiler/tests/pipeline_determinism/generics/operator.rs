use skald_compiler::{
    backend::{emit_assembly, BackendInput, Target},
    driver::EntrySelector,
    hir::dump_hir,
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
    resolve::{dump_resolved, resolve_module_graph},
    typeck::type_check,
};

use crate::standard_library::canonical_standard_library_sources;

use super::super::{
    fixture::{write_source, ModuleFixture},
    normalization::normalize_fixture_paths,
};

pub(crate) fn generic_operator_module_phase_dump(variant: usize) -> String {
    let fixture = ModuleFixture::new("generic-operator-products", variant);
    let application = fixture.path().join("application");
    let standard_library = fixture.path().join("standard-library");
    let mut sources = vec![
        (
            application.join("app.ska"),
            "import model;\n\
             fn main() -> i64 {\n\
               var primitive: model::Adder<u64> = model::Adder<u64>();\n\
               var object_adder: model::Adder<model::Number> = model::Adder<model::Number>();\n\
               var left: model::Number = model::Number(17);\n\
               var right: model::Number = model::Number(25);\n\
               var result: model::Number = object_adder.add(left, right);\n\
               return (i64) primitive.add(17u, 25u) + result.value - 42;\n\
             }\n",
        ),
        (
            application.join("model.ska"),
            "from std::ops import OpAdd;\n\
             public class Number implements OpAdd<Number, Number> {\n\
               value: i64;\n\
               init(value: i64) { self.value = value; }\n\
               fn op_add(ref rhs: Number) -> Number { return Number(self.value + rhs.value); }\n\
             }\n\
             public class Adder<T> where T: OpAdd<T, T> {\n\
               init() {}\n\
               fn add(ref left: T, ref right: T) -> T { return left + right; }\n\
             }\n",
        ),
    ];
    sources.extend(
        canonical_standard_library_sources(&[])
            .into_iter()
            .map(|(relative, source)| (standard_library.join(relative), source)),
    );
    if variant != 0 {
        sources.reverse();
    }
    for (path, source) in sources {
        write_source(&path, source);
    }

    let configurations = if variant == 0 {
        vec![
            ProviderRootConfiguration::standard_library(standard_library.clone()),
            ProviderRootConfiguration::module_root(application.clone()),
        ]
    } else {
        vec![
            ProviderRootConfiguration::module_root(application.clone()),
            ProviderRootConfiguration::standard_library(standard_library.clone()),
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
    let resolved_dump = dump_resolved(&resolved.program);
    assert!(
        resolved_dump.contains("primitive-intrinsic AddU64"),
        "{resolved_dump}"
    );
    assert!(resolved_dump.contains("class-witness"), "{resolved_dump}");

    let checked = type_check(&resolved.program);
    assert!(checked.diagnostics.is_empty(), "{:?}", checked.diagnostics);
    let hir = checked.hir.unwrap();
    let hir_dump = dump_hir(&hir);
    assert!(hir_dump.contains("AddU64"), "{hir_dump}");
    assert!(hir_dump.contains("ObjectCall interface"), "{hir_dump}");
    assert!(!hir_dump.contains("OperatorSelection"), "{hir_dump}");

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
    assert!(!final_dump.contains("Operator"), "{final_dump}");
    let assembly = emit_assembly(
        Target::X86_64SysV,
        BackendInput::without_runtime_trace(&final_mir),
    )
    .unwrap();
    assert!(!assembly.contains("ska_rt_operator"), "{assembly}");
    let public_symbols = assembly
        .lines()
        .filter(|line| line.starts_with(".globl "))
        .collect::<Vec<_>>();
    let pre_operator_public_symbol_baseline = [".globl main"];
    assert_eq!(
        public_symbols, pre_operator_public_symbol_baseline,
        "operator protocols must not change the public symbol surface:\n{assembly}"
    );
    let runtime_references = assembly
        .lines()
        .filter_map(|line| line.trim().strip_prefix("call ska_rt_"))
        .collect::<Vec<_>>();
    let pre_operator_runtime_reference_baseline = ["abi_v9"];
    assert_eq!(
        runtime_references, pre_operator_runtime_reference_baseline,
        "operator protocols must not add a runtime ABI service:\n{assembly}"
    );
    assert!(assembly.contains(".method.op_add."), "{assembly}");

    normalize_fixture_paths(
        fixture.path(),
        format!(
            "GRAPH\n{}RESOLVED\n{}HIR\n{}PRELIMINARY MIR\n{}PLANNED MIR\n{}FINAL MIR\n{}ASSEMBLY\n{}",
            dump_module_graph(&graph),
            resolved_dump,
            hir_dump,
            preliminary_dump,
            planned_dump,
            final_dump,
            assembly,
        ),
    )
}
