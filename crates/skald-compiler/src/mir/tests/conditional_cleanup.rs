use super::*;

fn errors_for(mir: &MirProgram) -> String {
    verify_mir(mir).unwrap_err().to_string()
}

#[test]
fn verifies_conditional_full_expression_cleanup_and_secured_result() {
    let mir = fixture_conditional_cleanup_program();

    verify_mir(&mir).unwrap();
    let function = mir.definitions.get(mir.entry_function).unwrap();
    let cleanup = &function.body.blocks[4].instructions;
    let MirInstruction::EndFullExpression(end) = &cleanup[0] else {
        panic!("selected cleanup path must end its temporaries");
    };
    assert_eq!(
        end.temporaries
            .iter()
            .map(|cleanup| cleanup.destination.base.expect_local_storage().index())
            .collect::<Vec<_>>(),
        [2, 1]
    );
    let secured_store = function.body.blocks[3]
        .instructions
        .iter()
        .position(|instruction| matches!(instruction, MirInstruction::Store(_)))
        .unwrap();
    let cleanup_test = function.body.blocks[3]
        .instructions
        .iter()
        .position(|instruction| {
            matches!(
                instruction,
                MirInstruction::Assign(MirAssignment {
                    rvalue: MirRvalue {
                        kind: MirRvalueKind::PathCondition(_),
                        ..
                    },
                    ..
                })
            )
        })
        .unwrap();
    assert!(secured_store < cleanup_test);
}

#[test]
fn conditional_cleanup_dump_is_exact_and_deterministic() {
    let mir = fixture_conditional_cleanup_program();
    let dump = dump_mir(&mir);

    assert!(dump.contains("PathConditions\n"));
    assert!(
        dump.contains("end-full-expression cleanup f0:s2 as c0 cleanup f0:s1 as c0"),
        "{dump}"
    );
    assert!(dump.contains("f0:v4 = path-condition f0:p0 from f0:s0 : bool"));
    assert_eq!(dump, dump_mir(&mir));
}

#[test]
fn rejects_skipped_selected_lost_early_and_duplicate_cleanup() {
    let mut skipped = fixture_conditional_cleanup_program();
    let function = skipped
        .definitions
        .get_mut_for_test(skipped.entry_function)
        .unwrap();
    function.body.blocks[5].instructions = function.body.blocks[4].instructions.clone();
    assert!(errors_for(&skipped).contains("full-expression cleanup destination is not live"));

    let mut lost = fixture_conditional_cleanup_program();
    let function = lost
        .definitions
        .get_mut_for_test(lost.entry_function)
        .unwrap();
    function.body.blocks[4].instructions.clear();
    let errors = errors_for(&lost);
    assert!(
        errors.contains("full-expression temporaries must be cleaned"),
        "{errors}"
    );
    assert!(errors.contains("owning temporary remains live"), "{errors}");

    let mut early = fixture_conditional_cleanup_program();
    let function = early
        .definitions
        .get_mut_for_test(early.entry_function)
        .unwrap();
    function.body.blocks[4].instructions.insert(
        0,
        fixture_storage_dead(StorageId::new(function.function, 2), function.span),
    );
    let errors = errors_for(&early);
    assert!(errors.contains("full-expression cleanup destination is not live"));
    assert!(errors.contains("used outside a live lifetime epoch"));

    let mut duplicate = fixture_conditional_cleanup_program();
    let function = duplicate
        .definitions
        .get_mut_for_test(duplicate.entry_function)
        .unwrap();
    let cleanup = function.body.blocks[4].instructions[0].clone();
    function.body.blocks[4].instructions.push(cleanup);
    assert!(errors_for(&duplicate).contains("full-expression cleanup destination is not live"));
}

#[test]
fn rejects_conditional_cleanup_in_the_wrong_completion_order() {
    let mut mir = fixture_conditional_cleanup_program();
    let function = mir
        .definitions
        .get_mut_for_test(mir.entry_function)
        .unwrap();
    let MirInstruction::EndFullExpression(end) = &mut function.body.blocks[4].instructions[0]
    else {
        panic!("selected path must contain full-expression cleanup");
    };
    end.temporaries.swap(0, 1);

    assert!(errors_for(&mir)
        .contains("full-expression temporaries must be cleaned in reverse completion order"));
}

#[test]
fn rejects_storage_death_or_final_convergence_before_conditional_cleanup_finishes() {
    let mut early_activation_end = fixture_conditional_cleanup_program();
    let function = early_activation_end
        .definitions
        .get_mut_for_test(early_activation_end.entry_function)
        .unwrap();
    let activation = function.body.path_conditions[0].activation;
    function.body.blocks[6]
        .instructions
        .insert(0, fixture_storage_dead(activation, function.span));
    assert!(
        errors_for(&early_activation_end).contains("conditional storage lifetime state remains")
    );

    let mut incompatible_join = fixture_conditional_cleanup_program();
    let function = incompatible_join
        .definitions
        .get_mut_for_test(incompatible_join.entry_function)
        .unwrap();
    function.body.blocks[7].instructions.pop();
    let errors = errors_for(&incompatible_join);
    assert!(errors.contains("conditional storage lifetime state remains"));
}
