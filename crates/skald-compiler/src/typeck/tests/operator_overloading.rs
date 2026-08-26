use super::*;
use crate::{
    identity::FunctionId,
    resolve::{dump_resolved, resolve_module_graph},
    test_support::{load_module_sources, CANONICAL_OPS_SOURCE},
};

fn resolve_operator_source(source: &str) -> crate::resolve::ResolveOutput {
    let (_workspace, graph) = load_module_sources(
        "app",
        &[("app.ska", source), ("std/ops.ska", CANONICAL_OPS_SOURCE)],
    );
    resolve_module_graph(&graph)
}

fn check_operator_source(source: &str) -> crate::hir::HirProgram {
    let resolved = resolve_operator_source(source);
    assert!(
        resolved.diagnostics.is_empty(),
        "operator source must resolve: {:?}",
        resolved.diagnostics
    );
    let checked = crate::typeck::type_check(&resolved.program);
    assert!(
        checked.diagnostics.is_empty(),
        "operator source must type check: {:?}",
        checked.diagnostics
    );
    checked.hir.expect("valid operator source must produce HIR")
}

#[test]
fn every_value_protocol_selects_once_and_erases_to_an_interface_call() {
    let source = r#"
from std::ops import OpNeg, OpBitNot, OpAdd, OpSub, OpMul, OpDiv, OpRem, OpBitAnd, OpBitOr, OpBitXor, OpShiftLeft, OpShiftRight;

class Number implements OpNeg<i64>, OpBitNot<i64>, OpAdd<Number, i64>, OpSub<Number, i64>, OpMul<Number, i64>, OpDiv<Number, i64>, OpRem<Number, i64>, OpBitAnd<Number, i64>, OpBitOr<Number, i64>, OpBitXor<Number, i64>, OpShiftLeft<Number, i64>, OpShiftRight<Number, i64> {
    init() {}
    fn op_neg() -> i64 { return 1; }
    fn op_bit_not() -> i64 { return 2; }
    fn op_add(ref rhs: Number) -> i64 { return 3; }
    fn op_sub(ref rhs: Number) -> i64 { return 4; }
    fn op_mul(ref rhs: Number) -> i64 { return 5; }
    fn op_div(ref rhs: Number) -> i64 { return 6; }
    fn op_rem(ref rhs: Number) -> i64 { return 7; }
    fn op_bit_and(ref rhs: Number) -> i64 { return 8; }
    fn op_bit_or(ref rhs: Number) -> i64 { return 9; }
    fn op_bit_xor(ref rhs: Number) -> i64 { return 10; }
    fn op_shift_left(ref rhs: Number) -> i64 { return 11; }
    fn op_shift_right(ref rhs: Number) -> i64 { return 12; }
}

fn exercise(ref left: Number, ref right: Number) -> i64 {
    var a: i64 = -left;
    var b: i64 = ~left;
    var c: i64 = left + right;
    var d: i64 = left - right;
    var e: i64 = left * right;
    var f: i64 = left / right;
    var g: i64 = left % right;
    var h: i64 = left & right;
    var i: i64 = left | right;
    var j: i64 = left ^ right;
    var k: i64 = left << right;
    var l: i64 = left >> right;
    return a + b + c + d + e + f + g + h + i + j + k + l;
}

fn main() -> i64 { return 0; }
"#;
    let resolved = resolve_operator_source(source);
    assert!(
        resolved.diagnostics.is_empty(),
        "{:?}",
        resolved.diagnostics
    );
    let resolved_dump = dump_resolved(&resolved.program);
    assert_eq!(resolved_dump.matches("ValueOperatorSelection").count(), 12);

    let checked = crate::typeck::type_check(&resolved.program);
    assert!(checked.diagnostics.is_empty(), "{:?}", checked.diagnostics);
    let hir_dump = dump_hir(&checked.hir.unwrap());
    assert_eq!(hir_dump.matches("InterfaceCall").count(), 12, "{hir_dump}");
    assert!(!hir_dump.contains("ValueOperatorSelection"), "{hir_dump}");
}

#[test]
fn class_output_is_consumed_through_the_ordinary_object_result_path() {
    let hir = check_operator_source(
        r#"
from std::ops import OpAdd;

class Number implements OpAdd<Number, Number> {
    private value: i64;
    init(value: i64) { self.value = value; }
    fn op_add(ref rhs: Number) -> Number { return Number(self.value + rhs.value); }
    fn get() -> i64 { return self.value; }
}

fn main() -> i64 {
    var left: Number = Number(17);
    var right: Number = Number(25);
    var answer: Number = left + right;
    return answer.get();
}
"#,
    );
    let dump = dump_hir(&hir);
    assert!(dump.contains("ObjectCall interface"), "{dump}");
    assert!(dump.contains("ObjectResult"), "{dump}");
}

