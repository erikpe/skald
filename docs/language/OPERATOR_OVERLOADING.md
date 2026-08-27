# Interface-Based Operator Overloading

Status: frozen operator-protocol language design with staged implementation.
The canonical `std::ops` declarations, whole-bundle validation, explicit
ordinary interface use, and the complete non-generic class operator surface
are implemented, including ordinary receiver carriers, dispatch, ownership,
evaluation, cleanup, result capabilities, panic traces, and static effects.
Definition-site generic punctuation and compiler-provided
primitive protocol evidence remain staged. Exact
primitive expressions continue to use the [implemented primitive operator
profile](TYPES_AND_VALUES.md#implemented-primitive-operator-profile).
This document fixes the complete source-visible contract;
the [status matrix](STATUS.md) remains authoritative for availability and the
[implemented grammar](GRAMMAR.md) remains authoritative for accepted source.
Its ordinary-call produced primitive read-only alias prerequisite is
implemented separately in [Aliases and
Ownership](ALIASES_AND_OWNERSHIP.md#implemented-produced-primitive-read-only-alias-arguments).

Operator overloading is nominal sugar over canonical generic interfaces. An
eager source operator selects either an existing exact primitive operation or
one exact application of a compiler-recognized interface declared in ordinary
`std::ops` source. It does not select same-named structural methods, box
primitives, or introduce a runtime operator service.

## Canonical `std::ops` protocols

The installed dependency-free `std::ops` module declares this complete public
bundle:

```ska
public interface OpNeg<Output> {
    fn op_neg() -> Output;
}

public interface OpBitNot<Output> {
    fn op_bit_not() -> Output;
}

public interface OpEq<Rhs> {
    fn op_eq(ref rhs: Rhs) -> bool;
}

public interface OpLess<Rhs> {
    fn op_less(ref rhs: Rhs) -> bool;
}

public interface OpLessEq<Rhs> {
    fn op_less_eq(ref rhs: Rhs) -> bool;
}

public interface OpGreater<Rhs> {
    fn op_greater(ref rhs: Rhs) -> bool;
}

public interface OpGreaterEq<Rhs> {
    fn op_greater_eq(ref rhs: Rhs) -> bool;
}

public interface OpAdd<Rhs, Output> {
    fn op_add(ref rhs: Rhs) -> Output;
}

public interface OpSub<Rhs, Output> {
    fn op_sub(ref rhs: Rhs) -> Output;
}

public interface OpMul<Rhs, Output> {
    fn op_mul(ref rhs: Rhs) -> Output;
}

public interface OpDiv<Rhs, Output> {
    fn op_div(ref rhs: Rhs) -> Output;
}

public interface OpRem<Rhs, Output> {
    fn op_rem(ref rhs: Rhs) -> Output;
}

public interface OpBitAnd<Rhs, Output> {
    fn op_bit_and(ref rhs: Rhs) -> Output;
}

public interface OpBitOr<Rhs, Output> {
    fn op_bit_or(ref rhs: Rhs) -> Output;
}

public interface OpBitXor<Rhs, Output> {
    fn op_bit_xor(ref rhs: Rhs) -> Output;
}

public interface OpShiftLeft<Rhs, Output> {
    fn op_shift_left(ref rhs: Rhs) -> Output;
}

public interface OpShiftRight<Rhs, Output> {
    fn op_shift_right(ref rhs: Rhs) -> Output;
}
```

The module path, interface names, parameter names and order, requirement
names, receiver mutability, parameter modes and types, and result types are
canonical. A same-named interface elsewhere has no operator meaning. The
compiler validates the reachable module as one complete bundle rather than
synthesizing or silently completing declarations.

These declarations are currently usable through ordinary imports,
`implements`, generic bounds, interface types, and explicit method calls.
Exact classes, inherited and closed-generic class conformances, and exact
canonical interface views can also use all overloadable punctuation listed
below. Definition-site generic punctuation remains staged.

Every requirement has the ordinary implicit read-only receiver. Binary
protocols take one call-scoped read-only `ref` operand. Value-producing
protocols state `Output` explicitly because Skald has no associated types;
predicate protocols instead return exact `bool` and have no redundant output
parameter.

## Operator surface

The frozen overloadable surface is:

| Source | Canonical protocol |
|---|---|
| unary `-` | `OpNeg<Output>` |
| unary `~` | `OpBitNot<Output>` |
| `+` | `OpAdd<Rhs, Output>` |
| `-` | `OpSub<Rhs, Output>` |
| `*` | `OpMul<Rhs, Output>` |
| `/` | `OpDiv<Rhs, Output>` |
| `%` | `OpRem<Rhs, Output>` |
| `&` | `OpBitAnd<Rhs, Output>` |
| `|` | `OpBitOr<Rhs, Output>` |
| `^` | `OpBitXor<Rhs, Output>` |
| `<<` | `OpShiftLeft<Rhs, Output>` |
| `>>` | `OpShiftRight<Rhs, Output>` |
| `==`, `!=` | `OpEq<Rhs>`; `!=` negates one `op_eq` result |
| `<` | `OpLess<Rhs>` |
| `<=` | `OpLessEq<Rhs>` |
| `>` | `OpGreater<Rhs>` |
| `>=` | `OpGreaterEq<Rhs>` |

Prefix `!` is never overloadable. It remains exact-`bool` negation and does
not create truthiness. `&&` and `||` retain mandatory short-circuit behavior
and cannot lower to eager calls. Postfix optional unwrap, explicit shared
dereference, type tests, casts, calls, member access, indexing, slicing,
construction, and assignment retain their specialized meanings.

## Selection and result typing

Given an overloadable expression, semantic selection evaluates no operand and
uses this order:

1. If the exact static primitive operand type or pair belongs to the
   implemented primitive matrix, select that existing primitive operation.
2. Otherwise map the operator to its one canonical protocol template.
3. Enumerate exact effective applications supplied by the static left type or
   its declared generic bounds, deduplicating identical canonical
   applications.
4. Retain applications whose `ref Rhs` accepts the static right source under
   ordinary read-only alias compatibility.
5. Require exactly one application. Zero is unsupported and more than one is
   ambiguous.
6. Use that application's declared `Output`, or the predicate protocol's
   fixed `bool`, as the expression type.

The expected result type never filters candidates. Selection performs no
implicit numeric cast, promotion, narrowing, contextual literal
reinterpretation, owning copy, shared-owner dereference, optional unwrap,
user-defined conversion, exact-match preference, inheritance-depth ranking,
or other specificity ranking. An explicit cast may produce a different static
operand type before selection.

Ordinary read-only class, interface, and `Obj` view compatibility applies to
the RHS because the requirement declares `ref`. This affects applicability,
not ranking: two applicable protocol applications are ambiguous even when one
has an exact `Rhs` and the other accepts a non-owning view.

Eligible left sources are an exact class with direct or inherited conformance,
a specialized generic class with closed conformance, an exact canonical
operator-interface view, or a type parameter authorized by one or more exact
declared bounds. `Obj`, unrelated interface views, raw shared handles,
optionals, arrays, and function values do not gain structural lookup.

The current implementation covers the first three eligible forms. Selection
from a type parameter's declared bounds is the staged definition-site generic
slice.

## Class implementations and generic bounds

A class implements an operator protocol through one ordinary public instance
method satisfying the exact closed interface requirement:

```ska
from std::ops import OpAdd;

class MyStr implements OpAdd<MyStr, MyStr> {
    private _value: Str;

    init(value: Str) {
        self._value = value;
    }

    fn op_add(ref rhs: MyStr) -> MyStr {
        return MyStr(self._value.concat(rhs._value));
    }
}

var greeting: MyStr = MyStr("hello ") + MyStr("world");
```

Ordinary conformance, inheritance, overrides, access, produced receivers,
checked views, shared anchors, and interface dispatch apply unchanged. The
method can also be called normally on a class value. Private or merely
same-named methods do not authorize punctuation.

Ordinary methods are not overloaded. A concrete class can therefore normally
implement only one differently typed application of each operator protocol.
For example, one class cannot currently provide both `Vector + Vector` and
`Vector + Scalar`. Ordinary conformance diagnoses incompatible applications;
operator syntax adds no hidden overload set. A future general method-overload
or explicit-interface-implementation design may extend that limitation
without changing already unique selections.

Canonical operator bounds admit either ordinary class conformance or the
compiler-provided primitive evidence described below:

```ska
from std::ops import OpAdd;

class Adder<T> where T: OpAdd<T, T> {
    init() {}

    fn add(ref left: T, ref right: T) -> T {
        return left + right;
    }
}

var adder: Adder<u64> = Adder<u64>();
var answer: u64 = adder.add(17u, 25u);
```

This definition-site punctuation and its primitive specialization are frozen
future behavior, not part of the currently implemented class punctuation
slice. Bounds and manual bound calls remain available through ordinary generic
interface support.

The operator expression is selected from declared bounds at the template
definition site. More than one applicable bound is an unranked definition-site
ambiguity. Specialization realizes the already selected application and never
searches the concrete argument for another meaning. A bound requirement may
also be called manually, such as `left.op_add(right)`. Direct primitive member
syntax such as `17u.op_add(2u)` remains invalid because primitives have no
members. Primitive satisfaction remains limited to canonical operator
protocols; other interface bounds retain their existing exact-class rule.

## Compiler-provided primitive applications

This section specifies staged behavior that is not yet implemented.

For every supported primitive operation, the compiler supplies one static
application of the corresponding canonical protocol. The set is exactly the
implemented primitive matrix: it neither creates a new primitive operation nor
changes a result type, wrapping rule, failure, short-circuit rule, or IEEE-754
behavior. Unsupported cells, such as `OpRem<f64, f64>`, remain unsatisfied.

This evidence may satisfy a canonical bound and specialize a bound-selected
operator or manual requirement call to the existing primitive operation. It
does not make a primitive an object, create an interface view or witness-table
entry, permit primitive/interface casts or shared ownership, authorize
unrelated primitive conformances, or allow user replacement of the built-in
mapping.

Primitive equality and ordering exactly match direct primitive operations.
For `f64`, either NaN makes `==` false, `!=` true, and all four ordering
predicates false; positive and negative zero compare equal; infinities use
ordinary numeric ordering. NaN payload, sign, and bit representation do not
participate. `std::f64::BoxF64.equals` remains separate explicit binary-
representation equality.

## Typed and dynamic equality

`OpEq<Rhs>` provides statically selected typed equality. `==` calls `op_eq`
once; overloaded `!=` calls the same requirement once and negates the exact
`bool` result. The two operators cannot be implemented inconsistently.

`std::lang::Equatable.equals(ref other: Obj)` remains a separate explicit
dynamic, heterogeneous comparison contract for object-oriented and hash-key
APIs. Neither interface satisfies or authorizes the other, and the compiler
inserts no bridge. A class may implement both and owns consistency where their
domains overlap. Generic-interface invariance is unchanged: `OpEq<Base>` is
not `OpEq<Derived>`, although a direct `Derived == Derived` may use inherited
`OpEq<Base>` when the RHS can take the ordinary read-only base view.

Ordering uses four independent direct predicates. Greater comparisons are not
derived by boolean complement or reversed operands, preserving partial orders,
unordered floating behavior, heterogeneous receiver orientation, and narrow
bounds such as `T: OpLess<T>`.

## Evaluation, aliases, and cleanup

An overloaded eager binary expression behaves as one ordinary interface call:

1. evaluate and secure the left receiver exactly once;
2. evaluate and secure the RHS exactly once;
3. bind the RHS through the protocol's read-only alias;
4. perform one selected call;
5. secure the owning or scalar result; and
6. clean completed temporaries in reverse completion order at the enclosing
   full-expression boundary.

A produced exact-class receiver or RHS uses the existing caller-owned
temporary rules. The implemented ordinary alias rule extends read-only
primitive binding so any successfully checked produced primitive expression may be
materialized once in hidden caller-owned scalar storage. An existing
compatible primitive place still borrows directly. `mut ref` remains
place-only, and no alias escapes or becomes independently storable.

The implemented non-generic path accepts the same receiver carriers as the
equivalent ordinary interface call: locals, fields, statics, `self`, aliases,
produced exact-class values, checked class or exact-interface views, explicit
shared dereference, explicit optional unwrap, and array-element class places.
Raw shared handles, optionals, `Obj`, unrelated interface views, arrays, and
function values do not cross to a receiver implicitly. Primitive, class,
shared, optional, array, function, and specialized-generic results retain
their ordinary assignment, nesting, argument, return, copy/adoption, anchor,
and cleanup rules. As with every expression statement, a non-`unit` result
cannot be silently discarded.

Operator methods are read-only with respect to their receiver but are not
implicitly pure, total, constant, commutative, associative, or symmetric.
Their calls, allocation, shared-state effects, panic, result production, and
cleanup remain observable. Optimization may inline or devirtualize only when
all such behavior is preserved.

## Modules, diagnostics, and determinism

`std::ops` becomes reachable only through ordinary explicit references that
name a protocol, including `implements`, `where`, interface types, and manual
bound use, or by direct compilation of the canonical module. Operator
punctuation creates no module dependency or source binding and never expands
the graph after semantic selection. A consuming module may use punctuation
without a redundant local import because the implementation or bound's
defining module already made the canonical protocol transitively reachable.

Exact primitive operations neither load nor validate `std::ops`, so primitive-
only programs retain their behavior under `--no-stdlib`. Once reachable, the
entire canonical bundle is currently validated before body type checking.
Replacement standard libraries must
provide that complete contract. Ordinary provider collision, missing-module,
and dependency-cycle diagnostics precede canonical-bundle validation, which
precedes operator selection and capability diagnostics.

Diagnostics distinguish an invalid canonical bundle, unsupported operands,
ambiguous applications, incompatible read-only RHS binding, unsatisfied
operator bounds, invalid output capabilities, unsupported primitive protocol
cells, and ordinary class conformance failures. Selection evidence retains the
operator and operand spans and types plus the selected or rejected primitive,
protocol, claim, and bound identities. Import evidence remains separate.
Candidate, specialization, diagnostic, dump, witness, effect, and target order
is independent of hash and module-discovery order.

## Deliberate exclusions

The frozen initial feature does not add short-circuit protocols, truthiness,
overloaded prefix `!`, compound assignment, increment or decrement, implicit
conversion, contextual literals, user-defined primitive implementations,
orphan implementations, primitive interface objects, method overloading,
associated types, default methods, interface inheritance, generic methods,
conversion protocols, range syntax, `Range<T>`, generator changes, guaranteed
devirtualization, or new runtime ABI behavior.

The frozen compiler representation and implementation obligations are defined
by [Operator-Protocol Lowering](../compiler/OPERATOR_OVERLOADING.md). The
archived [design record](../archive/OPERATOR_OVERLOADING_DESIGN_PROPOSAL.md)
preserves the alternatives and rationale.
