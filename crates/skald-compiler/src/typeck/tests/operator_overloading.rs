use super::*;
use crate::{
    identity::FunctionId,
    resolve::{
        dump_resolved, resolve_module_graph, CanonicalOperatorProtocol, ResolvedBinaryOperator,
        ResolvedUnaryOperator,
    },
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
fn generic_operator_bound_closes_to_primitive_intrinsic() {
    let resolved = resolve_operator_source(
        "from std::ops import OpAdd;\n\
         class Adder<T> where T: OpAdd<T, T> {\n\
           init() {}\n\
           fn punctuation(ref left: T, ref right: T) -> T { return left + right; }\n\
           fn manual(ref left: T, ref right: T) -> T { return left.op_add(right); }\n\
         }\n\
         fn use(ref value: Adder<u64>) -> unit {}\n\
         fn answer() -> u64 {\n\
           var adder: Adder<u64> = Adder<u64>();\n\
           return adder.punctuation(17u, 25u);\n\
         }\n\
         fn main() -> i64 { return 0; }\n",
    );
    assert!(
        resolved.diagnostics.is_empty(),
        "{:?}",
        resolved.diagnostics
    );
    let resolved_dump = dump_resolved(&resolved.program);
    assert!(
        resolved_dump.contains("ClosedOperatorSelection 0 primitive-intrinsic AddU64"),
        "{resolved_dump}"
    );
    assert!(
        resolved_dump.contains("ClosedBoundSelection 1 primitive-intrinsic AddU64"),
        "{resolved_dump}"
    );

    let checked = crate::typeck::type_check(&resolved.program);
    assert!(checked.diagnostics.is_empty(), "{:?}", checked.diagnostics);
    let hir = crate::hir::dump_hir(&checked.hir.expect("valid generic operators produce HIR"));
    assert_eq!(hir.matches("AddU64").count(), 2, "{hir}");
    assert!(!hir.contains("InterfaceCall"), "{hir}");
}

#[test]
fn generic_operator_bound_closes_to_class_witness_without_reselection() {
    let resolved = resolve_operator_source(
        "from std::ops import OpAdd;\n\
         class Value implements OpAdd<Value, Value> {\n\
           init() {}\n\
           fn op_add(ref rhs: Value) -> Value { return Value(); }\n\
         }\n\
         class Adder<T> where T: OpAdd<T, T> {\n\
           init() {}\n\
           fn add(ref left: T, ref right: T) -> T { return left + right; }\n\
           fn manual(ref left: T, ref right: T) -> T { return left.op_add(right); }\n\
         }\n\
         fn use(ref value: Adder<Value>) -> unit {}\n\
         fn main() -> i64 { return 0; }\n",
    );
    assert!(
        resolved.diagnostics.is_empty(),
        "{:?}",
        resolved.diagnostics
    );
    let resolved_dump = dump_resolved(&resolved.program);
    assert!(
        resolved_dump.contains("ClosedOperatorSelection 0 class-witness"),
        "{resolved_dump}"
    );
    assert!(
        resolved_dump.contains("ClosedBoundSelection 1 class-witness"),
        "{resolved_dump}"
    );
    let checked = crate::typeck::type_check(&resolved.program);
    assert!(checked.diagnostics.is_empty(), "{:?}", checked.diagnostics);
    let hir = crate::hir::dump_hir(&checked.hir.expect("valid generic operators produce HIR"));
    assert_eq!(hir.matches("ObjectCall interface").count(), 2, "{hir}");
}

#[test]
fn generic_operator_selection_supports_structural_rhs_and_output() {
    let hir = check_operator_source(
        "from std::ops import OpAdd;\n\
         class Right { init() {} }\n\
         class Output { init() {} }\n\
         class Left implements OpAdd<Right, Output> {\n\
           init() {}\n\
           fn op_add(ref rhs: Right) -> Output { return Output(); }\n\
         }\n\
         class Apply<L, R, O> where L: OpAdd<R, O> {\n\
           init() {}\n\
           fn apply(ref left: L, ref right: R) -> O { return left + right; }\n\
         }\n\
         fn use(ref value: Apply<Left, Right, Output>) -> unit {}\n\
         fn main() -> i64 { return 0; }\n",
    );
    let dump = crate::hir::dump_hir(&hir);
    assert!(dump.contains("ObjectCall interface"), "{dump}");
    assert!(dump.contains("ObjectResult"), "{dump}");
}

#[test]
fn generic_operator_bounds_cover_unary_algebraic_equality_and_ordering() {
    let resolved = resolve_operator_source(
        "from std::ops import OpNeg, OpBitNot, OpEq, OpLess, OpDiv;\n\
         class Negate<T> where T: OpNeg<T> {\n\
           init() {}\n\
           fn apply(ref value: T) -> T { return -value; }\n\
         }\n\
         class Complement<T> where T: OpBitNot<T> {\n\
           init() {}\n\
           fn apply(ref value: T) -> T { return ~value; }\n\
         }\n\
         class Equal<T> where T: OpEq<T> {\n\
           init() {}\n\
           fn apply(ref left: T, ref right: T) -> bool { return left == right; }\n\
         }\n\
         class Less<T> where T: OpLess<T> {\n\
           init() {}\n\
           fn apply(ref left: T, ref right: T) -> bool { return left < right; }\n\
         }\n\
         class Divide<T> where T: OpDiv<T, T> {\n\
           init() {}\n\
           fn apply(ref left: T, ref right: T) -> T { return left / right; }\n\
         }\n\
         fn use(ref n: Negate<i64>, ref c: Complement<u64>, ref e: Equal<u64>, ref l: Less<f64>, ref d: Divide<u64>) -> unit {}\n\
         fn main() -> i64 { return 0; }\n",
    );
    assert!(
        resolved.diagnostics.is_empty(),
        "{:?}",
        resolved.diagnostics
    );
    let dump = dump_resolved(&resolved.program);
    for operation in [
        "NegateI64",
        "BitwiseComplementU64",
        "EqualU64",
        "LessF64",
        "DivideU64",
    ] {
        assert!(dump.contains(operation), "missing {operation} in:\n{dump}");
    }
    let checked = crate::typeck::type_check(&resolved.program);
    assert!(checked.diagnostics.is_empty(), "{:?}", checked.diagnostics);
}

#[test]
fn generic_operator_ambiguity_and_missing_bound_fail_at_definition_site() {
    let ambiguous = resolve_operator_source(
        "from std::ops import OpAdd;\n\
         class Base { init() {} }\n\
         class Ambiguous<T> where T: OpAdd<Base, T>, T: OpAdd<Obj, T> {\n\
           init() {}\n\
           fn add(ref left: T, ref right: Base) -> T { return left + right; }\n\
         }\n\
         fn main() -> i64 { return 0; }\n",
    );
    assert!(ambiguous.diagnostics.iter().any(|diagnostic| {
        diagnostic.code == crate::resolve::AMBIGUOUS_GENERIC_OPERATOR_APPLICATION
    }));

    let missing = resolve_operator_source(
        "from std::ops import OpAdd, OpSub;\n\
         class Missing<T> where T: OpSub<T, T> {\n\
           init() {}\n\
           fn add(ref left: T, ref right: T) -> T { return left + right; }\n\
         }\n\
         fn main() -> i64 { return 0; }\n",
    );
    assert!(missing.diagnostics.iter().any(|diagnostic| {
        diagnostic.code == crate::resolve::UNSUPPORTED_GENERIC_OPERATOR_APPLICATION
    }));

    let foreign = resolve_operator_source(
        "interface FakeAdd<R, O> { fn op_add(ref rhs: R) -> O; }\n\
         class Foreign<T> where T: FakeAdd<T, T> {\n\
           init() {}\n\
           fn add(ref left: T, ref right: T) -> T { return left + right; }\n\
         }\n\
         fn main() -> i64 { return 0; }\n",
    );
    assert!(foreign.diagnostics.iter().any(|diagnostic| {
        diagnostic.code == crate::resolve::UNSUPPORTED_GENERIC_OPERATOR_APPLICATION
    }));
}

#[test]
fn generic_operator_rhs_incompatibility_has_ordered_bound_evidence() {
    let resolved = resolve_operator_source(
        "from std::ops import OpAdd;\n\
         class Apply<T, R> where T: OpAdd<T, T> {\n\
           init() {}\n\
           fn add(ref left: T, ref right: R) -> T { return left + right; }\n\
         }\n\
         fn main() -> i64 { return 0; }\n",
    );
    let diagnostic = resolved
        .diagnostics
        .iter()
        .find(|diagnostic| diagnostic.code == crate::resolve::INCOMPATIBLE_GENERIC_OPERATOR_RHS)
        .expect("incompatible structural RHS must have a focused diagnostic");
    assert_eq!(diagnostic.labels.len(), 2, "{diagnostic:?}");
    assert_eq!(
        diagnostic.labels[1].message,
        "candidate operator bound with incompatible `Rhs` declared here"
    );
}

#[test]
fn unsupported_primitive_operator_bound_fails_without_a_fake_witness() {
    let resolved = resolve_operator_source(
        "from std::ops import OpRem;\n\
         class Remainder<T> where T: OpRem<T, T> {\n\
           init() {}\n\
           fn apply(ref left: T, ref right: T) -> T { return left % right; }\n\
         }\n\
         fn use(ref value: Remainder<f64>) -> unit {}\n\
         fn main() -> i64 { return 0; }\n",
    );
    assert!(resolved
        .diagnostics
        .iter()
        .any(|diagnostic| { diagnostic.code == crate::resolve::UNSATISFIED_GENERIC_REQUIREMENT }));
    assert!(!dump_resolved(&resolved.program).contains("class-witness"));
}

fn primitive_application(
    protocol: CanonicalOperatorProtocol,
    rhs: Option<crate::resolve::ResolvedPrimitiveType>,
    output: crate::resolve::ResolvedPrimitiveType,
) -> String {
    let name = protocol.interface_name();
    match protocol.shape() {
        crate::resolve::CanonicalOperatorProtocolShape::Unary => {
            format!("{name}<{}>", output.name())
        }
        crate::resolve::CanonicalOperatorProtocolShape::Predicate => format!(
            "{name}<{}>",
            rhs.expect("predicate evidence has an RHS").name()
        ),
        crate::resolve::CanonicalOperatorProtocolShape::Binary => format!(
            "{name}<{}, {}>",
            rhs.expect("binary evidence has an RHS").name(),
            output.name()
        ),
    }
}

fn protocol_spelling(protocol: CanonicalOperatorProtocol) -> &'static str {
    match protocol {
        CanonicalOperatorProtocol::Neg => "-",
        CanonicalOperatorProtocol::BitNot => "~",
        CanonicalOperatorProtocol::Eq => "==",
        CanonicalOperatorProtocol::Less => "<",
        CanonicalOperatorProtocol::LessEq => "<=",
        CanonicalOperatorProtocol::Greater => ">",
        CanonicalOperatorProtocol::GreaterEq => ">=",
        CanonicalOperatorProtocol::Add => "+",
        CanonicalOperatorProtocol::Sub => "-",
        CanonicalOperatorProtocol::Mul => "*",
        CanonicalOperatorProtocol::Div => "/",
        CanonicalOperatorProtocol::Rem => "%",
        CanonicalOperatorProtocol::BitAnd => "&",
        CanonicalOperatorProtocol::BitOr => "|",
        CanonicalOperatorProtocol::BitXor => "^",
        CanonicalOperatorProtocol::ShiftLeft => "<<",
        CanonicalOperatorProtocol::ShiftRight => ">>",
    }
}

