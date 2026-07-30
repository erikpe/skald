use super::*;

fn compiler_storage(
    callable: crate::identity::CallableId,
    index: usize,
    name: &str,
    kind: MirStorageKind,
    ty: MirType,
    span: crate::source::Span,
) -> MirStorage {
    fixture_storage(StorageId::new(callable, index), None, name, kind, ty, span)
}

fn replace_main(
    mir: &mut MirProgram,
    storage: Vec<MirStorage>,
    values: Vec<MirValue>,
    blocks: Vec<MirBasicBlock>,
    path_conditions: Vec<MirPathCondition>,
) {
    let function = mir
        .definitions
        .get_mut_for_test(mir.entry_function)
        .unwrap();
    function.storage = storage;
    function.values = values;
    function.body = MirBody {
        entry: BlockId::new(function.function, 0),
        blocks,
        path_conditions,
        logical_expressions: Vec::new(),
    };
}

fn basic_path_condition_mir() -> MirProgram {
    let mut mir = lower_text("fn main() -> i64 { return 0; }");
    let function = mir.entry_function;
    let callable = function.into();
    let span = mir.definitions.get(function).unwrap().span;
    let activation = StorageId::new(function, 0);
    let selected_storage = StorageId::new(function, 1);
    let condition = PathConditionId::new(function, 0);
    let blocks: Vec<_> = (0..7).map(|index| BlockId::new(function, index)).collect();
    let values: Vec<_> = [
        MirType::Bool,
        MirType::Bool,
        MirType::Bool,
        MirType::Bool,
        MirType::I64,
    ]
    .into_iter()
    .enumerate()
    .map(|(index, ty)| fixture_value(ValueId::new(function, index), ty, span))
    .collect();

    replace_main(
        &mut mir,
        vec![
            compiler_storage(
                callable,
                0,
                "path0",
                MirStorageKind::PathCondition,
                MirType::Bool,
                span,
            ),
            compiler_storage(
                callable,
                1,
                "selected",
                MirStorageKind::ScalarSpill,
                MirType::I64,
                span,
            ),
        ],
        values,
        vec![
            fixture_block(
                blocks[0],
                vec![
                    fixture_storage_live(activation, span),
                    fixture_assign(
                        ValueId::new(function, 0),
                        MirRvalueKind::ConstantBool(true),
                        MirType::Bool,
                        span,
                    ),
                ],
                Some(MirTerminator::Branch {
                    condition: ValueId::new(function, 0),
                    true_target: blocks[1],
                    false_target: blocks[2],
                    span,
                }),
                span,
            ),
            fixture_block(
                blocks[1],
                vec![
                    fixture_storage_live(selected_storage, span),
                    fixture_assign(
                        ValueId::new(function, 1),
                        MirRvalueKind::ConstantBool(true),
                        MirType::Bool,
                        span,
                    ),
                    fixture_store(MirPlace::base(activation), ValueId::new(function, 1), span),
                ],
                Some(MirTerminator::Goto {
                    target: blocks[3],
                    span,
                }),
                span,
            ),
            fixture_block(
                blocks[2],
                vec![
                    fixture_assign(
                        ValueId::new(function, 2),
                        MirRvalueKind::ConstantBool(false),
                        MirType::Bool,
                        span,
                    ),
                    fixture_store(MirPlace::base(activation), ValueId::new(function, 2), span),
                ],
                Some(MirTerminator::Goto {
                    target: blocks[3],
                    span,
                }),
                span,
            ),
            fixture_block(
                blocks[3],
                vec![fixture_assign(
                    ValueId::new(function, 3),
                    MirRvalueKind::PathCondition(MirPathConditionValue {
                        condition,
                        activation,
                    }),
                    MirType::Bool,
                    span,
                )],
                Some(MirTerminator::Branch {
                    condition: ValueId::new(function, 3),
                    true_target: blocks[4],
                    false_target: blocks[5],
                    span,
                }),
                span,
            ),
            fixture_block(
                blocks[4],
                vec![fixture_storage_dead(selected_storage, span)],
                Some(MirTerminator::Goto {
                    target: blocks[6],
                    span,
                }),
                span,
            ),
            fixture_block(
                blocks[5],
                vec![],
                Some(MirTerminator::Goto {
                    target: blocks[6],
                    span,
                }),
                span,
            ),
            fixture_block(
                blocks[6],
                vec![
                    fixture_storage_dead(activation, span),
                    fixture_assign(
                        ValueId::new(function, 4),
                        MirRvalueKind::ConstantI64(0),
                        MirType::I64,
                        span,
                    ),
                ],
                Some(MirTerminator::Return {
                    value: Some(ValueId::new(function, 4)),
                    span,
                }),
                span,
            ),
        ],
        vec![MirPathCondition {
            id: condition,
            parent: None,
            activation,
            active_predecessor: blocks[1],
            inactive_predecessor: blocks[2],
            merge: blocks[3],
            span,
        }],
    );
    mir
}

