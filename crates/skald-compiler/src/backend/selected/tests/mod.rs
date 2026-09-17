use super::storage::ObjectRole;
use super::*;
use crate::backend::{
    effects::{Effect, Effects},
    graph::{self, GraphView},
    lir,
    plan::{
        self,
        test_fixtures::{facts, source},
        CheckedPlan,
    },
};
use std::{borrow::Cow, num::NonZeroU16};

fn repr() -> Representation {
    Representation::new(RepresentationKind::Bits, 64).unwrap()
}
fn catalog() -> (ResourceCatalog, ViewId, UnitId) {
    let mut r = ResourceCatalog::default();
    let bank: BankId = r.bank(BankKind::Integer);
    let unit = r.unit().unwrap();
    let view = r.view(bank, 64, &[unit], false).unwrap();
    (r, view, unit)
}
fn lower(plan: &CheckedPlan, key: plan::LirCallableId) -> lir::VerifiedCallable<'_> {
    let mut b = lir::DraftBuilder::new(plan.view().callable(key).unwrap()).unwrap();
    let entry = b.reserve_block().unwrap();
    b.define_block(entry, &[]).unwrap();
    b.set_entry(entry).unwrap();
    let constant = b
        .append(entry, lir::Operation::Constant(lir::Constant::I64(7)))
        .unwrap()[0];
    let parameter = b
        .reserve_value(plan::ScalarType::I64, Some(origin()))
        .unwrap();
    let exit = b.reserve_block().unwrap();
    b.define_block(exit, &[parameter]).unwrap();
    b.declare_object(lir::Object {
        layout: facts().layouts[0],
        role: lir::ObjectRole::SemanticStorage,
        lifetime: lir::LifetimeDisposition::WholeCallable,
        origin: Some(origin()),
    })
    .unwrap();
    b.terminate(
        entry,
        lir::Terminator::Jump(lir::Edge {
            target: exit,
            arguments: vec![constant],
        }),
    )
    .unwrap();
    b.terminate(exit, lir::Terminator::Return(vec![])).unwrap();
    lir::verify_callable(b.finish()).unwrap()
}

use payloads::{Opcode, Synthetic};
mod abi;
mod graphs;
mod payloads;
mod resources;

fn origin() -> crate::source::Span {
    let mut sources = crate::source::SourceDatabase::new();
    crate::source::Span::empty(sources.add("selected-origin.ska", ""), 7)
}
