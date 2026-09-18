//! Reconstruct trace records and transitions from concrete cells, independently
//! of the recipe producer. No opaque trace payload or trusted action annotation.
use super::super::{Instruction, Opcode, Site, ValueRef};
use crate::backend::{
    graph::{SelectedBlockId, SelectedObjectId},
    lir::{CallAttribution, TraceSite},
    plan::{ArtifactId, DataKey},
    selected::{ObjectRole, SelectedDraft, SelectedFact},
    RuntimeTracePolicy,
};
use std::collections::{BTreeMap, BTreeSet};
#[derive(Clone, Copy)]
enum Transition {
    Push(SelectedObjectId, ArtifactId),
    Pop(SelectedObjectId),
    Location(SelectedObjectId, ArtifactId),
}
fn same(a: &ValueRef, b: &ValueRef) -> bool {
    a.value == b.value && a.representation == b.representation
}
fn location(
    nodes: &[&Instruction],
    record: SelectedObjectId,
    base: &ValueRef,
) -> Option<ArtifactId> {
    match nodes {
        [constant, offset, symbol, store] => match (
            &constant.opcode,
            &offset.opcode,
            &symbol.opcode,
            &store.opcode,
        ) {
            (
                Opcode::Constant {
                    constant: crate::backend::lir::Constant::U64(8),
                    out: eight,
                },
                Opcode::ByteOffset {
                    base: actual,
                    offset,
                    out: address,
                },
                Opcode::SymbolAddress {
                    symbol: location @ ArtifactId::Data(DataKey::TraceLocation(_)),
                    out: value,
                },
                Opcode::TraceStore {
                    address: dest,
                    value: source,
                    record: Some(object),
                },
            ) if *object == record
                && same(base, actual)
                && same(eight, offset)
                && same(address, dest)
                && same(value, source) =>
            {
                Some(*location)
            }
            _ => None,
        },
        _ => None,
    }
}
fn transition(nodes: &[&Instruction]) -> Option<Transition> {
    let Opcode::ObjectAddress {
        object: record,
        out: address,
    } = &nodes.first()?.opcode
    else {
        return None;
    };
    match nodes {
        [_, tls, load, previous, rest @ ..] if rest.len() == 5 => {
            let (
                Opcode::TlsAddress { out: head },
                Opcode::TraceLoad {
                    address: source,
                    out: saved,
                    record: None,
                },
                Opcode::TraceStore {
                    address: dest,
                    value,
                    record: Some(object),
                },
                Opcode::TraceStore {
                    address: publish,
                    value: frame,
                    record: None,
                },
            ) = (&tls.opcode, &load.opcode, &previous.opcode, &rest[4].opcode)
            else {
                return None;
            };
            if *object == *record
                && same(head, source)
                && same(address, dest)
                && same(saved, value)
                && same(head, publish)
                && same(address, frame)
                && location(&rest[..4], *record, address).is_some()
            {
                Some(Transition::Push(
                    *record,
                    location(&rest[..4], *record, address)?,
                ))
            } else {
                None
            }
        }
        [_, load, tls, store] => {
            let (
                Opcode::TraceLoad {
                    address: source,
                    out: previous,
                    record: Some(object),
                },
                Opcode::TlsAddress { out: head },
                Opcode::TraceStore {
                    address: dest,
                    value,
                    record: None,
                },
            ) = (&load.opcode, &tls.opcode, &store.opcode)
            else {
                return None;
            };
            (*object == *record
                && same(address, source)
                && same(head, dest)
                && same(previous, value))
            .then_some(Transition::Pop(*record))
        }
        [_, rest @ ..] => {
            location(rest, *record, address).map(|location| Transition::Location(*record, location))
        }
        _ => None,
    }
}
pub(super) fn check(draft: &SelectedDraft<'_, Instruction>) -> Result<(), &'static str> {
    let mut blocks: BTreeMap<SelectedBlockId, Vec<&Instruction>> = BTreeMap::new();
    let mut edges = BTreeMap::new();
    let mut records = BTreeSet::new();
    let mut entry = None;
    let mut object_origins = BTreeMap::new();
    draft.visit(|fact| {
        match fact {
            SelectedFact::Entry { entry: e, .. } => entry = e,
            SelectedFact::Origins { objects, .. } => {
                object_origins.extend(objects.iter().map(|(a, b)| (*a, *b)))
            }
            SelectedFact::Object {
                id,
                layout,
                role: ObjectRole::Trace,
                ..
            } => {
                if layout.size != 16 || layout.alignment != 16 {
                    return Err("incorrect native trace record layout");
                }
                records.insert(id);
            }
            SelectedFact::Instruction { block, payload, .. } => {
                blocks.entry(block).or_default().push(payload);
            }
            SelectedFact::Terminal {
                block,
                payload: Some(payload),
                edges: successors,
            } => {
                blocks.entry(block).or_default().push(payload);
                edges.insert(
                    block,
                    successors.iter().map(|(to, _)| *to).collect::<Vec<_>>(),
                );
            }
            _ => {}
        }
        Ok(())
    })?;
    let trace = draft.lower_trace_plan();
    let expected_record = trace
        .and_then(|p| p.record)
        .and_then(|r| object_origins.get(&r).copied());
    if expected_record.into_iter().collect::<BTreeSet<_>>() != records {
        return Err("native trace records differ from frozen parent");
    }
    let mut transitions = BTreeMap::new();
    let mut used = BTreeSet::new();
    for (block, nodes) in &blocks {
        let mut actions = vec![];
        let mut start = 0;
        while start < nodes.len() {
            let site = nodes[start].origin.site;
            let end = (start + 1..nodes.len())
                .find(|i| nodes[*i].origin.site != site)
                .unwrap_or(nodes.len());
            let group = &nodes[start..end];
            if group.iter().any(|n| {
                matches!(
                    n.opcode,
                    Opcode::TlsAddress { .. }
                        | Opcode::TraceLoad { .. }
                        | Opcode::TraceStore { .. }
                )
            }) {
                if draft.context().catalog().plan().runtime_trace() != RuntimeTracePolicy::Enabled
                    || !matches!(site, Site::Instruction { .. })
                {
                    return Err("native trace cells outside enabled operation");
                }
                let action = transition(group).ok_or("malformed native trace memory sequence")?;
                let record = match action {
                    Transition::Push(r, _) | Transition::Pop(r) | Transition::Location(r, _) => r,
                };
                if !records.contains(&record) {
                    return Err("native trace access to non-trace record");
                }
                used.insert(record);
                match action {
                    Transition::Push(_, initial)
                        if trace.and_then(|p| p.initial_location) != Some(initial) =>
                    {
                        return Err("native trace initial location differs from frozen parent")
                    }
                    Transition::Location(_, location)
                        if !trace.is_some_and(|p| p.locations.contains(&location)) =>
                    {
                        return Err("native trace location outside frozen parent")
                    }
                    Transition::Push(_, _) if Some(*block) != entry || start != 0 => {
                        return Err("native trace push outside entry")
                    }
                    Transition::Pop(_)
                        if !nodes
                            .get(end)
                            .is_some_and(|n| matches!(n.opcode, Opcode::Return { .. })) =>
                    {
                        return Err("native trace pop not before return")
                    }
                    Transition::Location(_, location) => {
                        let attribution = match nodes.get(end).map(|n| &n.opcode) {
                            Some(Opcode::Call(call)) => Some(&call.attribution),
                            Some(Opcode::Failure { attribution, .. }) => Some(attribution),
                            _ => None,
                        };
                        if !matches!(attribution, Some(CallAttribution::SourceOperation { location: Some(actual), .. }) if *actual == location)
                        {
                            return Err("native trace location is not adjacent to attributed call");
                        }
                    }
                    _ => {}
                }
                actions.push(action);
            } else {
                for node in group {
                    let attribution = match &node.opcode {
                        Opcode::Call(call) => Some(&call.attribution),
                        Opcode::Failure { attribution, .. } => Some(attribution),
                        _ => None,
                    };
                    if expected_record.is_some()
                        && matches!(
                            attribution,
                            Some(CallAttribution::SourceOperation { location: None, .. })
                        )
                    {
                        return Err("native source call missing enabled location");
                    }
                    if let Some(CallAttribution::SourceOperation {
                        location: Some(location),
                        ..
                    }) = attribution
                    {
                        if !matches!(actions.last(), Some(Transition::Location(_, actual)) if actual == location)
                        {
                            return Err("native attributed call lacks trace location update");
                        }
                    }
                }
            }
            start = end;
        }
        transitions.insert(*block, actions);
    }
    if used != records {
        return Err("unused or missing native trace record");
    }
    // The pilot has one frame per eligible source body, none in generated entry.
    if records.len() > 1 {
        return Err("multiple native source trace frames");
    }
    let mut incoming = BTreeMap::new();
    let entry = entry.ok_or("missing native trace entry")?;
    incoming.insert(entry, None);
    let mut pending = vec![entry];
    while let Some(block) = pending.pop() {
        let mut active = incoming[&block];
        for action in &transitions[&block] {
            match *action {
                Transition::Push(record, _) if active.is_none() => active = Some(record),
                Transition::Pop(record) if active == Some(record) => active = None,
                Transition::Location(record, _) if active == Some(record) => {}
                _ => return Err("unbalanced native trace frame"),
            }
        }
        if blocks[&block]
            .last()
            .is_some_and(|n| matches!(n.opcode, Opcode::Return { .. }))
            && active.is_some()
        {
            return Err("native return leaks trace frame");
        }
        for next in &edges[&block] {
            if let Some(previous) = incoming.get(next) {
                if *previous != active {
                    return Err("inconsistent native trace frame at join");
                }
            } else {
                incoming.insert(*next, active);
                pending.push(*next);
            }
        }
    }
    // Keep all executable blocks subject to graph verification; this pass adds
    // path-sensitive frame obligations without inventing execution authority.
    Ok(())
}

impl super::Verifier {
    pub(in crate::backend::x86_64_sysv::native::selected) fn check_trace_attribution(
        &self,
        draft: &SelectedDraft<'_, Instruction>,
        node: &Instruction,
    ) -> Result<(), &'static str> {
        let attribution = match &node.opcode {
            Opcode::Call(call) => &call.attribution,
            Opcode::Failure { attribution, .. } => attribution,
            _ => return Ok(()),
        };
        let site = match node.origin.site {
            Site::Instruction { block, ordinal } => TraceSite::Instruction { block, ordinal },
            Site::Terminator(block) => TraceSite::Terminator(block),
            Site::Edge { .. } => return Err("native call has edge-only origin"),
        };
        let expected = draft.lower_trace_location(&site);
        let actual = match attribution {
            CallAttribution::SourceOperation { location, .. } => *location,
            _ => None,
        };
        if actual != expected {
            return Err("native call trace location differs from frozen parent site");
        }
        Ok(())
    }
}
