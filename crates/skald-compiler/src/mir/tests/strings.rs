use super::*;
use crate::{
    identity::{ArrayTypeId, ClassId, LiteralDataId},
    resolve::resolve_module_graph,
    test_support::load_module_sources,
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

fn shared_static_mut(program: &mut MirProgram) -> &mut MirSharedStatic {
    let entry = program.entry_function;
    program
        .definitions
        .get_mut_for_test(entry)
        .unwrap()
        .body
        .blocks
        .iter_mut()
        .flat_map(|block| &mut block.instructions)
        .find_map(|instruction| match instruction {
            MirInstruction::SharedStatic(static_owner) => Some(static_owner),
            _ => None,
        })
        .unwrap()
}

fn string_initialize_mut(program: &mut MirProgram) -> &mut MirStringInitialize {
    let entry = program.entry_function;
    program
        .definitions
        .get_mut_for_test(entry)
        .unwrap()
        .body
        .blocks
        .iter_mut()
        .flat_map(|block| &mut block.instructions)
        .find_map(|instruction| match instruction {
            MirInstruction::StringInitialize(initialize) => Some(initialize),
            _ => None,
        })
        .unwrap()
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
    let missing_metadata = errors_after(|program| {
        program.string_language_item = None;
    });
    assert!(missing_metadata.contains("literal data requires string language-item metadata"));

    let missing_class = errors_after(|program| {
        program.string_language_item.as_mut().unwrap().class = ClassId::new(99);
    });
    assert!(missing_class.contains("string language-item class is not declared"));

    let inherited_class = errors_after(|program| {
        let item = program.string_language_item.unwrap();
        let class = &mut program.classes.entries_mut_for_test()[item.class.index()];
        class.direct_base = Some(MirDirectBase {
            class: item.class,
            span: class.span,
        });
    });
    assert!(inherited_class.contains("string language-item class must be a root class"));

    let wrong_density = errors_after(|program| {
        program.literal_data.entries_mut_for_test()[0].id = LiteralDataId::new(1);
    });
    assert!(wrong_density.contains("literal-data table index 0 contains str1"));

    let wrong_storage_array = errors_after(|program| {
        program.literal_data.entries_mut_for_test()[0].array = ArrayTypeId::new(99);
    });
    assert!(wrong_storage_array.contains("does not use the string storage-array identity"));

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

    let wrong_element = errors_after(|program| {
        let array = program.string_language_item.unwrap().storage_array;
        program.array_types.entries_mut_for_test()[array.index()].element = MirType::U64;
    });
    assert!(wrong_element.contains("string language-item storage array must have u8 elements"));

    let wrong_lifecycle = errors_after(|program| {
        let class = program.string_language_item.unwrap().class;
        program.classes.entries_mut_for_test()[class.index()].copy_constructor =
            MirCopyCapability::Unavailable;
    });
    assert!(wrong_lifecycle.contains(
        "string language-item class must retain its exact synthesized descriptor lifecycle"
    ));
}

#[test]
fn rejects_malformed_static_literal_owners_one_invariant_at_a_time() {
    let undeclared_data = errors_after(|program| {
        shared_static_mut(program).data = LiteralDataId::new(99);
    });
    assert!(
        undeclared_data.contains("static shared owner references undeclared literal data str99")
    );

    let wrong_target = errors_after(|program| {
        let class = program.string_language_item.unwrap().class;
        shared_static_mut(program).target = MirSharedTarget::Class(class);
    });
    assert!(wrong_target.contains("static shared owner target does not match its literal data"));

    let mortal = errors_after(|program| {
        shared_static_mut(program).origin = MirStaticAllocationOrigin::Unspecified;
    });
    assert!(mortal.contains("static shared owner must have immortal provenance"));

    let wrong_destination = errors_after(|program| {
        let destination = shared_static_mut(program).destination;
        let entry = program.entry_function;
        program.definitions.get_mut_for_test(entry).unwrap().storage[destination.index()].kind =
            MirStorageKind::Local;
    });
    assert!(wrong_destination
        .contains("static shared owner destination must be a fresh exact shared temporary"));
}

#[test]
fn rejects_malformed_string_publication_one_invariant_at_a_time() {
    let missing_metadata = errors_after(|program| {
        program.string_language_item = None;
    });
    assert!(missing_metadata.contains("string initialization requires language-item metadata"));

    let undeclared_data = errors_after(|program| {
        string_initialize_mut(program).data = LiteralDataId::new(99);
    });
    assert!(
        undeclared_data.contains("string initialization references undeclared literal data str99")
    );

    let wrong_identity = errors_after(|program| {
        string_initialize_mut(program).class = ClassId::new(99);
    });
    assert!(wrong_identity
        .contains("string initialization does not use the exact language-item identities"));

    let wrong_descriptor = errors_after(|program| {
        string_initialize_mut(program).start = 1;
    });
    assert!(wrong_descriptor.contains("string initialization has invalid start or length metadata"));

    let wrong_destination = errors_after(|program| {
        let backing = string_initialize_mut(program).backing;
        string_initialize_mut(program).destination = MirPlace::shared_pointee(backing);
    });
    assert!(wrong_destination
        .contains("string initialization destination must be mutable owning string storage"));

    let wrong_backing = errors_after(|program| {
        let destination = string_initialize_mut(program).destination.base.storage();
        string_initialize_mut(program).backing = destination;
    });
    assert!(wrong_backing
        .contains("string initialization backing must be the exact shared u8[] temporary"));
}

#[test]
fn rejects_mismatched_publication_and_static_owner_escape() {
    let wrong_descriptor = errors_after(|program| {
        string_initialize_mut(program).length += 1;
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
