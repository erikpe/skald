use super::*;
use crate::hir::{HirIntegerType, HirShiftDirection};

const INTEGER_TYPES: &[(HirIntegerType, &str, &str)] = &[
    (HirIntegerType::I64, "i64", "-8"),
    (HirIntegerType::U64, "u64", "8u"),
    (HirIntegerType::U8, "u8", "8u8"),
];

#[test]
fn selects_both_shift_directions_for_every_integer_left_type() {
    for &(integer, type_name, left) in INTEGER_TYPES {
        for (direction, spelling) in [
            (HirShiftDirection::Left, "<<"),
            (HirShiftDirection::Right, ">>"),
        ] {
            let source = format!(
                "fn shift() -> {type_name} {{ return {left} {spelling} 1u; }} \
                 fn main() -> i64 {{ return 0; }}"
            );
            let output = check_text(&source);
            assert!(!output.has_errors(), "{source}");
            let hir = output.hir.unwrap();
            let expression = returned_expression(hir.definitions.get(FunctionId::new(0)).unwrap());
            let HirExpressionKind::CheckedShift(shift) = &expression.kind else {
                panic!("expected selected checked shift");
            };
            assert_eq!(shift.operation.direction, direction);
            assert_eq!(shift.operation.left, integer);
            assert_eq!(shift.left.ty, integer.operand_type());
            assert_eq!(shift.count.ty, Type::U64);
            assert_eq!(expression.ty, integer.operand_type());
        }
    }
}

#[test]
fn rejects_every_non_u64_count_without_conversion() {
    for (declarations, count, actual_type) in [
        ("", "1", "i64"),
        ("", "1u8", "u8"),
        ("", "true", "bool"),
        ("", "1.0", "f64"),
        ("fn notify() -> unit {} ", "notify()", "unit"),
        ("", "value", "i64?"),
        ("", "values", "array a0"),
    ] {
        let parameters = match count {
            "value" => "value: i64?",
            "values" => "values: i64[]",
            _ => "",
        };
        for spelling in ["<<", ">>"] {
            let source = format!(
                "{declarations}fn invalid({parameters}) -> i64 {{ return 1 {spelling} {count}; }} \
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
                    "shift `{spelling}` requires a primitive integer left operand and a `u64` count"
                )
            );
            assert!(diagnostic.labels.iter().any(|label| label
                .message
                .contains(&format!("count operand has type `{actual_type}`"))));
        }
    }
}

#[test]
fn rejects_noninteger_left_families_with_actual_types() {
    for (declaration, parameter, actual_type) in [
        ("", "value: bool", "bool"),
        ("", "value: f64", "f64"),
        ("", "value: i64?", "i64?"),
        ("", "value: i64[]", "array a0"),
        ("class Item { init() {} } ", "ref value: Item", "class c0"),
        (
            "class Item { init() {} } ",
            "value: shared Item",
            "shared class c0",
        ),
        ("", "ref value: Obj", "Obj"),
    ] {
        let source = format!(
            "{declaration}fn invalid({parameter}) -> i64 {{ return value << 1u; }} \
             fn main() -> i64 {{ return 0; }}"
        );
        let output = check_text(&source);
        assert!(output.hir.is_none(), "{source}");
        let diagnostic = output
            .diagnostics
            .iter()
            .find(|diagnostic| diagnostic.code == TYPE_MISMATCH)
            .unwrap();
        assert!(diagnostic.labels.iter().any(|label| label
            .message
            .contains(&format!("left operand has type `{actual_type}`"))));
        assert!(diagnostic
            .notes
            .iter()
            .any(|note| note.contains("count type is exactly `u64`")));
    }
}

#[test]
fn valid_shift_operands_are_checked_once_in_source_order() {
    let output = check_text(concat!(
        "fn consume(value: u64) -> u64 { return value; }\n",
        "fn invalid() -> u64 { return consume(true) << consume(1); }\n",
        "fn main() -> i64 { return 0; }\n",
    ));
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

#[test]
fn source_shift_composition_has_deterministic_hir_and_mir_dumps() {
    let output = check_text(concat!(
        "fn mix(left: u8, count: u64) -> bool {\n",
        "  return ((left + 1u8 << count) >> 1u) == left && true;\n",
        "}\n",
        "fn main() -> i64 { return 0; }\n",
    ));
    assert!(!output.has_errors());
    let hir = output.hir.unwrap();
    let hir_dump = dump_hir(&hir);
    assert_eq!(hir_dump, dump_hir(&hir));
    assert!(hir_dump.contains("CheckedShift shl.u8 count=u64 width=8"));
    assert!(hir_dump.contains("CheckedShift shr.u8 count=u64 width=8"));

    let mir = crate::mir::lower_hir(&hir);
    crate::mir::verify_mir(&mir).unwrap();
    let mir_dump = crate::mir::dump_mir(&mir);
    assert_eq!(mir_dump, crate::mir::dump_mir(&mir));
    assert!(mir_dump.contains("shift-count-check shl.u8"));
    assert!(mir_dump.contains("shift-count-check shr.u8"));
    assert!(mir_dump.contains("terminate shift-count-out-of-range"));
}
