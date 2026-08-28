# Generic Ranges and Concise Range Expressions

Status: frozen language contract; explicit generic ranges and the concise
range frontend implemented. The
canonical `Successor<Output>` protocol, ordinary `Range<T>` class, class
opt-in, static integer realizations, explicit half-open iteration, `..`
syntax, and exact canonical resolution are implemented. Concise range values
stop at an intentional typed-HIR gate until ordinary construction lowering is
implemented; the initial primitive tight-loop profile remains planned. The
[status matrix](STATUS.md) remains authoritative for compiler availability,
and the [implemented grammar](GRAMMAR.md) remains authoritative for accepted
source syntax.

This document owns the source-visible contract for explicit `Range<T>` values
and concise `lower .. upper` expressions. Compiler identities, primitive
realization, HIR provenance, loop fusion, verification, and performance
acceptance are owned by the
[range compiler contract](../compiler/RANGES.md).

## Canonical standard-library contract

The installed `std::range` module contains the complete declaration bundle:

```ska
from std::iter import Iterable;
from std::ops import OpLess;

public interface Successor<Output> {
    fn successor() -> Output;
}

public class Range<T> implements Iterable<T, T>
where T: OpLess<T>, T: Successor<T>
{
    private final _start: T;
    private final _end: T;

    init(start: T, end: T) {
        self._start = start;
        self._end = end;
    }

    fn iter_state() -> T {
        return self._start;
    }

    fn iter_next(mut ref state: T) -> T? {
        if (!(state < self._end)) {
            return none;
        }

        var item: T = state;
        state = state.successor();
        return some(item);
    }
}
```

Explicit reachability validates the exact successor and range templates,
parameters, initializer, bounds, and iterable claim. Same-named declarations
elsewhere are unrelated.

`Successor<Output>` has a read-only receiver and produces a new owning value.
A same-type range requires `T: Successor<T>`. Whenever `value < end`, a
coherent implementation must produce a strictly greater successor:

```text
value < value.successor()
```

Repeated successors should eventually reach or pass a finite reachable end.
The compiler does not prove these laws or add a progress check. A dishonest
implementation can repeat values or fail to terminate, just as an arbitrary
`Iterable` implementation can produce an unbounded sequence.

## Explicit generic ranges

Status: implemented through ordinary generic construction and general
iteration.

The explicit form imports and constructs the ordinary canonical class:

```ska
from std::range import Range;

for (i in Range<u64>(17u, 23u)) {
    // i is 17u through 22u
}
```

`Range<T>` implements exactly `Iterable<T, T>`. Its hidden iteration state is
the next candidate value. `iter_state` copies the start once. Each
`iter_next` compares the current state with the retained end, copies the
current value into the returned item, advances the state once, and returns
outer absence at the first state not less than the end.

Range semantics are:

- half-open: start is included and end is excluded;
- ascending: equal or descending bounds produce an empty range;
- exact-typed: start, end, item, and state all have the same `T`;
- deterministic: one comparison and at most one successor call occur per
  attempt; and
- finite when the conformance laws and endpoints describe a finite reachable
  interval.

The implementation uses the ordinary capabilities implied by its source.
`T` must be storable, the start and yielded item must be copyable, and the
live state must be assignable from the successor result. Existing contextual
generic capability diagnostics remain authoritative; ranges add no capability
bound syntax.

## Primitive successor evidence

The compiler supplies static exact applications for the discrete integer
primitives:

```text
u8  : Successor<u8>
u64 : Successor<u64>
i64 : Successor<i64>
```

Each maps to the existing same-typed wrapping addition of one. A valid
half-open primitive range invokes successor only while its state is below a
same-typed representable end, so range traversal does not wrap at the maximum
value. A manual bound-selected successor call at the maximum retains the
primitive's ordinary wrapping result.

Primitive evidence is static. It creates no primitive object, interface view,
witness table, box, cast, shared target, reflection entry, or user-replaceable
implementation. Direct primitive member syntax such as `17u.successor()` is
invalid; only a canonical generic bound may select the compiler realization.

There is no compiler-provided `Successor<f64>`. Adding one can stop progressing
if defined as addition by one, or produce surprising enormous traversals if
defined as the next representable value. `bool` and `unit` likewise have no
range successor evidence.

## Exact-class ranges

An exact class opts in nominally by implementing both ordering and successor:

```ska
from std::ops import OpAdd;
from std::ops import OpLess;
from std::range import Successor;

class BigInteger
implements OpAdd<BigInteger, BigInteger>,
           OpLess<BigInteger>,
           Successor<BigInteger>
{
    // representation and lifecycle omitted

    fn successor() -> BigInteger {
        return self + BigInteger(1u);
    }
}
```

The class can use explicit ranges now; its concise equivalent is accepted and
resolved but is not executable until range-expression HIR lowering lands:

```ska
from std::range import Range;

for (i in Range<BigInteger>(BigInteger(17u), BigInteger(23u))) {
}

// Frontend-only concise equivalent:
// for (i in BigInteger(17u) .. BigInteger(23u)) {}
```

