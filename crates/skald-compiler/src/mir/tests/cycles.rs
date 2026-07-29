use super::*;
use crate::passes::run_mir_pipeline;

#[test]
fn verifies_repeated_lifetime_epochs_for_every_stateful_storage_family() {
    let fixtures = [
        (
            "inline object",
            concat!(
                "class Resource { init() {} }\n",
                "fn main() -> i64 { var value: Resource = Resource(); return 0; }\n",
            ),
            FunctionId::new(0),
        ),
        (
            "shared owner",
            concat!(
                "class Resource { init() {} }\n",
                "fn main() -> i64 { var value: shared Resource = new Resource(); return 0; }\n",
            ),
            FunctionId::new(0),
        ),
        (
            "primitive optional",
            "fn main() -> i64 { var value: i64? = 7; return value!; }\n",
            FunctionId::new(0),
        ),
        (
            "array owner",
            "fn main() -> i64 { var values: i64[] = i64[](2u); return 0; }\n",
            FunctionId::new(0),
        ),
        (
            "optional guard",
            concat!(
                "class Item { value: i64; init(value: i64) { self.value = value; } }\n",
                "class Holder { item: Item?; init(item: Item?) { self.item = item; } }\n",
                "fn main() -> i64 { var holder: Holder = Holder(Item(42)); ",
                "return holder.item!.value; }\n",
            ),
            FunctionId::new(0),
        ),
        (
            "shared anchor",
            concat!(
                "class Leaf { init() {} fn read() -> i64 { return 7; } }\n",
                "class Holder { edge: shared Leaf; init() { self.edge = new Leaf(); } }\n",
                "fn main() -> i64 { var holder: Holder = Holder(); ",
                "return holder.edge->read(); }\n",
            ),
            FunctionId::new(0),
        ),
        (
            "checked view",
            concat!(
                "class Leaf { init() {} fn read() -> i64 { return 7; } }\n",
                "fn inspect(ref value: Obj) -> i64 { return ((Leaf) value).read(); }\n",
                "fn main() -> i64 { return 0; }\n",
            ),
            FunctionId::new(0),
        ),
    ];

    for (name, source, function) in fixtures {
        let mut program = lower_text(source);
        wrap_function_in_cycle(&mut program, function);
        verify_mir(&program).unwrap_or_else(|errors| panic!("{name} cycle must verify:\n{errors}"));
        run_mir_pipeline(program)
            .unwrap_or_else(|errors| panic!("{name} cycle must survive MIR passes:\n{errors}"));
    }
}

#[test]
fn rejects_incompatible_cycle_state_once_in_deterministic_order() {
    let mut program = lower_text(concat!(
        "class Resource { init() {} }\n",
        "fn main() -> i64 { var value: Resource = Resource(); return 0; }\n",
    ));
    let entry = program.entry_function;
    wrap_function_in_cycle(&mut program, entry);
    let function = program.definitions.get_mut_for_test(entry).unwrap();
    function.body.blocks[0]
        .instructions
        .retain(|instruction| !matches!(instruction, MirInstruction::Cleanup(_)));

    let first = verify_mir(&program).unwrap_err().to_string();
    let second = verify_mir(&program).unwrap_err().to_string();
    assert_eq!(first, second);
    assert!(first.contains("owning local remains live"));
    let lines: Vec<_> = first.lines().collect();
    let unique: std::collections::HashSet<_> = lines.iter().copied().collect();
    assert_eq!(
        lines.len(),
        unique.len(),
        "cyclic diagnostics must not be duplicated:\n{first}"
    );
    assert!(
        lines.len() < 32,
        "cyclic diagnostics must remain bounded:\n{first}"
    );
}

