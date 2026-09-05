use std::{collections::BTreeMap, sync::OnceLock};

use crate::{
    identity::{CallableId, FunctionId},
    mir::{
        MirAssignment, MirBinaryOperation, MirComparisonOperand, MirComparisonPredicate,
        MirIntegerBitwiseOperation, MirIntegerType, MirPrimitiveComparison, MirRvalue,
        MirRvalueKind, MirType, MirUnaryOperation, ValueId,
    },
    source::Span,
    test_support::lower_source_to_final_mir,
};

use super::{
    all_ones, catalog_replacement, one, zero, PrimitiveAlgebraicReplacement as Replacement,
    PrimitiveConstant, PrimitiveUnaryDefinition,
};

#[derive(Default)]
struct TestFacts {
    constants: BTreeMap<ValueId, PrimitiveConstant>,
    unary_definitions: BTreeMap<ValueId, PrimitiveUnaryDefinition>,
}

impl TestFacts {
    fn begin_block(&mut self) {
        self.constants.clear();
        self.unary_definitions.clear();
    }

    fn observe_assignment(&mut self, assignment: &MirAssignment) {
        let constant = match assignment.rvalue.kind {
            MirRvalueKind::ConstantI64(value) => Some(PrimitiveConstant::I64(value)),
            MirRvalueKind::ConstantU64(value) => Some(PrimitiveConstant::U64(value)),
            MirRvalueKind::ConstantU8(value) => Some(PrimitiveConstant::U8(value)),
            MirRvalueKind::ConstantBool(value) => Some(PrimitiveConstant::Bool(value)),
            _ => None,
        };
        if let Some(constant) = constant {
            self.constants.insert(assignment.result, constant);
        }
        if let MirRvalueKind::Unary { operation, operand } = assignment.rvalue.kind {
            self.unary_definitions.insert(
                assignment.result,
                PrimitiveUnaryDefinition { operation, operand },
            );
        }
    }

    fn replacement(&self, kind: &MirRvalueKind, ty: MirType) -> Option<Replacement> {
        catalog_replacement(
            kind,
            ty,
            |value| self.constants.get(&value).copied(),
            |value| self.unary_definitions.get(&value).copied(),
        )
    }
}

fn value(index: usize) -> ValueId {
    ValueId::new(CallableId::Function(FunctionId::new(0)), index)
}

fn span() -> Span {
    static SPAN: OnceLock<Span> = OnceLock::new();
    *SPAN.get_or_init(|| lower_source_to_final_mir("fn main() -> i64 { return 0; }").span)
}

fn assignment(result: usize, kind: MirRvalueKind, ty: MirType) -> MirAssignment {
    MirAssignment {
        result: value(result),
        rvalue: MirRvalue { kind, ty },
        span: span(),
    }
}

fn constant(kind: MirRvalueKind, ty: MirType) -> TestFacts {
    let mut facts = TestFacts::default();
    facts.begin_block();
    facts.observe_assignment(&assignment(0, kind, ty));
    facts
}

fn binary(operation: MirBinaryOperation, left: usize, right: usize) -> MirRvalueKind {
    MirRvalueKind::Binary {
        operation,
        left: value(left),
        right: value(right),
    }
}

type IntegerCase = (
    MirType,
    MirRvalueKind,
    MirRvalueKind,
    MirRvalueKind,
    MirBinaryOperation,
    MirBinaryOperation,
    MirBinaryOperation,
    MirIntegerType,
);

fn integer_cases() -> [IntegerCase; 3] {
    [
        (
            MirType::I64,
            MirRvalueKind::ConstantI64(0),
            MirRvalueKind::ConstantI64(1),
            MirRvalueKind::ConstantI64(-1),
            MirBinaryOperation::AddI64,
            MirBinaryOperation::SubtractI64,
            MirBinaryOperation::MultiplyI64,
            MirIntegerType::I64,
        ),
        (
            MirType::U64,
            MirRvalueKind::ConstantU64(0),
            MirRvalueKind::ConstantU64(1),
            MirRvalueKind::ConstantU64(u64::MAX),
            MirBinaryOperation::AddU64,
            MirBinaryOperation::SubtractU64,
            MirBinaryOperation::MultiplyU64,
            MirIntegerType::U64,
        ),
        (
            MirType::U8,
            MirRvalueKind::ConstantU8(0),
            MirRvalueKind::ConstantU8(1),
            MirRvalueKind::ConstantU8(u8::MAX),
            MirBinaryOperation::AddU8,
            MirBinaryOperation::SubtractU8,
            MirBinaryOperation::MultiplyU8,
            MirIntegerType::U8,
        ),
    ]
}

