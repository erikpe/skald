//! Sole physical acceptance owner. No call to realization or assembly parsing.
mod authority;
mod derivation;
mod encoding;
mod publication;
mod recipes;
mod state;
use super::{super::selected::Instruction as SelectedInstruction, model::*};
use crate::backend::{
    frame::FramePlan, placement::CheckedPlacement, plan::LirCallableId,
    selected::VerifiedSelectedCallable,
};
#[cfg_attr(not(test), allow(unused_imports))]
pub(in crate::backend) use publication::{
    Inspection, PhysicalFact, PhysicalReceipt, VerifiedPhysicalCallable,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::backend) enum Reason {
    Provenance,
    Topology,
    Encoding,
    Recipe,
    Dependency,
    StackJoin,
    StackState,
    Alignment,
    Preservation,
    Capacity,
}
#[derive(Debug, Eq, PartialEq)]
pub(in crate::backend) struct PhysicalError {
    pub callable: LirCallableId,
    pub target: crate::backend::Target,
    pub source_origin: Option<crate::source::Span>,
    pub block: Option<usize>,
    pub group: Option<usize>,
    pub instruction: Option<usize>,
    pub reason: Reason,
}
impl PhysicalError {
    fn new(
        callable: LirCallableId,
        block: Option<usize>,
        group: Option<usize>,
        instruction: Option<usize>,
        reason: Reason,
    ) -> Self {
        Self {
            callable,
            target: crate::backend::Target::X86_64SysV,
            source_origin: None,
            block,
            group,
            instruction,
            reason,
        }
    }
}
#[cfg_attr(not(test), allow(dead_code))]
pub(in crate::backend) fn check_native_physical<'a, 'f, 's, 'p>(
    draft: PhysicalDraft<'a, 'f, 's, 'p>,
    selected: &VerifiedSelectedCallable<'p, SelectedInstruction>,
    placement: &'f CheckedPlacement<'s, 'p, SelectedInstruction>,
    frame: &'a FramePlan<'f, 's, 'p, SelectedInstruction>,
) -> Result<VerifiedPhysicalCallable<'a, 'f, 's, 'p>, PhysicalError> {
    let key = selected.receipt().key();
    let failure = |reason| PhysicalError::new(key, None, None, None, reason);
    if !std::ptr::eq(draft.frame, frame)
        || frame.require_placement(placement).is_err()
        || placement.require_selected(selected).is_err()
    {
        return Err(failure(Reason::Provenance));
    }
    for (b, block) in draft.blocks.iter().enumerate() {
        if block.id.0 != b {
            return Err(failure(Reason::Topology));
        }
        for (g, group) in block.groups.iter().enumerate() {
            for (i, instruction) in group.instructions.iter().enumerate() {
                encoding::check(instruction, draft.blocks.len()).map_err(|reason| {
                    annotate(
                        PhysicalError::new(key, Some(b), Some(g), Some(i), reason),
                        &draft,
                        selected,
                    )
                })?;
                let artifact = match instruction {
                    Instruction::RipAddress { artifact, .. }
                    | Instruction::TlsOffset { artifact, .. }
                    | Instruction::Call(CallTarget::Direct(artifact)) => Some(*artifact),
                    _ => None,
                };
                if let Some(artifact) = artifact {
                    if !group.dependencies.contains(&artifact) {
                        return Err(annotate(
                            PhysicalError::new(key, Some(b), Some(g), Some(i), Reason::Dependency),
                            &draft,
                            selected,
                        ));
                    }
                }
            }
            for artifact in &group.dependencies {
                selected
                    .draft()
                    .context()
                    .catalog()
                    .artifact(*artifact, artifact.category())
                    .map_err(|_| {
                        annotate(
                            PhysicalError::new(key, Some(b), Some(g), None, Reason::Dependency),
                            &draft,
                            selected,
                        )
                    })?;
            }
        }
    }
    state::check(&draft, key).map_err(|error| annotate(error, &draft, selected))?;
    let mut position = (None, None);
    derivation::check(&draft, selected, placement, frame, &mut position).map_err(|reason| {
        annotate(
            PhysicalError::new(key, position.0, position.1, None, reason),
            &draft,
            selected,
        )
    })?;
    Ok(publication::publish(draft, selected.receipt()))
}

// Attribution reads immutable selected origins only; generated frame actions
// without a selected source site deliberately have no fabricated source span.
fn annotate(
    mut error: PhysicalError,
    draft: &PhysicalDraft<'_, '_, '_, '_>,
    selected: &VerifiedSelectedCallable<'_, SelectedInstruction>,
) -> PhysicalError {
    use crate::backend::{
        placement::{Site, TransferPoint},
        selected::SelectedFact,
    };
    let origin = error
        .block
        .zip(error.group)
        .and_then(|(b, g)| draft.blocks.get(b).and_then(|b| b.groups.get(g)))
        .map(|g| g.origin);
    let site = match origin {
        Some(Origin::Selected(site) | Origin::Epilogue(site)) => Some(site),
        Some(Origin::Transfer {
            point: TransferPoint::Before(site) | TransferPoint::After(site),
            ..
        }) => Some(site),
        _ => None,
    };
    if let Some(site) = site {
        let _ = selected.visit::<()>(|fact| {
            match fact {
                SelectedFact::Instruction {
                    block,
                    ordinal,
                    payload,
                } if site == Site::Instruction { block, ordinal } => {
                    error.source_origin = payload.origin().span
                }
                SelectedFact::Terminal {
                    block,
                    payload: Some(payload),
                    ..
                } if site == Site::Terminal(block) => error.source_origin = payload.origin().span,
                _ => {}
            }
            Ok(())
        });
    }
    error
}

#[cfg(test)]
mod tests;
