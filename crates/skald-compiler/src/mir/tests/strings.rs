use super::*;
use crate::{
    identity::LiteralDataId, resolve::resolve_module_graph, test_support::load_module_sources,
    typeck::type_check,
};

const VALID_STR: &str = concat!(
    "public class Str {\n",
    "  private storage: shared u8[];\n",
    "  private start: u64;\n",
    "  private length: u64;\n",
    "  init() {\n",
    "    self.storage = new u8[]();\n",
    "    self.start = 0u;\n",
    "    self.length = 0u;\n",
    "  }\n",
    "}\n",
);

fn string_mir(app: &str) -> MirProgram {
    let (_workspace, graph) =
        load_module_sources("app", &[("app.ska", app), ("std/str.ska", VALID_STR)]);
    let resolved = resolve_module_graph(&graph);
    assert!(
        resolved.diagnostics.is_empty(),
        "resolution failed: {:?}",
        resolved.diagnostics
    );
    let checked = type_check(&resolved.program);
    assert!(
        checked.diagnostics.is_empty(),
        "type checking failed: {:?}",
        checked.diagnostics
    );
    lower_hir(&checked.hir.expect("valid string source must produce HIR"))
}

fn one_literal_mir() -> MirProgram {
    string_mir(concat!(
        "from std::str import Str;\n",
        "fn main() -> i64 { var value: Str = \"a\\0\\xff\"; return 0; }\n",
    ))
}

fn errors_after(mutator: impl FnOnce(&mut MirProgram)) -> String {
    let mut program = one_literal_mir();
    mutator(&mut program);
    verify_mir(&program).unwrap_err().to_string()
}

#[test]
fn lowers_exact_literal_data_static_backing_and_ordinary_string_lifecycle() {
    let program = string_mir(concat!(
        "from std::str import Str;\n",
        "fn consume(value: Str) -> unit {}\n",
        "fn main() -> i64 {\n",
        "  var first: Str = \"a\\0\\xff\";\n",
        "  var second: Str = first;\n",
        "  second = \"b\";\n",
        "  consume(second);\n",
        "  return 0;\n",
        "}\n",
    ));

    verify_mir(&program).expect("lowered string MIR must verify");
    assert_eq!(
        program
            .literal_data
            .iter()
            .map(|data| (&data.bytes[..], data.length))
            .collect::<Vec<_>>(),
        [(b"a\0\xff".as_slice(), 3), (b"b".as_slice(), 1)]
    );
    let dump = dump_mir(&program);
    assert!(dump.contains("StringLanguageItem class"));
    assert!(dump.contains("Literal str0"));
    assert!(dump.contains("bytes 61 00 ff"));
    assert!(dump.contains("shared-static"));
    assert!(dump.contains("string-initialize"));
    assert!(dump.contains("start 0 length 3"));
    assert!(dump.contains("copy-construct"));
    assert!(dump.contains("copy-assign"));
    assert!(dump.contains("cleanup"));
}

#[test]
fn rejects_malformed_literal_declarations_one_invariant_at_a_time() {
    let wrong_density = errors_after(|program| {
        program.literal_data.entries_mut_for_test()[0].id = LiteralDataId::new(1);
    });
    assert!(wrong_density.contains("literal-data table index 0 contains str1"));

    let wrong_length = errors_after(|program| {
        program.literal_data.entries_mut_for_test()[0].length += 1;
    });
    assert!(wrong_length.contains("length does not match its exact byte payload"));

    let mutable = errors_after(|program| {
        program.literal_data.entries_mut_for_test()[0].mutability =
            MirStaticDataMutability::Mutable;
    });
    assert!(mutable.contains("literal data str0 is not immutable"));

    let mortal = errors_after(|program| {
        program.literal_data.entries_mut_for_test()[0].origin =
            MirStaticAllocationOrigin::Unspecified;
    });
    assert!(mortal.contains("literal data str0 is not immortal"));

    let wrong_fields = errors_after(|program| {
        let item = program.string_language_item.as_mut().unwrap();
        std::mem::swap(&mut item.start_field, &mut item.length_field);
    });
    assert!(wrong_fields.contains("fields must be the exact shared u8[]/u64/u64 descriptor"));
}

#[test]
fn rejects_mismatched_publication_and_static_owner_escape() {
    let wrong_descriptor = errors_after(|program| {
        let function = program
            .definitions
            .get_mut_for_test(program.entry_function)
            .unwrap();
        for instruction in &mut function.body.blocks[0].instructions {
            if let MirInstruction::StringInitialize(initialize) = instruction {
                initialize.length += 1;
                break;
            }
        }
    });
    assert!(wrong_descriptor.contains("invalid start or length metadata"));

    let escaped_backing = errors_after(|program| {
        let function = program
            .definitions
            .get_mut_for_test(program.entry_function)
            .unwrap();
        let block = &mut function.body.blocks[0];
        let index = block
            .instructions
            .iter()
            .position(|instruction| matches!(instruction, MirInstruction::StringInitialize(_)))
            .unwrap();
        let MirInstruction::StringInitialize(initialize) = block.instructions[index].clone() else {
            unreachable!()
        };
        block.instructions[index] = MirInstruction::SharedRelease(MirSharedRelease {
            owner: initialize.backing,
            span: initialize.span,
        });
    });
    assert!(escaped_backing.contains(
        "shared release cannot consume static literal backing before string initialization"
    ));
    assert!(escaped_backing.contains("static literal owner is not consumed"));
}