fn nested_path_condition_mir() -> MirProgram {
    let mut mir = lower_text("fn main() -> i64 { return 0; }");
    let function = mir.entry_function;
    let callable = function.into();
    let span = mir.definitions.get(function).unwrap().span;
    let outer_activation = StorageId::new(function, 0);
    let child_activation = StorageId::new(function, 1);
    let selected_storage = StorageId::new(function, 2);
    let outer = PathConditionId::new(function, 0);
    let child = PathConditionId::new(function, 1);
    let blocks: Vec<_> = (0..11).map(|index| BlockId::new(function, index)).collect();
    let values: Vec<_> = (0..9)
        .map(|index| {
            fixture_value(
                ValueId::new(function, index),
                if index == 8 {
                    MirType::I64
                } else {
                    MirType::Bool
                },
                span,
            )
        })
        .collect();
    let bool_assignment = |index, value| {
        fixture_assign(
            ValueId::new(function, index),
            MirRvalueKind::ConstantBool(value),
            MirType::Bool,
            span,
        )
    };

    replace_main(
        &mut mir,
        vec![
            compiler_storage(
                callable,
                0,
                "outer",
                MirStorageKind::PathCondition,
                MirType::Bool,
                span,
            ),
            compiler_storage(
                callable,
                1,
                "child",
                MirStorageKind::PathCondition,
                MirType::Bool,
                span,
            ),
            compiler_storage(
                callable,
                2,
                "selected",
                MirStorageKind::ScalarSpill,
                MirType::I64,
                span,
            ),
        ],
        values,
        vec![
            fixture_block(
                blocks[0],
                vec![
                    fixture_storage_live(outer_activation, span),
                    bool_assignment(0, true),
                ],
                Some(MirTerminator::Branch {
                    condition: ValueId::new(function, 0),
                    true_target: blocks[1],
                    false_target: blocks[2],
                    span,
                }),
                span,
            ),
            fixture_block(
                blocks[1],
                vec![
                    bool_assignment(1, true),
                    fixture_store(
                        MirPlace::base(outer_activation),
                        ValueId::new(function, 1),
                        span,
                    ),
                ],
                Some(MirTerminator::Goto {
                    target: blocks[3],
                    span,
                }),
                span,
            ),
            fixture_block(
                blocks[2],
                vec![
                    bool_assignment(2, false),
                    fixture_store(
                        MirPlace::base(outer_activation),
                        ValueId::new(function, 2),
                        span,
                    ),
                ],
                Some(MirTerminator::Goto {
                    target: blocks[3],
                    span,
                }),
                span,
            ),
            fixture_block(
                blocks[3],
                vec![fixture_assign(
                    ValueId::new(function, 3),
                    MirRvalueKind::PathCondition(MirPathConditionValue {
                        condition: outer,
                        activation: outer_activation,
                    }),
                    MirType::Bool,
                    span,
                )],
                Some(MirTerminator::Branch {
                    condition: ValueId::new(function, 3),
                    true_target: blocks[4],
                    false_target: blocks[10],
                    span,
                }),
                span,
            ),
            fixture_block(
                blocks[4],
                vec![
                    fixture_storage_live(child_activation, span),
                    bool_assignment(4, true),
                ],
                Some(MirTerminator::Branch {
                    condition: ValueId::new(function, 4),
                    true_target: blocks[5],
                    false_target: blocks[6],
                    span,
                }),
                span,
            ),
            fixture_block(
                blocks[5],
                vec![
                    fixture_storage_live(selected_storage, span),
                    bool_assignment(5, true),
                    fixture_store(
                        MirPlace::base(child_activation),
                        ValueId::new(function, 5),
                        span,
                    ),
                ],
                Some(MirTerminator::Goto {
                    target: blocks[7],
                    span,
                }),
                span,
            ),
            fixture_block(
                blocks[6],
                vec![
                    bool_assignment(6, false),
                    fixture_store(
                        MirPlace::base(child_activation),
                        ValueId::new(function, 6),
                        span,
                    ),
                ],
                Some(MirTerminator::Goto {
                    target: blocks[7],
                    span,
                }),
                span,
            ),
            fixture_block(
                blocks[7],
                vec![fixture_assign(
                    ValueId::new(function, 7),
                    MirRvalueKind::PathCondition(MirPathConditionValue {
                        condition: child,
                        activation: child_activation,
                    }),
                    MirType::Bool,
                    span,
                )],
                Some(MirTerminator::Branch {
                    condition: ValueId::new(function, 7),
                    true_target: blocks[8],
                    false_target: blocks[9],
                    span,
                }),
                span,
            ),
            fixture_block(
                blocks[8],
                vec![
                    fixture_storage_dead(selected_storage, span),
                    fixture_storage_dead(child_activation, span),
                ],
                Some(MirTerminator::Goto {
                    target: blocks[10],
                    span,
                }),
                span,
            ),
            fixture_block(
                blocks[9],
                vec![fixture_storage_dead(child_activation, span)],
                Some(MirTerminator::Goto {
                    target: blocks[10],
                    span,
                }),
                span,
            ),
            fixture_block(
                blocks[10],
                vec![
                    fixture_storage_dead(outer_activation, span),
                    fixture_assign(
                        ValueId::new(function, 8),
                        MirRvalueKind::ConstantI64(0),
                        MirType::I64,
                        span,
                    ),
                ],
                Some(MirTerminator::Return {
                    value: Some(ValueId::new(function, 8)),
                    span,
                }),
                span,
            ),
        ],
        vec![
            MirPathCondition {
                id: outer,
                parent: None,
                activation: outer_activation,
                active_predecessor: blocks[1],
                inactive_predecessor: blocks[2],
                merge: blocks[3],
                span,
            },
            MirPathCondition {
                id: child,
                parent: Some(outer),
                activation: child_activation,
                active_predecessor: blocks[5],
                inactive_predecessor: blocks[6],
                merge: blocks[7],
                span,
            },
        ],
    );
    mir
}

