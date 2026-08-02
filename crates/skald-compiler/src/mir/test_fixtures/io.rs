use crate::{
    mir::{lower_hir, MirProgram},
    resolve::resolve_module_graph,
    test_support::{
        load_module_sources, CANONICAL_ERROR_SOURCE, CANONICAL_IO_SOURCE, CANONICAL_STR_SOURCE,
    },
    typeck::type_check,
};

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
    let io = format!(
        "{CANONICAL_IO_SOURCE}\n{}{additional}",
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
    let (_workspace, graph) = load_module_sources(
        "app",
        &[
            ("app.ska", app),
            ("std/io.ska", &io),
            ("std/error.ska", CANONICAL_ERROR_SOURCE),
            ("std/str.ska", CANONICAL_STR_SOURCE),
        ],
    );
    let resolved = resolve_module_graph(&graph);
    assert!(
        resolved.diagnostics.is_empty(),
        "{:?}",
        resolved.diagnostics
    );
    let checked = type_check(&resolved.program);
    assert!(checked.diagnostics.is_empty(), "{:?}", checked.diagnostics);
    lower_hir(&checked.hir.expect("valid standard-I/O fixture has HIR"))
}