#[test]
fn identity_constants_preserve_the_exact_encoded_width() {
    assert_eq!(zero(MirType::I64), Some(PrimitiveConstant::I64(0)));
    assert_eq!(zero(MirType::U64), Some(PrimitiveConstant::U64(0)));
    assert_eq!(zero(MirType::U8), Some(PrimitiveConstant::U8(0)));
    assert_eq!(one(MirType::I64), Some(PrimitiveConstant::I64(1)));
    assert_eq!(one(MirType::U64), Some(PrimitiveConstant::U64(1)));
    assert_eq!(one(MirType::U8), Some(PrimitiveConstant::U8(1)));
    assert_eq!(all_ones(MirType::I64), Some(PrimitiveConstant::I64(-1)));
    assert_eq!(
        all_ones(MirType::U64),
        Some(PrimitiveConstant::U64(u64::MAX))
    );
    assert_eq!(all_ones(MirType::U8), Some(PrimitiveConstant::U8(u8::MAX)));
    assert_eq!(zero(MirType::Bool), None);
    assert_eq!(one(MirType::F64), None);
}

#[test]
fn additive_and_multiplicative_catalog_is_width_exact() {
    for (ty, zero, one, _, add, subtract, multiply, _) in integer_cases() {
        let zero_facts = constant(zero, ty);
        assert_eq!(
            zero_facts.replacement(&binary(add, 1, 0), ty),
            Some(Replacement::Forward(value(1)))
        );
        assert_eq!(
            zero_facts.replacement(&binary(add, 0, 1), ty),
            Some(Replacement::Forward(value(1)))
        );
        assert_eq!(
            zero_facts.replacement(&binary(subtract, 1, 0), ty),
            Some(Replacement::Forward(value(1)))
        );
        assert!(matches!(
            zero_facts.replacement(&binary(subtract, 1, 1), ty),
            Some(Replacement::Constant(constant)) if constant.ty() == ty
        ));
        assert!(matches!(
            zero_facts.replacement(&binary(multiply, 1, 0), ty),
            Some(Replacement::Constant(constant)) if constant.ty() == ty
        ));
        assert!(matches!(
            zero_facts.replacement(&binary(multiply, 0, 1), ty),
            Some(Replacement::Constant(constant)) if constant.ty() == ty
        ));

        let one_facts = constant(one, ty);
        assert_eq!(
            one_facts.replacement(&binary(multiply, 1, 0), ty),
            Some(Replacement::Forward(value(1)))
        );
        assert_eq!(
            one_facts.replacement(&binary(multiply, 0, 1), ty),
            Some(Replacement::Forward(value(1)))
        );
    }
}

#[test]
fn bitwise_catalog_covers_zero_all_ones_and_self_for_every_width() {
    for (ty, zero, _, all_ones, _, _, _, integer) in integer_cases() {
        let operation = |operation| MirBinaryOperation::IntegerBitwise {
            operation,
            operand: integer,
        };
        let zero_facts = constant(zero, ty);
        for (left, right) in [(1, 0), (0, 1)] {
            assert!(matches!(
                zero_facts.replacement(&binary(operation(MirIntegerBitwiseOperation::And), left, right), ty),
                Some(Replacement::Constant(constant)) if constant.ty() == ty
            ));
            assert_eq!(
                zero_facts.replacement(
                    &binary(operation(MirIntegerBitwiseOperation::Or), left, right),
                    ty,
                ),
                Some(Replacement::Forward(value(1)))
            );
            assert_eq!(
                zero_facts.replacement(
                    &binary(operation(MirIntegerBitwiseOperation::Xor), left, right),
                    ty,
                ),
                Some(Replacement::Forward(value(1)))
            );
        }

        let all_ones_facts = constant(all_ones, ty);
        for (left, right) in [(1, 0), (0, 1)] {
            assert_eq!(
                all_ones_facts.replacement(
                    &binary(operation(MirIntegerBitwiseOperation::And), left, right),
                    ty,
                ),
                Some(Replacement::Forward(value(1)))
            );
            assert!(matches!(
                all_ones_facts.replacement(&binary(operation(MirIntegerBitwiseOperation::Or), left, right), ty),
                Some(Replacement::Constant(constant)) if constant.ty() == ty
            ));
        }

        assert_eq!(
            zero_facts.replacement(
                &binary(operation(MirIntegerBitwiseOperation::And), 1, 1),
                ty,
            ),
            Some(Replacement::Forward(value(1)))
        );
        assert_eq!(
            zero_facts.replacement(&binary(operation(MirIntegerBitwiseOperation::Or), 1, 1), ty,),
            Some(Replacement::Forward(value(1)))
        );
        assert!(matches!(
            zero_facts.replacement(&binary(operation(MirIntegerBitwiseOperation::Xor), 1, 1), ty),
            Some(Replacement::Constant(constant)) if constant.ty() == ty
        ));
    }
}