fn sibling_path_condition_mir() -> MirProgram {
    let mut mir = lower_text("fn main() -> i64 { return 0; }");
    let function = mir.entry_function;
    let callable = function.into();
    let span = mir.definitions.get(function).unwrap().span;
    let first_activation = StorageId::new(function, 0);
    let second_activation = StorageId::new(function, 1);
    let first_selected = StorageId::new(function, 2);
    let second_selected = StorageId::new(function, 3);
    let unconditional = StorageId::new(function, 4);
    let first = PathConditionId::new(function, 0);
    let second = PathConditionId::new(function, 1);
    let blocks: Vec<_> = (0..13).map(|index| BlockId::new(function, index)).collect();
    let values: Vec<_> = (0..9)
        .map(|index| {
            fixture_value(
                ValueId::new(function, index),
                if index == 8 {
                    MirType::I64
                } else {
                    MirType::Bool
                },
                span,
            )
        })
        .collect();
    let bool_assignment = |index, value| {
        fixture_assign(
            ValueId::new(function, index),
            MirRvalueKind::ConstantBool(value),
            MirType::Bool,
            span,
        )
    };

    replace_main(
        &mut mir,
        vec![
            compiler_storage(
                callable,
                0,
                "first",
                MirStorageKind::PathCondition,
                MirType::Bool,
                span,
            ),
            compiler_storage(
                callable,
                1,
                "second",
                MirStorageKind::PathCondition,
                MirType::Bool,
                span,
            ),
            compiler_storage(
                callable,
                2,
                "first-selected",
                MirStorageKind::ScalarSpill,
                MirType::I64,
                span,
            ),
            compiler_storage(
                callable,
                3,
                "second-selected",
                MirStorageKind::ScalarSpill,
                MirType::I64,
                span,
            ),
            compiler_storage(
                callable,
                4,
                "unconditional",
                MirStorageKind::ScalarSpill,
                MirType::I64,
                span,
            ),
        ],
        values,
        vec![
            fixture_block(
                blocks[0],
                vec![
                    fixture_storage_live(unconditional, span),
                    fixture_storage_live(first_activation, span),
                    bool_assignment(0, true),
                ],
                Some(MirTerminator::Branch {
                    condition: ValueId::new(function, 0),
                    true_target: blocks[1],
                    false_target: blocks[2],
                    span,
                }),
                span,
            ),
            fixture_block(
                blocks[1],
                vec![
                    fixture_storage_live(first_selected, span),
                    bool_assignment(1, true),
                    fixture_store(
                        MirPlace::base(first_activation),
                        ValueId::new(function, 1),
                        span,
                    ),
                ],
                Some(MirTerminator::Goto {
                    target: blocks[3],
                    span,
                }),
                span,
            ),
            fixture_block(
                blocks[2],
                vec![
                    bool_assignment(2, false),
                    fixture_store(
                        MirPlace::base(first_activation),
                        ValueId::new(function, 2),
                        span,
                    ),
                ],
                Some(MirTerminator::Goto {
                    target: blocks[3],
                    span,
                }),
                span,
            ),
            fixture_block(
                blocks[3],
                vec![
                    fixture_storage_live(second_activation, span),
                    bool_assignment(3, true),
                ],
                Some(MirTerminator::Branch {
                    condition: ValueId::new(function, 3),
                    true_target: blocks[4],
                    false_target: blocks[5],
                    span,
                }),
                span,
            ),
            fixture_block(
                blocks[4],
                vec![
                    fixture_storage_live(second_selected, span),
                    bool_assignment(4, true),
                    fixture_store(
                        MirPlace::base(second_activation),
                        ValueId::new(function, 4),
                        span,
                    ),
                ],
                Some(MirTerminator::Goto {
                    target: blocks[6],
                    span,
                }),
                span,
            ),
            fixture_block(
                blocks[5],
                vec![
                    bool_assignment(5, false),
                    fixture_store(
                        MirPlace::base(second_activation),
                        ValueId::new(function, 5),
                        span,
                    ),
                ],
                Some(MirTerminator::Goto {
                    target: blocks[6],
                    span,
                }),
                span,
            ),
            fixture_block(
                blocks[6],
                vec![fixture_assign(
                    ValueId::new(function, 6),
                    MirRvalueKind::PathCondition(MirPathConditionValue {
                        condition: second,
                        activation: second_activation,
                    }),
                    MirType::Bool,
                    span,
                )],
                Some(MirTerminator::Branch {
                    condition: ValueId::new(function, 6),
                    true_target: blocks[7],
                    false_target: blocks[8],
                    span,
                }),
                span,
            ),
            fixture_block(
                blocks[7],
                vec![fixture_storage_dead(second_selected, span)],
                Some(MirTerminator::Goto {
                    target: blocks[9],
                    span,
                }),
                span,
            ),
            fixture_block(
                blocks[8],
                vec![],
                Some(MirTerminator::Goto {
                    target: blocks[9],
                    span,
                }),
                span,
            ),
            fixture_block(
                blocks[9],
                vec![
                    fixture_storage_dead(second_activation, span),
                    fixture_assign(
                        ValueId::new(function, 7),
                        MirRvalueKind::PathCondition(MirPathConditionValue {
                            condition: first,
                            activation: first_activation,
                        }),
                        MirType::Bool,
                        span,
                    ),
                ],
                Some(MirTerminator::Branch {
                    condition: ValueId::new(function, 7),
                    true_target: blocks[10],
                    false_target: blocks[11],
                    span,
                }),
                span,
            ),
            fixture_block(
                blocks[10],
                vec![fixture_storage_dead(first_selected, span)],
                Some(MirTerminator::Goto {
                    target: blocks[12],
                    span,
                }),
                span,
            ),
            fixture_block(
                blocks[11],
                vec![],
                Some(MirTerminator::Goto {
                    target: blocks[12],
                    span,
                }),
                span,
            ),
            fixture_block(
                blocks[12],
                vec![
                    fixture_storage_dead(first_activation, span),
                    fixture_storage_dead(unconditional, span),
                    fixture_assign(
                        ValueId::new(function, 8),
                        MirRvalueKind::ConstantI64(0),
                        MirType::I64,
                        span,
                    ),
                ],
                Some(MirTerminator::Return {
                    value: Some(ValueId::new(function, 8)),
                    span,
                }),
                span,
            ),
        ],
        vec![
            MirPathCondition {
                id: first,
                parent: None,
                activation: first_activation,
                active_predecessor: blocks[1],
                inactive_predecessor: blocks[2],
                merge: blocks[3],
                span,
            },
            MirPathCondition {
                id: second,
                parent: None,
                activation: second_activation,
                active_predecessor: blocks[4],
                inactive_predecessor: blocks[5],
                merge: blocks[6],
                span,
            },
        ],
    );
    mir
}

