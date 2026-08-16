use super::*;
use crate::{
    identity::ClassId,
    mir::{dump_mir, verify_mir},
    test_support::lower_hir_to_final_mir,
};

#[test]
fn carries_final_metadata_through_typed_hir_and_mir() {
    let output = check_text(concat!(
        "class Values {\n",
        "  final value: i64;\n",
        "  final static version: u64 = 1u;\n",
        "  init(value: i64) { self.value = value; }\n",
        "}\n",
        "fn main() -> i64 { return 0; }\n",
    ));

    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    let hir = output.hir.unwrap();
    let values = hir.class(ClassId::new(0)).unwrap();
    assert!(values.fields[0].final_span.is_some());
    assert!(values.static_fields[0].final_span.is_some());
    let hir_dump = dump_hir(&hir);
    assert!(
        hir_dump.contains("Field c0:field0 final \"value\""),
        "{hir_dump}"
    );
    assert!(
        hir_dump.contains("StaticField c0:static0 final \"version\""),
        "{hir_dump}"
    );

    let mir = lower_hir_to_final_mir(&hir);
    verify_mir(&mir).unwrap();
    let values = mir.class(ClassId::new(0)).unwrap();
    assert!(values.fields[0].final_span.is_some());
    assert!(values.static_fields[0].final_span.is_some());
    assert_eq!(dump_mir(&mir).matches("Final @").count(), 2);
}