#[test]
fn every_primitive_operator_cell_satisfies_its_exact_canonical_bound() {
    let mut source = String::from("import std::ops;\n");
    for (index, evidence) in crate::resolve::primitive_operator_registry()
        .iter()
        .copied()
        .enumerate()
    {
        let application =
            primitive_application(evidence.protocol(), evidence.rhs(), evidence.output());
        let receiver = evidence.receiver().name();
        source.push_str(&format!(
            "class Evidence{index}<T> where T: std::ops::{application} {{ init() {{}} }}\n\
             fn use{index}(ref value: Evidence{index}<{receiver}>) -> unit {{}}\n"
        ));
    }
    source.push_str("fn main() -> i64 { return 0; }\n");

    let resolved = resolve_operator_source(&source);
    assert!(
        resolved.diagnostics.is_empty(),
        "all registry cells must satisfy their bounds: {:?}",
        resolved.diagnostics
    );
    let dump = dump_resolved(&resolved.program);
    assert_eq!(dump.matches(" canonical Op").count(), 60, "{dump}");
    assert!(
        dump.contains("i64 canonical OpShiftLeft<u64, i64>"),
        "{dump}"
    );
    assert!(dump.contains("f64 canonical OpEq<f64>"), "{dump}");

    let checked = crate::typeck::type_check(&resolved.program);
    assert!(checked.diagnostics.is_empty(), "{:?}", checked.diagnostics);
    let hir_dump = dump_hir(&checked.hir.unwrap());
    assert!(!hir_dump.contains("Conformances"), "{hir_dump}");
}

