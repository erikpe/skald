use super::*;
use crate::{
    identity::{ClassId, FieldId, StaticFieldId},
    test_support::load_module_sources_with_standard_library,
};

#[test]
fn preserves_final_metadata_on_existing_field_identities() {
    let output = resolve_text(concat!(
        "class Values {\n",
        "  final value: i64;\n",
        "  final static version: u64 = 1u;\n",
        "  init(value: i64) { self.value = value; }\n",
        "}\n",
        "fn main() -> i64 { return 0; }\n",
    ));

    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    let values = output.program.class(ClassId::new(0)).unwrap();
    assert_eq!(values.fields[0].id, FieldId::new(values.id, 0));
    assert!(values.fields[0].final_span.is_some());
    assert_eq!(values.static_fields[0].id, StaticFieldId::new(values.id, 0));
    assert!(values.static_fields[0].final_span.is_some());

    let dump = dump_resolved(&output.program);
    assert!(dump.contains("Field c0:field0 final \"value\""), "{dump}");
    assert!(
        dump.contains("StaticField c0:static0 final \"version\""),
        "{dump}"
    );
    assert_eq!(dump.matches("Final @").count(), 2, "{dump}");
}

#[test]
fn specialization_carries_final_metadata_without_replacing_field_ids() {
    let program = crate::test_support::resolve_generic_source(concat!(
        "class Box<T> {\n",
        "  final value: T;\n",
        "  final static marker: u64 = 1u;\n",
        "  init(value: T) { self.value = value; }\n",
        "}\n",
        "fn main() -> i64 {\n",
        "  var left: Box<i64> = Box<i64>(7);\n",
        "  var right: Box<u64> = Box<u64>(8u);\n",
        "  return left.value;\n",
        "}\n",
    ));

    let specialized = program
        .classes
        .iter()
        .filter(|class| class.name.starts_with("Box<"))
        .collect::<Vec<_>>();
    assert_eq!(specialized.len(), 2);
    for class in specialized {
        assert_eq!(class.fields[0].id, FieldId::new(class.id, 0));
        assert!(class.fields[0].final_span.is_some());
        assert_eq!(class.static_fields[0].id, StaticFieldId::new(class.id, 0));
        assert!(class.static_fields[0].final_span.is_some());
    }
    assert!(dump_resolved(&program).contains("final \"value\""));
}

#[test]
fn canonical_primitive_boxes_expose_one_public_final_payload() {
    let (_workspace, graph) = load_module_sources_with_standard_library(
        "app",
        &[(
            "app.ska",
            concat!(
                "from std::bool import BoxBool;\n",
                "from std::f64 import BoxF64;\n",
                "from std::i64 import BoxI64;\n",
                "from std::u64 import BoxU64;\n",
                "from std::u8 import BoxU8;\n",
                "fn main() -> i64 {\n",
                "  var truth: BoxBool = BoxBool(true);\n",
                "  var floating: BoxF64 = BoxF64(1.0);\n",
                "  var signed: BoxI64 = BoxI64(2);\n",
                "  var unsigned: BoxU64 = BoxU64(3u);\n",
                "  var byte: BoxU8 = BoxU8(4u8);\n",
                "  return signed.value + (i64) unsigned.value + (i64) byte.value;\n",
                "}\n",
            ),
        )],
    );
    let output = crate::resolve::resolve_module_graph(&graph);
    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);

    for name in ["BoxBool", "BoxF64", "BoxI64", "BoxU64", "BoxU8"] {
        let class = output
            .program
            .classes
            .iter()
            .find(|class| class.name == name)
            .unwrap_or_else(|| panic!("canonical standard library must declare {name}"));
        assert_eq!(class.fields.len(), 1, "{name}");
        let field = &class.fields[0];
        assert_eq!(field.name, "value", "{name}");
        assert_eq!(field.visibility, ResolvedMemberVisibility::Public, "{name}");
        assert!(field.final_span.is_some(), "{name}");
        assert!(field.cell_span.is_none(), "{name}");
    }
}