#[test]
fn verifier_accepts_selected_and_skipped_conditional_storage() {
    let mir = basic_path_condition_mir();

    verify_mir(&mir).unwrap();
}

#[test]
fn verifier_accepts_nested_conditions_only_inside_the_active_parent() {
    let mir = nested_path_condition_mir();

    verify_mir(&mir).unwrap();
}

#[test]
fn verifier_accepts_sibling_conditions_that_can_both_be_active() {
    let mir = sibling_path_condition_mir();

    verify_mir(&mir).unwrap();
}

#[test]
fn verifier_allows_a_condition_identity_to_start_a_new_loop_epoch() {
    let mut mir = basic_path_condition_mir();
    let function = mir
        .definitions
        .get_mut_for_test(mir.entry_function)
        .unwrap();
    function.values.pop();
    function.body.blocks[6].instructions.pop();
    function.body.blocks[6].terminator = Some(MirTerminator::Goto {
        target: function.body.entry,
        span: function.span,
    });

    verify_mir(&mir).unwrap();
}

#[test]
fn path_condition_dump_is_exact_and_deterministic() {
    let mir = basic_path_condition_mir();
    let dump = dump_mir(&mir);

    assert!(dump.contains(concat!(
        "      PathConditions\n",
        "        f0:p0 parent <root> activation f0:s0 active f0:b1 inactive f0:b2 merge f0:b3",
    )));
    assert!(dump.contains("f0:v3 = path-condition f0:p0 from f0:s0 : bool"));
    assert_eq!(dump, dump_mir(&mir));
}

