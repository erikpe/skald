use super::{Gpr, NativeResources};
use crate::backend::{
    plan::{
        Abi, Architecture, ComponentRole, Convention, PlanError, PlanView, ReturnShape, ScalarType,
        SignatureId,
    },
    selected::{AbiArea, AbiBinding, AbiBindings, AbiLocation, Representation},
};

/// SysV scalar stack slots retain full word footprints, including byte arguments.
pub(super) const STACK_SLOT_BYTES: usize = 8;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::backend) enum CallArity {
    Fixed,
    Variadic,
}
#[derive(Debug, Eq, PartialEq)]
pub(in crate::backend) enum AbiError {
    Plan(PlanError),
    UnsupportedExternal,
    UnsupportedVariadic,
    StackSizeOverflow,
}
impl From<PlanError> for AbiError {
    fn from(error: PlanError) -> Self {
        Self::Plan(error)
    }
}

/// Entry and call bindings preserve logical signature order. Stack slot indices
/// are symbolic component locations, not byte offsets or frame positions.
pub(in crate::backend) struct ComponentAbi {
    entry: AbiBindings,
    call: AbiBindings,
    stack_slots: Vec<Representation>,
    outgoing_bytes: usize,
    noreturn: bool,
}
impl ComponentAbi {
    pub(in crate::backend) fn entry(&self) -> &AbiBindings {
        &self.entry
    }
    pub(in crate::backend) fn call(&self) -> &AbiBindings {
        &self.call
    }
    pub(in crate::backend) fn stack_slots(&self) -> &[Representation] {
        &self.stack_slots
    }
    pub(in crate::backend) fn outgoing_bytes(&self) -> usize {
        self.outgoing_bytes
    }
    pub(in crate::backend) fn noreturn(&self) -> bool {
        self.noreturn
    }
}
pub(in crate::backend) fn classify(
    plan: PlanView<'_>,
    signature: SignatureId,
    resources: &NativeResources,
    arity: CallArity,
) -> Result<ComponentAbi, AbiError> {
    if plan.profile().architecture != Architecture::X86_64 || plan.profile().abi != Abi::SysV {
        return Err(PlanError::WrongTarget.into());
    }
    if arity == CallArity::Variadic {
        return Err(AbiError::UnsupportedVariadic);
    }
    let signature = plan.signature(plan.signature_id(signature.index())?)?;
    if signature.convention == Convention::ExternC
        && (matches!(
            signature.returns,
            ReturnShape::Aggregate(_) | ReturnShape::Never
        ) || signature
            .inputs
            .iter()
            .any(|c| !matches!(c.role, ComponentRole::Parameter(_)) || !external_scalar(c.ty))
            || signature.results.iter().any(|c| !external_scalar(c.ty)))
    {
        return Err(AbiError::UnsupportedExternal);
    }
    let mut order: Vec<_> = signature.inputs.iter().enumerate().collect();
    order.sort_by_key(|(_, component)| role_order(component.role));
    let mut integer = 0;
    let mut float = 0;
    let mut stack_slots = vec![];
    let mut bindings = vec![None; order.len()];
    for (index, component) in order {
        let representation = representation(component.ty);
        let location = if component.ty == ScalarType::F64 && float < 8 {
            let view = resources
                .xmm(float, 64)
                .map_err(|_| PlanError::InvalidSignature)?;
            float += 1;
            AbiLocation::Fixed(view)
        } else if component.ty != ScalarType::F64 && integer < Gpr::ARGUMENTS.len() {
            let view = resources
                .gpr(Gpr::ARGUMENTS[integer], representation.bits())
                .map_err(|_| PlanError::InvalidSignature)?;
            integer += 1;
            AbiLocation::Fixed(view)
        } else {
            let slot = stack_slots.len();
            stack_slots.push(representation);
            AbiLocation::Slot {
                area: AbiArea::Outgoing,
                index: slot,
            }
        };
        bindings[index] = Some(AbiBinding {
            component: *component,
            representation,
            location,
        });
    }
    let call_inputs: Vec<_> = bindings
        .into_iter()
        .collect::<Option<_>>()
        .ok_or(PlanError::InvalidSignature)?;
    let entry_inputs = call_inputs
        .iter()
        .map(|binding| AbiBinding {
            location: match binding.location {
                AbiLocation::Slot { index, .. } => AbiLocation::Slot {
                    area: AbiArea::Incoming,
                    index,
                },
                fixed => fixed,
            },
            ..*binding
        })
        .collect();
    let results = signature
        .results
        .iter()
        .map(|component| {
            let representation = representation(component.ty);
            let view = if component.ty == ScalarType::F64 {
                resources.xmm(0, 64)
            } else {
                resources.gpr(Gpr::Rax, representation.bits())
            }
            .map_err(|_| PlanError::InvalidSignature)?;
            Ok(AbiBinding {
                component: *component,
                representation,
                location: AbiLocation::Fixed(view),
            })
        })
        .collect::<Result<Vec<_>, PlanError>>()?;
    let bytes = outgoing_bytes(stack_slots.len())?;
    Ok(ComponentAbi {
        entry: AbiBindings::new(
            signature,
            64,
            resources.catalog(),
            entry_inputs,
            results.clone(),
        )?,
        call: AbiBindings::new(signature, 64, resources.catalog(), call_inputs, results)?,
        stack_slots,
        outgoing_bytes: bytes,
        noreturn: signature.returns == ReturnShape::Never,
    })
}
/// Canonical slot footprints and area padding; never inferred from scalar widths.
pub(super) fn area_layout(
    slots: usize,
    area: AbiArea,
) -> Result<crate::backend::selected::AbiAreaLayout, AbiError> {
    use crate::backend::selected::{AbiAreaLayout, AbiSlotLayout};
    let bytes = if area == AbiArea::Outgoing {
        outgoing_bytes(slots)?
    } else {
        slots
            .checked_mul(STACK_SLOT_BYTES)
            .filter(|n| *n <= i32::MAX as usize)
            .ok_or(AbiError::StackSizeOverflow)?
    };
    Ok(AbiAreaLayout {
        slots: (0..slots)
            .map(|i| AbiSlotLayout {
                offset: i * STACK_SLOT_BYTES,
                bytes: STACK_SLOT_BYTES,
                alignment: STACK_SLOT_BYTES,
            })
            .collect(),
        bytes,
        alignment: if area == AbiArea::Outgoing {
            16
        } else {
            STACK_SLOT_BYTES
        },
    })
}
fn external_scalar(ty: ScalarType) -> bool {
    matches!(
        ty,
        ScalarType::I64 | ScalarType::U64 | ScalarType::U8 | ScalarType::Bool | ScalarType::F64
    )
}
fn representation(ty: ScalarType) -> Representation {
    Representation::from_scalar(ty, 64).expect("native scalar representations have nonzero widths")
}
fn role_order(role: ComponentRole) -> (u8, usize, u8) {
    match role {
        ComponentRole::ResultDestination(_) => (0, 0, 0),
        ComponentRole::ReceiverStatic => (1, 0, 0),
        ComponentRole::ReceiverComplete => (1, 0, 1),
        ComponentRole::ReceiverMetadata => (1, 0, 2),
        ComponentRole::Parameter(i)
        | ComponentRole::AggregateAddress { parameter: i, .. }
        | ComponentRole::AliasAddress(i)
        | ComponentRole::RuntimeParameter(i) => (2, i, 0),
        ComponentRole::AliasComplete(i) => (2, i, 1),
        ComponentRole::AliasMetadata(i) => (2, i, 2),
        ComponentRole::Result => (3, 0, 0), // checked signatures disallow result roles in inputs
    }
}

