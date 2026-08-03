use super::*;
use crate::{
    hir::{HirCallArgument, HirExpressionKind, HirPrimitiveStorage},
    identity::{ClassId, MethodId, StaticFieldId},
    resolve::resolve_module_graph,
    test_support::load_module_sources_with_standard_library,
    typeck::{INVALID_STATIC_FIELD_TYPE, STATIC_FIELD_USE_UNAVAILABLE},
};

#[test]
fn lowers_primitive_reads_writes_and_aliases_to_identity_based_places() {
    let output = check_text(concat!(
        "fn observe(ref value: i64) -> i64 { return value; }\n",
        "fn increment(mut ref value: i64) -> unit { value = value + 1; }\n",
        "class State {\n",
        "  static signed: i64;\n",
        "  static unsigned: u64;\n",
        "  static byte: u8;\n",
        "  static float: f64;\n",
        "  static flag: bool;\n",
        "  init() {}\n",
        "  static fn exercise() -> i64 {\n",
        "    State.signed = observe(State.signed) + 1;\n",
        "    State.unsigned = (u64) State.signed;\n",
        "    State.byte = (u8) State.unsigned;\n",
        "    State.float = (f64) State.byte;\n",
        "    State.flag = State.signed < 10 && !State.flag;\n",
        "    increment(State.signed);\n",
        "    return State.signed;\n",
        "  }\n",
        "}\n",
        "fn main() -> i64 { return State.exercise(); }\n",
    ));

    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    let hir = output.hir.unwrap();
    let class = hir.class(ClassId::new(0)).unwrap();
    assert_eq!(
        class
            .static_fields
            .iter()
            .map(|field| field.id)
            .collect::<Vec<_>>(),
        (0..5)
            .map(|index| StaticFieldId::new(class.id, index))
            .collect::<Vec<_>>()
    );

    let method = hir
        .member_definition(MethodId::new(class.id, 0).into())
        .unwrap();
    let static_assignments = method
        .body
        .statements
        .iter()
        .filter_map(|statement| match statement {
            HirStatement::PrimitiveAssignment(assignment) => {
                let HirPrimitiveStorage::Static(place) = assignment.destination.storage else {
                    return None;
                };
                Some(place.field)
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(
        static_assignments,
        (0..5)
            .map(|index| StaticFieldId::new(class.id, index))
            .collect::<Vec<_>>()
    );

    let HirStatement::Call(increment) = &method.body.statements[5] else {
        panic!("expected primitive alias call");
    };
    let HirExpressionKind::DirectCall { arguments, .. } = &increment.call.kind else {
        panic!("expected direct call");
    };
    let HirCallArgument::PrimitivePlace(place) = &arguments[0] else {
        panic!("expected primitive-place alias argument");
    };
    assert_eq!(
        place.storage,
        HirPrimitiveStorage::Static(crate::hir::HirStaticPlace {
            field: StaticFieldId::new(class.id, 0),
            span: place.span,
        })
    );

    let dump = dump_hir(&hir);
    assert!(dump.contains("StaticField c0:static0 \"signed\" : i64"));
    assert!(dump.contains("PrimitiveStaticAssignment c0:static0"));
    assert!(dump.contains("StaticRead c0:static0 : i64"));
    assert!(dump.contains("PrimitivePlaceArgument static c0:static0"));
}

#[test]
fn accepts_the_complete_zero_default_declaration_set() {
    let output = check_text(concat!(
        "class Item { init() {} }\n",
        "class State {\n",
        "  static signed: i64; static unsigned: u64; static byte: u8;\n",
        "  static float: f64; static flag: bool;\n",
        "  static maybe_signed: i64?; static maybe_item: Item?;\n",
        "  static maybe_owner: shared? Item; static items: Item[];\n",
        "  init() {}\n",
        "}\n",
        "fn main() -> i64 { return 0; }\n",
    ));

    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    assert_eq!(
        output
            .hir
            .unwrap()
            .class(ClassId::new(1))
            .unwrap()
            .static_fields
            .len(),
        9
    );
}

#[test]
fn rejects_each_non_zero_default_type_at_its_declaration() {
    let output = check_text(concat!(
        "interface View { fn read() -> i64; }\n",
        "class Item { init() {} }\n",
        "class Invalid {\n",
        "  static item: Item;\n",
        "  static owner: shared Item;\n",
        "  static owner_array: shared Item[];\n",
        "  static view: View;\n",
        "  static erased: Obj;\n",
        "  static empty: unit;\n",
        "  init() {}\n",
        "}\n",
        "fn main() -> i64 { return 0; }\n",
    ));

    assert!(output.hir.is_none());
    let diagnostics = output
        .diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.code == INVALID_STATIC_FIELD_TYPE)
        .collect::<Vec<_>>();
    assert_eq!(diagnostics.len(), 6, "{:?}", output.diagnostics);
    assert!(diagnostics.iter().all(|diagnostic| diagnostic
        .labels
        .iter()
        .any(|label| label.message.contains("all-zero live value"))));
}

#[test]
fn rejects_owning_static_uses_at_the_primitive_source_boundary() {
    let output = check_text(concat!(
        "class Item { init() {} }\n",
        "class State { static items: Item[]; init() {} }\n",
        "fn count() -> u64 { return State.items.len(); }\n",
        "fn main() -> i64 { return 0; }\n",
    ));

    assert!(output.hir.is_none());
    assert_eq!(
        output
            .diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.code == STATIC_FIELD_USE_UNAVAILABLE)
            .count(),
        1
    );
}

#[test]
fn preserves_source_argument_order_without_a_static_receiver() {
    let output = check_text(concat!(
        "fn middle() -> i64 { return 2; }\n",
        "fn combine(first: i64, second: i64, third: i64) -> i64 {\n",
        "  return first + second + third;\n",
        "}\n",
        "class State { static first: i64; static third: i64; init() {} }\n",
        "fn main() -> i64 { return combine(State.first, middle(), State.third); }\n",
    ));

    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    let hir = output.hir.unwrap();
    let HirExpressionKind::DirectCall { arguments, .. } =
        &returned_expression(hir.definitions.get(hir.entry_function).unwrap()).kind
    else {
        panic!("expected direct call");
    };
    assert!(matches!(
        &arguments[0],
        HirCallArgument::Value(HirExpression {
            kind: HirExpressionKind::StaticRead(_),
            ..
        })
    ));
    assert!(matches!(
        &arguments[1],
        HirCallArgument::Value(HirExpression {
            kind: HirExpressionKind::DirectCall { .. },
            ..
        })
    ));
    assert!(matches!(
        &arguments[2],
        HirCallArgument::Value(HirExpression {
            kind: HirExpressionKind::StaticRead(_),
            ..
        })
    ));
}

#[test]
fn primitive_statics_compose_with_bit_intrinsics_and_io_scalar_arguments() {
    let (_workspace, graph) = load_module_sources_with_standard_library(
        "app",
        &[(
            "app.ska",
            concat!(
                "import std::f64; import std::io;\n",
                "class State {\n",
                "  static signed: i64; static unsigned: u64; static byte: u8;\n",
                "  static float: f64; static flag: bool; init() {}\n",
                "}\n",
                "fn main() -> i64 {\n",
                "  State.unsigned = std::f64::to_bits(State.float);\n",
                "  State.float = std::f64::from_bits(State.unsigned);\n",
                "  std::io::println_i64(State.signed); std::io::println_u64(State.unsigned);\n",
                "  std::io::println_u8(State.byte); std::io::println_f64(State.float);\n",
                "  std::io::println_bool(State.flag);\n",
                "  return 0;\n",
                "}\n",
            ),
        )],
    );
    let resolved = resolve_module_graph(&graph);
    assert!(!resolved.has_errors(), "{:?}", resolved.diagnostics);
    let output = type_check(&resolved.program);
    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    let dump = dump_hir(&output.hir.unwrap());
    assert!(dump.contains("bit_reinterpretation f64.u64"));
    assert!(dump.contains("bit_reinterpretation u64.f64"));
    assert_eq!(dump.matches("StaticRead").count(), 7);
}

#[test]
fn primitive_statics_reuse_operator_and_control_flow_semantics() {
    let output = check_text(concat!(
        "class State {\n",
        "  static signed: i64; static unsigned: u64; static byte: u8;\n",
        "  static flag: bool; init() {}\n",
        "}\n",
        "fn main() -> i64 {\n",
        "  State.signed = (State.signed / 1) % 1;\n",
        "  State.signed = (State.signed & 7) | (State.signed ^ 2);\n",
        "  State.signed = (State.signed << State.unsigned) >> State.unsigned;\n",
        "  State.unsigned = (State.unsigned / 1u) % 1u;\n",
        "  State.byte = (State.byte << State.unsigned) >> State.unsigned;\n",
        "  State.flag = State.flag == true;\n",
        "  if (State.flag && State.signed <= 0) { State.signed = 1; }\n",
        "  while (State.flag || false) { State.flag = false; }\n",
        "  return State.signed;\n",
        "}\n",
    ));

    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    let dump = dump_hir(&output.hir.unwrap());
    assert!(dump.contains("CheckedIntegerDivision"));
    assert!(dump.contains("CheckedShift"));
    assert!(dump.contains("Logical"));
    assert!(dump.contains("Comparison"));
}