#[test]
fn verifier_rejects_noncanonical_or_missing_path_selection() {
    let mut noncanonical = basic_path_condition_mir();
    let function = noncanonical
        .definitions
        .get_mut_for_test(noncanonical.entry_function)
        .unwrap();
    let MirInstruction::Assign(assignment) = &mut function.body.blocks[1].instructions[1] else {
        panic!("active predecessor must assign its selection");
    };
    assignment.rvalue.kind = MirRvalueKind::ConstantBool(false);
    assert!(verify_mir(&noncanonical)
        .unwrap_err()
        .to_string()
        .contains("must store canonical `true`"));

    let mut missing = basic_path_condition_mir();
    let function = missing
        .definitions
        .get_mut_for_test(missing.entry_function)
        .unwrap();
    function.body.blocks[2].instructions.pop();
    assert!(verify_mir(&missing)
        .unwrap_err()
        .to_string()
        .contains("predecessor must end by storing its selection"));

    let mut duplicate = basic_path_condition_mir();
    let function = duplicate
        .definitions
        .get_mut_for_test(duplicate.entry_function)
        .unwrap();
    let duplicate_store = function.body.blocks[1].instructions[2].clone();
    function.body.blocks[1]
        .instructions
        .insert(2, duplicate_store);
    assert!(verify_mir(&duplicate)
        .unwrap_err()
        .to_string()
        .contains("must write its activation exactly once"));
}