fn outgoing_bytes(slots: usize) -> Result<usize, AbiError> {
    slots
        .checked_mul(STACK_SLOT_BYTES)
        .and_then(|bytes| bytes.checked_add(15))
        .map(|bytes| bytes & !15)
        .filter(|bytes| *bytes <= i32::MAX as usize)
        .ok_or(AbiError::StackSizeOverflow)
}
#[cfg(test)]
mod tests {
    #[test]
    fn explicit_area_layout_preserves_word_footprints_and_rejects_overflow() {
        use super::*;
        let incoming = area_layout(3, AbiArea::Incoming).unwrap();
        let outgoing = area_layout(3, AbiArea::Outgoing).unwrap();
        assert_eq!((incoming.bytes, incoming.alignment), (24, 8));
        assert_eq!((outgoing.bytes, outgoing.alignment), (32, 16));
        assert_eq!(incoming.slots, outgoing.slots);
        assert_eq!(
            area_layout(usize::MAX, AbiArea::Incoming).err(),
            Some(AbiError::StackSizeOverflow)
        );
        assert_eq!(
            area_layout(usize::MAX, AbiArea::Outgoing).err(),
            Some(AbiError::StackSizeOverflow)
        );
    }
    use super::*;
    #[test]
    fn outgoing_area_limits_include_alignment_and_arithmetic_overflow() {
        assert_eq!(outgoing_bytes(0), Ok(0));
        assert_eq!(outgoing_bytes(1), Ok(16));
        assert_eq!(outgoing_bytes(2), Ok(16));
        let maximum = (i32::MAX as usize & !15) / 8;
        assert_eq!(outgoing_bytes(maximum), Ok(maximum * 8));
        assert_eq!(
            outgoing_bytes(maximum + 1),
            Err(AbiError::StackSizeOverflow)
        );
        assert_eq!(outgoing_bytes(usize::MAX), Err(AbiError::StackSizeOverflow));
    }
}
