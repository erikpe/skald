use super::CompilerStderrView;

#[test]
fn absent_empty_and_unmatched_prefixes_borrow_raw_bytes() {
    let raw = b"error: unchanged";
    for prefix in [None, Some(&b""[..]), Some(&b"/other/"[..])] {
        let view = CompilerStderrView::from_raw(raw, prefix);
        assert_eq!(view, CompilerStderrView::Raw);
        assert_eq!(view.bytes(raw), raw);
    }
}

#[test]
fn every_non_overlapping_path_occurrence_is_removed() {
    let raw = b"/case//case/file.ska\nerror: mention /case/inside message\n";
    let view = CompilerStderrView::from_raw(raw, Some(b"/case/"));
    assert_eq!(
        view.bytes(raw),
        b"file.ska\nerror: mention inside message\n"
    );
}

#[test]
fn normalization_preserves_arbitrary_surrounding_bytes() {
    let raw = b"\xff/case/a\0/case/b\xfe";
    let view = CompilerStderrView::from_raw(raw, Some(b"/case/"));
    assert_eq!(view.bytes(raw), b"\xffa\0b\xfe");
    assert_eq!(raw, b"\xff/case/a\0/case/b\xfe");
}