#[test]
fn inherited_closed_generic_and_exact_interface_receivers_are_eligible() {
    let hir = check_operator_source(
        r#"
from std::ops import OpAdd;

class Base implements OpAdd<Base, i64> {
    init() {}
    fn op_add(ref rhs: Base) -> i64 { return 10; }
}

class Derived extends Base {
    init() { super(); }
}

class Box<T> implements OpAdd<Box<T>, T> {
    private value: T;
    init(value: T) { self.value = value; }
    fn op_add(ref rhs: Box<T>) -> T { return self.value; }
}

fn through_view(ref left: OpAdd<Base, i64>, ref right: Base) -> i64 {
    return left + right;
}

fn main() -> i64 {
    var left: Derived = Derived();
    var right: Derived = Derived();
    var inherited: i64 = left + right;
    var first: Box<i64> = Box<i64>(32);
    var second: Box<i64> = Box<i64>(0);
    var specialized: i64 = first + second;
    return inherited + specialized;
}
"#,
    );
    let dump = dump_hir(&hir);
    assert!(dump.matches("InterfaceCall").count() >= 3, "{dump}");
}

#[test]
fn same_named_methods_and_missing_rhs_applications_do_not_authorize_punctuation() {
    let source = r#"
from std::ops import OpAdd;

class Other { init() {} }
class Pretender {
    init() {}
    fn op_add(ref rhs: Pretender) -> i64 { return 1; }
}
class Number implements OpAdd<Number, i64> {
    init() {}
    fn op_add(ref rhs: Number) -> i64 { return 2; }
}

fn bad_same_name(ref left: Pretender, ref right: Pretender) -> i64 { return left + right; }
fn bad_rhs(ref left: Number, ref right: Other) -> i64 { return left + right; }
fn main() -> i64 { return 0; }
"#;
    let resolved = resolve_operator_source(source);
    assert!(
        resolved.diagnostics.is_empty(),
        "{:?}",
        resolved.diagnostics
    );
    let checked = crate::typeck::type_check(&resolved.program);
    let diagnostics = checked.diagnostics.iter().collect::<Vec<_>>();
    assert_eq!(diagnostics.len(), 2, "{:?}", checked.diagnostics);
    assert!(diagnostics
        .iter()
        .all(|diagnostic| diagnostic.code
            == crate::typeck::program::UNSUPPORTED_OPERATOR_APPLICATION));
}

#[test]
fn expected_results_do_not_select_and_explicit_rhs_casts_do() {
    let source = r#"
from std::ops import OpAdd;

class Base { init() {} }
class Number implements OpAdd<Base, i64> {
    init() {}
    fn op_add(ref rhs: Base) -> i64 { return 7; }
}

fn cast_rhs(ref left: Number, ref right: Obj) -> i64 {
    return left + (Base) right;
}

fn wrong_expected(ref left: Number, ref right: Base) -> u64 {
    return left + right;
}

fn main() -> i64 { return 0; }
"#;
    let resolved = resolve_operator_source(source);
    assert!(
        resolved.diagnostics.is_empty(),
        "{:?}",
        resolved.diagnostics
    );
    let dump = dump_resolved(&resolved.program);
    assert_eq!(dump.matches("ValueOperatorSelection").count(), 2, "{dump}");

    let checked = crate::typeck::type_check(&resolved.program);
    assert_eq!(checked.diagnostics.len(), 1, "{:?}", checked.diagnostics);
    let diagnostic = checked.diagnostics.iter().next().unwrap();
    assert_eq!(diagnostic.code, crate::typeck::program::TYPE_MISMATCH);
}

#[test]
fn primitive_precedence_remains_the_existing_hir_with_reachable_protocols() {
    let source = "from std::ops import OpAdd;\nfn main() -> i64 { return 20 + 22; }\n";
    let resolved = resolve_operator_source(source);
    assert!(
        resolved.diagnostics.is_empty(),
        "{:?}",
        resolved.diagnostics
    );
    assert!(
        !dump_resolved(&resolved.program).contains("ValueOperatorResolution"),
        "primitive punctuation must not consult canonical applications"
    );
    let checked = crate::typeck::type_check(&resolved.program);
    assert!(checked.diagnostics.is_empty(), "{:?}", checked.diagnostics);
    let dump = dump_hir(&checked.hir.unwrap());
    assert!(dump.contains("Binary AddI64"), "{dump}");
    assert!(!dump.contains("InterfaceCall"), "{dump}");
}

