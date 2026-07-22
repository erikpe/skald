use crate::{
    mir::{verify_mir, BlockId, MirBasicBlock, MirProgram, MirTerminator},
    test_support::lower_source_to_mir,
};

fn two_block_program() -> MirProgram {
    let mut program =
        lower_source_to_mir("fn main() -> i64 { var result: i64 = 0; return result; }");
    let function = program
        .definitions
        .get_mut_for_test(program.entry_function)
        .unwrap();
    let entry = &mut function.body.blocks[0];
    let join_id = BlockId::new(function.function, 1);
    let join_instructions = entry.instructions.split_off(2);
    let join_terminator = entry.terminator.take();
    entry.terminator = Some(MirTerminator::Goto {
        target: join_id,
        span: entry.span,
    });
    function.body.blocks.push(MirBasicBlock {
        id: join_id,
        instructions: join_instructions,
        terminator: join_terminator,
        span: function.span,
    });
    program
}

#[test]
fn transient_values_remain_block_local() {
    let mut program = two_block_program();
    let function = program
        .definitions
        .get_mut_for_test(program.entry_function)
        .unwrap();
    let entry_value = function.values[0].id;
    let join = &mut function.body.blocks[1];
    join.terminator = Some(MirTerminator::Return {
        value: Some(entry_value),
        span: join.span,
    });

    assert!(verify_mir(&program).unwrap_err().iter().any(|error| error
        .message
        .contains("used before it is defined in this block")));
}

#[test]
fn unreachable_blocks_still_receive_structural_verification() {
    let mut program = lower_source_to_mir("fn main() -> i64 { return 0; }");
    let function = program
        .definitions
        .get_mut_for_test(program.entry_function)
        .unwrap();
    let function_id = function.function;
    let unreachable = BlockId::new(function_id, 1);
    function.body.blocks.push(MirBasicBlock {
        id: unreachable,
        instructions: Vec::new(),
        terminator: None,
        span: function.span,
    });

    assert!(verify_mir(&program).unwrap_err().iter().any(|error| {
        error.block == Some(unreachable) && error.message == "block has no terminator"
    }));
}