#[test]
fn rejects_live_storage_crossing_a_backedge_and_double_initialization() {
    let mut live = lower_text("fn main() -> i64 { var value: i64 = 1; return value; }\n");
    let entry = live.entry_function;
    wrap_function_in_cycle(&mut live, entry);
    let function = live.definitions.get_mut_for_test(entry).unwrap();
    let local = function
        .storage
        .iter()
        .find(|storage| storage.kind == MirStorageKind::Local)
        .unwrap()
        .id;
    for block in &mut function.body.blocks {
        block.instructions.retain(|instruction| {
            !matches!(
                instruction,
                MirInstruction::StorageDead(operation) if operation.storage == local
            )
        });
    }
    let errors = verify_mir(&live).unwrap_err().to_string();
    assert!(
        errors.contains("storage lifetime state disagrees at control-flow join"),
        "{errors}"
    );

    let mut double = lower_text(concat!(
        "class Resource { init() {} }\n",
        "fn main() -> i64 { var value: Resource = Resource(); return 0; }\n",
    ));
    let entry = double.entry_function;
    wrap_function_in_cycle(&mut double, entry);
    let function = double.definitions.get_mut_for_test(entry).unwrap();
    let initialize = function.body.blocks[0]
        .instructions
        .iter()
        .find(|instruction| matches!(instruction, MirInstruction::Initialize(_)))
        .unwrap()
        .clone();
    let position = function.body.blocks[0]
        .instructions
        .iter()
        .position(|instruction| matches!(instruction, MirInstruction::Initialize(_)))
        .unwrap();
    function.body.blocks[0]
        .instructions
        .insert(position + 1, initialize);
    assert!(verify_mir(&double)
        .unwrap_err()
        .to_string()
        .contains("already live"));
}

#[test]
fn verifies_disconnected_cyclic_components_and_bounded_cycle_sizes() {
    for block_count in 1..=16 {
        let mut program = lower_text("fn main() -> i64 { return 0; }\n");
        append_unreachable_ring(&mut program, block_count, None);
        verify_mir(&program).unwrap_or_else(|errors| {
            panic!("unreachable ring with {block_count} blocks must verify:\n{errors}")
        });
    }

    let mut malformed = lower_text("fn main() -> i64 { var value: i64 = 1; return value; }\n");
    let entry = malformed.entry_function;
    let local = malformed
        .definitions
        .get(entry)
        .unwrap()
        .storage
        .iter()
        .find(|storage| storage.kind == MirStorageKind::Local)
        .unwrap()
        .id;
    append_unreachable_ring(&mut malformed, 3, Some(local));
    let errors = verify_mir(&malformed).unwrap_err().to_string();
    assert!(
        errors.contains("storage lifetime state disagrees at control-flow join"),
        "{errors}"
    );
}

#[test]
fn rejects_cyclic_optional_view_anchor_shared_and_array_state_leaks() {
    let mut shared = lower_text(concat!(
        "class Resource { init() {} }\n",
        "fn main() -> i64 { var value: shared Resource = new Resource(); return 0; }\n",
    ));
    let entry = shared.entry_function;
    wrap_function_in_cycle(&mut shared, entry);
    remove_instructions(&mut shared, entry, |instruction| {
        matches!(instruction, MirInstruction::SharedRelease(_))
    });
    assert_cycle_error(&shared, "shared owner remains live");

    let mut optional = lower_text("fn main() -> i64 { var value: i64? = 7; return value!; }\n");
    let entry = optional.entry_function;
    wrap_function_in_cycle(&mut optional, entry);
    remove_instructions(&mut optional, entry, |instruction| {
        matches!(instruction, MirInstruction::OptionalInitialize(_))
    });
    assert_cycle_error(
        &optional,
        "optional unwrap source is not definitely initialized",
    );

    let mut guard = lower_text(concat!(
        "class Item { value: i64; init(value: i64) { self.value = value; } }\n",
        "class Holder { item: Item?; init(item: Item?) { self.item = item; } }\n",
        "fn main() -> i64 { var holder: Holder = Holder(Item(42)); ",
        "return holder.item!.value; }\n",
    ));
    let entry = guard.entry_function;
    wrap_function_in_cycle(&mut guard, entry);
    remove_instructions(&mut guard, entry, |instruction| {
        matches!(instruction, MirInstruction::EndOptionalView(_))
    });
    assert_cycle_error(&guard, "optional payload guard remains active");

    let mut checked = lower_text(concat!(
        "class Leaf { init() {} fn read() -> i64 { return 7; } }\n",
        "fn inspect(ref value: Obj) -> i64 { return ((Leaf) value).read(); }\n",
        "fn main() -> i64 { return 0; }\n",
    ));
    let inspect = FunctionId::new(0);
    wrap_function_in_cycle(&mut checked, inspect);
    remove_instructions(&mut checked, inspect, |instruction| {
        matches!(instruction, MirInstruction::EndCheckedView(_))
    });
    assert_cycle_error(&checked, "checked-view carrier remains active");

    let mut anchor = lower_text(concat!(
        "class Leaf { init() {} fn read() -> i64 { return 7; } }\n",
        "class Holder { edge: shared Leaf; init() { self.edge = new Leaf(); } }\n",
        "fn main() -> i64 { var holder: Holder = Holder(); ",
        "return holder.edge->read(); }\n",
    ));
    let entry = anchor.entry_function;
    wrap_function_in_cycle(&mut anchor, entry);
    let anchor_storage = anchor
        .definitions
        .get(entry)
        .unwrap()
        .storage
        .iter()
        .find(|storage| storage.kind == MirStorageKind::SharedAnchor)
        .unwrap()
        .id;
    remove_instructions(&mut anchor, entry, |instruction| {
        matches!(
            instruction,
            MirInstruction::SharedRelease(release) if release.owner == anchor_storage
        )
    });
    assert_cycle_error(&anchor, "shared owner remains live");

    let mut array = lower_text("fn main() -> i64 { var values: i64[] = i64[](2u); return 0; }\n");
    let entry = array.entry_function;
    wrap_function_in_cycle(&mut array, entry);
    remove_instructions(&mut array, entry, |instruction| {
        matches!(
            instruction,
            MirInstruction::Array(MirArrayInstruction::Adopt { .. })
        )
    });
    assert_cycle_error(&array, "never consumed");
}

