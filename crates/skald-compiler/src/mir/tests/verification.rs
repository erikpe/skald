use super::*;

#[test]
fn verifier_rejects_u8_constant_and_operation_type_corruption() {
    let mut constant_mismatch =
        lower_text("fn value() -> u8 { return 1u8; } fn main() -> i64 { return 0; }");
    let function = constant_mismatch
        .definitions
        .get_mut_for_test(FunctionId::new(0))
        .unwrap();
    let MirInstruction::Assign(assignment) = &mut function.body.blocks[0].instructions[0] else {
        panic!("expected constant assignment");
    };
    assignment.rvalue.ty = MirType::U64;
    assert!(verify_mir(&constant_mismatch)
        .unwrap_err()
        .to_string()
        .contains("u8 constant is not `u8`"));

    let mut operation_mismatch =
        lower_text("fn add() -> u8 { return 1u8 + 2u8; } fn main() -> i64 { return 0; }");
    let function = operation_mismatch
        .definitions
        .get_mut_for_test(FunctionId::new(0))
        .unwrap();
    let MirInstruction::Assign(assignment) = &mut function.body.blocks[0].instructions[2] else {
        panic!("expected binary assignment");
    };
    let MirRvalueKind::Binary { operation, .. } = &mut assignment.rvalue.kind else {
        panic!("expected binary rvalue");
    };
    *operation = MirBinaryOperation::AddU64;
    let errors = verify_mir(&operation_mismatch).unwrap_err().to_string();
    assert!(errors.contains("binary operation result type mismatch"));
    assert!(errors.contains("arithmetic operand is not `u64`"));
}

#[test]
fn verifier_rejects_f64_constant_unary_and_binary_corruption() {
    let mut constant = f64_arithmetic_mir();
    let function = constant
        .definitions
        .get_mut_for_test(FunctionId::new(0))
        .unwrap();
    let MirInstruction::Assign(assignment) = &mut function.body.blocks[0].instructions[0] else {
        panic!("expected constant assignment");
    };
    assignment.rvalue.ty = MirType::I64;
    assert!(verify_mir(&constant)
        .unwrap_err()
        .to_string()
        .contains("f64 constant is not `f64`"));

    let mut operations = f64_arithmetic_mir();
    let function = operations
        .definitions
        .get_mut_for_test(FunctionId::new(0))
        .unwrap();
    let MirInstruction::Assign(binary) = &mut function.body.blocks[0].instructions[3] else {
        panic!("expected binary assignment");
    };
    binary.rvalue.ty = MirType::I64;
    let MirInstruction::Assign(unary) = &mut function.body.blocks[0].instructions[7] else {
        panic!("expected unary assignment");
    };
    unary.rvalue.ty = MirType::I64;
    let errors = verify_mir(&operations).unwrap_err().to_string();
    assert!(errors.contains("binary operation result type mismatch"));
    assert!(errors.contains("unary operation result type mismatch"));
}

#[test]
fn verifier_rejects_a_boolean_constant_with_a_non_boolean_result() {
    let mut mir = lower_text("fn flag() -> bool { return true; } fn main() -> i64 { return 0; }");
    let flag = mir
        .definitions
        .get_mut_for_test(FunctionId::new(0))
        .unwrap();
    let MirInstruction::Assign(assignment) = &mut flag.body.blocks[0].instructions[0] else {
        panic!("expected constant assignment");
    };
    assignment.rvalue.ty = MirType::I64;

    let errors = verify_mir(&mir).unwrap_err();
    assert!(errors
        .iter()
        .any(|error| error.message.contains("boolean constant is not `bool`")));
}

