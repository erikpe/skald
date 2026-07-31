use super::*;
use crate::hir::{HirPrimitiveCast, HirPrimitiveType};

const INTEGER_TYPES: &[(HirPrimitiveType, &str, &str)] = &[
    (HirPrimitiveType::I64, "i64", "-1"),
    (HirPrimitiveType::U64, "u64", "18446744073709551615u"),
    (HirPrimitiveType::U8, "u8", "255u8"),
];

const PRIMITIVE_SOURCES: &[(&str, &str)] = &[
    ("i64", "-1"),
    ("u64", "1u"),
    ("u8", "1u8"),
    ("f64", "1.5"),
    ("bool", "true"),
];

const PRIMITIVE_TARGETS: &[&str] = &["i64", "u64", "u8", "f64", "bool"];

#[test]
fn checks_the_complete_implemented_integer_subset() {
    for &(source_type, source_name, operand) in INTEGER_TYPES {
        for &(target_type, target_name, _) in INTEGER_TYPES {
            let source = format!(
                "fn cast() -> {target_name} {{ return ({target_name}) {operand}; }} \
                 fn main() -> i64 {{ return 0; }}"
            );
            let output = check_text(&source);
            assert!(
                !output.has_errors(),
                "{source_name}-to-{target_name}: {:?}",
                output.diagnostics
            );
            let hir = output.hir.unwrap();
            let expression = returned_expression(hir.definitions.get(FunctionId::new(0)).unwrap());
            let HirExpressionKind::PrimitiveCast { operation, operand } = &expression.kind else {
                panic!("expected typed primitive cast");
            };
            assert_eq!(*operation, HirPrimitiveCast::new(source_type, target_type));
            assert_eq!(operation.source_type(), source_type.value_type());
            assert_eq!(operation.result_type(), target_type.value_type());
            assert_eq!(operand.ty, operation.source_type());
            assert_eq!(expression.ty, operation.result_type());

            let dump = dump_hir(&hir);
            assert_eq!(dump, dump_hir(&hir));
            assert!(dump.contains(&format!(
                "PrimitiveCast {} {source_name}.{target_name} : {target_name}",
                operation.kind().mnemonic()
            )));
        }
    }
}

#[test]
fn casts_do_not_change_literal_validity_and_enable_exact_type_comparisons() {
    for source in [
        "fn cast() -> u8 { return (u8) 258u; } fn main() -> i64 { return 0; }",
        "fn cast() -> u64 { return (u64) -1; } fn main() -> i64 { return 0; }",
        "fn cast() -> i64 { return (i64) 18446744073709551615u; } fn main() -> i64 { return 0; }",
        "fn compare() -> bool { return (u64) 1 == 1u; } fn main() -> i64 { return 0; }",
    ] {
        let output = check_text(source);
        assert!(!output.has_errors(), "{source}: {:?}", output.diagnostics);
        assert!(output.hir.is_some(), "{source}");
    }

    let invalid =
        check_text("fn cast() -> u8 { return (u8) 256u8; } fn main() -> i64 { return 0; }");
    assert!(invalid.has_errors());
    assert!(invalid.hir.is_none());
}

#[test]
fn pending_primitive_cast_pairs_have_one_focused_temporary_diagnostic() {
    let mut pending_pairs = 0;
    for &(source_name, operand) in PRIMITIVE_SOURCES {
        for &target_name in PRIMITIVE_TARGETS {
            if matches!(source_name, "i64" | "u64" | "u8")
                && matches!(target_name, "i64" | "u64" | "u8")
            {
                continue;
            }
            pending_pairs += 1;
            let source = format!(
                "fn cast() -> {target_name} {{ return ({target_name}) {operand}; }} \
                 fn main() -> i64 {{ return 0; }}"
            );
            let output = check_text(&source);
            assert!(output.has_errors(), "{source_name}-to-{target_name}");
            assert!(output.hir.is_none(), "{source_name}-to-{target_name}");
            assert_eq!(
                output.diagnostics.len(),
                1,
                "{source_name}-to-{target_name}: {:?}",
                output.diagnostics
            );
            let diagnostic = output
                .diagnostics
                .iter()
                .find(|diagnostic| {
                    diagnostic.message.contains("primitive cast")
                        && diagnostic.message.contains("not implemented yet")
                })
                .unwrap_or_else(|| {
                    panic!(
                        "missing pending-pair diagnostic for {source_name}-to-{target_name}: {:?}",
                        output.diagnostics
                    )
                });
            assert_eq!(diagnostic.code, TYPE_MISMATCH);
            assert!(diagnostic.message.contains(source_name));
            assert!(diagnostic.message.contains(target_name));
        }
    }
    assert_eq!(pending_pairs, 16);
}

#[test]
fn rejects_every_nonprimitive_source_family_for_each_primitive_target_before_hir() {
    const SOURCES: &[&str] = &[
        "fn cast() -> {target} { return ({target}) 1.0; } fn main() -> i64 { return 0; }",
        "fn cast() -> {target} { return ({target}) true; } fn main() -> i64 { return 0; }",
        "fn notify() -> unit {} fn cast() -> {target} { return ({target}) notify(); } fn main() -> i64 { return 0; }",
        "fn cast(value: i64?) -> {target} { return ({target}) value; } fn main() -> i64 { return 0; }",
        "fn cast(value: i64[]) -> {target} { return ({target}) value; } fn main() -> i64 { return 0; }",
        "class Item { init() {} } fn cast(value: Item) -> {target} { return ({target}) value; } fn main() -> i64 { return 0; }",
        "fn cast(ref value: Obj) -> {target} { return ({target}) value; } fn main() -> i64 { return 0; }",
    ];

    for &target in PRIMITIVE_TARGETS {
        for template in SOURCES {
            let source = template.replace("{target}", target);
            let output = check_text(&source);
            assert!(output.has_errors(), "{source}");
            assert!(output.hir.is_none(), "{source}");
        }
    }
}

#[test]
fn casts_remain_required_at_exact_type_boundaries() {
    let implicit =
        check_text("fn cast(value: u64) -> i64 { return value; } fn main() -> i64 { return 0; }");
    assert!(implicit.has_errors());
    assert!(implicit.hir.is_none());

    let explicit = check_text(
        "fn cast(value: u64) -> i64 { return (i64) value; } fn main() -> i64 { return 0; }",
    );
    assert!(!explicit.has_errors(), "{:?}", explicit.diagnostics);
    assert!(explicit.hir.is_some());
}

#[test]
fn explicit_casts_compose_with_value_boundaries_and_arithmetic() {
    let output = check_text(
        "class Holder {\n\
           small: u8;\n\
           init(value: u64) { self.small = (u8) value; }\n\
           mut fn replace(value: u64) -> u8 {\n\
             self.small = (u8) value;\n\
             return self.small;\n\
           }\n\
         }\n\
         fn consume(value: u8) -> u8 { return value; }\n\
         fn cast(value: u64) -> u8 {\n\
           var local: u8 = (u8) value;\n\
           return consume((u8) value) + local;\n\
         }\n\
         fn main() -> i64 { return 0; }\n",
    );

    assert!(!output.has_errors(), "{:?}", output.diagnostics);
    assert!(output.hir.is_some());
}
