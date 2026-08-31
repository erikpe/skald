use super::*;

pub(super) fn lower_text(text: &str) -> MirProgram {
    lower_source_to_mir(text)
}

pub(super) fn assembly(text: &str) -> String {
    lower_source_to_assembly(text, Target::X86_64SysV).unwrap()
}

/// Emits verified final MIR without running target-independent optimizations.
///
/// Use this for instruction-selection tests that deliberately define isolated
/// callables which a whole-world pipeline would otherwise remove.
pub(super) fn complete_assembly(text: &str) -> String {
    let mir = lower_source_to_final_mir(text);
    crate::test_support::emit_assembly_without_runtime_trace(Target::X86_64SysV, &mir).unwrap()
}

pub(super) fn function_assembly<'assembly>(
    assembly: &'assembly str,
    symbol: &str,
) -> &'assembly str {
    let start = assembly
        .find(&format!(".type {symbol}, @function"))
        .expect("function symbol must be emitted");
    let remaining = &assembly[start..];
    let end = remaining
        .find(&format!(".size {symbol}, .-{symbol}"))
        .expect("function size must be emitted");
    &remaining[..end]
}

pub(super) fn test_span() -> crate::source::Span {
    let mut sources = SourceDatabase::new();
    let source = sources.add("backend-mir-test.ska", "");
    crate::source::Span::empty(source, 0)
}
