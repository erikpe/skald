//! Recompute address facts from definitions, never from supplied provenance.
use super::super::*;
use super::domains::constant;
use crate::backend::graph::LoweredValueId;
use crate::backend::plan::{ArtifactCategory, ArtifactId, DataKey, LayoutFact};
#[cfg_attr(not(test), allow(dead_code))]
pub(super) fn provenance(draft: &CallableDraft<'_>) -> Result<Vec<AddressProvenance>, ()> {
    let mut facts = vec![AddressProvenance::Unknown; draft.values.iter().len()];
    // Forward definitions and unreachable cycles are legal structural shapes.
    // Information grows from unknown; cycles without an address root stay unknown.
    loop {
        let mut changed = false;
        for (_, block) in draft.blocks.iter() {
            for instruction in &block.instructions {
                let fact = match &instruction.operation {
                    Operation::ObjectAddress(object) => AddressProvenance::Object {
                        object: *object,
                        offset: 0,
                    },
                    Operation::SymbolAddress {
                        symbol: ArtifactId::Data(DataKey::Static(field)),
                        ..
                    } => AddressProvenance::Static {
                        field: *field,
                        offset: 0,
                    },
                    Operation::Convert {
                        conversion: Conversion::Identity,
                        value,
                        ..
                    } => facts[value.index()],
                    Operation::ByteOffset { base, offset } => {
                        offset_fact(facts[base.index()], integer(draft, *offset))?
                    }
                    Operation::ScaledIndex {
                        base,
                        index,
                        stride,
                    } => {
                        let offset =
                            integer(draft, *index).map(|v| v.checked_mul(stride.bytes() as i128));
                        if matches!(offset, Some(None))
                            && facts[base.index()] != AddressProvenance::Unknown
                        {
                            return Err(());
                        }
                        offset_fact(facts[base.index()], offset.flatten())?
                    }
                    _ => AddressProvenance::Unknown,
                };
                for result in &instruction.results {
                    let target = &mut facts[result.index()];
                    if *target != fact {
                        *target = fact;
                        changed = true;
                    }
                }
            }
        }
        if !changed {
            return Ok(facts);
        }
        // Each known fact must be rooted in a unique immutable definition;
        // a cycle of known nonzero offsets cannot be justified by a root.
        // Graph ordering forbids such cycles in reachable code; unreachable
        // rootless cycles remain unknown with this least fixed point.
    }
}
#[cfg_attr(not(test), allow(dead_code))]
fn offset_fact(base: AddressProvenance, offset: Option<i128>) -> Result<AddressProvenance, ()> {
    let Some(offset) = offset else {
        return Ok(AddressProvenance::Unknown);
    };
    let derived = base.offset(offset);
    if base != AddressProvenance::Unknown && derived == AddressProvenance::Unknown {
        Err(())
    } else {
        Ok(derived)
    }
}
#[cfg_attr(not(test), allow(dead_code))]
fn integer(draft: &CallableDraft<'_>, value: LoweredValueId) -> Option<i128> {
    match constant(draft, value)? {
        Constant::I64(v) => Some(v as i128),
        Constant::U64(v) => Some(v as i128),
        Constant::U8(v) => Some(v as i128),
        _ => None,
    }
}
#[cfg_attr(not(test), allow(dead_code))]
pub(super) fn access(
    draft: &CallableDraft<'_>,
    fact: AddressProvenance,
    representation: MemoryRepresentation,
) -> Result<(), BuildError> {
    let view = draft.owner.context();
    representation.check(view.profile().data_layout.pointer_bytes)?;
    let layout: Option<(LayoutFact, usize)> = match fact {
        AddressProvenance::Object { object, offset } => {
            Some((draft.objects.get_id(object)?.layout, offset))
        }
        AddressProvenance::Static { field, offset } => {
            if !view.is_active_static(field) {
                return Err(BuildError::InvalidMemory);
            }
            let declaration = view.artifact(
                view.artifact_id(ArtifactId::Data(DataKey::Static(field)))?,
                ArtifactCategory::Data,
            )?;
            let layout = view.layout(
                view.layout_id(declaration.layout.ok_or(BuildError::InvalidMemory)?.index())?,
            )?;
            Some((*layout, offset))
        }
        AddressProvenance::Unknown => None,
    };
    if let Some((layout, offset)) = layout {
        layout.checked_access(offset, representation.bytes)?;
        if representation.alignment > layout.alignment || offset % representation.alignment != 0 {
            return Err(BuildError::InvalidMemory);
        }
    }
    Ok(())
}
