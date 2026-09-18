use super::*;
use crate::{
    backend::{
        effects::Effects,
        graph::GraphView,
        lir::{self, ProgramBuilder, TargetDeclarations},
        pilot::{admit, lower_next},
        plan::{self, LirCallableId},
        selected::{self, Bundle, Constraint, Payload, SelectedFact, TargetVerifier, Timing},
    },
    test_support::lower_source_to_complete_final_mir_with_sources,
};

fn for_sources(
    source: &str,
    mut check: impl for<'p> FnMut(&'p selected::SelectionContext<'p>, &lir::VerifiedCallable<'p>),
) {
    let fixture = lower_source_to_complete_final_mir_with_sources("native.ska", source);
    let admitted = admit(crate::backend::BackendInput::without_runtime_trace(
        &fixture.mir,
    ))
    .unwrap();
    let catalog = TargetDeclarations::new(admitted.plan().view())
        .freeze()
        .unwrap();
    let context = selection_context(&catalog).unwrap();
    let mut worklist = ProgramBuilder::new(admitted.plan().view());
    while worklist.next() != Some(LirCallableId::Entry) {
        let body = lower_next(&admitted, &mut worklist).unwrap().unwrap();
        check(&context, &body);
    }
}

mod calls;
mod editing;
mod malformed;
mod numeric;
mod oracle;
mod predicates;
mod selection;

mod frame;
mod placement;
