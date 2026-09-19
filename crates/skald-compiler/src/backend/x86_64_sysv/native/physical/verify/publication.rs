//! Immutable physical publication and exact snapshot receipts; no draft renderer.
use super::super::model::*;
use crate::backend::{
    plan::{ArtifactId, PlanError},
    selected::SelectedReceipt,
};
use std::{
    collections::BTreeSet,
    fmt::{self, Write},
    sync::Arc,
};
pub(in crate::backend) struct VerifiedPhysicalCallable<'a, 'f, 's, 'p> {
    draft: PhysicalDraft<'a, 'f, 's, 'p>,
    receipt: PhysicalReceipt<'p>,
}
#[derive(Clone)]
pub(in crate::backend) struct PhysicalReceipt<'p> {
    parent: SelectedReceipt<'p>,
    snapshot: Arc<()>,
    references: BTreeSet<ArtifactId>,
}
#[derive(Clone, Copy, Default)]
pub(in crate::backend) struct Inspection {
    pub placement: bool,
    pub frame: bool,
}
#[cfg_attr(not(test), allow(dead_code))]
impl<'a, 'f, 's, 'p> VerifiedPhysicalCallable<'a, 'f, 's, 'p> {
    pub(in crate::backend) fn receipt(&self) -> PhysicalReceipt<'p> {
        self.receipt.clone()
    }
    pub(in crate::backend) fn inspect(
        &self,
        out: &mut dyn Write,
        options: Inspection,
    ) -> fmt::Result {
        writeln!(
            out,
            "skald-lir schema=1 stage=physical status=verified callable={:?}",
            self.receipt.parent.key()
        )?;
        let plan = self
            .draft
            .frame
            .checked_placement()
            .selected()
            .draft()
            .context()
            .catalog()
            .plan();
        writeln!(
            out,
            "profile {:?} trace={:?} artifacts={:?}",
            plan.profile(),
            plan.runtime_trace(),
            plan.artifact_policy()
        )?;
        writeln!(out, "entry b{}", self.draft.entry.0)?;
        for block in &self.draft.blocks {
            writeln!(out, "block b{} origin={:?}", block.id.0, block.origin)?;
            for group in &block.groups {
                writeln!(
                    out,
                    "group {:?} dependencies={:?}",
                    group.origin, group.dependencies
                )?;
                for instruction in &group.instructions {
                    write!(out, "  ")?;
                    super::super::format::format_instruction(
                        out,
                        instruction,
                        |id| format!("{id:?}"),
                        |id| format!("b{}", id.0),
                    )?;
                    writeln!(out)?;
                }
            }
        }
        if options.placement {
            let p = self.draft.frame.checked_placement();
            for (assignment, location) in p.assignments() {
                writeln!(out, "assignment {assignment:?} {location:?}")?;
            }
            for point in p.transfer_points() {
                for (i, t) in p.transfers(point).iter().enumerate() {
                    writeln!(out, "transfer {point:?} {i} {t:?}")?;
                }
            }
        }
        if options.frame {
            writeln!(
                out,
                "frame bytes={} outgoing={} policy={:?}",
                self.draft.frame.bytes(),
                self.draft.frame.outgoing_bytes(),
                self.draft.frame.policy()
            )?;
            self.draft
                .frame
                .checked_placement()
                .selected()
                .visit::<fmt::Error>(|fact| {
                    if let crate::backend::selected::SelectedFact::Object { id, .. } = fact {
                        writeln!(out, "object {id:?} {:?}", self.draft.frame.object(id))?;
                    }
                    Ok(())
                })?;
            for (id, storage) in self.draft.frame.checked_placement().storages() {
                writeln!(
                    out,
                    "storage {} {storage:?} region={:?}",
                    id.index(),
                    self.draft.frame.storage(id)
                )?;
            }
        }
        Ok(())
    }
    /// Internal immutable visitor. Callbacks cannot change or acquire the draft.
    pub(in crate::backend) fn visit(&self, mut visitor: impl FnMut(PhysicalFact<'_>)) {
        visitor(PhysicalFact::Entry(self.draft.entry.0));
        for block in &self.draft.blocks {
            visitor(PhysicalFact::Block(block.id.0));
            for group in &block.groups {
                for instruction in &group.instructions {
                    visitor(PhysicalFact::Instruction(instruction));
                }
            }
        }
    }
    #[cfg(test)]
    pub(in crate::backend::x86_64_sysv::native::physical) fn body(
        &self,
    ) -> &PhysicalDraft<'a, 'f, 's, 'p> {
        &self.draft
    }
}
#[cfg_attr(not(test), allow(dead_code))]
pub(in crate::backend) enum PhysicalFact<'a> {
    Entry(usize),
    Block(usize),
    Instruction(&'a Instruction),
}
#[cfg_attr(not(test), allow(dead_code))]
impl<'p> PhysicalReceipt<'p> {
    pub(in crate::backend) fn parent(&self) -> &SelectedReceipt<'p> {
        &self.parent
    }
    pub(in crate::backend) fn references(&self) -> &BTreeSet<ArtifactId> {
        &self.references
    }
    pub(in crate::backend) fn same_snapshot(&self, other: &Self) -> bool {
        self.parent.same_snapshot(&other.parent) && Arc::ptr_eq(&self.snapshot, &other.snapshot)
    }
    pub(in crate::backend) fn matches(
        &self,
        product: &VerifiedPhysicalCallable<'_, '_, '_, 'p>,
    ) -> bool {
        self.same_snapshot(&product.receipt)
    }
    pub(in crate::backend) fn require_parent(
        &self,
        parent: &SelectedReceipt<'p>,
    ) -> Result<(), PlanError> {
        if self.parent.same_snapshot(parent) {
            Ok(())
        } else {
            Err(PlanError::WrongContext)
        }
    }
}
pub(super) fn publish<'a, 'f, 's, 'p>(
    draft: PhysicalDraft<'a, 'f, 's, 'p>,
    parent: SelectedReceipt<'p>,
) -> VerifiedPhysicalCallable<'a, 'f, 's, 'p> {
    let references = draft
        .blocks
        .iter()
        .flat_map(|b| &b.groups)
        .flat_map(|g| g.dependencies.iter().copied())
        .collect();
    VerifiedPhysicalCallable {
        draft,
        receipt: PhysicalReceipt {
            parent,
            snapshot: Arc::new(()),
            references,
        },
    }
}
