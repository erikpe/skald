use super::*;
use crate::backend::{
    graph::EditError,
    lir::*,
    plan::{
        test_fixtures::{facts, source},
        CheckedPlan,
    },
};
#[test]
fn private_definition_corruption_fails_in_consuming_finish() {
    let p = CheckedPlan::check(facts()).unwrap();
    let mut b = DraftBuilder::new(p.view().callable(source(0)).unwrap()).unwrap();
    let entry = b.reserve_block().unwrap();
    b.define_block(entry, &[]).unwrap();
    b.set_entry(entry).unwrap();
    b.append(entry, Operation::Constant(Constant::I64(1)))
        .unwrap();
    b.terminate(entry, Terminator::Return(vec![])).unwrap();
    let mut edit = verify_callable(b.finish()).unwrap().into_editor();
    let block = edit.draft.blocks.get_mut(entry).unwrap();
    block.instructions.push(block.instructions[0].clone());
    assert!(matches!(
        edit.finish(),
        Err(LoweredEditFailure::Edit(EditError::DefinitionConflict))
    ));
}

#[test]
fn published_callable_program_and_analysis_authority_cannot_be_cloned() {
    let _: fn(VerifiedCallable<'static>) -> LoweredEditor<'static> = VerifiedCallable::into_editor;
    let _: fn(
        VerifiedProgram<'static>,
        VerifiedCallable<'static>,
    ) -> Result<(ProgramBuilder<'static>, LoweredEditor<'static>), ProgramError> =
        VerifiedProgram::edit;
    // Type inference becomes ambiguous if any of these actual API types gains Clone.
    trait AmbiguousIfClone<A> {
        fn check() {}
    }
    impl<T> AmbiguousIfClone<()> for T {}
    impl<T: Clone> AmbiguousIfClone<u8> for T {}
    let _ = <VerifiedCallable<'static> as AmbiguousIfClone<_>>::check;
    let _ = <VerifiedProgram<'static> as AmbiguousIfClone<_>>::check;
    let _=<crate::backend::graph::GraphSession<'static,CallableDraft<'static>> as AmbiguousIfClone<_>>::check;
}
