use super::*;

fn assert_explicit_epochs(program: &MirProgram) {
    verify_mir(program).expect("lifetime lowering fixture must verify");
    for definition in program
        .definitions
        .iter()
        .map(MirDefinitionRef::from)
        .chain(
            program
                .member_definitions
                .iter()
                .map(MirDefinitionRef::from),
        )
    {
        for storage in definition.storage_entries().iter().filter(|storage| {
            !matches!(
                storage.kind,
                MirStorageKind::Return
                    | MirStorageKind::Receiver
                    | MirStorageKind::Parameter
                    | MirStorageKind::AliasParameter(_)
            )
        }) {
            let operations: Vec<_> = definition
                .body()
                .blocks
                .iter()
                .flat_map(|block| &block.instructions)
                .filter_map(|instruction| match instruction {
                    MirInstruction::StorageLive(operation) if operation.storage == storage.id => {
                        Some(true)
                    }
                    MirInstruction::StorageDead(operation) if operation.storage == storage.id => {
                        Some(false)
                    }
                    _ => None,
                })
                .collect();
            assert!(
                operations.contains(&true) && operations.contains(&false),
                "{} {:?} has no complete explicit lifetime epoch: {operations:?}",
                storage.id,
                storage.kind
            );
        }
    }
}

fn has_storage_kind(program: &MirProgram, expected: impl Fn(MirStorageKind) -> bool) -> bool {
    program
        .definitions
        .iter()
        .map(MirDefinitionRef::from)
        .chain(
            program
                .member_definitions
                .iter()
                .map(MirDefinitionRef::from),
        )
        .flat_map(|definition| definition.storage_entries())
        .any(|storage| expected(storage.kind))
}

#[test]
fn lowers_explicit_epochs_for_every_current_storage_family() {
    let primitive_and_inline = lower_text(concat!(
        "class Item { init() {} }\n",
        "fn take(value: Item) -> unit {}\n",
        "fn main() -> i64 {\n",
        "  var count: i64 = 1;\n",
        "  var item: Item = Item();\n",
        "  take(Item());\n",
        "  return count;\n",
        "}\n",
    ));
    let shared_and_anchor = lower_text(concat!(
        "class Leaf { init() {} fn read() -> i64 { return 7; } }\n",
        "class Holder { edge: shared Leaf; init() { self.edge = new Leaf(); } }\n",
        "fn main() -> i64 {\n",
        "  var holder: Holder = Holder();\n",
        "  return holder.edge->read();\n",
        "}\n",
    ));
    let optional = lower_text(concat!(
        "fn main() -> i64 {\n",
        "  var value: i64? = 7;\n",
        "  return value!;\n",
        "}\n",
    ));
    let arrays = lower_text(concat!(
        "fn main() -> i64 {\n",
        "  var values: i64[] = i64[](2u);\n",
        "  values[0] = 7;\n",
        "  return values[0];\n",
        "}\n",
    ));
    let checked_view = super::type_operation_fixtures::type_operation_mir();

    for program in [
        &primitive_and_inline,
        &shared_and_anchor,
        &optional,
        &arrays,
        &checked_view,
    ] {
        assert_explicit_epochs(program);
    }

    for (program, expected) in [
        (&primitive_and_inline, MirStorageKind::Local),
        (&primitive_and_inline, MirStorageKind::Argument),
        (&shared_and_anchor, MirStorageKind::SharedAllocation),
        (&shared_and_anchor, MirStorageKind::SharedAnchor),
        (&optional, MirStorageKind::OptionalUnwrap),
        (&arrays, MirStorageKind::ArrayBacking),
        (&arrays, MirStorageKind::ArrayProduced),
        (
            &checked_view,
            MirStorageKind::CheckedView(MirAliasAccess::ReadOnly),
        ),
    ] {
        assert!(
            has_storage_kind(program, |kind| kind == expected),
            "fixture did not produce {expected:?}"
        );
    }
}

#[test]
fn lifetime_epochs_do_not_activate_source_loop_syntax() {
    let (_, parsed) = parse_source("fn main() -> i64 { while (true) { return 0; } return 1; }\n");
    assert!(
        !parsed.diagnostics.is_empty(),
        "L0 must not activate source while syntax"
    );
}
