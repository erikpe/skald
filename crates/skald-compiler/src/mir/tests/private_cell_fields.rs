use super::*;
use crate::source::Span;

const SOURCE: &str = concat!(
    "class Cache {\n",
    "  private cell value: i64;\n",
    "  init(value: i64) { self.value = value; }\n",
    "  fn read() -> i64 { return self.value; }\n",
    "}\n",
    "fn main() -> i64 { var cache: Cache = Cache(7); return cache.read(); }\n",
);

#[test]
fn lowers_exact_cell_modifier_evidence_and_dumps_it() {
    let program = lower_text(SOURCE);
    verify_mir(&program).unwrap();
    let field = &program.class(ClassId::new(0)).unwrap().fields[0];
    let cell_span = field.cell_span.expect("cell field must retain its span");
    assert!(!cell_span.range().is_empty());

    let dump = dump_mir(&program);
    assert!(
        dump.contains("Field c0:field0 cell \"value\" : i64"),
        "{dump}"
    );
    assert!(dump.contains("Cell @"), "{dump}");
    assert_eq!(dump, dump_mir(&program));
}

#[test]
fn verifier_rejects_malformed_cell_modifier_evidence() {
    let mut program = lower_text(SOURCE);
    let field = &mut program.classes.entries_mut_for_test()[0].fields[0];
    field.cell_span = Some(Span::empty(
        field.span.source_id(),
        field.span.range().end(),
    ));

    let errors = verify_mir(&program).unwrap_err().to_string();
    assert!(
        errors.contains("cell modifier span must be nonempty"),
        "{errors}"
    );
}