#[test]
fn verifier_rejects_invalid_parent_and_child_reads_outside_it() {
    let mut invalid_parent = nested_path_condition_mir();
    let function = invalid_parent
        .definitions
        .get_mut_for_test(invalid_parent.entry_function)
        .unwrap();
    function.body.path_conditions[1].parent = Some(PathConditionId::new(function.function, 1));
    assert!(verify_mir(&invalid_parent)
        .unwrap_err()
        .to_string()
        .contains("invalid or non-preceding parent"));

    let mut outside_parent = nested_path_condition_mir();
    let function = outside_parent
        .definitions
        .get_mut_for_test(outside_parent.entry_function)
        .unwrap();
    let child = function.body.path_conditions[1].id;
    let child_activation = function.body.path_conditions[1].activation;
    let value = function.values[3].id;
    function.body.blocks[3].instructions[0] = fixture_assign(
        value,
        MirRvalueKind::PathCondition(MirPathConditionValue {
            condition: child,
            activation: child_activation,
        }),
        MirType::Bool,
        function.span,
    );
    let errors = verify_mir(&outside_parent).unwrap_err().to_string();
    assert!(errors.contains("read before selection or outside its active parent path"));

    let mut sibling_as_child = sibling_path_condition_mir();
    let function = sibling_as_child
        .definitions
        .get_mut_for_test(sibling_as_child.entry_function)
        .unwrap();
    let first = function.body.path_conditions[0].id;
    function.body.path_conditions[1].parent = Some(first);
    assert!(verify_mir(&sibling_as_child)
        .unwrap_err()
        .to_string()
        .contains("outside active parent"));
}

#[test]
fn verifier_rejects_wrong_activation_ownership_and_leakage() {
    let mut wrong_activation = basic_path_condition_mir();
    let function = wrong_activation
        .definitions
        .get_mut_for_test(wrong_activation.entry_function)
        .unwrap();
    function.body.path_conditions[0].activation = StorageId::new(function.function, 1);
    assert!(verify_mir(&wrong_activation)
        .unwrap_err()
        .to_string()
        .contains("requires matching `bool` path-condition storage"));

    let mut leakage = basic_path_condition_mir();
    let function = leakage
        .definitions
        .get_mut_for_test(leakage.entry_function)
        .unwrap();
    function.body.blocks[6].instructions.remove(0);
    assert!(verify_mir(&leakage)
        .unwrap_err()
        .to_string()
        .contains("remains live on normal return"));
}

#[test]
fn verifier_keeps_ordinary_join_lifetimes_strict() {
    let mut mir = basic_path_condition_mir();
    let function = mir
        .definitions
        .get_mut_for_test(mir.entry_function)
        .unwrap();
    function.body.path_conditions.clear();
    function.storage[0].kind = MirStorageKind::ScalarSpill;
    function.body.blocks[3].instructions.clear();
    function.body.blocks[3].terminator = Some(MirTerminator::Goto {
        target: function.body.blocks[6].id,
        span: function.span,
    });
    function.body.blocks[1].instructions.pop();
    function.body.blocks[2].instructions.pop();

    let errors = verify_mir(&mir).unwrap_err().to_string();
    assert!(errors.contains("storage lifetime state disagrees at control-flow join"));
}

#[test]
fn verifier_rejects_unresolved_conditional_state_when_activation_ends() {
    let mut mir = basic_path_condition_mir();
    let function = mir
        .definitions
        .get_mut_for_test(mir.entry_function)
        .unwrap();
    function.body.blocks[4].instructions.clear();

    let errors = verify_mir(&mir).unwrap_err().to_string();
    assert!(errors
        .contains("conditional storage lifetime state remains when path condition f0:p0 ends"));
}

#[test]
fn verifier_rejects_cleanup_guarded_by_the_wrong_sibling_condition() {
    let mut mir = sibling_path_condition_mir();
    let function = mir
        .definitions
        .get_mut_for_test(mir.entry_function)
        .unwrap();
    let first = function.body.path_conditions[0].id;
    let first_activation = function.body.path_conditions[0].activation;
    let MirInstruction::Assign(assignment) = &mut function.body.blocks[6].instructions[0] else {
        panic!("second condition cleanup must begin with a condition read");
    };
    assignment.rvalue.kind = MirRvalueKind::PathCondition(MirPathConditionValue {
        condition: first,
        activation: first_activation,
    });

    let errors = verify_mir(&mir).unwrap_err().to_string();
    assert!(errors.contains("is already dead"));
    assert!(errors.contains("conditional storage lifetime state remains"));
}