fn wrap_function_in_cycle(program: &mut MirProgram, function_id: FunctionId) {
    let function = program.definitions.get_mut_for_test(function_id).unwrap();
    let span = function.span;
    let header = BlockId::new(function.function, function.body.blocks.len());
    let exit = BlockId::new(function.function, function.body.blocks.len() + 1);
    let original_entry = function.body.entry;

    for block in &mut function.body.blocks {
        if matches!(block.terminator, Some(MirTerminator::Return { .. })) {
            block.terminator = Some(MirTerminator::Goto {
                target: header,
                span: block.span,
            });
        }
    }

    let condition = ValueId::new(function.function, function.values.len());
    function
        .values
        .push(fixture_value(condition, MirType::Bool, span));
    let exit_value = ValueId::new(function.function, function.values.len());
    function
        .values
        .push(fixture_value(exit_value, MirType::I64, span));
    function.body.blocks.push(fixture_block(
        header,
        vec![fixture_assign(
            condition,
            MirRvalueKind::ConstantBool(true),
            MirType::Bool,
            span,
        )],
        Some(MirTerminator::Branch {
            condition,
            true_target: original_entry,
            false_target: exit,
            span,
        }),
        span,
    ));
    function.body.blocks.push(fixture_block(
        exit,
        vec![fixture_assign(
            exit_value,
            MirRvalueKind::ConstantI64(0),
            MirType::I64,
            span,
        )],
        Some(MirTerminator::Return {
            value: Some(exit_value),
            span,
        }),
        span,
    ));
    function.body.entry = header;
}

fn append_unreachable_ring(
    program: &mut MirProgram,
    block_count: usize,
    live_storage: Option<StorageId>,
) {
    let entry = program.entry_function;
    let function = program.definitions.get_mut_for_test(entry).unwrap();
    let span = function.span;
    let first = function.body.blocks.len();
    for offset in 0..block_count {
        let block = BlockId::new(function.function, first + offset);
        let next = BlockId::new(function.function, first + (offset + 1) % block_count);
        let instructions = if offset == 0 {
            live_storage
                .map(|storage| vec![fixture_storage_live(storage, span)])
                .unwrap_or_default()
        } else {
            vec![]
        };
        function.body.blocks.push(fixture_block(
            block,
            instructions,
            Some(MirTerminator::Goto { target: next, span }),
            span,
        ));
    }
}

fn remove_instructions(
    program: &mut MirProgram,
    function: FunctionId,
    predicate: impl Fn(&MirInstruction) -> bool,
) {
    let function = program.definitions.get_mut_for_test(function).unwrap();
    for block in &mut function.body.blocks {
        block
            .instructions
            .retain(|instruction| !predicate(instruction));
    }
}

fn assert_cycle_error(program: &MirProgram, expected: &str) {
    let errors = verify_mir(program).unwrap_err().to_string();
    assert!(
        errors.contains(expected),
        "expected `{expected}` in bounded cyclic errors:\n{errors}"
    );
    assert!(
        errors.lines().count() < 64,
        "cyclic diagnostics must remain bounded:\n{errors}"
    );
}
