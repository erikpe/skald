use super::*;
use crate::{
    source::{Span, TextRange},
    test_support::lower_source_to_final_mir,
};

const SOURCE: &str = concat!(
    "class Values {\n",
    "  final value: i64;\n",
    "  final static version: u64 = 1u;\n",
    "  init(value: i64) { self.value = value; }\n",
    "}\n",
    "fn main() -> i64 { return 0; }\n",
);

#[test]
fn lowers_exact_final_modifier_evidence_and_dumps_it() {
    let program = lower_source_to_final_mir(SOURCE);
    verify_mir(&program).unwrap();
    let values = program.class(ClassId::new(0)).unwrap();
    assert!(values.fields[0].final_span.is_some());
    assert!(values.static_fields[0].final_span.is_some());

    let dump = dump_mir(&program);
    assert!(
        dump.contains("Field c0:field0 final \"value\" : i64"),
        "{dump}"
    );
    assert!(
        dump.contains("StaticField c0:static0 final \"version\""),
        "{dump}"
    );
    assert_eq!(dump.matches("Final @").count(), 2, "{dump}");
    assert_eq!(dump, dump_mir(&program));
}

#[test]
fn verifier_rejects_malformed_final_declaration_metadata() {
    let program = lower_source_to_final_mir(SOURCE);

    let mut empty = program.clone();
    let field = &mut empty.classes.entries_mut_for_test()[0].fields[0];
    field.final_span = Some(Span::empty(
        field.span.source_id(),
        field.span.range().end(),
    ));
    let errors = verify_mir(&empty).unwrap_err().to_string();
    assert!(
        errors.contains("final modifier span must be nonempty"),
        "{errors}"
    );

    let mut outside = program.clone();
    let field = &mut outside.classes.entries_mut_for_test()[0].fields[0];
    field.final_span = Some(Span::new(
        field.span.source_id(),
        TextRange::new(field.span.range().end(), field.span.range().end() + 1).unwrap(),
    ));
    let errors = verify_mir(&outside).unwrap_err().to_string();
    assert!(
        errors.contains("contained by its declaration span"),
        "{errors}"
    );

    let mut incompatible = program.clone();
    let field = &mut incompatible.classes.entries_mut_for_test()[0].fields[0];
    field.cell_span = field.final_span;
    let errors = verify_mir(&incompatible).unwrap_err().to_string();
    assert!(
        errors.contains("cannot carry both cell and final metadata"),
        "{errors}"
    );

    let mut missing_initializer = program;
    let field = &mut missing_initializer.classes.entries_mut_for_test()[0].static_fields[0];
    field.initialization = MirStaticFieldInitialization::ZeroDefault;
    let errors = verify_mir(&missing_initializer).unwrap_err().to_string();
    assert!(
        errors.contains("must have explicit initialization"),
        "{errors}"
    );
}
