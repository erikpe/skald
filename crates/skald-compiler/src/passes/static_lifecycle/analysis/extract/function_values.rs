//! Deterministic address-taken inventory for indirect analysis expansion.

use std::collections::BTreeMap;

use crate::{
    identity::{CallableId, FunctionTypeId},
    mir::{MirDefinitionRef, MirInstruction, MirProgram, MirRvalueKind, MirStaticInitializerBody},
    source::Span,
};

use super::super::model::{span_key, StaticFunctionValueCandidates, StaticFunctionValueTarget};

pub(super) fn collect(
    program: &MirProgram,
    initializers: &[MirStaticInitializerBody],
) -> Vec<StaticFunctionValueCandidates> {
    let mut candidates = BTreeMap::<FunctionTypeId, BTreeMap<CallableId, Span>>::new();
    for definition in program
        .definitions
        .iter()
        .map(MirDefinitionRef::Function)
        .chain(
            program
                .member_definitions
                .iter()
                .map(MirDefinitionRef::Member),
        )
        .chain(initializers.iter().map(MirDefinitionRef::from))
    {
        for block in &definition.body().blocks {
            for instruction in &block.instructions {
                let MirInstruction::Assign(assignment) = instruction else {
                    continue;
                };
                let MirRvalueKind::CallableAddress(address) = assignment.rvalue.kind else {
                    continue;
                };
                let targets = candidates.entry(address.function_type).or_default();
                match targets.entry(address.target) {
                    std::collections::btree_map::Entry::Vacant(entry) => {
                        entry.insert(assignment.span);
                    }
                    std::collections::btree_map::Entry::Occupied(mut entry) => {
                        if span_key(assignment.span) < span_key(*entry.get()) {
                            entry.insert(assignment.span);
                        }
                    }
                }
            }
        }
    }

    candidates
        .into_iter()
        .map(|(function_type, targets)| StaticFunctionValueCandidates {
            function_type,
            targets: targets
                .into_iter()
                .map(
                    |(callable, first_reference_span)| StaticFunctionValueTarget {
                        callable,
                        first_reference_span,
                    },
                )
                .collect(),
        })
        .collect()
}
