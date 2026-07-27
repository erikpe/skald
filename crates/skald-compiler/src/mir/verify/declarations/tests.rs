use crate::{
    identity::{ArrayTypeId, ClassId, ExternalLinkId, FieldId, FunctionId, ModuleId},
    mir::{
        verify_mir, MirCopyCapability, MirFunctionLinkage, MirReceiverAccess,
        MirSynthesizedFieldCopy,
    },
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
    program.external_links.entries_mut_for_test()[0].symbol = "wrong-symbol".to_owned();

    assert!(messages(&program).iter().any(|message| message
        .contains("external link symbol must be the declaration's exact source identifier")));
}

#[test]
fn rejects_corrupt_external_link_identity_membership_and_signatures() {
    let source = concat!(
        "extern fn foreign(value: i64) -> i64;\n",
        "fn main() -> i64 { return foreign(7); }\n",
    );

    let mut non_dense = lower_source_to_mir(source);
    non_dense.external_links.entries_mut_for_test()[0].id = ExternalLinkId::new(7);
    assert!(messages(&non_dense)
        .iter()
        .any(|message| message.contains("external-link table index 0 contains ext7")));

    let mut absent = lower_source_to_mir(source);
    absent.external_links.entries_mut_for_test()[0]
        .declarations
        .clear();
    let absent_messages = messages(&absent);
    assert!(absent_messages
        .iter()
        .any(|message| message.contains("external link ext0 has no declarations")));
    assert!(absent_messages
        .iter()
        .any(|message| message.contains("function is absent from external link ext0")));

    let mut unknown = lower_source_to_mir(source);
    unknown.declarations.entries_mut_for_test()[0].linkage = MirFunctionLinkage::External {
        link: ExternalLinkId::new(7),
    };
    assert!(messages(&unknown)
        .iter()
        .any(|message| message.contains("function references unknown external link ext7")));

    let mut incompatible = lower_source_to_mir(source);
    incompatible.declarations.entries_mut_for_test()[1].linkage = MirFunctionLinkage::External {
        link: ExternalLinkId::new(0),
    };
    incompatible.external_links.entries_mut_for_test()[0]
        .declarations
        .push(FunctionId::new(1));
    incompatible.definitions.remove_for_test(FunctionId::new(1));
    assert!(messages(&incompatible)
        .iter()
        .any(|message| message.contains("contains incompatible function signatures")));

    let mut unordered = lower_source_to_mir(concat!(
        "extern fn alpha() -> i64;\n",
        "extern fn beta() -> i64;\n",
        "fn main() -> i64 { return alpha() + beta(); }\n",
    ));
    unordered.external_links.entries_mut_for_test().swap(0, 1);
    assert!(messages(&unordered)
        .iter()
        .any(|message| message.contains("external-link symbols are not unique and ordered")));
}

#[test]
fn rejects_invalid_module_metadata_and_declaration_ownership() {
    let mut non_dense = lower_source_to_mir("fn main() -> i64 { return 0; }");
    non_dense
        .modules
        .set_module_id_for_test(0, ModuleId::new(7));
    assert!(messages(&non_dense)
        .iter()
        .any(|message| message.contains("module table index 0 contains m7")));

    let mut unknown_owner = lower_source_to_mir("fn main() -> i64 { return 0; }");
    unknown_owner.declarations.entries_mut_for_test()[0].module = ModuleId::new(7);
    let errors = messages(&unknown_owner);
    assert!(errors
        .iter()
        .any(|message| message.contains("function has unknown module owner m7")));
    assert!(errors.iter().any(|message| {
        message.contains("entry function f0 belongs to m7, but selected entry module is m0")
    }));

    let mut unknown_class = lower_source_to_mir(concat!(
        "class Value { init() {} }\n",
        "fn main() -> i64 { return 0; }\n",
    ));
    unknown_class.classes.entries_mut_for_test()[0].module = ModuleId::new(7);
    assert!(messages(&unknown_class)
        .iter()
        .any(|message| message.contains("class c0 has unknown module owner m7")));

    let mut unknown_interface = lower_source_to_mir(concat!(
        "interface Value { fn value() -> i64; }\n",
        "fn main() -> i64 { return 0; }\n",
    ));
    unknown_interface.interfaces.entries_mut_for_test()[0].module = ModuleId::new(7);
    assert!(messages(&unknown_interface)
        .iter()
        .any(|message| message.contains("interface i0 has unknown module owner m7")));
}

#[test]
fn rejects_an_unknown_selected_entry_module() {
    let mut program = lower_source_to_mir("fn main() -> i64 { return 0; }");
    program.modules.set_selected_for_test(ModuleId::new(7));

    let errors = messages(&program);
    assert!(errors
        .iter()
        .any(|message| message.contains("selected entry module m7 is not in the module table")));
    assert!(errors.iter().any(|message| {
        message.contains("entry function f0 belongs to m0, but selected entry module is m7")
    }));
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
        .any(|message| message.contains("user body first and owning fields in reverse")));
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
fn verifies_array_field_copy_and_destruction_metadata() {
    let program = lower_source_to_mir(concat!(
        "class Holder { values: i64[]; init() { self.values = i64[](); } }\n",
        "fn main() -> i64 { return 0; }\n",
    ));

    verify_mir(&program).expect("canonical array field lifecycle metadata must verify");
}

#[test]
fn rejects_synthesized_array_field_copy_with_the_wrong_array() {
    let mut program = lower_source_to_mir(concat!(
        "class Holder { values: i64[]; init() { self.values = i64[](); } }\n",
        "fn main() -> i64 { return 0; }\n",
    ));
    let class = &mut program.classes.entries_mut_for_test()[0];
    let MirCopyCapability::Synthesized(copy) = &mut class.copy_constructor else {
        panic!("expected synthesized copy constructor");
    };
    let MirSynthesizedFieldCopy::Array { array, .. } = &mut copy.fields[0] else {
        panic!("expected synthesized array field copy");
    };
    *array = ArrayTypeId::new(7);

    assert!(messages(&program)
        .iter()
        .any(|message| message.contains("synthesized copy-construction plan is invalid")));
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
