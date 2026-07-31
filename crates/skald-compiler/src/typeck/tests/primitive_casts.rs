use super::*;
use crate::hir::{HirPrimitiveCast, HirPrimitiveType};

const PRIMITIVE_TYPES: &[(HirPrimitiveType, &str, &str)] = &[
    (HirPrimitiveType::I64, "i64", "-1"),
    (HirPrimitiveType::U64, "u64", "18446744073709551615u"),
    (HirPrimitiveType::U8, "u8", "255u8"),
    (HirPrimitiveType::F64, "f64", "1.5"),
    (HirPrimitiveType::Bool, "bool", "true"),
];

#[test]
fn checks_the_complete_non_failing_primitive_cast_matrix() {
    let mut implemented_pairs = 0;
    for &(source_type, source_name, operand) in PRIMITIVE_TYPES {
        for &(target_type, target_name, _) in PRIMITIVE_TYPES {
            let operation = HirPrimitiveCast::new(source_type, target_type);
            if operation.may_terminate() {
                continue;
            }
            implemented_pairs += 1;
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
    assert_eq!(implemented_pairs, 22);
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
fn checked_primitive_cast_pairs_have_one_focused_temporary_diagnostic() {
    let mut pending_pairs = 0;
    for &(source_type, source_name, operand) in PRIMITIVE_TYPES {
        for &(target_type, target_name, _) in PRIMITIVE_TYPES {
            if !HirPrimitiveCast::new(source_type, target_type).may_terminate() {
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
    assert_eq!(pending_pairs, 3);
}

#[test]
fn rejects_every_nonprimitive_source_family_for_each_primitive_target_before_hir() {
    const SOURCES: &[&str] = &[
        "fn notify() -> unit {} fn cast() -> {target} { return ({target}) notify(); } fn main() -> i64 { return 0; }",
        "fn cast(value: i64?) -> {target} { return ({target}) value; } fn main() -> i64 { return 0; }",
        "fn cast(value: i64[]) -> {target} { return ({target}) value; } fn main() -> i64 { return 0; }",
        "class Item { init() {} } fn cast(value: Item) -> {target} { return ({target}) value; } fn main() -> i64 { return 0; }",
        "fn cast(ref value: Obj) -> {target} { return ({target}) value; } fn main() -> i64 { return 0; }",
    ];

    for &(_, target, _) in PRIMITIVE_TYPES {
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
    let mut implicit_pairs = 0;
    for &(_, source, _) in PRIMITIVE_TYPES {
        for &(_, target, _) in PRIMITIVE_TYPES {
            if source == target {
                continue;
            }
            implicit_pairs += 1;
            let text = format!(
                "fn cast(value: {source}) -> {target} {{ return value; }} \
                 fn main() -> i64 {{ return 0; }}"
            );
            let output = check_text(&text);
            assert!(output.has_errors(), "implicit {source}-to-{target}");
            assert!(output.hir.is_none(), "implicit {source}-to-{target}");
        }
    }
    assert_eq!(implicit_pairs, 20);

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
           enabled: bool;\n\
           value: f64;\n\
           init(value: u64) {\n\
             self.enabled = (bool) value;\n\
             self.value = (f64) value;\n\
           }\n\
           mut fn replace(value: bool) -> f64 {\n\
             self.value = (f64) value;\n\
             return self.value;\n\
           }\n\
         }\n\
         fn consume(value: f64) -> f64 { return value; }\n\
         fn cast(value: u64) -> f64 {\n\
           var local: f64 = (f64) value;\n\
           local = (f64) (bool) value;\n\
           var values: f64[] = f64[](1u);\n\
           values[0] = (f64) value;\n\
           var optional: f64? = (f64) value;\n\
           if ((bool) value) {\n\
             return consume(values[0] + optional! + local);\n\
           }\n\
           return (f64) (bool) false;\n\
         }\n\
         fn main() -> i64 { return 0; }\n",
    );

    assert!(!output.has_errors(), "{:?}", output.diagnostics);
    assert!(output.hir.is_some());
}