#[test]
fn self_comparisons_are_limited_to_integer_and_boolean_equality() {
    let comparison = |operand, predicate| MirRvalueKind::PrimitiveComparison {
        operation: MirPrimitiveComparison { operand, predicate },
        left: value(0),
        right: value(0),
    };
    let facts = TestFacts::default();

    for operand in [
        MirComparisonOperand::Integer(MirIntegerType::I64),
        MirComparisonOperand::Integer(MirIntegerType::U64),
        MirComparisonOperand::Integer(MirIntegerType::U8),
        MirComparisonOperand::Bool,
    ] {
        assert_eq!(
            facts.replacement(
                &comparison(operand, MirComparisonPredicate::Equal),
                MirType::Bool
            ),
            Some(Replacement::Constant(PrimitiveConstant::Bool(true)))
        );
        assert_eq!(
            facts.replacement(
                &comparison(operand, MirComparisonPredicate::NotEqual),
                MirType::Bool,
            ),
            Some(Replacement::Constant(PrimitiveConstant::Bool(false)))
        );
    }
    assert_eq!(
        facts.replacement(
            &comparison(MirComparisonOperand::F64, MirComparisonPredicate::Equal),
            MirType::Bool,
        ),
        None
    );
    assert_eq!(
        facts.replacement(
            &comparison(
                MirComparisonOperand::Integer(MirIntegerType::I64),
                MirComparisonPredicate::LessThan,
            ),
            MirType::Bool,
        ),
        None
    );
}

#[test]
fn unary_involutions_match_only_the_same_exact_operation() {
    let cases = [
        (MirUnaryOperation::NegateI64, MirType::I64),
        (MirUnaryOperation::LogicalNotBool, MirType::Bool),
        (
            MirUnaryOperation::BitwiseComplement(MirIntegerType::I64),
            MirType::I64,
        ),
        (
            MirUnaryOperation::BitwiseComplement(MirIntegerType::U64),
            MirType::U64,
        ),
        (
            MirUnaryOperation::BitwiseComplement(MirIntegerType::U8),
            MirType::U8,
        ),
    ];

    for (operation, ty) in cases {
        let mut facts = TestFacts::default();
        facts.begin_block();
        facts.observe_assignment(&assignment(
            0,
            match ty {
                MirType::I64 => MirRvalueKind::ConstantI64(2),
                MirType::U64 => MirRvalueKind::ConstantU64(2),
                MirType::U8 => MirRvalueKind::ConstantU8(2),
                MirType::Bool => MirRvalueKind::ConstantBool(true),
                _ => unreachable!(),
            },
            ty,
        ));
        facts.observe_assignment(&assignment(
            1,
            MirRvalueKind::Unary {
                operation,
                operand: value(0),
            },
            ty,
        ));
        assert_eq!(
            facts.replacement(
                &MirRvalueKind::Unary {
                    operation,
                    operand: value(1),
                },
                ty,
            ),
            Some(Replacement::Forward(value(0)))
        );
    }

    let mut floating = TestFacts::default();
    floating.begin_block();
    floating.observe_assignment(&assignment(
        0,
        MirRvalueKind::ConstantF64Bits(0),
        MirType::F64,
    ));
    floating.observe_assignment(&assignment(
        1,
        MirRvalueKind::Unary {
            operation: MirUnaryOperation::NegateF64,
            operand: value(0),
        },
        MirType::F64,
    ));
    assert_eq!(
        floating.replacement(
            &MirRvalueKind::Unary {
                operation: MirUnaryOperation::NegateF64,
                operand: value(1),
            },
            MirType::F64,
        ),
        None
    );
}