#[test]
fn primitive_registry_matches_the_complete_direct_operation_matrix() {
    let mut source = String::new();
    for (index, evidence) in crate::resolve::primitive_operator_registry()
        .iter()
        .copied()
        .enumerate()
    {
        let receiver = evidence.receiver().name();
        let output = evidence.output().name();
        let spelling = protocol_spelling(evidence.protocol());
        match evidence.rhs() {
            None => source.push_str(&format!(
                "fn direct{index}(left: {receiver}) -> {output} {{ return {spelling}left; }}\n"
            )),
            Some(rhs) => source.push_str(&format!(
                "fn direct{index}(left: {receiver}, right: {}) -> {output} {{ return left {spelling} right; }}\n",
                rhs.name()
            )),
        }
    }
    source.push_str("fn main() -> i64 { return 0; }\n");

    let resolved = resolve_operator_source(&source);
    assert!(
        resolved.diagnostics.is_empty(),
        "{:?}",
        resolved.diagnostics
    );
    let checked = crate::typeck::type_check(&resolved.program);
    assert!(checked.diagnostics.is_empty(), "{:?}", checked.diagnostics);
    let dump = dump_hir(&checked.hir.unwrap());
    assert!(!dump.contains("InterfaceCall"), "{dump}");
    assert_eq!(dump.matches("Unary ").count(), 5, "{dump}");
    assert_eq!(dump.matches("Comparison ").count(), 21, "{dump}");
    assert_eq!(dump.matches("Binary ").count(), 22, "{dump}");
    assert_eq!(dump.matches("CheckedIntegerDivision ").count(), 6, "{dump}");
    assert_eq!(dump.matches("CheckedShift ").count(), 6, "{dump}");
}

#[test]
fn canonical_primitive_evidence_also_satisfies_generic_interface_bounds() {
    let resolved = resolve_operator_source(
        r#"
import std::ops;
interface Envelope<T> where T: std::ops::OpAdd<T, T> {}
fn use(ref value: Envelope<u64>) -> unit {}
fn main() -> i64 { return 0; }
"#,
    );
    assert!(
        resolved.diagnostics.is_empty(),
        "{:?}",
        resolved.diagnostics
    );
    let checked = crate::typeck::type_check(&resolved.program);
    assert!(checked.diagnostics.is_empty(), "{:?}", checked.diagnostics);
}

