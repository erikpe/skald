use std::cell::Cell;

use crate::{
    backend::{emit_assembly, BackendInput, Target},
    identity::CallableId,
    mir::{
        BlockId, MirAssignment, MirBasicBlock, MirInstruction, MirPlace, MirRvalueKind,
        MirTerminator, ValueId,
    },
    test_support::{lower_source_to_final_mir, lower_source_to_mir},
};

use super::{
    rewrite::{
        rewrite_verified_final_mir, run_transforming_mir_pipeline, MirTransformPipelineError,
    },
    *,
};
use crate::mir::rewrite::{BlockPlacement, MirLocalIdentity, MirReferenceFailure, MirRewriteError};

fn lowered_program() -> MirProgram {
    lower_source_to_mir("fn main() -> i64 { return 0; }")
}

#[test]
fn empty_pipeline_preserves_valid_mir_and_reports_only_verification() {
    let mir = lowered_program();
    let expected = mir.clone();
    let measured = run_mir_pipeline_measured(mir);

    assert_eq!(measured.result.unwrap().program(), &expected);
    assert_eq!(measured.statistics.verification_executions(), 1);
    assert_eq!(measured.statistics.pass_executions(), 0);
    assert_eq!(measured.statistics.rewritten_callables(), 0);
    assert_eq!(
        measured.statistics.rewrite_changes(),
        MirRewriteChangeSummary::default()
    );
}

#[test]
fn pipeline_preserves_logical_path_and_cleanup_metadata() {
    let mir = lower_source_to_mir(
        "class Flag {
           truth: bool;
           init(truth: bool) { self.truth = truth; }
           fn read() -> bool { return self.truth; }
           destroy {}
         }
         fn make(truth: bool) -> shared Flag { return new Flag(truth); }
         fn evaluate(left: bool) -> bool {
           return left && make(true)->read();
         }
         fn main() -> i64 { return 0; }",
    );
    assert!(mir
        .definitions
        .iter()
        .any(|definition| !definition.body.path_conditions.is_empty()));
    let expected = mir.clone();

    assert_eq!(run_mir_pipeline(mir).unwrap().program(), &expected);
}

#[test]
fn pipeline_preserves_valid_multi_block_mir() {
    let mut mir = lowered_program();
    let function = mir
        .definitions
        .get_mut_for_test(mir.entry_function)
        .unwrap();
    let span = function.span;
    let second = BlockId::new(function.function, 1);
    function.body.blocks.push(MirBasicBlock {
        id: second,
        instructions: Vec::new(),
        terminator: Some(MirTerminator::Goto {
            target: second,
            span,
        }),
        span,
    });
    let expected = mir.clone();

    assert_eq!(run_mir_pipeline(mir).unwrap().program(), &expected);
}

#[test]
fn pipeline_preserves_pure_and_checked_primitive_casts_exactly() {
    let mir = lower_source_to_mir(
        "fn source() -> f64 { return 7.9; }
         fn main() -> i64 { return (i64) source() + (i64) (f64) 1u; }",
    );
    let expected = mir.clone();

    assert_eq!(run_mir_pipeline(mir).unwrap().program(), &expected);
}

#[test]
fn rejected_mir_still_reports_the_verification_execution() {
    let mut mir = lowered_program();
    mir.definitions
        .get_mut_for_test(mir.entry_function)
        .unwrap()
        .body
        .blocks[0]
        .terminator = None;

    let measured = run_mir_pipeline_measured(mir);
    assert!(measured
        .result
        .unwrap_err()
        .to_string()
        .contains("block has no terminator"));
    assert_eq!(measured.statistics.verification_executions(), 1);
    assert_eq!(measured.statistics.pass_executions(), 0);
}

#[test]
fn transformation_never_runs_on_unverified_input() {
    let mut mir = lowered_program();
    mir.definitions
        .get_mut_for_test(mir.entry_function)
        .unwrap()
        .body
        .blocks[0]
        .terminator = None;
    let executed = Cell::new(false);

    let measured = run_transforming_mir_pipeline(mir, |_callable, _edit| {
        executed.set(true);
        Ok(())
    });

    assert!(!executed.get());
    assert!(matches!(
        measured.result,
        Err(MirTransformPipelineError::InputVerification(_))
    ));
    assert!(measured.callables.is_none());
    assert_eq!(measured.statistics.verification_executions(), 1);
    assert_eq!(measured.statistics.pass_executions(), 0);
}

