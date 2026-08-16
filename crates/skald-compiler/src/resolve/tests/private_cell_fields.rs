use super::*;
use crate::{
    identity::{ClassId, FieldId},
    resolve::{DUPLICATE_MEMBER, PRIVATE_MEMBER_ACCESS},
};

#[test]
fn preserves_cell_metadata_on_the_existing_field_identity() {
    let output = resolve_text(concat!(
        "class Cache {\n",
        "  private cell cached: u64?;\n",
        "  ordinary: i64;\n",
        "  init() { self.cached = none; self.ordinary = 0; }\n",
        "}\n",
    ));

    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    let cache = output.program.class(ClassId::new(0)).unwrap();
    assert_eq!(cache.fields[0].id, FieldId::new(cache.id, 0));
    assert!(cache.fields[0].cell_span.is_some());
    assert!(cache.fields[1].cell_span.is_none());

    let dump = dump_resolved(&output.program);
    assert!(
        dump.contains("Field c0:field0 private cell \"cached\""),
        "{dump}"
    );
    assert!(dump.contains("Cell @"), "{dump}");
    assert_eq!(dump, dump_resolved(&output.program));
}

#[test]
fn specialization_carries_cell_metadata_without_replacing_field_ids() {
    let program = crate::test_support::resolve_generic_source(concat!(
        "class Box<T> {\n",
        "  private cell value: T;\n",
        "  init(value: T) { self.value = value; }\n",
        "  fn get() -> T { return self.value; }\n",
        "}\n",
        "fn main() -> i64 { var box: Box<i64> = Box<i64>(7); return box.get(); }\n",
    ));

    let specialized = program
        .classes
        .iter()
        .find(|class| class.name.starts_with("Box<"))
        .expect("generic use must create a specialized class");
    assert_eq!(specialized.fields[0].id, FieldId::new(specialized.id, 0));
    assert!(specialized.fields[0].cell_span.is_some());
    assert!(dump_resolved(&program).contains("cell \"value\""));
}

#[test]
fn cell_fields_reuse_ordinary_collision_inheritance_and_privacy_rules() {
    let output = resolve_text(concat!(
        "class Base {\n",
        "  private cell value: i64;\n",
        "  init() { self.value = 0; }\n",
        "  fn value() -> i64 { return self.value; }\n",
        "}\n",
        "class Derived extends Base {\n",
        "  init() { super(); }\n",
        "  fn expose() -> i64 { return self.value; }\n",
        "}\n",
        "fn main() -> i64 { return 0; }\n",
    ));

    let codes = output
        .diagnostics
        .iter()
        .map(|diagnostic| diagnostic.code)
        .collect::<Vec<_>>();
    assert!(codes.contains(&DUPLICATE_MEMBER), "{codes:?}");
    assert!(codes.contains(&PRIVATE_MEMBER_ACCESS), "{codes:?}");
}