#[test]
fn primitive_evidence_rejects_unsupported_wrong_and_noncanonical_bounds() {
    let source = r#"
import std::ops;

interface OpAdd<Rhs, Output> { fn op_add(ref rhs: Rhs) -> Output; }
interface Marker {}

class Unsupported<T> where T: std::ops::OpRem<T, T> { init() {} }
class WrongRhs<T> where T: std::ops::OpAdd<i64, T> { init() {} }
class WrongOutput<T> where T: std::ops::OpAdd<T, bool> { init() {} }
class Foreign<T> where T: OpAdd<T, T> { init() {} }
class Unrelated<T> where T: Marker { init() {} }

fn unsupported(ref value: Unsupported<f64>) -> unit {}
fn wrong_rhs(ref value: WrongRhs<u64>) -> unit {}
fn wrong_output(ref value: WrongOutput<u64>) -> unit {}
fn foreign(ref value: Foreign<u64>) -> unit {}
fn unrelated(ref value: Unrelated<u64>) -> unit {}
fn main() -> i64 { return 0; }
"#;
    let resolved = resolve_operator_source(source);
    let failures = resolved
        .diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.code == crate::resolve::UNSATISFIED_GENERIC_REQUIREMENT)
        .collect::<Vec<_>>();
    assert_eq!(failures.len(), 5, "{:?}", resolved.diagnostics);
    assert_eq!(
        failures
            .iter()
            .filter(|failure| failure.labels.iter().any(|label| {
                label.message.contains(
                    "has no compiler-provided evidence for the exact canonical application",
                )
            }))
            .count(),
        3
    );
    assert!(failures.iter().all(|failure| failure
        .notes
        .iter()
        .any(|note| { note.contains("exact canonical operator application") })));
}