#[test]
fn seal_invalidation_preserves_raw_commit_maps_and_change_summaries() {
    let mir = lower_source_to_final_mir("fn main() -> i64 { return 1 + 1; }");
    let verified = verify_final_mir(mir).unwrap();
    let removed = Cell::new(None);

    let rewritten = rewrite_verified_final_mir(verified, |_callable, edit| {
        let constants = constant_values(edit);
        let (block, replacement) = constants[0];
        let deleted = constants[1].1;
        removed.set(Some(deleted));
        delete_equivalent_value(edit, block, replacement, deleted)
    })
    .expect("pipeline-owned invalidation returns dense raw MIR");

    let report = &rewritten.callables[0];
    assert!(matches!(
        report.maps.values.committed(removed.get().unwrap()),
        Err(MirRewriteError::DeletedIdentity {
            identity: MirLocalIdentity::Value(_)
        })
    ));
    assert_eq!(report.changes.values.removed, 1);
    verify_final_mir(rewritten.program).expect("raw rewrite result can only reseal centrally");
}

#[test]
fn valid_value_deletion_reseals_and_reports_truthful_changes() {
    let mir = lower_source_to_final_mir("fn main() -> i64 { return 1 + 1; }");
    let measured = run_transforming_mir_pipeline(mir, |_callable, edit| {
        let constants = constant_values(edit);
        delete_equivalent_value(edit, constants[0].0, constants[0].1, constants[1].1)
    });

    let verified = measured
        .result
        .expect("dominance-preserving deletion must reseal");
    emit_assembly(
        Target::X86_64SysV,
        BackendInput::without_runtime_trace(&verified),
    )
    .expect("backend accepts the resealed result");
    assert_eq!(measured.statistics.verification_executions(), 2);
    assert_eq!(measured.statistics.pass_executions(), 1);
    assert_eq!(measured.statistics.rewritten_callables(), 1);
    assert_eq!(measured.statistics.rewrite_changes().values.removed, 1);
    assert_eq!(measured.callables.unwrap()[0].changes.values.removed, 1);
}

#[test]
fn valid_cfg_rewrite_reseals_and_counts_the_inserted_block() {
    let mir = lower_source_to_final_mir(
        "fn main() -> i64 {
           var count: i64 = 0;
           while (count < 2) { count = count + 1; }
           return count;
         }",
    );
    let measured = run_transforming_mir_pipeline(mir, |_callable, edit| {
        let target = edit
            .block_order()
            .iter()
            .find_map(|block| {
                edit.block(*block)
                    .ok()?
                    .terminator
                    .as_ref()?
                    .successors()
                    .next()
            })
            .expect("loop contains an executable edge");
        let span = edit.block(target)?.span;
        let forwarding = edit.allocate_block(BlockPlacement::Before(target), |identity| {
            empty_block(identity, span)
        })?;
        assert!(edit.redirect_edges(target, forwarding)? > 0);
        edit.rewrite_block_terminator(forwarding, |_| Some(MirTerminator::Goto { target, span }))?;
        Ok(())
    });

    measured.result.expect("forwarding block must reseal");
    assert_eq!(measured.statistics.rewrite_changes().blocks.inserted, 1);
}

#[test]
fn structured_rewrite_failure_is_not_mistaken_for_verification() {
    let measured = run_transforming_mir_pipeline(lowered_program(), |_callable, edit| {
        edit.remove_block(edit.entry())?;
        Ok(())
    });

    assert!(matches!(
        measured.result,
        Err(MirTransformPipelineError::Rewrite(
            MirRewriteError::InvalidReference {
                failure: MirReferenceFailure::Deleted,
                ..
            }
        ))
    ));
    assert!(measured.callables.is_none());
    assert_eq!(measured.statistics.verification_executions(), 1);
    assert_eq!(measured.statistics.pass_executions(), 1);
    assert_eq!(measured.statistics.rewritten_callables(), 0);
}

