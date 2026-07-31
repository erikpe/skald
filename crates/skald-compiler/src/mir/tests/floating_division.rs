use super::*;
use crate::hir::{
    dump_hir, HirBinaryOperation, HirExpression, HirExpressionKind, HirFunctionDefinition,
    HirReturnValue, HirStatement,
};

fn returned_expression_mut(definition: &mut HirFunctionDefinition) -> &mut HirExpression {
    let HirStatement::Return(statement) = definition.body.statements.last_mut().unwrap() else {
        panic!("expected final return statement");
    };
    let HirReturnValue::Scalar(expression) = statement.value.as_mut().unwrap() else {
        panic!("expected scalar return value");
    };
    expression
}

fn select_floating_division(
    hir: &mut crate::hir::HirProgram,
    function: FunctionId,
) -> crate::source::Span {
    let expression = returned_expression_mut(hir.definitions.get_mut_for_test(function).unwrap());
    let HirExpressionKind::Binary { operation, .. } = &mut expression.kind else {
        panic!("expected binary expression");
    };
    *operation = HirBinaryOperation::DivideF64;
    expression.span
}

fn floating_division_mir() -> MirProgram {
    let mut hir = type_check_source(concat!(
        "fn divide(left: f64, right: f64) -> f64 { return left * right; }\n",
        "fn main() -> i64 { return 0; }\n",
    ))
    .hir
    .unwrap();
    select_floating_division(&mut hir, FunctionId::new(0));
    lower_hir(&hir)
}

fn division_assignment_mut(definition: &mut MirFunctionDefinition) -> &mut MirAssignment {
    definition
        .body
        .blocks
        .iter_mut()
        .flat_map(|block| &mut block.instructions)
        .find_map(|instruction| match instruction {
            MirInstruction::Assign(assignment)
                if matches!(
                    assignment.rvalue.kind,
                    MirRvalueKind::Binary {
                        operation: MirBinaryOperation::DivideF64,
                        ..
                    }
                ) =>
            {
                Some(assignment)
            }
            _ => None,
        })
        .expect("fixture must contain floating division")
}

#[test]
fn floating_division_lowers_with_exact_types_and_deterministic_vocabulary() {
    let mut hir = type_check_source(concat!(
        "fn divide() -> f64 { return 8.0 * 2.0; }\n",
        "fn main() -> i64 { return 0; }\n",
    ))
    .hir
    .unwrap();
    select_floating_division(&mut hir, FunctionId::new(0));

    let hir_dump = dump_hir(&hir);
    assert_eq!(hir_dump, dump_hir(&hir));
    assert!(hir_dump.contains("Binary DivideF64 : f64"));

    let mir = lower_hir(&hir);
    verify_mir(&mir).unwrap();
    let definition = mir.definitions.get(FunctionId::new(0)).unwrap();
    assert_eq!(definition.body.blocks.len(), 1);
    let assignment = definition.body.blocks[0]
        .instructions
        .iter()
        .find_map(|instruction| match instruction {
            MirInstruction::Assign(assignment)
                if matches!(
                    assignment.rvalue.kind,
                    MirRvalueKind::Binary {
                        operation: MirBinaryOperation::DivideF64,
                        ..
                    }
                ) =>
            {
                Some(assignment)
            }
            _ => None,
        })
        .expect("floating division must lower to a binary rvalue");
    assert_eq!(assignment.rvalue.ty, MirType::F64);
    assert_eq!(
        assignment.rvalue.ty,
        MirBinaryOperation::DivideF64.result_type()
    );
    assert_eq!(MirBinaryOperation::DivideF64.operand_type(), MirType::F64);

    let mir_dump = dump_mir(&mir);
    assert_eq!(mir_dump, dump_mir(&mir));
    assert!(mir_dump.contains("div.f64"));
    assert!(!mir_dump.contains("divisor-check"));
    assert!(!mir_dump.contains("integer-division-by-zero"));
}