#[test]
fn primitive_evidence_does_not_create_members_views_or_type_test_relations() {
    let cases = [
        r#"
from std::ops import OpAdd;
fn invalid(value: u64) -> u64 { return value.op_add(value); }
fn main() -> i64 { return 0; }
"#,
        r#"
from std::ops import OpAdd;
fn take(ref value: OpAdd<u64, u64>) -> unit {}
fn invalid(value: u64) -> unit { take(value); }
fn main() -> i64 { return 0; }
"#,
        r#"
from std::ops import OpAdd;
fn invalid(value: u64) -> bool { return value is OpAdd<u64, u64>; }
fn main() -> i64 { return 0; }
"#,
    ];

    for source in cases {
        let resolved = resolve_operator_source(source);
        if resolved.diagnostics.is_empty() {
            let checked = crate::typeck::type_check(&resolved.program);
            assert!(
                checked.hir.is_none(),
                "primitive view unexpectedly accepted: {source}"
            );
            assert!(!checked.diagnostics.is_empty(), "{source}");
        } else {
            assert!(resolved.program.classes.is_empty(), "{source}");
        }
    }
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
    assert_eq!(resolved_dump.matches("OperatorSelection").count(), 12);

    let checked = crate::typeck::type_check(&resolved.program);
    assert!(checked.diagnostics.is_empty(), "{:?}", checked.diagnostics);
    let hir_dump = dump_hir(&checked.hir.unwrap());
    assert_eq!(hir_dump.matches("InterfaceCall").count(), 12, "{hir_dump}");
    assert!(!hir_dump.contains("OperatorSelection"), "{hir_dump}");
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
fn operator_outputs_reuse_every_ordinary_result_capability() {
    let hir = check_operator_source(
        r#"
from std::ops import OpAdd, OpNeg;

class Product {
    value: i64;
    init(value: i64) { self.value = value; }
}
class Box<T> {
    value: T;
    init(value: T) { self.value = value; }
}
fn increment(value: i64) -> i64 { return value + 1; }

class Signed implements OpAdd<Signed, i64> {
    init() {}
    fn op_add(ref rhs: Signed) -> i64 { return 1; }
}
class Unsigned implements OpAdd<Unsigned, u64> {
    init() {}
    fn op_add(ref rhs: Unsigned) -> u64 { return 2u; }
}
class Byte implements OpAdd<Byte, u8> {
    init() {}
    fn op_add(ref rhs: Byte) -> u8 { return 3u8; }
}
class Floating implements OpAdd<Floating, f64> {
    init() {}
    fn op_add(ref rhs: Floating) -> f64 { return 4.5; }
}
class Flag implements OpAdd<Flag, bool> {
    init() {}
    fn op_add(ref rhs: Flag) -> bool { return true; }
}
class ObjectFactory implements OpAdd<ObjectFactory, Product> {
    init() {}
    fn op_add(ref rhs: ObjectFactory) -> Product { return Product(6); }
}
class SharedFactory implements OpAdd<SharedFactory, shared Product> {
    init() {}
    fn op_add(ref rhs: SharedFactory) -> shared Product { return new Product(7); }
}
class OptionalFactory implements OpAdd<OptionalFactory, i64?> {
    init() {}
    fn op_add(ref rhs: OptionalFactory) -> i64? { return 8; }
}
class ArrayFactory implements OpAdd<ArrayFactory, i64[]> {
    init() {}
    fn op_add(ref rhs: ArrayFactory) -> i64[] { return i64[]{9}; }
}
class FunctionFactory implements OpAdd<FunctionFactory, fn(i64) -> i64> {
    init() {}
    fn op_add(ref rhs: FunctionFactory) -> fn(i64) -> i64 { return increment; }
}
class GenericFactory implements OpAdd<GenericFactory, Box<i64>> {
    init() {}
    fn op_add(ref rhs: GenericFactory) -> Box<i64> { return Box<i64>(10); }
}
class UnaryFactory implements OpNeg<Product> {
    init() {}
    fn op_neg() -> Product { return Product(11); }
}

fn consume(value: i64) -> i64 { return value; }
fn returned(ref left: ObjectFactory, ref right: ObjectFactory) -> Product {
    return left + right;
}
fn returned_shared(ref left: SharedFactory, ref right: SharedFactory) -> shared Product {
    return left + right;
}
fn returned_optional(ref left: OptionalFactory, ref right: OptionalFactory) -> i64? {
    return left + right;
}
fn returned_array(ref left: ArrayFactory, ref right: ArrayFactory) -> i64[] {
    return left + right;
}
fn returned_function(ref left: FunctionFactory, ref right: FunctionFactory) -> fn(i64) -> i64 {
    return left + right;
}
fn returned_generic(ref left: GenericFactory, ref right: GenericFactory) -> Box<i64> {
    return left + right;
}

fn main() -> i64 {
    var signed: i64 = consume(Signed() + Signed()) + 10;
    var unsigned: u64 = Unsigned() + Unsigned();
    var byte: u8 = Byte() + Byte();
    var floating: f64 = Floating() + Floating();
    var flag: bool = Flag() + Flag();
    var product: Product = returned(ObjectFactory(), ObjectFactory());
    var owner: shared Product = returned_shared(SharedFactory(), SharedFactory());
    var maybe: i64? = returned_optional(OptionalFactory(), OptionalFactory());
    var values: i64[] = returned_array(ArrayFactory(), ArrayFactory());
    var callback: fn(i64) -> i64 = returned_function(FunctionFactory(), FunctionFactory());
    var boxed: Box<i64> = returned_generic(GenericFactory(), GenericFactory());
    var unary_product: Product = -UnaryFactory();
    if (!flag || floating < 4.0) { return 1; }
    return signed + (i64) unsigned + (i64) byte + product.value + owner->value
        + maybe! + values[0] + callback(1) + boxed.value + unary_product.value;
}
"#,
    );
    let dump = dump_hir(&hir);
    assert_eq!(dump.matches("InterfaceCall").count(), 9, "{dump}");
    for result in [
        "ObjectResult",
        "SharedCallResult",
        "OptionalProduced",
        "ArrayInitialization adopt",
        "fn(i64) -> i64",
        "app::Box<i64>",
    ] {
        assert!(dump.contains(result), "missing `{result}`:\n{dump}");
    }
    let mir = crate::test_support::lower_hir_to_final_mir(&hir);
    crate::mir::verify_mir(&mir).expect("every operator result capability must lower and verify");
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
fn operator_receivers_reuse_the_complete_ordinary_carrier_matrix() {
    let hir = check_operator_source(
        r#"
from std::ops import OpAdd;

class Number implements OpAdd<Number, i64> {
    static stored: Number = Number(10);
    private value: i64;

    init(value: i64) { self.value = value; }
    fn op_add(ref rhs: Number) -> i64 { return self.value + rhs.value; }
    fn through_self(ref rhs: Number) -> i64 { return self + rhs; }
}

class Holder {
    value: Number;
    maybe: Number?;
    init(value: i64) {
        self.value = Number(value);
        self.maybe = Number(value + 1);
    }
}

fn through_local(ref right: Number) -> i64 {
    var left: Number = Number(1);
    return left + right;
}
fn through_field(ref holder: Holder, ref right: Number) -> i64 { return holder.value + right; }
fn through_static(ref right: Number) -> i64 { return Number.stored + right; }
fn through_produced(ref right: Number) -> i64 { return Number(2) + right; }
fn through_checked(ref value: Obj, ref right: Number) -> i64 { return ((Number) value) + right; }
fn through_shared(shared_left: shared Number, ref right: Number) -> i64 { return (*shared_left) + right; }
fn through_optional(ref holder: Holder, ref right: Number) -> i64 { return holder.maybe! + right; }
fn through_array(ref values: Number[], ref right: Number) -> i64 { return values[0] + right; }
fn through_interface(ref left: OpAdd<Number, i64>, ref right: Number) -> i64 { return left + right; }
fn through_checked_interface(ref value: Obj, ref right: Number) -> i64 {
    return ((OpAdd<Number, i64>) value) + right;
}
fn through_shared_interface(shared_left: shared OpAdd<Number, i64>, ref right: Number) -> i64 {
    return (*shared_left) + right;
}
fn through_optional_interface(box: shared OpAdd<Number, i64>?, ref right: Number) -> i64 {
    return (*box)! + right;
}

fn main() -> i64 { return 0; }
"#,
    );
    let dump = dump_hir(&hir);
    assert_eq!(dump.matches("InterfaceCall").count(), 13, "{dump}");
    for carrier in [
        "StaticView",
        "ProducedView",
        "CheckedViewArgument",
        "ArrayElementPlace",
        "CheckedOptionalBoxPayload",
    ] {
        assert!(dump.contains(carrier), "missing `{carrier}`:\n{dump}");
    }
}

#[test]
fn ineligible_left_types_do_not_gain_implicit_operator_crossings() {
    let source = r#"
from std::ops import OpAdd;

interface Other { fn other() -> i64; }
class Number implements OpAdd<Number, i64>, Other {
    init() {}
    fn op_add(ref rhs: Number) -> i64 { return 1; }
    fn other() -> i64 { return 2; }
}
fn make() -> Number { return Number(); }

fn raw_shared(left: shared Number, ref right: Number) -> i64 { return left + right; }
fn unrelated(ref left: Other, ref right: Number) -> i64 { return left + right; }
fn erased(ref left: Obj, ref right: Number) -> i64 { return left + right; }
fn optional(left: Number?, ref right: Number) -> i64 { return left + right; }
fn array(left: Number[], ref right: Number) -> i64 { return left + right; }
fn function(left: fn() -> Number, ref right: Number) -> i64 { return left + right; }
fn discard(ref left: Number, ref right: Number) -> unit { left + right; }
fn main() -> i64 { return 0; }
"#;
    let resolved = resolve_operator_source(source);
    assert!(
        resolved.diagnostics.is_empty(),
        "{:?}",
        resolved.diagnostics
    );
    let checked = crate::typeck::type_check(&resolved.program);
    assert!(checked.hir.is_none());
    assert!(checked.diagnostics.len() >= 7, "{:?}", checked.diagnostics);
    assert!(checked
        .diagnostics
        .iter()
        .any(|diagnostic| { diagnostic.code == crate::typeck::program::INVALID_CALL_STATEMENT }));
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
    let resolved_dump = dump_resolved(&resolved.program);
    assert!(
        resolved_dump.contains("OperatorResolution Add candidates 0 incompatible-rhs 1"),
        "{resolved_dump}"
    );
    assert!(resolved_dump.contains("IncompatibleRhs"), "{resolved_dump}");
    let checked = crate::typeck::type_check(&resolved.program);
    let diagnostics = checked.diagnostics.iter().collect::<Vec<_>>();
    assert_eq!(diagnostics.len(), 2, "{:?}", checked.diagnostics);
    assert_eq!(
        diagnostics[0].code,
        crate::typeck::program::UNSUPPORTED_OPERATOR_APPLICATION
    );
    assert_eq!(
        diagnostics[1].code,
        crate::typeck::program::INCOMPATIBLE_OPERATOR_RHS
    );
    assert!(diagnostics[1].labels.iter().any(|label| label
        .message
        .contains("candidate requires read-only `Rhs` `Number`")));
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
    assert_eq!(dump.matches("OperatorSelection").count(), 2, "{dump}");

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
        !dump_resolved(&resolved.program).contains("OperatorResolution"),
        "primitive punctuation must not consult canonical applications"
    );
    let checked = crate::typeck::type_check(&resolved.program);
    assert!(checked.diagnostics.is_empty(), "{:?}", checked.diagnostics);
    let dump = dump_hir(&checked.hir.unwrap());
    assert!(dump.contains("Binary AddI64"), "{dump}");
    assert!(!dump.contains("InterfaceCall"), "{dump}");
}

#[test]
fn primitive_predicates_retain_exact_comparison_hir_with_reachable_protocols() {
    let source = r#"
from std::ops import OpEq, OpLess, OpLessEq, OpGreater, OpGreaterEq;
fn compare(left: f64, right: f64) -> bool {
    return (left == right) || (left != right) || (left < right) || (left <= right) || (left > right) || (left >= right);
}
fn boolean_equal(left: bool, right: bool) -> bool { return left == right; }
fn main() -> i64 { return 0; }
"#;
    let resolved = resolve_operator_source(source);
    assert!(
        resolved.diagnostics.is_empty(),
        "{:?}",
        resolved.diagnostics
    );
    assert!(
        !dump_resolved(&resolved.program).contains("OperatorResolution"),
        "exact primitive comparisons must not consult canonical applications"
    );
    let checked = crate::typeck::type_check(&resolved.program);
    assert!(checked.diagnostics.is_empty(), "{:?}", checked.diagnostics);
    let dump = dump_hir(&checked.hir.unwrap());
    assert_eq!(dump.matches("FloatingComparison").count(), 6, "{dump}");
    assert_eq!(dump.matches("BooleanComparison").count(), 1, "{dump}");
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
fn malformed_resolved_operator_mapping_is_diagnosed_before_interface_erasure() {
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
        .unwrap();
    let crate::resolve::ResolvedStatement::Return(returned) = &mut definition.body.statements[0]
    else {
        panic!("expected return statement");
    };
    let Some(crate::resolve::ResolvedExpression::Binary(binary)) = &mut returned.value else {
        panic!("expected selected binary expression");
    };
    binary.selection.as_mut().unwrap().candidates[0].requirement =
        crate::identity::InterfaceRequirementId::new(crate::identity::InterfaceId::new(99), 0);

    let checked = crate::typeck::type_check(&resolved.program);
    assert!(checked.hir.is_none());
    assert_eq!(checked.diagnostics.len(), 1, "{:?}", checked.diagnostics);
    assert_eq!(
        checked.diagnostics.iter().next().unwrap().code,
        crate::typeck::INVALID_OPERATOR_SELECTION
    );
}

#[test]
fn multiple_predicate_applications_are_diagnosed_before_hir_completion() {
    let source = r#"
from std::ops import OpEq;
class Number implements OpEq<Number> {
    init() {}
    fn op_eq(ref rhs: Number) -> bool { return true; }
}
fn equal(ref left: Number, ref right: Number) -> bool { return left == right; }
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
        .expect("equality function must have a resolved body");
    let crate::resolve::ResolvedStatement::Return(returned) = &mut definition.body.statements[0]
    else {
        panic!("expected return statement");
    };
    let Some(crate::resolve::ResolvedExpression::Binary(binary)) = &mut returned.value else {
        panic!("expected selected comparison expression");
    };
    let resolution = binary
        .selection
        .as_mut()
        .expect("class predicate must retain candidate resolution");
    let duplicate = resolution
        .selected()
        .expect("source must select one equality application");
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

#[test]
fn every_overloadable_punctuation_maps_exhaustively_to_its_canonical_protocol() {
    assert_eq!(
        [
            ResolvedUnaryOperator::Negate.protocol(),
            ResolvedUnaryOperator::LogicalNot.protocol(),
            ResolvedUnaryOperator::BitwiseComplement.protocol(),
        ],
        [
            Some(CanonicalOperatorProtocol::Neg),
            None,
            Some(CanonicalOperatorProtocol::BitNot),
        ]
    );
    assert_eq!(
        [
            ResolvedBinaryOperator::Add.protocol(),
            ResolvedBinaryOperator::Subtract.protocol(),
            ResolvedBinaryOperator::Multiply.protocol(),
            ResolvedBinaryOperator::Divide.protocol(),
            ResolvedBinaryOperator::Remainder.protocol(),
            ResolvedBinaryOperator::ShiftLeft.protocol(),
            ResolvedBinaryOperator::ShiftRight.protocol(),
            ResolvedBinaryOperator::BitwiseAnd.protocol(),
            ResolvedBinaryOperator::BitwiseOr.protocol(),
            ResolvedBinaryOperator::BitwiseXor.protocol(),
            ResolvedBinaryOperator::Equal.protocol(),
            ResolvedBinaryOperator::NotEqual.protocol(),
            ResolvedBinaryOperator::LessThan.protocol(),
            ResolvedBinaryOperator::LessEqual.protocol(),
            ResolvedBinaryOperator::GreaterThan.protocol(),
            ResolvedBinaryOperator::GreaterEqual.protocol(),
        ],
        [
            CanonicalOperatorProtocol::Add,
            CanonicalOperatorProtocol::Sub,
            CanonicalOperatorProtocol::Mul,
            CanonicalOperatorProtocol::Div,
            CanonicalOperatorProtocol::Rem,
            CanonicalOperatorProtocol::ShiftLeft,
            CanonicalOperatorProtocol::ShiftRight,
            CanonicalOperatorProtocol::BitAnd,
            CanonicalOperatorProtocol::BitOr,
            CanonicalOperatorProtocol::BitXor,
            CanonicalOperatorProtocol::Eq,
            CanonicalOperatorProtocol::Eq,
            CanonicalOperatorProtocol::Less,
            CanonicalOperatorProtocol::LessEq,
            CanonicalOperatorProtocol::Greater,
            CanonicalOperatorProtocol::GreaterEq,
        ]
    );
}

#[test]
fn predicates_select_direct_protocols_and_not_equal_negates_one_equality_call() {
    let source = r#"
from std::ops import OpEq, OpLess, OpLessEq, OpGreater, OpGreaterEq;

class Number implements OpEq<Number>, OpLess<Number>, OpLessEq<Number>, OpGreater<Number>, OpGreaterEq<Number> {
    init() {}
    fn op_eq(ref rhs: Number) -> bool { return true; }
    fn op_less(ref rhs: Number) -> bool { return true; }
    fn op_less_eq(ref rhs: Number) -> bool { return false; }
    fn op_greater(ref rhs: Number) -> bool { return true; }
    fn op_greater_eq(ref rhs: Number) -> bool { return false; }
}

fn compare(ref left: Number, ref right: Number) -> bool {
    var equal: bool = left == right;
    var different: bool = left != right;
    var less: bool = left < right;
    var less_equal: bool = left <= right;
    var greater: bool = left > right;
    var greater_equal: bool = left >= right;
    return equal && !different && less && !less_equal && greater && !greater_equal;
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
    assert_eq!(resolved_dump.matches("OperatorSelection Eq").count(), 2);
    for protocol in ["Less", "LessEq", "Greater", "GreaterEq"] {
        assert_eq!(
            resolved_dump
                .matches(&format!("OperatorSelection {protocol} interface"))
                .count(),
            1,
            "{resolved_dump}"
        );
    }

    let checked = crate::typeck::type_check(&resolved.program);
    assert!(checked.diagnostics.is_empty(), "{:?}", checked.diagnostics);
    let dump = dump_hir(&checked.hir.unwrap());
    assert_eq!(dump.matches("InterfaceCall").count(), 6, "{dump}");
    // Three source `!` expressions plus the one derived overloaded `!=`.
    assert_eq!(dump.matches("Unary LogicalNotBool").count(), 4, "{dump}");
}

#[test]
fn predicate_rhs_uses_ordinary_base_and_interface_view_compatibility() {
    let hir = check_operator_source(
        r#"
from std::ops import OpEq, OpLess;

interface Marker { fn marker() -> i64; }
class Base { init() {} }
class Derived extends Base implements Marker {
    init() { super(); }
    fn marker() -> i64 { return 0; }
}
class Comparer implements OpEq<Base>, OpLess<Marker> {
    init() {}
    fn op_eq(ref rhs: Base) -> bool { return true; }
    fn op_less(ref rhs: Marker) -> bool { return false; }
}

fn compare(ref left: Comparer, ref right: Derived) -> bool {
    return (left == right) && !(left < right);
}
fn main() -> i64 { return 0; }
"#,
    );
    let dump = dump_hir(&hir);
    assert_eq!(dump.matches("InterfaceCall").count(), 2, "{dump}");
}

#[test]
fn dynamic_equatable_and_typed_operator_equality_remain_independent() {
    let source = r#"
from std::lang import Equatable;
from std::ops import OpEq;

class DynamicOnly implements Equatable {
    init() {}
    fn equals(ref other: Obj) -> bool { return true; }
}
class TypedOnly implements OpEq<TypedOnly> {
    init() {}
    fn op_eq(ref rhs: TypedOnly) -> bool { return true; }
}

fn dynamic_explicit(ref left: DynamicOnly, ref right: DynamicOnly) -> bool {
    return left.equals(right);
}
fn dynamic_punctuation(ref left: DynamicOnly, ref right: DynamicOnly) -> bool {
    return left == right;
}
fn typed_punctuation(ref left: TypedOnly, ref right: TypedOnly) -> bool {
    return left == right;
}
fn main() -> i64 { return 0; }
"#;
    let (_workspace, graph) = load_module_sources(
        "app",
        &[
            ("app.ska", source),
            ("std/ops.ska", CANONICAL_OPS_SOURCE),
            (
                "std/lang.ska",
                "public interface Equatable { fn equals(ref other: Obj) -> bool; }\n",
            ),
        ],
    );
    let resolved = resolve_module_graph(&graph);
    assert!(
        resolved.diagnostics.is_empty(),
        "{:?}",
        resolved.diagnostics
    );
    let checked = crate::typeck::type_check(&resolved.program);
    assert!(checked.hir.is_none());
    assert_eq!(checked.diagnostics.len(), 1, "{:?}", checked.diagnostics);
    assert_eq!(
        checked.diagnostics.iter().next().unwrap().code,
        crate::typeck::program::UNSUPPORTED_OPERATOR_APPLICATION
    );
}

#[test]
fn logical_syntax_never_consults_operator_protocols() {
    let source = r#"
from std::ops import OpEq, OpBitAnd;
class Flag implements OpEq<Flag>, OpBitAnd<Flag, bool> {
    init() {}
    fn op_eq(ref rhs: Flag) -> bool { return true; }
    fn op_bit_and(ref rhs: Flag) -> bool { return true; }
}
fn not_flag(ref value: Flag) -> bool { return !value; }
fn and_flag(ref left: Flag, ref right: Flag) -> bool { return left && right; }
fn or_flag(ref left: Flag, ref right: Flag) -> bool { return left || right; }
fn main() -> i64 { return 0; }
"#;
    let resolved = resolve_operator_source(source);
    assert!(
        resolved.diagnostics.is_empty(),
        "{:?}",
        resolved.diagnostics
    );
    assert!(
        !dump_resolved(&resolved.program).contains("OperatorResolution"),
        "excluded logical syntax must not acquire protocol evidence"
    );
    let checked = crate::typeck::type_check(&resolved.program);
    assert!(checked.hir.is_none());
    assert_eq!(
        checked
            .diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.code == crate::typeck::program::TYPE_MISMATCH)
            .count(),
        3,
        "{:?}",
        checked.diagnostics
    );
    assert!(!checked.diagnostics.iter().any(|diagnostic| matches!(
        diagnostic.code,
        crate::typeck::program::UNSUPPORTED_OPERATOR_APPLICATION
            | crate::typeck::program::AMBIGUOUS_OPERATOR_APPLICATION
    )));
}
