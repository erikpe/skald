use crate::{
    mir::{MirInstruction, MirRvalueKind},
    test_support::lower_source_to_final_mir,
};

use super::super::{solve_local_constants, BlockLocalConstantView};
use crate::passes::pipeline::optimizations::primitive_evaluation::PrimitiveConstant;

#[test]
fn block_view_exposes_only_constants_after_their_local_definition() {
    let program = lower_source_to_final_mir("fn main() -> i64 { return 1 + 2; }");
    let definition = program.definitions.get(program.entry_function).unwrap();
    let assignments = definition.body.blocks[0]
        .instructions
        .iter()
        .filter_map(|instruction| match instruction {
            MirInstruction::Assign(assignment) => Some(assignment),
            _ => None,
        })
        .collect::<Vec<_>>();
    let one = assignments
        .iter()
        .find(|assignment| assignment.rvalue.kind == MirRvalueKind::ConstantI64(1))
        .unwrap();
    let sum = assignments
        .iter()
        .find(|assignment| matches!(assignment.rvalue.kind, MirRvalueKind::Binary { .. }))
        .unwrap();
    let solution = solve_local_constants(definition.into()).unwrap();
    let mut view = BlockLocalConstantView::new(&solution);

    assert_eq!(view.constant(one.result), None);
    assert_eq!(view.constant(sum.result), None);
    view.observe_assignment(one);
    assert_eq!(view.constant(one.result), Some(PrimitiveConstant::I64(1)));
    assert_eq!(view.constant(sum.result), None);
    view.observe_assignment(sum);
    assert_eq!(view.constant(sum.result), Some(PrimitiveConstant::I64(3)));

    let mut rewritten = (*sum).clone();
    rewritten.rvalue.kind = MirRvalueKind::ConstantI64(9);
    view.observe_assignment(&rewritten);
    assert_eq!(
        view.constant(sum.result),
        Some(PrimitiveConstant::I64(9)),
        "a consumer-local literal rewrite overrides its stale source-seal fact"
    );

    view.begin_block();
    assert_eq!(view.constant(one.result), None);
    assert_eq!(view.constant(sum.result), None);
}