#[test]
fn multiple_resolved_applications_are_diagnosed_before_hir_completion() {
    let source = r#"
from std::ops import OpAdd;
class Number implements OpAdd<Number, i64> {
    init() {}
    fn op_add(ref rhs: Number) -> i64 { return 1; }
}
fn add(ref left: Number, ref right: Number) -> i64 { return left + right; }
fn main() -> i64 { return 0; }
"#;
    let mut resolved = resolve_operator_source(source);
    assert!(
        resolved.diagnostics.is_empty(),
        "{:?}",
        resolved.diagnostics
    );
    let definition = resolved
        .program
        .definitions
        .get_mut_for_test(FunctionId::new(0))
        .expect("add must have a resolved body");
    let crate::resolve::ResolvedStatement::Return(returned) = &mut definition.body.statements[0]
    else {
        panic!("expected return statement");
    };
    let Some(crate::resolve::ResolvedExpression::Binary(binary)) = &mut returned.value else {
        panic!("expected selected binary expression");
    };
    let resolution = binary
        .selection
        .as_mut()
        .expect("class operator must retain candidate resolution");
    let duplicate = resolution
        .selected()
        .expect("source must select one canonical application");
    resolution.candidates.push(duplicate);

    let checked = crate::typeck::type_check(&resolved.program);
    assert!(checked.hir.is_none());
    assert_eq!(checked.diagnostics.len(), 1, "{:?}", checked.diagnostics);
    assert_eq!(
        checked.diagnostics.iter().next().unwrap().code,
        crate::typeck::program::AMBIGUOUS_OPERATOR_APPLICATION
    );
}

#[test]
fn operator_selection_is_independent_of_module_source_creation_order() {
    let source = r#"
from std::ops import OpAdd;
class Number implements OpAdd<Number, i64> {
    init() {}
    fn op_add(ref rhs: Number) -> i64 { return 42; }
}
fn add(ref left: Number, ref right: Number) -> i64 { return left + right; }
fn main() -> i64 { return 0; }
"#;
    let sources = [("app.ska", source), ("std/ops.ska", CANONICAL_OPS_SOURCE)];
    let (_first_workspace, first_graph) = load_module_sources("app", &sources);
    let (_second_workspace, second_graph) = load_module_sources("app", &[sources[1], sources[0]]);
    let first = resolve_module_graph(&first_graph);
    let second = resolve_module_graph(&second_graph);
    assert!(first.diagnostics.is_empty(), "{:?}", first.diagnostics);
    assert!(second.diagnostics.is_empty(), "{:?}", second.diagnostics);
    assert_eq!(
        dump_resolved(&first.program),
        dump_resolved(&second.program)
    );

    let first = crate::typeck::type_check(&first.program);
    let second = crate::typeck::type_check(&second.program);
    assert!(first.diagnostics.is_empty(), "{:?}", first.diagnostics);
    assert!(second.diagnostics.is_empty(), "{:?}", second.diagnostics);
    assert_eq!(
        dump_hir(&first.hir.unwrap()),
        dump_hir(&second.hir.unwrap())
    );
}

#[test]
fn punctuation_does_not_make_the_operator_module_reachable() {
    let source = r#"
class Pretender {
    init() {}
    fn op_add(ref rhs: Pretender) -> i64 { return 1; }
}
fn bad(ref left: Pretender, ref right: Pretender) -> i64 { return left + right; }
fn main() -> i64 { return 0; }
"#;
    let (_workspace, graph) = load_module_sources(
        "app",
        &[("app.ska", source), ("std/ops.ska", CANONICAL_OPS_SOURCE)],
    );
    assert_eq!(graph.modules().len(), 1);
    let resolved = resolve_module_graph(&graph);
    assert!(
        resolved.diagnostics.is_empty(),
        "{:?}",
        resolved.diagnostics
    );
    assert!(resolved.program.operator_language_item.is_none());
    let checked = crate::typeck::type_check(&resolved.program);
    assert_eq!(checked.diagnostics.len(), 1, "{:?}", checked.diagnostics);
    assert_eq!(
        checked.diagnostics.iter().next().unwrap().code,
        crate::typeck::program::UNSUPPORTED_OPERATOR_APPLICATION
    );
}
