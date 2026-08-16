use super::*;
use crate::{
    identity::ClassId,
    mir::{lower_hir, verify_mir},
    typeck::READ_ONLY_RECEIVER,
};

#[test]
fn carries_cell_metadata_through_typed_generic_ir() {
    let hir = check_generic_source(concat!(
        "class Box<T> {\n",
        "  private cell value: T;\n",
        "  init(value: T) { self.value = value; }\n",
        "  fn get() -> T { return self.value; }\n",
        "}\n",
        "fn main() -> i64 { var box: Box<i64> = Box<i64>(7); return box.get(); }\n",
    ));

    let class = hir.classes.iter().next().expect("specialized class");
    assert!(class.fields[0].cell_span.is_some());
    assert!(dump_hir(&hir).contains("Field c0:field0 cell \"value\""));

    let mir = lower_hir(&hir);
    assert!(mir.class(ClassId::new(0)).unwrap().fields[0]
        .cell_span
        .is_some());
    verify_mir(&mir).unwrap();
}

#[test]
fn cell_does_not_yet_authorize_assignment_through_a_read_only_receiver() {
    let output = check_text(concat!(
        "class Cache {\n",
        "  private cell value: i64;\n",
        "  init() { self.value = 0; }\n",
        "  fn write() -> unit { self.value = 1; }\n",
        "}\n",
        "fn main() -> i64 { return 0; }\n",
    ));

    assert!(output.hir.is_none());
    assert!(output
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == READ_ONLY_RECEIVER));
}
