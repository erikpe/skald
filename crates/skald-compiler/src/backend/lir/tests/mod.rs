use super::*;
use crate::backend::plan::{
    test_fixtures::{facts, source},
    CheckedPlan, Component, ComponentRole, LayoutDisposition, LayoutFact, PlanError, ReturnShape,
    ScalarType,
};
use crate::primitive_comparison::PrimitiveComparisonPredicate as Predicate;

fn builder(plan: &CheckedPlan) -> DraftBuilder<'_> {
    DraftBuilder::new(plan.view().callable(source(0)).unwrap()).unwrap()
}
fn block<'p>(builder: &mut DraftBuilder<'p>) -> BlockHandle<'p> {
    let block = builder.reserve_block().unwrap();
    builder.define_block(block, &[]).unwrap();
    block
}
fn entry_block<'p>(builder: &mut DraftBuilder<'p>) -> BlockHandle<'p> {
    let entry = block(builder);
    builder.set_entry(entry).unwrap();
    entry
}
fn constant<'p>(
    builder: &mut DraftBuilder<'p>,
    block: BlockHandle<'p>,
    value: Constant,
) -> ValueHandle<'p> {
    builder.append(block, Operation::Constant(value)).unwrap()[0]
}
fn object(
    disposition: LayoutDisposition,
    size: usize,
    alignment: usize,
    role: ObjectRole,
    lifetime: LifetimeDisposition,
) -> Object {
    Object {
        layout: LayoutFact {
            size,
            alignment,
            disposition,
        },
        role,
        lifetime,
        origin: None,
    }
}
fn empty_edge<'p>(target: BlockHandle<'p>) -> Edge<ValueHandle<'p>, BlockHandle<'p>> {
    Edge {
        target,
        arguments: vec![],
    }
}
fn err<T>(result: Result<T, BuildError>) -> BuildError {
    result.err().expect("construction should fail")
}

mod drafts;
mod memory;
mod scalars;

mod calls;
mod release;
mod tracing;