#[test]
fn floating_division_lowers_operands_once_in_left_to_right_order() {
    let mut hir = type_check_source(concat!(
        "fn left() -> f64 { return 8.0; }\n",
        "fn right() -> f64 { return 2.0; }\n",
        "fn divide() -> f64 { return left() * right(); }\n",
        "fn main() -> i64 { return 0; }\n",
    ))
    .hir
    .unwrap();
    select_floating_division(&mut hir, FunctionId::new(2));

    let mir = lower_hir(&hir);
    verify_mir(&mir).unwrap();
    let definition = mir.definitions.get(FunctionId::new(2)).unwrap();
    let calls: Vec<_> = definition.body.blocks[0]
        .instructions
        .iter()
        .filter_map(|instruction| match instruction {
            MirInstruction::Call(MirCall {
                target: MirCallTarget::Direct(function),
                ..
            }) => Some(*function),
            _ => None,
        })
        .collect();
    assert_eq!(calls, [FunctionId::new(0), FunctionId::new(1)]);

    let dump = dump_mir(&mir);
    let left = dump.find("call f0").unwrap();
    let right = dump.find("call f1").unwrap();
    let division = dump.find("div.f64").unwrap();
    assert!(left < right && right < division);
}

#[test]
fn floating_division_spills_left_before_control_affecting_right_operand() {
    let mut hir = type_check_source(concat!(
        "fn divide(left: f64, values: f64[]) -> f64 { return left * values[0]; }\n",
        "fn main() -> i64 { return 0; }\n",
    ))
    .hir
    .unwrap();
    select_floating_division(&mut hir, FunctionId::new(0));

    let mir = lower_hir(&hir);
    verify_mir(&mir).unwrap();
    let definition = mir.definitions.get(FunctionId::new(0)).unwrap();
    let spill = definition
        .storage
        .iter()
        .find(|storage| storage.kind == MirStorageKind::ScalarSpill)
        .expect("left operand must survive checked right-operand evaluation");
    assert_eq!(spill.ty, MirType::F64);

    let dump = dump_mir(&mir);
    let store = dump.find(&format!("store {}", spill.id)).unwrap();
    let check = dump.find("array-position-check").unwrap();
    let reload = dump.rfind(&format!("load {}", spill.id)).unwrap();
    let division = dump.find("div.f64").unwrap();
    assert!(store < check && check < reload && reload < division);
}

#[test]
fn verifier_rejects_floating_division_corruption_deterministically() {
    let mut wrong_result = floating_division_mir();
    division_assignment_mut(
        wrong_result
            .definitions
            .get_mut_for_test(FunctionId::new(0))
            .unwrap(),
    )
    .rvalue
    .ty = MirType::I64;
    let errors = verify_mir(&wrong_result).unwrap_err().to_string();
    assert_eq!(errors, verify_mir(&wrong_result).unwrap_err().to_string());
    assert!(errors.contains("binary operation result type mismatch"));

    let mut wrong_operand = floating_division_mir();
    let definition = wrong_operand
        .definitions
        .get_mut_for_test(FunctionId::new(0))
        .unwrap();
    let left = {
        let assignment = division_assignment_mut(definition);
        let MirRvalueKind::Binary { left, .. } = assignment.rvalue.kind else {
            unreachable!()
        };
        left
    };
    definition.values[left.index()].ty = MirType::I64;
    let errors = verify_mir(&wrong_operand).unwrap_err().to_string();
    assert_eq!(errors, verify_mir(&wrong_operand).unwrap_err().to_string());
    assert!(errors.contains("binary operand is not `f64`"));

    let mut use_before_definition = floating_division_mir();
    let definition = use_before_definition
        .definitions
        .get_mut_for_test(FunctionId::new(0))
        .unwrap();
    let assignment = division_assignment_mut(definition);
    let result = assignment.result;
    let MirRvalueKind::Binary { left, .. } = &mut assignment.rvalue.kind else {
        unreachable!()
    };
    *left = result;
    let errors = verify_mir(&use_before_definition).unwrap_err().to_string();
    assert_eq!(
        errors,
        verify_mir(&use_before_definition).unwrap_err().to_string()
    );
    assert!(errors.contains("is used before it is defined in this block"));
}
