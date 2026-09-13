use skald_compiler::{
    hir::dump_hir,
    lexer::{dump_tokens, lex},
    mir::{dump_mir, lower_hir},
    resolve::{dump_resolved, resolve},
    source::SourceDatabase,
    syntax::{dump_ast, parse},
    typeck::type_check,
};

use super::source::{single_source_full_phase_dump, StandardLibraryInput};

pub(crate) fn array_phase_dump() -> String {
    single_source_full_phase_dump(
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../tests/golden/arrays/array_views.ska"
        )),
        StandardLibraryInput::GoldenCallsAsExternalStubs,
    )
}

pub(crate) fn array_element_list_phase_dump() -> String {
    let mut sources = SourceDatabase::new();
    let source_id = sources.add(
        "array-element-lists.ska",
        concat!(
            "class Item { value: i64; init(value: i64) { self.value = value; } ",
            "copy(ref other: Item) { self.value = other.value; } }\n",
            "class Holder { values: i64[]; init(first: i64, values: i64[]) { self.values = values; } }\n",
            "fn consume(values: i64[]) -> i64 { return values[0]; }\n",
            "fn produce() -> i64[] { return i64[]{8, 9}; }\n",
            "fn main() -> i64 {\n",
            "  var primitives: i64[] = i64[]{};\n",
            "  primitives = i64[]{1, 2};\n",
            "  var objects: Item[] = Item[]{Item(1), Item(2)};\n",
            "  var optional_values: i64?[] = i64?[]{none, 3};\n",
            "  var optional_objects: Item?[] = Item?[]{none, Item(4)};\n",
            "  var rows: i64[][] = i64[][]{i64[]{1, 2}, i64[]{3}};\n",
            "  var owners: (shared Item)[] = (shared Item)[]{new Item(5)};\n",
            "  var optional_owners: (shared? Item)[] = (shared? Item)[]{none, new Item(6)};\n",
            "  var shared_outer: shared i64[] = new i64[]{10, 11};\n",
            "  var holder: Holder = Holder(12, i64[]{6, 7});\n",
            "  return consume(produce()) + i64[]{4, 5}[0] + shared_outer->[0];\n",
            "}\n",
        ),
    );
    let source = sources.get(source_id).unwrap();
    let lexed = lex(source);
    assert!(lexed.diagnostics.is_empty());
    let parsed = parse(source, &lexed.tokens);
    assert!(parsed.diagnostics.is_empty());
    let resolved = resolve(&parsed.ast);
    assert!(resolved.diagnostics.is_empty());
    let checked = type_check(&resolved.program);
    assert!(checked.diagnostics.is_empty(), "{:?}", checked.diagnostics);
    let hir = checked.hir.unwrap();
    let mir = lower_hir(&hir);

    format!(
        "TOKENS\n{}AST\n{}RESOLVED\n{}HIR\n{}MIR\n{}",
        dump_tokens(source, &lexed.tokens),
        dump_ast(&parsed.ast),
        dump_resolved(&resolved.program),
        dump_hir(&hir),
        dump_mir(&mir),
    )
}

pub(crate) fn indexed_array_frontend_phase_dump() -> String {
    single_source_full_phase_dump(
        concat!(
            "class Item { value: i64; init(value: i64) { self.value = value; } ",
            "copy(ref other: Item) { self.value = other.value; } }\n",
            "fn main() -> i64 {\n",
            "  var length: u64 = 2u;\n",
            "  var rows: i64[][] = i64[][](length; row =>\n",
            "    i64[](2u; column => row + column));\n",
            "  var items: Item[] = Item[](length; index => Item(index));\n",
            "  var owner: shared Item[] = new Item[](length; index => Item(index));\n",
            "  return rows[1][1] + items[0].value + owner->[1].value;\n",
            "}\n",
        ),
        StandardLibraryInput::None,
    )
}
