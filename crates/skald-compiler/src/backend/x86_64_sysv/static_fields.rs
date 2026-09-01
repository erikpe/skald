//! Deterministic writable storage planning for executable static fields.

use std::collections::BTreeSet;

use crate::{backend::BackendError, identity::StaticFieldId, mir::MirProgram};

use super::{
    artifacts,
    layout::DataLayout,
    machine::{AssemblyFunction, AssemblyStaticSlot},
    planning::{PlanningObserver, StaticPlanningPhase},
    symbol,
};

pub(super) fn plan(
    program: &MirProgram,
    active_fields: &[StaticFieldId],
    functions: &[AssemblyFunction],
    data_layout: &DataLayout,
    observer: &mut impl PlanningObserver,
) -> Result<Vec<AssemblyStaticSlot>, BackendError> {
    let referenced_symbols = functions
        .iter()
        .flat_map(|function| &function.instructions)
        .filter_map(artifacts::instruction_symbol)
        .collect::<BTreeSet<_>>();

    program
        .classes
        .iter()
        .flat_map(|class| &class.static_fields)
        .filter_map(|field| {
            observer.visits_static_field(StaticPlanningPhase::Declared, field.id);
            let active = active_fields.binary_search(&field.id).is_ok();
            if active {
                observer.visits_static_field(StaticPlanningPhase::Active, field.id);
            }
            let symbol = symbol::static_field(program, field.id);
            let conservative_fallback = !active && referenced_symbols.contains(symbol.as_str());
            if conservative_fallback {
                observer.visits_static_field(StaticPlanningPhase::ConservativeFallback, field.id);
            }
            (active || conservative_fallback).then_some((field, symbol))
        })
        .map(|(field, symbol)| {
            let layout = data_layout.ty(field.ty)?;
            Ok(AssemblyStaticSlot {
                field: field.id,
                symbol,
                size: layout.size(),
                alignment_power: u8::try_from(layout.alignment().trailing_zeros())
                    .expect("target alignment power must fit u8"),
            })
        })
        .collect()
}
