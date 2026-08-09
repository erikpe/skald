//! Inline shadow-frame maintenance sequences.

use super::super::{
    frame::{TraceFrameLayout, TraceFrameWord},
    machine::{Instruction, Operand, Register},
};

const TRACE_SCRATCH: Register = Register::R11;

pub(in crate::backend::x86_64_sysv) fn emit_push(
    frame: TraceFrameLayout,
    initial_location: &str,
    output: &mut Vec<Instruction>,
) {
    output.extend([
        Instruction::LoadRuntimeTraceTop {
            destination: TRACE_SCRATCH,
        },
        Instruction::Move {
            source: TRACE_SCRATCH.into(),
            destination: word_operand(frame.previous()),
        },
        Instruction::LoadRuntimeTraceLocationAddress {
            symbol: initial_location.to_owned(),
            destination: TRACE_SCRATCH,
        },
        Instruction::Move {
            source: TRACE_SCRATCH.into(),
            destination: word_operand(frame.location()),
        },
        Instruction::LoadEffectiveAddress {
            source: word_operand(frame.previous()),
            destination: TRACE_SCRATCH,
        },
        Instruction::StoreRuntimeTraceTop {
            source: TRACE_SCRATCH,
        },
    ]);
}

pub(in crate::backend::x86_64_sysv) fn emit_pop(
    frame: Option<TraceFrameLayout>,
    output: &mut Vec<Instruction>,
) {
    let Some(frame) = frame else {
        return;
    };
    output.extend([
        Instruction::Move {
            source: word_operand(frame.previous()),
            destination: TRACE_SCRATCH.into(),
        },
        Instruction::StoreRuntimeTraceTop {
            source: TRACE_SCRATCH,
        },
    ]);
}

pub(in crate::backend::x86_64_sysv) fn emit_location_replace(
    frame: TraceFrameLayout,
    location: &str,
    output: &mut Vec<Instruction>,
) {
    output.extend([
        Instruction::LoadRuntimeTraceLocationAddress {
            symbol: location.to_owned(),
            destination: TRACE_SCRATCH,
        },
        Instruction::Move {
            source: TRACE_SCRATCH.into(),
            destination: word_operand(frame.location()),
        },
    ]);
}

fn word_operand(word: TraceFrameWord) -> Operand {
    Operand::Memory {
        base: Register::Rbp,
        displacement: word.displacement(),
    }
}
