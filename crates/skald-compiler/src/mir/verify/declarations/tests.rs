use crate::{
    identity::{ClassId, FieldId, FunctionId},
    mir::{verify_mir, MirCopyCapability, MirFunctionLinkage, MirReceiverAccess},
    test_support::lower_source_to_mir,
};

fn messages(source: &crate::mir::MirProgram) -> Vec<String> {
    verify_mir(source)
        .unwrap_err()
        .iter()
        .map(|error| error.message.clone())
        .collect()
}

#[test]
fn rejects_invalid_external_symbol_metadata() {
    let mut program = lower_source_to_mir(concat!(
        "extern fn foreign(value: i64) -> i64;\n",
        "fn main() -> i64 { return foreign(7); }\n",
    ));
    let foreign = FunctionId::new(0);
    program.declarations.entries_mut_for_test()[foreign.index()].linkage =
        MirFunctionLinkage::External {
            symbol: "wrong-symbol".to_owned(),
        };

    assert!(messages(&program).iter().any(|message| message
        .contains("external symbol must be the declaration's exact source identifier")));
}

#[test]
fn preserves_entry_then_declaration_error_order_for_a_missing_definition() {
    let mut program = lower_source_to_mir("fn main() -> i64 { return 0; }");
    program.definitions.remove_for_test(program.entry_function);

    assert_eq!(
        messages(&program),
        [
            "entry function f0 has no definition",
            "internal function has no definition",
        ]
    );
}

#[test]
fn rejects_field_metadata_with_the_wrong_owner() {
    let mut program = lower_source_to_mir(concat!(
        "class Item { value: i64; init(value: i64) { self.value = value; } }\n",
        "fn main() -> i64 { return 0; }\n",
    ));
    program.classes.entries_mut_for_test()[0].fields[0].id = FieldId::new(ClassId::new(7), 0);

    assert!(messages(&program)
        .iter()
        .any(|message| message.contains("field table index 0")));
}

#[test]
fn rejects_noncanonical_destruction_metadata() {
    let mut program = lower_source_to_mir(concat!(
        "class Leaf { init() {} }\n",
        "class Pair { left: Leaf; right: Leaf; init() { self.left = Leaf(); self.right = Leaf(); } destroy {} }\n",
        "fn main() -> i64 { return 0; }\n",
    ));
    program.classes.entries_mut_for_test()[1]
        .destruction
        .steps
        .swap(0, 2);

    assert!(messages(&program)
        .iter()
        .any(|message| message.contains("user body first and class fields in reverse")));
}

#[test]
fn rejects_synthesized_copy_metadata_with_the_wrong_owner() {
    let mut program = lower_source_to_mir(concat!(
        "class Value { value: i64; init(value: i64) { self.value = value; } }\n",
        "fn main() -> i64 { return 0; }\n",
    ));
    let class = &mut program.classes.entries_mut_for_test()[0];
    let MirCopyCapability::Synthesized(copy) = &mut class.copy_assignment else {
        panic!("expected synthesized copy assignment");
    };
    copy.class = ClassId::new(7);

    assert!(messages(&program).iter().any(|message| message
        .contains("synthesized copy-assignment plan has the wrong owner or field count")));
}

#[test]
fn rejects_destructor_access_metadata_without_inspecting_its_body() {
    let mut program = lower_source_to_mir(concat!(
        "class Resource { init() {} destroy {} }\n",
        "fn main() -> i64 { return 0; }\n",
    ));
    program.classes.entries_mut_for_test()[0]
        .destruction
        .destructor
        .as_mut()
        .unwrap()
        .receiver_access = MirReceiverAccess::ReadOnly;

    assert!(messages(&program)
        .iter()
        .any(|message| message.contains("must have mutable receiver access")));
}
