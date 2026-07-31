use super::*;
use crate::hir::{HirIntegerDivisionKind, HirIntegerType};

const INTEGER_TYPES: &[(HirIntegerType, &str, &str, &str)] = &[
    (HirIntegerType::I64, "i64", "-7", "3"),
    (HirIntegerType::U64, "u64", "7u", "3u"),
    (HirIntegerType::U8, "u8", "7u8", "3u8"),
];

#[test]
fn selects_division_and_remainder_for_every_exact_integer_type() {
    for &(integer, type_name, dividend, divisor) in INTEGER_TYPES {
        for (kind, spelling) in [
            (HirIntegerDivisionKind::Quotient, "/"),
            (HirIntegerDivisionKind::Remainder, "%"),
        ] {
            let source = format!(
                "fn calculate() -> {type_name} {{ return {dividend} {spelling} {divisor}; }} \
                 fn main() -> i64 {{ return 0; }}"
            );
            let output = check_text(&source);
            assert!(!output.has_errors(), "{source}");
            let hir = output.hir.unwrap();
            let expression = returned_expression(hir.definitions.get(FunctionId::new(0)).unwrap());
            let HirExpressionKind::CheckedIntegerDivision(division) = &expression.kind else {
                panic!("expected checked integer division");
            };
            assert_eq!(division.operation.kind, kind);
            assert_eq!(division.operation.operand, integer);
            assert_eq!(division.dividend.ty, integer.operand_type());
            assert_eq!(division.divisor.ty, integer.operand_type());
            assert_eq!(expression.ty, integer.operand_type());
        }
    }
}

#[test]
fn rejects_mixed_and_noninteger_operands_with_focused_actual_types() {
    let cases = [
        ("1", "1u", "i64", "u64"),
        ("1u", "1", "u64", "i64"),
        ("1", "1u8", "i64", "u8"),
        ("1u8", "1", "u8", "i64"),
        ("1u", "1u8", "u64", "u8"),
        ("1u8", "1u", "u8", "u64"),
        ("true", "false", "bool", "bool"),
        ("1.0", "2.0", "f64", "f64"),
        ("notify()", "notify()", "unit", "unit"),
        ("optional", "optional", "i64?", "i64?"),
        ("values", "values", "array a0", "array a0"),
        ("item", "item", "class c0", "class c0"),
        (
            "shared_item",
            "shared_item",
            "shared class c0",
            "shared class c0",
        ),
        ("object", "object", "Obj", "Obj"),
    ];
    for (left, right, left_type, right_type) in cases {
        for spelling in ["/", "%"] {
            let source = format!(
                "class Item {{ init() {{}} }} \
                 fn notify() -> unit {{}} \
                 fn invalid(ref item: Item, shared_item: shared Item, ref object: Obj, optional: i64?, values: i64[]) -> i64 {{ \
                   return {left} {spelling} {right}; \
                 }} \
                 fn main() -> i64 {{ return 0; }}"
            );
            let output = check_text(&source);
            assert!(output.hir.is_none(), "{source}");
            let diagnostic = output
                .diagnostics
                .iter()
                .find(|diagnostic| diagnostic.code == TYPE_MISMATCH)
                .unwrap();
            assert_eq!(
                diagnostic.message,
                format!(
                    "integer `{spelling}` requires operands of the same primitive integer type"
                )
            );
            assert!(diagnostic.labels.iter().any(|label| label
                .message
                .contains(&format!("left operand has type `{left_type}`"))));
            assert!(diagnostic.labels.iter().any(|label| label
                .message
                .contains(&format!("right operand has type `{right_type}`"))));
            assert!(diagnostic
                .notes
                .iter()
                .any(|note| note.contains("`i64`, `u64`, or `u8`")));
        }
    }
}

#[test]
fn valid_operands_are_checked_once_in_source_order() {
    for spelling in ["/", "%"] {
        let source = format!(
            "fn consume(value: u64) -> u64 {{ return value; }} \
             fn invalid() -> u64 {{ return consume(true) {spelling} consume(1); }} \
             fn main() -> i64 {{ return 0; }}"
        );
        let output = check_text(&source);
        assert!(output.hir.is_none());
        let starts: Vec<_> = output
            .diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.code == TYPE_MISMATCH)
            .filter_map(|diagnostic| diagnostic.labels.first())
            .map(|label| label.span.range().start())
            .collect();
        assert_eq!(starts.len(), 2);
        assert!(starts[0] < starts[1]);
    }
}

#[test]
fn source_division_composition_has_deterministic_hir_mir_and_assembly() {
    let output = check_text(concat!(
        "fn produce(value: i64) -> i64 { return value; }\n",
        "fn calculate(value: i64, optional: i64?, count: u64) -> bool {\n",
        "  return ((produce(value) / optional!) % ((i64)(8u >> count))) == value && true;\n",
        "}\n",
        "fn main() -> i64 { return 0; }\n",
    ));
    assert!(!output.has_errors());
    let hir = output.hir.unwrap();
    let hir_dump = dump_hir(&hir);
    assert_eq!(hir_dump, dump_hir(&hir));
    assert!(hir_dump.contains("CheckedIntegerDivision div.i64"));
    assert!(hir_dump.contains("CheckedIntegerDivision rem.i64"));
    assert!(hir_dump.contains("CheckedShift shr.u64"));

    let mir = crate::mir::lower_hir(&hir);
    crate::mir::verify_mir(&mir).unwrap();
    let mir_dump = crate::mir::dump_mir(&mir);
    assert_eq!(mir_dump, crate::mir::dump_mir(&mir));
    assert!(mir_dump.contains("integer-divisor-check div.i64"));
    assert!(mir_dump.contains("integer-divisor-check rem.i64"));
    let assembly = crate::backend::emit_assembly(crate::backend::Target::X86_64SysV, &mir).unwrap();
    assert_eq!(
        assembly,
        crate::backend::emit_assembly(crate::backend::Target::X86_64SysV, &mir).unwrap()
    );
    assert!(assembly.contains("idiv rcx"));
}