Class comparisons and successors use ordinary witness dispatch. Endpoint
copies, state assignment, yielded-item copying, effects, allocation, and
destruction retain ordinary class behavior. The initial tight-loop guarantee
does not apply to class ranges.

## Concise `..` expression

Status: syntax and canonical resolution implemented; typed HIR intentionally
gated.

The frozen syntax adds one lowest-precedence, non-associative expression tier:

```text
expression               = range-expression
range-expression         = logical-or-expression
                           [".." logical-or-expression]
```

`..` is a general expression, not a special `for` header:

```ska
from std::range import Range;

var values: Range<u64> = 17u .. 23u;
for (i in values) {
}
```

The lexer uses longest match before member-access `.` and does not require
whitespace. One ungrouped expression may contain at most one `..`; `a .. b ..
c` is invalid rather than associative.

For `lower .. upper`, both operands are evaluated in the enclosing scope and
must have the same exact static type `T`. The expression selects the canonical
`Range<T>.init(T, T)` and has exact owning type `std::range::Range<T>`.
Selection requires a valid canonical specialization, including exact
`OpLess<T>` and `Successor<T>` satisfaction.

There is no expected-type filtering, numeric promotion, narrowing,
common-base inference, optional unwrap, shared dereference, user conversion,
constructor search on `T`, or overloadable `OpRange`. Mixed endpoints such as
`17u .. 23` require an explicit correction to the same type. This narrow
inference does not add general generic-argument inference.

Successfully parsed range syntax acquires `std::range` as a compiler
dependency without creating a source import binding. An explicit import is
still required to name `Range` or `Successor` directly.

The resolved expression retains exact endpoint, range-template,
specialization, initializer, `OpLess<T>`, `Successor<T>`, iterable-claim, and
primitive-intrinsic or class-witness identities. Until ordinary construction
HIR is implemented, any successfully resolved range expression reports the
dedicated range-HIR-pending diagnostic before general expression or `for-in`
type checking begins.

## Evaluation, iteration, and cleanup

The lower endpoint evaluates and is secured exactly once before the upper
endpoint evaluates exactly once. Both complete before range initialization or
loop execution. Produced class values, scalar temporaries, owners, checked
views, and failures retain ordinary expression and construction rules.

Ordinary range iteration follows the complete
[general-iteration contract](ITERATION.md). The range receiver remains live
for the loop, start initializes one owning state, each attempt returns one
`T?`, and each present payload initializes a fresh immutable owning item.
Normal body completion, `continue`, `break`, and return retain the existing
item, body, state, receiver, and enclosing-scope cleanup order.

Successor runs before body entry because it is part of `iter_next`: the body
receives the previous state as its item after the hidden state has advanced.
This order is observable for class successor effects and is the semantic
reference even when an eligible primitive loop is fused.

## Tight primitive range loops

Only an immediately consumed concise integer range is eligible for the initial
tight-loop guarantee:

```ska
for (i in lower .. upper) {
    // body
}
```

For exact `u8`, `u64`, or `i64` endpoints with canonical primitive ordering
and successor evidence, the compiler may omit the materialized range,
interface calls, and optional result. It must preserve endpoint evaluation,
half-open comparison, item value, advance-before-body order, loop exits, and
cleanup while emitting ordinary scalar control flow.

Explicit `Range<T>(lower, upper)`, a range stored before iteration, class
ranges, generic-bound ranges, interface views, and lookalike types remain on
the ordinary protocol path in the initial profile. Explicit construction may
be optimized later only through a separately justified proof or frozen
constructor-semantic boundary.

The durable performance promise is structural: the fused hot loop has no
interface or runtime call, optional wrapper, allocation, ownership operation,
or loop-carried range aggregate; it performs one termination comparison and
one same-typed induction increment beyond the source body. A documented
reference benchmark must additionally place its median time within 10% of the
matched handwritten `while` loop before the performance milestone is complete.
Wall time is acceptance evidence, not a portable language guarantee or noisy
repository correctness gate.

## Diagnostics and deliberate exclusions

Diagnostics distinguish malformed canonical declarations, unavailable module
providers, missing or chained endpoints, mismatched exact endpoint types,
missing ordering or successor applications, unsupported primitive evidence,
and ordinary construction, storage, copy, assignment, result, destruction, or
iteration failures. They retain the `..` span, endpoint spans and types, and
the rejected declaration, bound, or capability site.

The initial contract excludes inclusive `..=`, omitted endpoints, unbounded
ranges, automatic descending traversal, explicit or heterogeneous steps,
`RangeBy`, `StepRange`, adapters, floating ranges, structural discovery,
overloadable `..`, implicit conversion, general generic inference, borrowed or
mutable items, class-range fusion, stored-range fusion, vectorization,
unrolling, and new runtime or ABI behavior.

An explicit stepped range remains separate future work because it must settle
zero step, direction, overshoot, wrapping, heterogeneous step types, and
termination without changing the frozen meaning of half-open `lower ..
upper`.

The rationale and rejected alternatives are preserved in the
[design record](../archive/GENERIC_RANGES_DESIGN_PROPOSAL.md). Delivery order
and acceptance belong to the
[implementation roadmap](../roadmaps/GENERIC_RANGES_ROADMAP.md).
