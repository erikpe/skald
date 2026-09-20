use crate::{
    mir::MirProgram,
    resolve::resolve_module_graph,
    test_support::{
        load_module_sources_with_standard_library_overrides, lower_hir_to_final_mir,
        CANONICAL_IO_SOURCE,
    },
    typeck::type_check,
};

const IO_INTRINSIC_DECLARATIONS: &str = concat!(
    "import std::str;\n",
    "from std::error import panic;\n",
    "\n",
    "intrinsic fn _io_standard_handle(stream: u8) -> i64;\n",
    "intrinsic fn _io_open(ref path: u8[], mode: u8) -> i64;\n",
    "intrinsic fn _io_read(handle: i64, mut ref destination: u8[], offset: u64) -> i64;\n",
    "intrinsic fn _io_write(handle: i64, ref source: u8[], offset: u64) -> i64;\n",
    "intrinsic fn _io_close(handle: i64) -> i64;\n",
);

const MINIMAL_STRING_LANGUAGE_ITEM: &str = concat!(
    "public class Str {\n",
    "  private _storage: shared u8[];\n",
    "  private _start: i64;\n",
    "  private _length: u64;\n",
    "  private cell _hash_code: u64?;\n",
    "  init() {\n",
    "    self._storage = new u8[]();\n",
    "    self._start = 0;\n",
    "    self._length = 0u;\n",
    "    self._hash_code = none;\n",
    "  }\n",
    "}\n",
);

pub(crate) fn io_program() -> MirProgram {
    io_program_with_additional_bodies("")
}

pub(crate) fn io_program_with_additional_bodies(additional: &str) -> MirProgram {
    io_program_with_app_and_additional_bodies(
        "import std::io;\nfn main() -> i64 { return 0; }\n",
        additional,
    )
}

pub(crate) fn io_program_with_app_and_additional_bodies(app: &str, additional: &str) -> MirProgram {
    io_graph_with_app_and_additional_bodies(app, additional, lower_graph)
}

#[cfg(test)]
pub(crate) fn verified_io_fixture_with_sources(
    app: &str,
    additional: &str,
) -> crate::test_support::FinalMirWithSources {
    io_graph_with_app_and_additional_bodies(app, additional, |graph| {
        let mir = crate::passes::verify_final_mir(lower_graph_ref(&graph)).unwrap();
        crate::test_support::FinalMirWithSources {
            sources: graph.into_sources(),
            mir,
        }
    })
}

fn io_graph_with_app_and_additional_bodies<T>(
    app: &str,
    additional: &str,
    finish: impl FnOnce(crate::module::ModuleGraph) -> T,
) -> T {
    let io = format!(
        "{IO_INTRINSIC_DECLARATIONS}\n{}{additional}",
        concat!(
            "public fn standard(stream: u8) -> i64 { return _io_standard_handle(stream); }\n",
            "public fn open(ref path: u8[], mode: u8) -> i64 { return _io_open(path, mode); }\n",
            "public fn read(handle: i64, mut ref destination: u8[], offset: u64) -> i64 {\n",
            "  return _io_read(handle, destination, offset);\n",
            "}\n",
            "public fn write(handle: i64, ref source: u8[], offset: u64) -> i64 {\n",
            "  return _io_write(handle, source, offset);\n",
            "}\n",
            "public fn close(handle: i64) -> i64 { return _io_close(handle); }\n",
        )
    );
    let (_workspace, graph) = load_module_sources_with_standard_library_overrides(
        "app",
        &[("app.ska", app)],
        &[
            ("std/io.ska", &io),
            ("std/str.ska", MINIMAL_STRING_LANGUAGE_ITEM),
        ],
    );
    finish(graph)
}

pub(crate) fn standard_io_program(app: &str) -> MirProgram {
    lower_standard_io_program(app, CANONICAL_IO_SOURCE)
}

pub(crate) fn standard_io_program_with_additional_bodies(
    app: &str,
    additional: &str,
) -> MirProgram {
    let io = format!("{CANONICAL_IO_SOURCE}\n{additional}");
    lower_standard_io_program(app, &io)
}

fn lower_standard_io_program(app: &str, io: &str) -> MirProgram {
    let (_workspace, graph) = load_module_sources_with_standard_library_overrides(
        "app",
        &[("app.ska", app)],
        &[("std/io.ska", io)],
    );
    lower_graph(graph)
}

fn lower_graph(graph: crate::module::ModuleGraph) -> MirProgram {
    lower_graph_ref(&graph)
}

fn lower_graph_ref(graph: &crate::module::ModuleGraph) -> MirProgram {
    let resolved = resolve_module_graph(graph);
    assert!(
        resolved.diagnostics.is_empty(),
        "{:?}",
        resolved.diagnostics
    );
    let checked = type_check(&resolved.program);
    assert!(checked.diagnostics.is_empty(), "{:?}", checked.diagnostics);
    lower_hir_to_final_mir(&checked.hir.expect("valid standard-I/O fixture has HIR"))
}
