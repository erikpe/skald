//! Target-owned opcode spelling and immutable shared structural inspection.
use super::{Payload, SelectedDraft, VerifiedSelectedCallable, VerifiedSelectedProgram};
use crate::backend::{
    graph::{
        DefinitionSite, LoweredBlockId, LoweredObjectId, LoweredValueId, SelectedBlockId,
        SelectedObjectId, SelectedValueId,
    },
    plan::LayoutFact,
};
use crate::source::Span;
use std::fmt::{self, Write};

/// Required target formatting covers opcode distinctions and immediates absent
/// from `describe`. Use deterministic scalar data, never Debug of a whole owner.
pub(in crate::backend) trait InspectPayload: Payload {
    fn fmt_opcode(&self, out: &mut dyn Write) -> fmt::Result;
}

pub(in crate::backend) enum SelectedFact<'a, P> {
    Entry {
        entry: Option<SelectedBlockId>,
        inputs: &'a [SelectedValueId],
        abi: Option<&'a super::AbiBindings>,
    },
    Resources {
        units: usize,
        abi_areas:
            &'a std::collections::BTreeMap<crate::backend::plan::SignatureId, super::AbiAreas>,
    },
    Bank(super::BankId, super::BankKind),
    Resource(super::ViewId, &'a super::ResourceView),
    Origins {
        values: &'a std::collections::BTreeMap<LoweredValueId, SelectedValueId>,
        blocks: &'a std::collections::BTreeMap<LoweredBlockId, SelectedBlockId>,
        objects: &'a std::collections::BTreeMap<LoweredObjectId, SelectedObjectId>,
    },

    Value {
        id: SelectedValueId,
        representation: super::Representation,
        definition: Option<DefinitionSite>,
        origin: Option<Span>,
    },
    Object {
        id: SelectedObjectId,
        layout: LayoutFact,
        role: super::ObjectRole,
        origin: Option<Span>,
    },
    Block {
        id: SelectedBlockId,
        parameters: &'a [SelectedValueId],
        origin: Option<Span>,
    },
    Instruction {
        block: SelectedBlockId,
        ordinal: usize,
        payload: &'a P,
    },
    Terminal {
        block: SelectedBlockId,
        payload: Option<&'a P>,
        edges: &'a [(SelectedBlockId, Vec<SelectedValueId>)],
    },
}

impl<P> SelectedDraft<'_, P> {
    pub(in crate::backend) fn visit<'a, E>(
        &'a self,
        visitor: impl FnMut(SelectedFact<'a, P>) -> Result<(), E>,
    ) -> Result<(), E> {
        visit(self, visitor)
    }
}

