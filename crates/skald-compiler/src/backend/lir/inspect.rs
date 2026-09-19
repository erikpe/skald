//! Immutable visits and canonical text from the lowered owner.
use super::{
    Block, CallableDraft, Constant, Object, Operation, Value, VerifiedCallable, VerifiedProgram,
};
use crate::backend::graph::{LoweredBlockId, LoweredObjectId, LoweredValueId};
use std::fmt::{self, Write};

pub(in crate::backend) enum LoweredFact<'a> {
    Value(LoweredValueId, &'a Value),
    Object(LoweredObjectId, &'a Object),
    Block(LoweredBlockId, &'a Block),
}

impl VerifiedCallable<'_> {
    /// Borrowed records cannot modify, certify or outlive this snapshot.
    #[cfg_attr(not(test), allow(dead_code))]
    pub(in crate::backend) fn visit<E>(
        &self,
        visitor: impl FnMut(LoweredFact<'_>) -> Result<(), E>,
    ) -> Result<(), E> {
        visit(self.draft(), visitor)
    }
    pub(in crate::backend) fn dump(&self, out: &mut dyn Write) -> fmt::Result {
        render(self.draft(), "verified", out)?;
        writeln!(out, "references {:?}", self.receipt().references())
    }
}

fn visit<E>(
    draft: &CallableDraft<'_>,
    mut visitor: impl FnMut(LoweredFact<'_>) -> Result<(), E>,
) -> Result<(), E> {
    for (id, value) in draft.values() {
        visitor(LoweredFact::Value(id, value))?;
    }
    for (id, object) in draft.objects() {
        visitor(LoweredFact::Object(id, object))?;
    }
    for (id, block) in draft.blocks() {
        visitor(LoweredFact::Block(id, block))?;
    }
    Ok(())
}

#[cfg_attr(not(test), allow(dead_code))]
impl CallableDraft<'_> {
    /// Diagnostic-only rendering: does not verify or traverse referenced IDs.
    pub(in crate::backend) fn dump_draft(&self, out: &mut dyn Write) -> fmt::Result {
        render(self, "unverified-draft", out)
    }
}

fn render(draft: &CallableDraft<'_>, status: &str, out: &mut dyn Write) -> fmt::Result {
    crate::backend::inspection::header(out, "lowered", status, draft.owner())?;
    match draft.entry() {
        Some(entry) => writeln!(out, "entry {entry:?}")?,
        None => writeln!(out, "entry <unresolved>")?,
    }
    writeln!(
        out,
        "inputs {:?}\ntrace {:?}",
        draft.inputs(),
        draft.trace_plan()
    )?;
    visit(draft, |fact| match fact {
        LoweredFact::Value(id, value) => {
            let bits = match value.ty {
                crate::backend::plan::ScalarType::I64
                | crate::backend::plan::ScalarType::U64
                | crate::backend::plan::ScalarType::F64 => 64,
                crate::backend::plan::ScalarType::U8 | crate::backend::plan::ScalarType::Bool => 8,
                crate::backend::plan::ScalarType::DataAddress
                | crate::backend::plan::ScalarType::CodeAddress(_) => {
                    draft.owner().context().profile().data_layout.pointer_bytes * 8
                }
            };
            writeln!(out, "value {id:?} bits={bits} {value:?}")?;
            if value.definition.is_none() {
                writeln!(out, "  definition <unresolved>")?;
            }
            Ok(())
        }
        LoweredFact::Object(id, object) => writeln!(out, "object {id:?} {object:?}"),
        LoweredFact::Block(id, block) => {
            match &block.parameters {
                Some(parameters) => writeln!(out, "block {id:?} parameters={parameters:?}")?,
                None => writeln!(out, "block {id:?} parameters=<unresolved>")?,
            }
            for (ordinal, instruction) in block.instructions.iter().enumerate() {
                write!(
                    out,
                    "  instruction {ordinal} results={:?} ",
                    instruction.results
                )?;
                match instruction.operation {
                    Operation::Constant(Constant::F64(bits)) => {
                        write!(out, "Constant(F64 bits=0x{bits:016x})")?
                    }
                    _ => write!(out, "{:?}", instruction.operation)?,
                }
                writeln!(out, " effects={:?}", instruction.effects)?;
            }
            match &block.terminator {
                Some(terminal) => {
                    writeln!(
                        out,
                        "  terminal {terminal:?} effects={:?}",
                        block.terminal_effects
                    )?;
                    for (edge, transfer) in terminal.edges(id) {
                        writeln!(
                            out,
                            "  edge slot={} target={:?} arguments={:?}",
                            edge.slot, transfer.target, transfer.arguments
                        )?;
                    }
                }
                None => writeln!(out, "  terminal <unresolved>")?,
            }
            Ok(())
        }
    })
}

impl VerifiedProgram<'_> {
    /// Receipts describe closure without retaining or reconstructing released bodies.
    pub(in crate::backend) fn dump_inventory(&self, out: &mut dyn Write) -> fmt::Result {
        writeln!(
            out,
            "skald-lir schema=1 stage=lowered-inventory status=verified"
        )?;
        crate::backend::inspection::declarations(out, self.parent())?;
        for (key, receipt) in self.receipts() {
            writeln!(
                out,
                "completed {key:?} references={:?}",
                receipt.references()
            )?;
        }
        for data in self.data() {
            writeln!(out, "data {data:?}")?;
        }
        Ok(())
    }
}