#[test]
fn semantically_invalid_commit_fails_output_resealing() {
    let mir = lower_source_to_final_mir("fn main() -> i64 { return 1 + 1; }");
    let measured = run_transforming_mir_pipeline(mir, |_callable, edit| {
        let constants = constant_values(edit);
        let block = constants[0].0;
        let deleted = constants[0].1;
        let later_definition = edit
            .block(block)?
            .instructions
            .iter()
            .filter_map(|instruction| match instruction {
                MirInstruction::Assign(assignment) => Some(assignment.result),
                _ => None,
            })
            .next_back()
            .expect("binary result follows constants");
        delete_equivalent_value(edit, block, later_definition, deleted)
    });

    assert!(matches!(
        measured.result,
        Err(MirTransformPipelineError::OutputVerification(_))
    ));
    assert!(measured.callables.is_some());
    assert_eq!(measured.statistics.verification_executions(), 2);
    assert_eq!(measured.statistics.pass_executions(), 1);
}

#[test]
fn lifecycle_effect_change_rechecks_immutable_baseline_authority() {
    let mir = lower_source_to_final_mir(
        "fn read() -> i64 { return State.base; }
         class State {
           static base: i64 = 1;
           static other: i64 = 2;
           static result: i64 = read();
           init() {}
         }
         fn main() -> i64 { return 0; }",
    );
    let read = mir
        .declarations
        .iter()
        .find(|declaration| declaration.name == "read")
        .unwrap()
        .id;
    let other = mir
        .classes
        .iter()
        .flat_map(|class| &class.static_fields)
        .find(|field| field.name == "other")
        .unwrap()
        .id;
    let baseline = mir
        .static_lifecycle
        .as_ref()
        .unwrap()
        .lifecycle()
        .proof()
        .authority()
        .clone();

    let measured = run_transforming_mir_pipeline(mir, |callable, edit| {
        if callable != CallableId::Function(read) {
            return Ok(());
        }
        for block in edit.block_order().to_vec() {
            edit.rewrite_block_instructions(block, |instructions| {
                instructions
                    .iter()
                    .cloned()
                    .map(|instruction| retarget_static_load(instruction, other))
                    .collect()
            })?;
        }
        Ok(())
    });

    let Err(MirTransformPipelineError::OutputVerification(errors)) = measured.result else {
        panic!("new lifecycle fact must fail output realization verification")
    };
    assert!(errors.to_string().contains("unauthorized fact"));
    let rewritten = measured.callables.expect("structural rewrite committed");
    assert!(!rewritten.is_empty());
    // The editor exposes no lifecycle authority; the only authority observed
    // by output verification is the immutable one carried through raw MIR.
    assert!(!baseline.roots().collect::<Vec<_>>().is_empty());
}

fn constant_values(edit: &crate::mir::rewrite::MirCallableEdit) -> Vec<(BlockId, ValueId)> {
    edit.block_order()
        .iter()
        .flat_map(|block| {
            edit.block(*block)
                .expect("block order contains live blocks")
                .instructions
                .iter()
                .filter_map(move |instruction| match instruction {
                    MirInstruction::Assign(assignment)
                        if assignment.rvalue.kind == MirRvalueKind::ConstantI64(1) =>
                    {
                        Some((*block, assignment.result))
                    }
                    _ => None,
                })
        })
        .collect()
}

fn delete_equivalent_value(
    edit: &mut crate::mir::rewrite::MirCallableEdit,
    block: BlockId,
    replacement: ValueId,
    deleted: ValueId,
) -> Result<(), MirRewriteError> {
    edit.replace_value_uses(deleted, replacement)?;
    edit.rewrite_block_instructions(block, |instructions| {
        instructions
            .iter()
            .filter(|instruction| {
                !matches!(instruction, MirInstruction::Assign(assignment) if assignment.result == deleted)
            })
            .cloned()
            .collect()
    })?;
    edit.remove_value(deleted)?;
    Ok(())
}

fn empty_block(identity: BlockId, span: crate::source::Span) -> MirBasicBlock {
    MirBasicBlock {
        id: identity,
        instructions: Vec::new(),
        terminator: None,
        span,
    }
}

fn retarget_static_load(
    instruction: MirInstruction,
    target: crate::identity::StaticFieldId,
) -> MirInstruction {
    match instruction {
        MirInstruction::Assign(MirAssignment {
            result,
            mut rvalue,
            span,
        }) if matches!(rvalue.kind, MirRvalueKind::Load(_)) => {
            rvalue.kind = MirRvalueKind::Load(MirPlace::static_field(target));
            MirInstruction::Assign(MirAssignment {
                result,
                rvalue,
                span,
            })
        }
        instruction => instruction,
    }
}