#[test]
fn verifies_jumps_joins_diamonds_and_multiple_returns() {
    let join = goto_join_mir();
    let diamond = diamond_mir();

    assert!(verify_mir(&join).is_ok());
    assert!(verify_mir(&diamond).is_ok());
    assert!(dump_mir(&join).contains("goto f0:b1"));
    let join_function = join.definitions.get(join.entry_function).unwrap();
    assert_eq!(
        join_function.body.blocks[0]
            .terminator
            .as_ref()
            .unwrap()
            .successors()
            .collect::<Vec<_>>(),
        [join_function.body.blocks[1].id]
    );
    let function = diamond.definitions.get(diamond.entry_function).unwrap();
    let successors: Vec<_> = function.body.blocks[0]
        .terminator
        .as_ref()
        .unwrap()
        .successors()
        .collect();
    assert_eq!(
        successors,
        [function.body.blocks[1].id, function.body.blocks[2].id]
    );
    assert_eq!(
        function.body.blocks[1]
            .terminator
            .as_ref()
            .unwrap()
            .successors()
            .count(),
        0
    );
}

#[test]
fn verifier_rejects_missing_and_foreign_control_flow_targets() {
    let mut missing = goto_join_mir();
    let function = missing
        .definitions
        .get_mut_for_test(missing.entry_function)
        .unwrap();
    function.body.blocks[0].terminator = Some(MirTerminator::Goto {
        target: BlockId::new(function.function, 99),
        span: function.span,
    });
    let errors = verify_mir(&missing).unwrap_err();
    assert!(errors
        .iter()
        .any(|error| error.message.contains("target f0:b99 is not declared")));

    let mut foreign = goto_join_mir();
    let function = foreign
        .definitions
        .get_mut_for_test(foreign.entry_function)
        .unwrap();
    function.body.blocks[0].terminator = Some(MirTerminator::Goto {
        target: BlockId::new(FunctionId::new(99), 0),
        span: function.span,
    });
    let errors = verify_mir(&foreign).unwrap_err();
    assert!(errors.iter().any(|error| error
        .message
        .contains("target f99:b0 is owned by another callable body")));
}

#[test]
fn verifier_rejects_invalid_entry_and_non_dense_block_ids() {
    let mut missing_entry = goto_join_mir();
    let function = missing_entry
        .definitions
        .get_mut_for_test(missing_entry.entry_function)
        .unwrap();
    function.body.entry = BlockId::new(function.function, 99);
    let errors = verify_mir(&missing_entry).unwrap_err();
    assert!(errors
        .iter()
        .any(|error| error.message.contains("entry block f0:b99 is not declared")));

    let mut foreign_entry = goto_join_mir();
    let function = foreign_entry
        .definitions
        .get_mut_for_test(foreign_entry.entry_function)
        .unwrap();
    function.body.entry = BlockId::new(FunctionId::new(99), 0);
    let errors = verify_mir(&foreign_entry).unwrap_err();
    assert!(errors.iter().any(|error| error
        .message
        .contains("entry block f99:b0 is owned by another callable body")));

    let mut sparse = goto_join_mir();
    let function = sparse
        .definitions
        .get_mut_for_test(sparse.entry_function)
        .unwrap();
    function.body.blocks[1].id = BlockId::new(function.function, 2);
    let errors = verify_mir(&sparse).unwrap_err();
    assert!(errors
        .iter()
        .any(|error| error.message.contains("block table index 1 contains f0:b2")));
}

#[test]
fn verifier_requires_a_boolean_branch_condition() {
    let mut mir = lower_text("fn main() -> i64 { return 0; }");
    let function = mir
        .definitions
        .get_mut_for_test(mir.entry_function)
        .unwrap();
    let condition = function.values[0].id;
    let target = function.body.entry;
    function.body.blocks[0].terminator = Some(MirTerminator::Branch {
        condition,
        true_target: target,
        false_target: target,
        span: function.span,
    });

    let errors = verify_mir(&mir).unwrap_err();
    assert!(errors
        .iter()
        .any(|error| error.message.contains("branch condition is not `bool`")));
}

#[test]
fn verifier_rejects_unterminated_blocks() {
    let mut mir = lower_text("fn main() -> i64 { return 0; }");
    mir.definitions
        .get_mut_for_test(mir.entry_function)
        .unwrap()
        .body
        .blocks[0]
        .terminator = None;

    let errors = super::verify_mir(&mir).unwrap_err();
    assert!(errors
        .iter()
        .any(|error| error.message.contains("no terminator")));
}

