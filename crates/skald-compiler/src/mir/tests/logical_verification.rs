//! Adversarial verification coverage for structured logical-expression MIR.

use super::*;

fn lowered(operation: &str) -> MirProgram {
    lower_text(&format!(
        "fn evaluate(left: bool, right: bool) -> bool {{ return left {operation} right; }}\n\
         fn main() -> i64 {{ return 0; }}\n"
    ))
}

fn errors(program: &MirProgram) -> String {
    verify_mir(program).unwrap_err().to_string()
}

#[test]
fn rejects_non_boolean_values_and_invalid_result_carriers() {
    let mut left_type = lowered("&&");
    let function = left_type
        .definitions
        .get_mut_for_test(FunctionId::new(0))
        .unwrap();
    let logical = function.body.logical_expressions[0].clone();
    function.values[logical.left_result.index()].ty = MirType::I64;
    assert!(errors(&left_type).contains("logical left result must have exact type `bool`"));

    let mut selected_type = lowered("||");
    let function = selected_type
        .definitions
        .get_mut_for_test(FunctionId::new(0))
        .unwrap();
    let logical = function.body.logical_expressions[0].clone();
    function.values[logical.selected_result.index()].ty = MirType::I64;
    assert!(errors(&selected_type).contains("logical selected result must have exact type `bool`"));

    let mut carrier_type = lowered("&&");
    let function = carrier_type
        .definitions
        .get_mut_for_test(FunctionId::new(0))
        .unwrap();
    let logical = function.body.logical_expressions[0].clone();
    function.storage[logical.result.index()].ty = MirType::I64;
    assert!(errors(&carrier_type)
        .contains("logical result carrier must be `bool` scalar-spill storage"));
}

#[test]
fn rejects_noncanonical_short_results_and_incomplete_carrier_lifetimes() {
    let mut wrong_short = lowered("&&");
    let function = wrong_short
        .definitions
        .get_mut_for_test(FunctionId::new(0))
        .unwrap();
    let short = function.body.logical_expressions[0].short;
    let fixed = function.body.blocks[short.index()]
        .instructions
        .iter_mut()
        .find_map(|instruction| match instruction {
            MirInstruction::Assign(assignment) => Some(assignment),
            _ => None,
        })
        .unwrap();
    fixed.rvalue.kind = MirRvalueKind::ConstantBool(true);
    assert!(errors(&wrong_short).contains("logical short path stores the wrong selected result"));

    for carrier in ["result", "activation"] {
        let mut not_live = lowered("||");
        let function = not_live
            .definitions
            .get_mut_for_test(FunctionId::new(0))
            .unwrap();
        let logical = function.body.logical_expressions[0].clone();
        let activation = function
            .body
            .path_conditions
            .iter()
            .find(|condition| condition.id == logical.condition)
            .unwrap()
            .activation;
        let removed = if carrier == "result" {
            logical.result
        } else {
            activation
        };
        function.body.blocks[logical.split.index()]
            .instructions
            .retain(|instruction| {
                !matches!(
                    instruction,
                    MirInstruction::StorageLive(operation) if operation.storage == removed
                )
            });
        assert!(errors(&not_live).contains(&format!(
            "logical carrier {removed} is not live before the split"
        )));
    }
}

#[test]
fn rejects_missing_duplicate_and_premature_result_use() {
    let mut missing_store = lowered("||");
    let function = missing_store
        .definitions
        .get_mut_for_test(FunctionId::new(0))
        .unwrap();
    let right = function.body.logical_expressions[0].right_exit;
    function.body.blocks[right.index()].instructions.pop();
    assert!(errors(&missing_store)
        .contains("logical right result block must end by storing its result"));

    let mut duplicate_store = lowered("&&");
    let function = duplicate_store
        .definitions
        .get_mut_for_test(FunctionId::new(0))
        .unwrap();
    let logical = function.body.logical_expressions[0].clone();
    let duplicated = function.body.blocks[logical.right_exit.index()]
        .instructions
        .last()
        .unwrap()
        .clone();
    function.body.blocks[logical.right_exit.index()]
        .instructions
        .push(duplicated);
    assert!(
        errors(&duplicate_store).contains("logical result carrier must be written exactly once")
    );

    let mut premature = lowered("||");
    let function = premature
        .definitions
        .get_mut_for_test(FunctionId::new(0))
        .unwrap();
    let logical = function.body.logical_expressions[0].clone();
    let join = &mut function.body.blocks[logical.join.index()];
    let selected = join
        .instructions
        .iter()
        .position(|instruction| {
            matches!(
                instruction,
                MirInstruction::Assign(assignment)
                    if assignment.result == logical.selected_result
            )
        })
        .unwrap();
    let selected = join.instructions.remove(selected);
    function.body.blocks[logical.split.index()]
        .instructions
        .push(selected);
    assert!(errors(&premature)
        .contains("logical selected result must load its carrier in the result join"));
}

