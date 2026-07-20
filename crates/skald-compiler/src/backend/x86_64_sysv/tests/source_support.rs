use super::*;

pub(super) fn lower_text(text: &str) -> MirProgram {
    lower_source_to_mir(text)
}

pub(super) fn assembly(text: &str) -> String {
    lower_source_to_assembly(text, Target::X86_64SysV).unwrap()
}

pub(super) fn test_span() -> crate::source::Span {
    let mut sources = SourceDatabase::new();
    let source = sources.add("backend-mir-test.ska", "");
    crate::source::Span::empty(source, 0)
}