#[test]
fn verifier_rejects_use_before_definition() {
    let mut mir = lower_text("fn main() -> i64 { return 1 + 2; }");
    let function = mir
        .definitions
        .get_mut_for_test(mir.entry_function)
        .unwrap();
    function.body.blocks[0].instructions.swap(0, 2);

    let errors = super::verify_mir(&mir).unwrap_err();
    assert!(errors
        .iter()
        .any(|error| error.message.contains("used before it is defined")));
}

#[test]
fn verifier_rejects_a_value_defined_in_terms_of_itself() {
    let mut mir = lower_text("fn main() -> i64 { return 1; }");
    let function = mir
        .definitions
        .get_mut_for_test(mir.entry_function)
        .unwrap();
    let MirInstruction::Assign(assignment) = &mut function.body.blocks[0].instructions[0] else {
        panic!("expected assignment");
    };
    assignment.rvalue.kind = MirRvalueKind::Unary {
        operation: MirUnaryOperation::NegateI64,
        operand: assignment.result,
    };

    let errors = super::verify_mir(&mir).unwrap_err();
    assert!(errors
        .iter()
        .any(|error| error.message.contains("used before it is defined")));
}

#[test]
fn verifier_rejects_ids_owned_by_another_callable_body() {
    let mut mir = lower_text("fn main() -> i64 { return 0; }");
    let foreign = MethodId::new(ClassId::new(7), 2);
    mir.definitions
        .get_mut_for_test(mir.entry_function)
        .unwrap()
        .values[0]
        .id = ValueId::new(foreign, 0);

    let errors = super::verify_mir(&mir).unwrap_err();
    assert!(errors
        .iter()
        .any(|error| error.message.contains("owned by another callable body")));
}

#[test]
fn verifier_accepts_an_external_declaration_without_a_definition() {
    let mir = lower_text(concat!(
        "extern fn foreign(value: i64) -> i64;\n",
        "fn main() -> i64 { return foreign(7); }\n",
    ));
    let foreign = FunctionId::new(0);

    assert!(verify_mir(&mir).is_ok());
    assert!(mir.declarations.get(foreign).is_some());
    assert!(mir.definitions.get(foreign).is_none());
    assert!(dump_mir(&mir).contains("Declaration f0 \"foreign\" external \"foreign\""));
}

#[test]
fn verifier_rejects_return_operand_presence_mismatches() {
    let mut unit_with_value = lower_text(concat!(
        "fn helper() -> i64 { return 1; }\n",
        "fn main() -> i64 { return 0; }\n",
    ));
    unit_with_value.declarations.entries_mut_for_test()[0].return_type = MirType::Unit;
    let errors = verify_mir(&unit_with_value).unwrap_err();
    assert!(errors.iter().any(|error| error
        .message
        .contains("unit and object returns must not have a scalar operand")));

    let mut value_without_operand = lower_text("fn main() -> i64 { return 0; }");
    let main = value_without_operand
        .definitions
        .get_mut_for_test(value_without_operand.entry_function)
        .unwrap();
    let Some(MirTerminator::Return { value, .. }) = &mut main.body.blocks[0].terminator else {
        panic!("expected return terminator");
    };
    *value = None;
    let errors = verify_mir(&value_without_operand).unwrap_err();
    assert!(errors.iter().any(|error| error
        .message
        .contains("value-returning function return has no operand")));
}

#[test]
fn verifier_rejects_definition_signature_mismatches() {
    let mut mir = lower_text(concat!(
        "fn helper(value: i64) -> i64 { return value; }\n",
        "fn main() -> i64 { return helper(1); }\n",
    ));
    mir.definitions
        .get_mut_for_test(FunctionId::new(0))
        .unwrap()
        .parameters
        .clear();

    let errors = verify_mir(&mir).unwrap_err();
    assert!(errors.iter().any(|error| error
        .message
        .contains("definition has 0 parameters but declaration requires 1")));
}