#[test]
fn rejects_invalid_split_selection_join_and_right_region_edges() {
    let mut wrong_split = lowered("&&");
    let function = wrong_split
        .definitions
        .get_mut_for_test(FunctionId::new(0))
        .unwrap();
    let logical = function.body.logical_expressions[0].clone();
    let Some(MirTerminator::Branch {
        true_target,
        false_target,
        ..
    }) = &mut function.body.blocks[logical.split.index()].terminator
    else {
        unreachable!()
    };
    std::mem::swap(true_target, false_target);
    assert!(errors(&wrong_split).contains("logical split has the wrong operand or branch targets"));

    let mut wrong_selection = lowered("||");
    let function = wrong_selection
        .definitions
        .get_mut_for_test(FunctionId::new(0))
        .unwrap();
    let logical = function.body.logical_expressions[0].clone();
    let Some(MirTerminator::Branch { true_target, .. }) =
        &mut function.body.blocks[logical.selection.index()].terminator
    else {
        unreachable!()
    };
    *true_target = logical.short;
    assert!(errors(&wrong_selection)
        .contains("logical selection must branch on its activation to right and short paths"));

    let mut wrong_join = lowered("&&");
    let function = wrong_join
        .definitions
        .get_mut_for_test(FunctionId::new(0))
        .unwrap();
    let selection = function.body.logical_expressions[0].selection;
    function.body.logical_expressions[0].join = selection;
    assert!(errors(&wrong_join)
        .contains("logical result join must have exactly its short and right predecessors"));

    let mut short_reentry = lowered("||");
    let function = short_reentry
        .definitions
        .get_mut_for_test(FunctionId::new(0))
        .unwrap();
    let logical = function.body.logical_expressions[0].clone();
    function.body.blocks[logical.short.index()].terminator = Some(MirTerminator::Goto {
        target: logical.right_entry,
        span: function.span,
    });
    assert!(errors(&short_reentry).contains("has an incoming edge outside its selected region"));
}

#[test]
fn rejects_undeclared_or_reused_logical_conditions() {
    let mut undeclared = lowered("&&");
    let function = undeclared
        .definitions
        .get_mut_for_test(FunctionId::new(0))
        .unwrap();
    function.body.path_conditions.clear();
    assert!(errors(&undeclared).contains("logical expression references undeclared path condition"));

    let mut reused = lowered("||");
    let function = reused
        .definitions
        .get_mut_for_test(FunctionId::new(0))
        .unwrap();
    let duplicate = function.body.logical_expressions[0].clone();
    function.body.logical_expressions.push(duplicate);
    assert!(errors(&reused).contains("describes more than one logical expression"));
}

#[test]
fn rejects_duplicated_right_evaluation_and_failure_reachable_from_the_short_path() {
    let mut duplicated_evaluation = lowered("&&");
    let function = duplicated_evaluation
        .definitions
        .get_mut_for_test(FunctionId::new(0))
        .unwrap();
    let logical = function.body.logical_expressions[0].clone();
    let right_definition = function.body.blocks[logical.right_exit.index()]
        .instructions
        .iter()
        .find(|instruction| {
            matches!(
                instruction,
                MirInstruction::Assign(assignment)
                    if assignment.result == logical.right_result
            )
        })
        .unwrap()
        .clone();
    function.body.blocks[logical.split.index()]
        .instructions
        .push(right_definition);
    assert!(errors(&duplicated_evaluation).contains("is defined more than once"));

    let mut skipped_failure = lower_text(concat!(
        "fn evaluate(left: bool, value: bool?) -> bool { return left && value!; }\n",
        "fn main() -> i64 { return 0; }\n",
    ));
    let function = skipped_failure
        .definitions
        .get_mut_for_test(FunctionId::new(0))
        .unwrap();
    let logical = function.body.logical_expressions[0].clone();
    let failure = function
        .body
        .blocks
        .iter()
        .find(|block| {
            matches!(
                block.terminator,
                Some(MirTerminator::Terminate {
                    reason: MirTerminationReason::OptionalAccessFailure,
                    ..
                })
            )
        })
        .expect("selected right operand must contain an optional failure block")
        .id;
    function.body.blocks[logical.short.index()].terminator = Some(MirTerminator::Goto {
        target: failure,
        span: function.span,
    });
    assert!(errors(&skipped_failure)
        .contains("logical short result block must jump directly to the result join"));
}
