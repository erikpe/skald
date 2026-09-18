use super::super::{Instruction, Opcode, Site};
use super::Verifier;
use crate::backend::{
    effects::MemoryRegion,
    graph::{SelectedObjectId, SelectedValueId},
    lir::Constant,
    plan::ArtifactId,
    selected::{RepresentationKind, SelectedDraft, SelectedFact},
};
use std::collections::BTreeMap;
impl Verifier {
    pub(super) fn check_callable(
        &self,
        draft: &SelectedDraft<'_, Instruction>,
    ) -> Result<(), &'static str> {
        let context = draft.context();
        let owner = context
            .binding(crate::backend::graph::GraphView::identity(draft).callable)
            .map_err(|_| "unknown native callable")?;
        let expected = super::super::super::classify(
            context.catalog().plan(),
            owner.signature_id(),
            &self.resources,
            super::super::super::CallArity::Fixed,
        )
        .map_err(|_| "unsupported native entry signature")?;
        let actual = draft.abi().ok_or("missing native entry ABI")?;
        if actual.inputs() != expected.entry().inputs()
            || actual.results() != expected.entry().results()
        {
            return Err("noncanonical native entry ABI");
        }
        // Shared layout validation proves shape/bounds, not canonical SysV stride.
        draft.visit(|fact| {
            if let SelectedFact::Resources { abi_areas, .. } = fact {
                for (&signature, areas) in abi_areas {
                    let classified = super::super::super::classify(
                        context.catalog().plan(),
                        signature,
                        &self.resources,
                        super::super::super::CallArity::Fixed,
                    )
                    .map_err(|_| "unsupported native ABI area signature")?;
                    if areas.incoming != classified.stack_slots()
                        || areas.outgoing != classified.stack_slots()
                        || !areas.results.is_empty()
                    {
                        return Err("noncanonical native ABI area shape");
                    }
                    for area in [
                        crate::backend::selected::AbiArea::Incoming,
                        crate::backend::selected::AbiArea::Outgoing,
                        crate::backend::selected::AbiArea::Results,
                    ] {
                        let expected =
                            super::super::super::abi::area_layout(areas.slots(area).len(), area)
                                .map_err(|_| "native ABI area overflow")?;
                        if context.abi_layout(signature, area) != Some(&expected) {
                            return Err("noncanonical native ABI area layout");
                        }
                    }
                }
            }
            Ok(())
        })?;
        let mut objects = BTreeMap::new();
        let mut concrete_definitions = BTreeMap::new();
        let mut failures = vec![];
        let mut definitions = vec![];
        let mut accesses = vec![];
        let mut origins = std::collections::BTreeSet::new();
        let mut sites = vec![];
        let mut parameters = BTreeMap::new();
        let mut edges = vec![];
        let mut resource_count = 0;
        let mut bank_count = 0;
        draft.visit(|fact| {
            match fact {
                SelectedFact::Resources { units, .. }
                    if units != self.resources.catalog().units() =>
                {
                    return Err("incorrect native unit catalog")
                }
                SelectedFact::Bank(id, kind) => {
                    if !self
                        .resources
                        .catalog()
                        .banks()
                        .any(|(known, k)| known == id && k == kind)
                    {
                        return Err("incorrect native register bank");
                    }
                    bank_count += 1;
                }
                SelectedFact::Origins { blocks, .. } => origins.extend(blocks.keys().copied()),
                SelectedFact::Object { id, layout, .. } => {
                    objects.insert(id, layout);
                }
                SelectedFact::Block {
                    id, parameters: p, ..
                } => {
                    parameters.insert(id, p.to_vec());
                }
                SelectedFact::Resource(id, actual) => {
                    let expected = self
                        .resources
                        .catalog()
                        .views()
                        .find(|(known, _)| *known == id)
                        .ok_or("foreign native resource")?
                        .1;
                    if actual.bank != expected.bank
                        || actual.bits != expected.bits
                        || actual.units != expected.units
                        || actual.reserved != expected.reserved
                    {
                        return Err("incorrect native register footprint");
                    }
                    resource_count += 1;
                }
                SelectedFact::Instruction { payload, .. }
                | SelectedFact::Terminal {
                    payload: Some(payload),
                    ..
                } => {
                    self.check_trace_attribution(draft, payload)?;
                    sites.push(payload.origin.site);
                    for (value, definition) in payload.operands() {
                        if definition {
                            concrete_definitions.insert(value.value, &payload.opcode);
                        }
                    }
                    if let Opcode::Failure {
                        reason,
                        signature,
                        arguments,
                        bindings,
                        ..
                    } = &payload.opcode
                    {
                        let reporter = context
                            .catalog()
                            .plan()
                            .artifact(
                                context
                                    .catalog()
                                    .plan()
                                    .artifact_id(ArtifactId::Runtime(
                                        crate::backend::plan::RuntimeService::Panic,
                                    ))
                                    .map_err(|_| "missing native failure reporter")?,
                                ArtifactId::Runtime(crate::backend::plan::RuntimeService::Panic)
                                    .category(),
                            )
                            .map_err(|_| "missing native failure reporter")?;
                        if reporter.signature != Some(*signature) {
                            return Err("noncanonical native failure signature");
                        }
                        let call = super::super::super::classify(
                            context.catalog().plan(),
                            *signature,
                            &self.resources,
                            super::super::super::CallArity::Fixed,
                        )
                        .map_err(|_| "unsupported native failure signature")?;
                        if bindings != call.call().inputs() || !call.noreturn() {
                            return Err("noncanonical native failure ABI");
                        }
                        failures.push((*reason, arguments));
                    }
                    match &payload.opcode {
                        Opcode::Call(_) => self.check_call(context, payload)?,
                        Opcode::TlsAddress { .. }
                            if context.catalog().plan().runtime_trace()
                                == crate::backend::RuntimeTracePolicy::Omitted =>
                        {
                            return Err("native TLS in omitted trace mode")
                        }
                        Opcode::TraceLoad {
                            address, record, ..
                        }
                        | Opcode::TraceStore {
                            address, record, ..
                        } => {
                            accesses.push((
                                address.value,
                                8,
                                8,
                                record.map_or(MemoryRegion::Unknown, MemoryRegion::Object),
                            ));
                        }

                        Opcode::Return { bindings, .. }
                            if bindings != expected.entry().results() =>
                        {
                            return Err("noncanonical native return ABI")
                        }
                        Opcode::SymbolAddress { symbol, out } => {
                            if let RepresentationKind::CodeAddress(signature) =
                                out.representation.kind
                            {
                                let actual = match symbol {
                                    ArtifactId::Callable(key) => Some(
                                        context
                                            .catalog()
                                            .selection_binding(*key)
                                            .map_err(|_| "unknown native callable symbol")?
                                            .signature_id(),
                                    ),
                                    _ => {
                                        context
                                            .catalog()
                                            .plan()
                                            .artifact(
                                                context
                                                    .catalog()
                                                    .plan()
                                                    .artifact_id(*symbol)
                                                    .map_err(|_| "unknown native symbol")?,
                                                symbol.category(),
                                            )
                                            .map_err(|_| "unknown native symbol")?
                                            .signature
                                    }
                                };
                                if actual != Some(signature) {
                                    return Err("native code address signature mismatch");
                                }
                            }
                        }

                        Opcode::Constant { constant, out } => definitions.push((
                            out.value,
                            AddressDefinition::Constant(match constant {
                                Constant::I64(v) => u64::try_from(*v).ok(),
                                Constant::U64(v) => Some(*v),
                                Constant::U8(v) => Some(u64::from(*v)),
                                _ => None,
                            }),
                        )),
                        Opcode::ObjectAddress { object, out } => {
                            definitions.push((out.value, AddressDefinition::Object(*object)))
                        }
                        Opcode::ByteOffset { base, offset, out } => definitions.push((
                            out.value,
                            AddressDefinition::Offset(base.value, offset.value),
                        )),
                        Opcode::Load {
                            address,
                            bytes,
                            alignment,
                            region,
                            ..
                        }
                        | Opcode::Store {
                            address,
                            bytes,
                            alignment,
                            region,
                            ..
                        } => accesses.push((address.value, *bytes, *alignment, *region)),
                        _ => {}
                    }
                }
                _ => {}
            }
            if let SelectedFact::Terminal {
                block, edges: e, ..
            } = fact
            {
                if e.len() > 1 && e.iter().any(|(_, arguments)| !arguments.is_empty()) {
                    return Err("parameter transfers require forwarding blocks");
                }
                edges.extend(e.iter().map(|(to, args)| (block, *to, args.clone())));
            }
            Ok(())
        })?;
        if resource_count != self.resources.catalog().views().len()
            || bank_count != self.resources.catalog().banks().len()
        {
            return Err("incomplete native resource catalog");
        }
        for site in sites {
            match site {
                Site::Instruction { block, .. }
                | Site::Terminator(block)
                | Site::Edge { block, .. }
                    if !origins.contains(&block) =>
                {
                    return Err("missing lower recipe provenance")
                }
                _ => {}
            }
        }
        let mut constants = BTreeMap::new();
        let mut addresses: BTreeMap<SelectedValueId, (SelectedObjectId, usize)> = BTreeMap::new();
        for (out, definition) in &definitions {
            if let AddressDefinition::Constant(Some(value)) = definition {
                constants.insert(*out, *value);
            }
        }
        loop {
            let before = addresses.len();
            for (out, definition) in &definitions {
                let address = match definition {
                    AddressDefinition::Object(object) => Some((*object, 0)),
                    AddressDefinition::Offset(base, offset) => {
                        addresses.get(base).and_then(|(object, base)| {
                            constants
                                .get(offset)
                                .and_then(|offset| usize::try_from(*offset).ok())
                                .and_then(|offset| base.checked_add(offset))
                                .map(|offset| (*object, offset))
                        })
                    }
                    _ => None,
                };
                if let Some(address) = address {
                    addresses.insert(*out, address);
                }
            }
            for (block, params) in &parameters {
                let incoming = edges
                    .iter()
                    .filter(|(_, to, _)| to == block)
                    .collect::<Vec<_>>();
                if incoming.is_empty() {
                    continue;
                }
                for (ordinal, param) in params.iter().enumerate() {
                    let candidates = incoming
                        .iter()
                        .map(|(_, _, args)| {
                            args.get(ordinal)
                                .and_then(|arg| addresses.get(arg))
                                .copied()
                        })
                        .collect::<Vec<_>>();
                    if let Some(Some(first)) = candidates.first() {
                        if candidates.iter().all(|c| *c == Some(*first)) {
                            addresses.insert(*param, *first);
                        }
                    }
                }
            }
            if addresses.len() == before {
                break;
            }
        }
        for (value, bytes, alignment, region) in accesses {
            if matches!(region, MemoryRegion::Static(_)) {
                return Err("native static memory provenance requires a static recipe");
            }
            if let MemoryRegion::Object(object) = region {
                let (actual, offset) = addresses
                    .get(&value)
                    .ok_or("unproven native object memory access")?;
                let layout = objects.get(&object).ok_or("unknown native memory object")?;
                if alignment == 0
                    || *actual != object
                    || layout.alignment < alignment
                    || offset % alignment != 0
                    || offset
                        .checked_add(bytes)
                        .is_none_or(|end| end > layout.size)
                {
                    return Err("native object memory access is out of bounds or misaligned");
                }
            }
        }
        for (reason, arguments) in failures {
            let [address, length] = arguments.as_slice() else {
                return Err("incorrect native failure argument count");
            };
            if !matches!(concrete_definitions.get(&address.value),
                Some(Opcode::SymbolAddress { symbol: ArtifactId::Data(crate::backend::plan::DataKey::FailureMessage(actual)), .. }) if *actual == reason)
                || !matches!(concrete_definitions.get(&length.value),
                    Some(Opcode::Constant {constant: Constant::U64(actual), ..}) if *actual == reason.bytes().len() as u64)
            {
                return Err("native failure message association mismatch");
            }
        }
        super::numeric::graph(draft)?;
        super::tracing::check(draft)?;
        Ok(())
    }
}
enum AddressDefinition {
    Constant(Option<u64>),
    Object(SelectedObjectId),
    Offset(SelectedValueId, SelectedValueId),
}
