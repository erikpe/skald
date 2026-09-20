//! Rendering from checked typed products. No text is parsed back into authority.
use super::super::{format, verify::VerifiedPhysicalCallable};
use crate::backend::{
    lir::{DataDefinition, DataInitializer},
    plan::{ArtifactId, LirCallableId},
};
use std::fmt::{self, Write};

pub(super) fn callable(
    body: &VerifiedPhysicalCallable<'_, '_, '_, '_>,
    symbol: &impl Fn(ArtifactId) -> String,
) -> Result<String, fmt::Error> {
    let draft = body.draft();
    let owner = body.receipt().parent().key();
    let name = symbol(ArtifactId::Callable(owner));
    let mut out = String::new();
    if owner == LirCallableId::Entry {
        writeln!(out, ".globl {name}")?;
    }
    writeln!(out, ".type {name},@function")?;
    writeln!(out, "{name}:")?;
    writeln!(out, "jmp .Lnative_{}_{}", ordinal(owner), draft.entry.0)?;
    for block in &draft.blocks {
        writeln!(out, ".Lnative_{}_{}:", ordinal(owner), block.id.0)?;
        for group in &block.groups {
            for instruction in &group.instructions {
                format::format_instruction(&mut out, instruction, symbol, |id| {
                    format!(".Lnative_{}_{}", ordinal(owner), id.0)
                })?;
                out.push('\n');
            }
        }
    }
    writeln!(out, ".size {name}, .-{name}")?;
    Ok(out)
}

pub(super) fn data(
    out: &mut String,
    definition: &DataDefinition,
    symbol: &impl Fn(ArtifactId) -> String,
) -> fmt::Result {
    let id = ArtifactId::Data(definition.key);
    let name = symbol(id);
    writeln!(out, ".p2align 3\n{name}:")?;
    for initializer in &definition.initializers {
        match initializer {
            DataInitializer::Bytes(bytes) => {
                for chunk in bytes.chunks(24) {
                    write!(out, ".byte ")?;
                    for (index, byte) in chunk.iter().enumerate() {
                        if index != 0 {
                            out.push(',');
                        }
                        write!(out, "{byte}")?;
                    }
                    out.push('\n');
                }
            }
            DataInitializer::Zero(0) => {}
            DataInitializer::Zero(bytes) => writeln!(out, ".zero {bytes}")?,
            DataInitializer::Address { target, addend, .. } => {
                writeln!(out, ".quad {}{:+}", symbol(*target), addend)?;
            }
        }
    }
    Ok(())
}

pub(super) fn data_section(definition: &DataDefinition) -> &'static str {
    if matches!(definition.key, crate::backend::plan::DataKey::Static(_)) {
        return ".section .bss\n";
    }
    if definition
        .initializers
        .iter()
        .any(|initializer| matches!(initializer, DataInitializer::Address { .. }))
    {
        ".section .data.rel.ro.local,\"aw\",@progbits\n"
    } else {
        ".section .rodata\n"
    }
}

fn ordinal(key: LirCallableId) -> String {
    format!("{:?}", key)
        .bytes()
        .map(|b| format!("{b:02x}"))
        .collect()
}
