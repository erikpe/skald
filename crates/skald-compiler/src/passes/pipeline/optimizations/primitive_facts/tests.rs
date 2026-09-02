use crate::{
    identity::{CallableId, FunctionId},
    mir::{
        MirAssignment, MirBinaryOperation, MirRvalue, MirRvalueKind, MirType, MirUnaryOperation,
        ValueId,
    },
    source::Span,
    test_support::lower_source_to_final_mir,
};

use super::{PrimitiveConstant, PrimitiveConstantFacts, PrimitiveFoldKind};

fn fixture() -> (CallableId, Span) {
    let program = lower_source_to_final_mir("fn main() -> i64 { return 0; }");
    let definition = program.definitions.get(program.entry_function).unwrap();
    (CallableId::Function(definition.function), definition.span)
}

fn assignment(
    callable: CallableId,
    span: Span,
    index: usize,
    kind: MirRvalueKind,
    ty: MirType,
) -> MirAssignment {
    MirAssignment {
        result: ValueId::new(callable, index),
        rvalue: MirRvalue { kind, ty },
        span,
    }
}

#[test]
fn facts_are_instruction_ordered_and_make_folded_results_immediately_available() {
    let (callable, span) = fixture();
    let mut facts = PrimitiveConstantFacts::default();
    facts.begin_block();

    let left = assignment(
        callable,
        span,
        0,
        MirRvalueKind::ConstantI64(i64::MAX),
        MirType::I64,
    );
    let right = assignment(
        callable,
        span,
        1,
        MirRvalueKind::ConstantI64(1),
        MirType::I64,
    );
    let sum = assignment(
        callable,
        span,
        2,
        MirRvalueKind::Binary {
            operation: MirBinaryOperation::AddI64,
            left: left.result,
            right: right.result,
        },
        MirType::I64,
    );
    let negated = assignment(
        callable,
        span,
        3,
        MirRvalueKind::Unary {
            operation: MirUnaryOperation::NegateI64,
            operand: sum.result,
        },
        MirType::I64,
    );

    assert!(facts.observe_assignment(&left).is_none());
    assert!(facts.observe_assignment(&right).is_none());
    let sum_fold = facts.observe_assignment(&sum).unwrap();
    assert_eq!(sum_fold.kind(), PrimitiveFoldKind::Binary);
    assert_eq!(sum_fold.constant(), PrimitiveConstant::I64(i64::MIN));
    let negated_fold = facts.observe_assignment(&negated).unwrap();
    assert_eq!(negated_fold.kind(), PrimitiveFoldKind::Unary);
    assert_eq!(negated_fold.constant(), PrimitiveConstant::I64(i64::MIN));
    assert_eq!(
        facts.constant(negated.result),
        Some(PrimitiveConstant::I64(i64::MIN))
    );
}

#[test]
fn beginning_a_block_discards_every_fact_from_the_previous_block() {
    let (callable, span) = fixture();
    let mut facts = PrimitiveConstantFacts::default();
    let constant = assignment(
        callable,
        span,
        0,
        MirRvalueKind::ConstantI64(7),
        MirType::I64,
    );
    let use_in_another_block = assignment(
        callable,
        span,
        1,
        MirRvalueKind::Unary {
            operation: MirUnaryOperation::NegateI64,
            operand: constant.result,
        },
        MirType::I64,
    );

    facts.begin_block();
    assert!(facts.observe_assignment(&constant).is_none());
    assert_eq!(
        facts.constant(constant.result),
        Some(PrimitiveConstant::I64(7))
    );

    facts.begin_block();
    assert_eq!(facts.constant(constant.result), None);
    assert!(facts.observe_assignment(&use_in_another_block).is_none());
    assert_eq!(facts.constant(use_in_another_block.result), None);
}

#[test]
fn unsupported_and_mistyped_results_never_become_facts() {
    let (callable, span) = fixture();
    let mut facts = PrimitiveConstantFacts::default();
    facts.begin_block();

    let floating = assignment(
        callable,
        span,
        0,
        MirRvalueKind::ConstantF64Bits(1.0_f64.to_bits()),
        MirType::F64,
    );
    let mistyped = assignment(
        callable,
        span,
        1,
        MirRvalueKind::ConstantI64(1),
        MirType::U64,
    );

    assert!(facts.observe_assignment(&floating).is_none());
    assert!(facts.observe_assignment(&mistyped).is_none());
    assert_eq!(facts.constant(floating.result), None);
    assert_eq!(facts.constant(mistyped.result), None);
}

#[test]
fn facts_are_callable_owned_through_value_identity() {
    let (callable, span) = fixture();
    let mut facts = PrimitiveConstantFacts::default();
    facts.begin_block();
    let constant = assignment(callable, span, 0, MirRvalueKind::ConstantU8(9), MirType::U8);
    assert!(facts.observe_assignment(&constant).is_none());

    let foreign = ValueId::new(
        CallableId::Function(FunctionId::new(1)),
        constant.result.index(),
    );
    assert_eq!(facts.constant(foreign), None);
    assert_eq!(
        facts.constant(constant.result),
        Some(PrimitiveConstant::U8(9))
    );
}
