use super::*;

#[test]
fn boolean_functions_require_a_boolean_return_value() {
    let output = check_text("fn flag() -> bool {} fn main() -> i64 { return 0; }");

    assert!(output.hir.is_none());
    assert!(output.diagnostics.iter().any(|diagnostic| {
        diagnostic.code == MISSING_RETURN
            && diagnostic
                .notes
                .iter()
                .any(|note| note.contains("return type `bool`"))
    }));
}

#[test]
fn checks_external_calls_from_bodyless_signatures() {
    let output = check_text(concat!(
        "extern fn read_value(seed: i64) -> i64;\n",
        "extern fn emit(value: i64) -> unit;\n",
        "fn main() -> i64 { var value: i64 = read_value(7); emit(value); return value; }\n",
    ));

    assert!(!output.has_errors());
    let hir = output.hir.unwrap();
    for id in [FunctionId::new(0), FunctionId::new(1)] {
        assert!(matches!(
            hir.declarations.get(id).unwrap().linkage,
            crate::hir::HirFunctionLinkage::External { .. }
        ));
        assert!(hir.definitions.get(id).is_none());
    }
    let dump = dump_hir(&hir);
    assert!(dump.contains("Declaration f0 \"read_value\" external \"read_value\""));
    assert!(dump.contains("Declaration f1 \"emit\" external \"emit\""));
    assert!(!dump.contains("Definition f0"));
    assert!(!dump.contains("Definition f1"));
}

#[test]
fn rejects_an_external_main_even_with_the_entry_signature() {
    let output = check_text("extern fn main() -> i64;");

    assert!(output.hir.is_none());
    let diagnostic = output.diagnostics.iter().next().unwrap();
    assert_eq!(diagnostic.code, INVALID_ENTRY_POINT);
    assert!(diagnostic.message.contains("fn main() -> i64"));
    assert!(diagnostic
        .labels
        .iter()
        .any(|label| label.message.contains("cannot be the entry point")));
}

#[test]
fn rejects_external_signatures_outside_the_restricted_abi_profile() {
    let mut resolved = resolve_text(concat!(
        "extern fn emit(value: i64) -> unit;\n",
        "fn main() -> i64 { return 0; }\n",
    ));
    resolved.declarations.entries_mut_for_test()[0].parameters[0]
        .type_syntax
        .kind = crate::resolve::ResolvedTypeKind::Unit;

    let output = type_check(&resolved);

    assert!(output.hir.is_none());
    assert!(output.diagnostics.iter().any(|diagnostic| {
        diagnostic.code == INVALID_EXTERNAL_DECLARATION
            && diagnostic.message.contains("unsupported signature")
    }));
}

#[test]
fn missing_entry_point_prevents_hir_construction() {
    let output = check_text("fn helper() -> i64 { return 0; }");

    assert!(output.hir.is_none());
    assert_eq!(output.diagnostics.len(), 1);
    assert_eq!(
        output.diagnostics.iter().next().unwrap().code,
        MISSING_ENTRY_POINT
    );
}

#[test]
fn entry_point_must_have_the_exact_first_slice_signature() {
    let output = check_text("fn main(value: i64) -> i64 { return value; }");

    assert!(output.hir.is_none());
    let diagnostic = output.diagnostics.iter().next().unwrap();
    assert_eq!(diagnostic.code, INVALID_ENTRY_POINT);
    assert!(diagnostic.message.contains("fn main() -> i64"));
}

#[test]
fn direct_call_arity_is_checked_against_the_resolved_target() {
    let output = check_text(concat!(
        "fn one(value: i64) -> i64 { return value; }\n",
        "fn main() -> i64 { return one(); }\n",
    ));

    assert!(output.hir.is_none());
    let diagnostic = output.diagnostics.iter().next().unwrap();
    assert_eq!(diagnostic.code, WRONG_ARGUMENT_COUNT);
    assert_eq!(diagnostic.labels.len(), 2);
    assert!(diagnostic
        .message
        .contains("expects 1 argument but received 0"));
}