impl<P> VerifiedSelectedCallable<'_, P> {
    pub(in crate::backend) fn visit<'a, E>(
        &'a self,
        visitor: impl FnMut(SelectedFact<'a, P>) -> Result<(), E>,
    ) -> Result<(), E> {
        visit(self.draft(), visitor)
    }
}
impl<P: InspectPayload> VerifiedSelectedCallable<'_, P> {
    pub(in crate::backend) fn dump(&self, out: &mut dyn Write) -> fmt::Result {
        render(self.draft(), "verified", out)?;
        writeln!(out, "references {:?}", self.receipt().references())
    }
}
#[cfg_attr(not(test), allow(dead_code))]
impl<P: InspectPayload> SelectedDraft<'_, P> {
    /// Never follows potentially invalid references or claims verification.
    pub(in crate::backend) fn dump_draft(&self, out: &mut dyn Write) -> fmt::Result {
        render(self, "unverified-draft", out)
    }
}
fn visit<'a, P, E>(
    draft: &'a SelectedDraft<'_, P>,
    mut visitor: impl FnMut(SelectedFact<'a, P>) -> Result<(), E>,
) -> Result<(), E> {
    visitor(SelectedFact::Entry {
        entry: draft.entry,
        inputs: &draft.inputs,
        abi: draft.abi(),
    })?;
    let resources = &draft.context.resources;
    visitor(SelectedFact::Resources {
        units: resources.units(),
        abi_areas: &draft.context.abi_areas,
    })?;
    for (id, bank) in resources.banks() {
        visitor(SelectedFact::Bank(id, bank))?;
    }
    for (id, view) in resources.views() {
        visitor(SelectedFact::Resource(id, view))?;
    }
    visitor(SelectedFact::Origins {
        values: &draft.origins,
        blocks: &draft.block_origins,
        objects: &draft.object_origins,
    })?;
    for (id, value) in draft.values.iter() {
        visitor(SelectedFact::Value {
            id,
            representation: value.ty,
            definition: value.definition,
            origin: value.origin,
        })?;
    }
    for (id, object) in draft.objects.iter() {
        visitor(SelectedFact::Object {
            id,
            layout: object.layout,
            role: object.role,
            origin: object.origin,
        })?;
    }
    for (id, block) in draft.blocks.iter() {
        visitor(SelectedFact::Block {
            id,
            parameters: &block.parameters,
            origin: block.origin,
        })?;
        for (ordinal, payload) in block.instructions.iter().enumerate() {
            visitor(SelectedFact::Instruction {
                block: id,
                ordinal,
                payload,
            })?;
        }
        visitor(SelectedFact::Terminal {
            block: id,
            payload: block.terminal.as_ref().map(|t| &t.payload),
            edges: block.terminal.as_ref().map_or(&[], |t| t.edges.as_slice()),
        })?;
    }
    Ok(())
}
fn render<P: InspectPayload>(
    draft: &SelectedDraft<'_, P>,
    status: &str,
    out: &mut dyn Write,
) -> fmt::Result {
    crate::backend::inspection::header(out, "selected", status, draft.owner)?;
    match draft.input.as_ref() {
        Some(input) => writeln!(
            out,
            "lower-input {:?} references={:?}",
            input.owner().key(),
            input.references()
        )?,
        None => writeln!(out, "lower-input <none>")?,
    }
    for declaration in draft.context.catalog.declarations() {
        writeln!(out, "target-artifact {declaration:?}")?;
    }
    visit(draft, |fact| match fact {
        SelectedFact::Entry { entry, inputs, abi } => {
            match entry {
                Some(entry) => writeln!(out, "entry {entry:?}")?,
                None => writeln!(out, "entry <unresolved>")?,
            }
            writeln!(out, "inputs {inputs:?}")?;
            match abi {
                Some(abi) => writeln!(
                    out,
                    "abi inputs={:?} results={:?}",
                    abi.inputs(),
                    abi.results()
                ),
                None => writeln!(out, "abi <unresolved>"),
            }
        }
        SelectedFact::Resources { units, abi_areas } => {
            writeln!(out, "abi-areas {abi_areas:?}\nunits {units}")
        }
        SelectedFact::Bank(id, bank) => writeln!(out, "bank {id:?} {bank:?}"),
        SelectedFact::Resource(id, view) => writeln!(out, "resource {id:?} {view:?}"),
        SelectedFact::Origins {
            values,
            blocks,
            objects,
        } => writeln!(
            out,
            "value-origins {values:?}\nblock-origins {blocks:?}\nobject-origins {objects:?}"
        ),

        SelectedFact::Value {
            id,
            representation,
            definition,
            origin,
        } => writeln!(
            out,
            "value {id:?} {representation:?} definition={definition:?} origin={origin:?}"
        ),
        SelectedFact::Object {
            id,
            layout,
            role,
            origin,
        } => writeln!(
            out,
            "object {id:?} {layout:?} role={role:?} origin={origin:?}"
        ),
        SelectedFact::Block {
            id,
            parameters,
            origin,
        } => writeln!(
            out,
            "block {id:?} parameters={parameters:?} origin={origin:?}"
        ),
        SelectedFact::Instruction {
            block,
            ordinal,
            payload,
        } => {
            write!(out, "  instruction {block:?}/{ordinal} ")?;
            describe(payload, out)
        }
        SelectedFact::Terminal {
            block,
            payload,
            edges,
        } => {
            match payload {
                Some(payload) => {
                    write!(out, "  terminal ")?;
                    describe(payload, out)?;
                }
                None => writeln!(out, "  terminal <unresolved>")?,
            }
            for (slot, (target, arguments)) in edges.iter().enumerate() {
                writeln!(
                    out,
                    "  edge {block:?} slot={slot} target={target:?} arguments={arguments:?}"
                )?;
            }
            Ok(())
        }
    })
}
fn describe<P: InspectPayload>(payload: &P, out: &mut dyn Write) -> fmt::Result {
    payload.fmt_opcode(out)?;
    let d = payload.describe();
    writeln!(
        out,
        " flow={:?} successors={} signature={:?} attribution={:?} indirect-target={:?}",
        d.flow, d.successors, d.call_signature, d.call_attribution, d.indirect_target
    )?;
    for (slot, operand) in d.operands.iter().enumerate() {
        writeln!(out, "    operand {slot} {operand:?}")?;
    }
    writeln!(
        out,
        "    ties={:?} clobbers={:?} bundle={:?}",
        d.ties, d.clobbers, d.bundle
    )?;
    for event in d.events() {
        writeln!(out, "    event {event:?}")?;
    }
    writeln!(
        out,
        "    effects={:?} artifacts={:?} objects={:?}",
        d.effects, d.artifacts, d.objects
    )?;
    writeln!(
        out,
        "    abi-inputs={:?} abi-results={:?}",
        d.abi_inputs, d.abi_results
    )
}
impl VerifiedSelectedProgram<'_> {
    pub(in crate::backend) fn dump_inventory(&self, out: &mut dyn Write) -> fmt::Result {
        writeln!(
            out,
            "skald-lir schema=1 stage=selected-inventory status=verified"
        )?;
        let ctx = self.context();
        crate::backend::inspection::declarations(out, ctx.catalog.plan())?;
        for declaration in ctx.catalog.declarations() {
            writeln!(out, "target-artifact {declaration:?}")?;
        }
        for data in self.parent().data() {
            writeln!(out, "data {data:?}")?;
        }
        for data in ctx.catalog.data() {
            writeln!(out, "target-data {data:?}")?;
        }
        for receipt in self.receipts() {
            let key = receipt.key();
            writeln!(
                out,
                "completed {key:?} lower-input={:?} references={:?}",
                receipt.input().map(|i| i.owner().key()),
                receipt.references()
            )?;
        }
        Ok(())
    }
}
