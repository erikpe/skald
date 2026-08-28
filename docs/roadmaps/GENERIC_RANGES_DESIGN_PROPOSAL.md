# Generic Ranges and Tight Range Loops Design Proposal

Status: active design proposal. The `Successor<Output>` direction, ordinary
generic `Range<T>`, opt-in class ranges, half-open `..` syntax, and final
primitive range-loop optimization were confirmed as the core direction on
2026-08-28. The detailed phase, diagnostic, and performance boundaries below
remain subject to review before promotion into living contracts. This record
does not make ranges or `..` valid source.

This proposal covers the remaining three layers after implemented general
iteration and interface-based operator overloading:

1. third, explicit generic ranges work through the existing
   `Iterable<Item, State>` protocol;
2. fourth, `lower .. upper` constructs the same range concisely; and
3. finally, direct primitive range loops approach equivalent handwritten
   `while` performance without changing the general iteration protocol.

The central direction is one canonical `std::range` module containing a
library-declared, compiler-recognized `Successor<Output>` interface and an
ordinary generic `Range<T>` class. Discrete primitive types receive static
compiler-provided successor evidence. Exact classes such as `BigInteger` opt
in through ordinary nominal implementations. Range syntax constructs the
canonical class rather than defining a second loop mechanism, and the final
optimization recognizes only semantically exact canonical primitive ranges
before emitting existing verified MIR.

The [status matrix](../language/STATUS.md) remains authoritative for compiler
availability. The [implemented grammar](../language/GRAMMAR.md) remains the
accepted source surface until this proposal is promoted and implemented.

## Intended outcomes and sequence

### Third layer: explicit generic ranges

Ordinary source can import and construct the canonical generic class:

```ska
from std::range import Range;

for (i in Range<u64>(17u, 23u)) {
    // 17u through 22u
}
```

`Range<T>` is an ordinary `Iterable<T, T>` implementation. This layer proves
the reusable protocol, primitive evidence, class opt-in, generic
specialization, lifecycle behavior, and native execution before adding new
punctuation.

### Fourth layer: concise range syntax

The equivalent range can be written as:

```ska
for (i in 17u .. 23u) {
    // 17u through 22u
}
```

`..` is a general expression rather than special syntax in a `for` header:

```ska
from std::range import Range;

var values: Range<u64> = 17u .. 23u;
for (i in values) {
}
```

The syntax infers the exact endpoint type only for this canonical
construction. It does not introduce general generic-argument inference,
implicit numeric conversion, or an overloadable range operator.

### Final layer: tight primitive range loops

A direct canonical integer range consumed immediately by `for-in` should
lower to the semantic equivalent of:

```ska
var range_end: u64 = upper;
var range_state: u64 = lower;
while (range_state < range_end) {
    var i: u64 = range_state;
    range_state = range_state + 1u;
    // body
}
```

The compiler may omit the materialized `Range<u64>`, interface calls, and
optional result wrapper only after exact canonical identities and primitive
operations have been selected. The resulting CFG still uses ordinary MIR
branches, scalar operations, storage epochs, loop exits, and cleanup. There is
no range runtime service or target-specific range instruction.

## Current boundary and architectural evidence

Skald already provides the important foundations:

- [`Iterable<Item, State>`](../language/ITERATION.md) evaluates one receiver,
  initializes one hidden state, and lowers structured `for-in` to ordinary
  cyclic MIR;
- exact, inherited, specialized-generic, and generic-bound iterable
  applications are selected nominally before HIR;
- primitive, exact-class, optional, array, and owner states and items already
  use ordinary lifecycle plans;
- [`std::ops`](../language/OPERATOR_OVERLOADING.md) establishes compiler-owned
  static primitive evidence for canonical library-declared interfaces without
  boxing, witness tables, or runtime primitive objects;
- generic-bound manual calls can specialize either to ordinary class witness
  calls or existing primitive operations;
- `OpLess<T>` already expresses the ordering operation required by a half-open
  ascending range;
- produced scalar expressions can already materialize read-only alias
  temporaries where canonical protocol calls require them; and
- HIR-to-MIR lowering owns structured `for-in` receiver, state, item, result,
  and cleanup plans before the backend sees ordinary verified control flow.

There are three material gaps:

- primitive static bound evidence is currently deliberately limited to
  canonical `std::ops` protocols, so it must gain one similarly narrow
  canonical `Successor<T>` realization;
- the lexer, parser, resolver, specialization request scanner, dumps, and
  diagnostics have no range expression; and
- the MIR pass pipeline currently performs verification but no
  transformations, so tight range lowering needs an explicit semantic owner
  rather than an accidental backend peephole.

The existing iteration contract explicitly leaves `Range<T>`, `..`, and range
fast paths as later extensions. This proposal consumes those extension points
without changing how any other iterable is selected or executed.

## Niflheim precedent

Niflheim's standard library contains an `i64` `Range` with half-open,
ascending semantics. Equal and descending bounds are empty. Its implementation
computes a structural iteration length and returns `start + index`, and its
tests demonstrate explicit construction, negative bounds, empty ranges, and
nested loops.

That behavior is useful evidence for the source-level expectations:

- the upper endpoint is excluded;
- a non-increasing interval is empty rather than implicitly descending; and
- a range is an ordinary iterable value usable outside special loop syntax.

Niflheim does not supply the generic nominal contract needed here. Its range
is fixed to `i64`, relies on the sibling project's indexable structural
iteration protocol, and has neither class opt-in through `Successor<T>` nor
`..` expression syntax. Skald's generic interfaces, exact inline values,
operator bounds, deterministic cleanup, and state-based `Iterable<T, T>`
remain authoritative.

## Design principles

1. **There remains one loop protocol.** `for-in` continues to consume
   `Iterable<Item, State>`; ranges do not add a parallel numeric-loop
   statement.
2. **The explicit class works before its sugar.** `Range<T>` and class opt-in
   must be complete and executable before `..` depends on them.
3. **A basic range advances by semantic succession.** `lower .. upper` does
   not hide an inferred step value, constructor conversion, or additive
   identity.
4. **Discrete classes may opt in.** Range syntax is not restricted to
   primitives when an exact class provides the canonical ordering and
   successor applications.
5. **Primitive support remains static.** Primitive successor evidence creates
   no object conformance, interface view, box, witness, cast, or runtime
   dispatch.
6. **Ranges are half-open and ascending.** Equal or descending bounds are
   empty. Inclusive, descending, and explicitly stepped forms require later
   contracts.
7. **Range syntax is canonical construction, not operator overloading.** A
   lookalike `Range` or `Successor` has no language meaning, and classes do not
   implement an `OpRange` protocol.
8. **Evaluation and ownership remain ordinary.** Bounds evaluate once from
   left to right, items are fresh owning bindings, and loop exits retain the
   existing cleanup contract.
9. **Optimization follows correctness.** The unfused ordinary iterable path
   is the semantic reference and remains available for every noneligible
   range.
10. **Performance evidence is structural first.** Verified MIR and assembly
    shape provide deterministic gates; repeated wall time records practical
    impact without becoming a noisy correctness test.

## Decision register

| ID | Question | Current direction | State |
|---|---|---|---|
| [RANGE1](#range1--canonical-module-and-protocol-ownership) | Where do range declarations live? | Canonical `std::range` source, with exact compiler-recognized `Successor` and `Range` identities | **Confirmed direction** |
| [RANGE2](#range2--successor-contract) | How does an implicit unit range advance? | `Successor<Output>` with an exact `T: Successor<T>` bound | **Confirmed direction** |
| [RANGE3](#range3--generic-range-shape-and-semantics) | How does the explicit class iterate? | Half-open ascending `Range<T> implements Iterable<T, T>` using `OpLess<T>` and `Successor<T>` | **Confirmed direction** |
| [RANGE4](#range4--primitive-evidence-and-class-opt-in) | Which types may use a range? | Static `u8`, `u64`, and `i64` evidence plus ordinary exact-class conformance; no `f64` evidence | **Confirmed direction** |
| [RANGE5](#range5--range-expression-syntax-and-typing) | What does `..` mean? | A low-precedence, non-associative expression constructing canonical `Range<T>` from exact same-typed endpoints | **Confirmed direction** |
| [RANGE6](#range6--dependencies-selection-and-phase-boundaries) | Where is syntax resolved and erased? | Syntax acquires `std::range`, retains canonical construction through typed HIR, then uses ordinary construction or a selected fused loop plan | **Proposed detail** |
| [RANGE7](#range7--evaluation-lifetimes-and-failures) | What is observable? | One left-to-right bound evaluation, ordinary range/item lifecycle, half-open termination, and existing loop cleanup | **Proposed detail** |
| [RANGE8](#range8--tight-loop-eligibility-and-lowering) | Which loops may be fused? | Immediate exact canonical `Range<u8|u64|i64>` values only; emit ordinary scalar MIR with no interface or optional protocol traffic | **Proposed detail** |
| [RANGE9](#range9--performance-acceptance) | What does “approach handwritten `while`” require? | Equivalent hot-loop operation shape, no calls or allocation, plus a recorded median native comparison within 10% on the reference procedure | **Proposed detail** |
| [RANGE10](#range10--diagnostics-determinism-and-promotion) | How is the feature hardened and promoted? | Focused canonical, syntax, specialization, lifecycle, verifier, native, assembly, benchmark, and determinism evidence before living contracts and a roadmap | **Proposed detail** |

## Proposed standard-library surface

The canonical `std::range` module should contain ordinary source equivalent to:

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

The source is intentionally unsurprising. `Range<T>` stores no hidden step,
iterator allocation, length, direction flag, or optional sentinel. Its state
is the next candidate value. The outer optional returned by `iter_next`
retains exactly the existing end-of-iteration meaning.

The implementation requires the ordinary capabilities implied by its body:
`T` must be storable, its start and yielded item must be copyable, and its
live state must be assignable from the successor result. Skald's existing
contextual generic capability analysis should report unavailable concrete
operations; this proposal does not add capability bounds.

## RANGE1 — Canonical module and protocol ownership

`Successor<Output>` and `Range<T>` should be declared in ordinary
`std::range` source. The compiler recognizes their exact module and template
identities because primitive bound satisfaction, `..` construction, and range
fusion cannot use structural spelling.

Canonical validation should establish:

- one public `Successor` interface with one generic output parameter and one
  read-only zero-argument `successor() -> Output` requirement;
- one public `Range<T>` class with one generic parameter;
- an ordinary two-value `init(T, T)` selected by range syntax;
- the exact `Iterable<T, T>` claim; and
- the exact `OpLess<T>` and `Successor<T>` bounds required by its semantics.

The installed class body remains ordinary source and passes ordinary generic
specialization, conformance, lifecycle, HIR, MIR, and native checking. A
replacement standard library must preserve the canonical declaration and
semantic contract. A same-named module, interface, class, method, or
constructor elsewhere is unrelated.

This follows the existing division used by `std::iter` and `std::ops`: users
can import, read, implement, and call ordinary declarations, while the
compiler owns only canonical identity, syntax consumption, and primitive
static realization.

## RANGE2 — Successor contract

The implicit advance of `lower .. upper` should use:

```ska
public interface Successor<Output> {
    fn successor() -> Output;
}
```

Without associated types, the same-type range states its requirement
explicitly as `T: Successor<T>`. The method has a read-only receiver and
returns a new owning value. It does not mutate arbitrary user-visible values
merely because a hidden range state must advance.

For a coherent ascending range, an implementation must obey this semantic
law whenever `value < end`:

```text
value < value.successor()
```

Repeated successors should eventually reach or pass a finite reachable end.
The compiler cannot prove these laws. A dishonest implementation can repeat a
value or fail to terminate, just as an arbitrary `Iterable` can return an
unbounded sequence. The contract should document the law but add no mandatory
per-iteration runtime check.

A required step field was rejected for the basic range. `Range(start, end,
step)` can express wider traversal, but it does not define where the implicit
unit in `start .. end` comes from for a class such as `BigInteger`. It also
introduces zero-step, direction, overshoot, and bounded-overflow policy before
those behaviors are needed. A later `RangeBy` or `StepRange` may define an
explicit advance protocol independently.

Deriving succession from `OpAdd` plus a synthetic `1` was also rejected.
Skald has no generic conversion or constructor protocol that can produce a
class-valued one, and the compiler should not know how to construct
`BigInteger(1u)`. `Successor<T>` states the intended discrete-domain operation
directly.

## RANGE3 — Generic range shape and semantics

`Range<T>` should implement exactly `Iterable<T, T>`. `iter_state` copies the
start value into the loop-owned state. Each `iter_next` compares that state
with the retained end, copies the current value for the item, advances the
state once, and returns the present item. At the first state not less than the
end, it returns outer `none` without invoking `successor`.

The semantics are:

- half-open: start is eligible and end is excluded;
- ascending: `start >= end` is empty;
- deterministic: one comparison and at most one successor call per attempt;
- finite when the conformance laws and endpoints describe a finite reachable
  interval; and
- allocation-free when the chosen `T` operations and ordinary interface
  dispatch require no allocation.

The range does not snapshot external mutable state beyond owning its two
endpoint values. Class endpoint copies, assignments, successor results, and
destruction retain their ordinary lifecycle and effects. A class range may be
substantially more expensive than a primitive range, and the basic language
contract makes no class-range optimization promise.

Explicit generic ranges are complete when direct construction, storage,
copying where available, argument and result transport, ordinary `for-in`,
nested loops, generic-bound consumers, empty intervals, loop exits, class
items, primitive items, native execution, and deterministic cleanup all work
through the existing protocol without range-specific MIR or runtime support.

## RANGE4 — Primitive evidence and class opt-in

The compiler should provide static exact applications for the discrete
integer primitives:

```text
u8  : Successor<u8>
u64 : Successor<u64>
i64 : Successor<i64>
```

Each realization maps to the existing same-typed wrapping addition of one.
Within a valid half-open primitive range, successor is called only while the
state is less than a same-typed representable end, so the built-in range does
not wrap at the maximum value. A manual generic bound call at the primitive
maximum retains the primitive's ordinary wrapping result.

This evidence extends the existing static primitive-realization model by one
canonical protocol. It must not create a primitive interface view, witness
table, box, cast, `shared` target, object conformance, reflection entry, or
user-overridable implementation. Direct primitive member syntax such as
`17u.successor()` remains invalid; a definition-site bound call may specialize
to the intrinsic operation exactly as canonical operator-bound calls do.

No `Successor<f64>` evidence should be provided. “Add one” can stop making
progress at large magnitudes, while “next representable value” would produce
surprising and potentially enormous ranges. `bool` and `unit` are likewise
outside the discrete numeric profile.

An exact class opts in nominally:

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

Its explicit and concise ranges then use ordinary class witness dispatch:

```ska
from std::range import Range;

for (i in Range<BigInteger>(BigInteger(17u), BigInteger(23u))) {
}

for (i in BigInteger(17u) .. BigInteger(23u)) {
}
```

The same opt-in can represent other discrete ordered domains, such as dates or
version positions, without teaching the compiler their construction rules.

## RANGE5 — Range expression syntax and typing

The proposed grammar adds one lowest-precedence, non-associative tier:

```text
expression               = range-expression
range-expression         = logical-or-expression
                           [".." logical-or-expression]
```

The lexer recognizes `..` by longest match before member-access `.`. Whitespace
is not required. Decimal floating syntax continues to require its ordinary
valid token shape, so integer endpoints in `1..3` remain separate integer,
range, and integer tokens.

Only one ungrouped range operator is accepted. `a .. b .. c` receives a
focused diagnostic and does not associate. Grouping may place a range inside
another expression, but a range endpoint must still satisfy exact range
typing.

For `lower .. upper`:

1. resolve both operands in the enclosing scope;
2. require the same exact static type `T` for both operands;
3. require a valid canonical `Range<T>` specialization, including
   `T: OpLess<T>` and `T: Successor<T>`;
4. select the canonical `Range<T>.init(T, T)` construction; and
5. type the expression as exact owning `std::range::Range<T>`.

There is no expected-type candidate filtering, numeric promotion, narrowing,
common-base selection, optional unwrap, shared dereference, class conversion,
constructor search on `T`, or user-defined `..` implementation. Mixed forms
such as `17u .. 23` are invalid until the user writes explicit casts or
same-typed literals.

Because this is a general expression, existing `for-in` selection sees the
result's ordinary exact `Iterable<T, T>` claim. The `for` parser and resolver
need no numeric-range branch.

## RANGE6 — Dependencies, selection, and phase boundaries

A successfully parsed `..` token should add a typed compiler dependency on
the canonical `std::range` module without creating a source import binding.
This mirrors `for-in` acquisition of `std::iter`. Explicit imports and direct
canonical-module compilation remain ordinary equivalent validation triggers.
`std::range` then reaches dependency-free `std::iter` and `std::ops` through
its own explicit imports.

The phase flow should be:

```text
`lower .. upper`
    -> source-shaped range syntax with both operands and complete spans
    -> resolved canonical Range template, initializer, bounds, and primitive
       or class realizations
    -> typed HIR canonical range construction
    -> ordinary class construction when stored, passed, or otherwise consumed
    -> ordinary HirForIn protocol plan, or an eligible primitive range-loop plan
    -> existing verified scalar/call/optional/lifecycle MIR
```

Parser-level source rewriting is excluded. It would fabricate imports,
generic arguments, initializer spans, temporaries, and diagnostics, and it
would discard the provenance needed by the final optimization.

Resolved evidence should retain exact endpoint types, the canonical range and
initializer identities, `OpLess<T>` and `Successor<T>` applications and
realization kinds, source spans, and the resulting exact class identity. No
name lookup, structural protocol selection, unresolved type parameter, or
candidate set may survive completed HIR.

HIR may retain a dedicated canonical range-construction expression or an
equivalent explicit provenance record on ordinary construction. The durable
requirement is that ordinary value consumers receive exact `Range<T>`
semantics while an immediately consuming `HirForIn` can select a verified
primitive range execution plan without rediscovering source spelling or
standard-library names.

## RANGE7 — Evaluation, lifetimes, and failures

Range construction evaluates and secures `lower` exactly once, then `upper`
exactly once. Both complete before the range initializer or loop execution.
Produced class endpoints, scalar temporaries, owners, checked views, and
failure suppression retain ordinary expression and construction rules.

Unfused iteration follows the existing contract exactly:

1. construct and retain the range receiver;
2. copy its start into one owning state;
3. compare state with end once per attempt;
4. when present, copy the current state into a fresh item, compute and assign
   its successor, then enter the body;
5. on outer absence, clean the result, state, and receiver; and
6. preserve ordinary item/body cleanup on normal completion, `continue`,
   `break`, and return.

Successor runs before the body because it is part of `iter_next`. The hidden
state is inaccessible to source, but this order matters for class successor
effects and must remain the semantic reference.

Static failures should distinguish:

- a missing, ambiguous, inaccessible, or malformed canonical range module;
- malformed canonical `Successor` or `Range` declarations;
- missing right endpoint or a chained `..` expression;
- mismatched exact endpoint types;
- unavailable `OpLess<T>` or `Successor<T>` applications;
- unsupported primitive successor evidence, including `f64`;
- ordinary constructor, storage, copy, assignment, result, or destruction
  capability failures; and
- ordinary `Iterable<T, T>` conformance or loop-selection failure if a
  replacement canonical class is inconsistent.

Diagnostics should label the `..` span, both endpoint spans and static types,
the rejected bound or canonical declaration, and the concrete capability site
without cascading through fabricated source.

## RANGE8 — Tight-loop eligibility and lowering

The final performance layer should select a primitive range-loop plan only
when all of these facts are known before MIR:

- the iterable is an immediately consumed canonical range construction,
  either `lower .. upper` or the exact canonical two-argument
  `Range<T>(lower, upper)` construction;
- `T` is exactly `u8`, `u64`, or `i64`;
- ordering and successor close to the compiler-provided canonical primitive
  realizations;
- the selected iterable application is exactly `Iterable<T, T>` from the
  canonical `Range<T>` claim; and
- construction has not crossed storage, alias, interface-view, call, shared,
  optional, or other boundaries that make the range value independently
  observable.

A range stored in a variable and later iterated remains on the ordinary
protocol path in the initial optimization. Class-valued ranges, generic type
parameters, lookalike classes, inherited claims, interface views, and custom
iterables are never fused merely because their methods resemble a range.

The selected HIR plan should retain the source loop identity, item binding,
exact endpoint and item types, ordered bound evaluations, primitive comparison
and successor operations, capability and cleanup plans, body, exits, and
spans. HIR-to-MIR lowering then emits ordinary control flow:

```text
preheader: evaluate lower -> evaluate upper -> own current/end
header:    current < end -> present or exit
present:   initialize item = current -> current = current + 1 -> body
latch:     clean iteration scope -> header
exit:      clean scalar loop state -> continue after loop
```

This ordering matches ordinary `Range<T>.iter_next`: the item receives the
old current value and the state advances before body entry. `continue` targets
the latch after that advance. `break` and return compose the same item, body,
and enclosing-scope cleanup as general iteration.

The fused path emits no interface receiver, witness call, `iter_state` call,
`iter_next` call, `T?` result storage, optional presence branch, range
aggregate, heap allocation, runtime call, range MIR opcode, or backend
intrinsic. Preliminary and final MIR verification should instead validate the
ordinary scalar storage, initialization, comparison, update, loop edges, and
cleanup it actually contains.

Fusion is a semantic optimization, not a different source contract. Focused
tests should lower the same boundary and exit matrix through fused primitive
ranges and unfused ordinary ranges and compare native observations. Disabling
eligibility by storing the range first should change implementation shape but
not results, evaluation order, or cleanup.

## RANGE9 — Performance acceptance

“Approach handwritten `while` performance” should have deterministic
structural requirements and a recorded practical measurement.

For a representative tight accumulation loop over each supported integer
type, the fused steady-state loop must have:

- no direct or indirect call instruction;
- no allocation, retain/release, optional, interface, or runtime operation;
- one loop termination comparison and conditional edge;
- one same-typed induction increment;
- no loop-carried `Range<T>` aggregate; and
- no additional loop-carried memory traffic beyond the hidden scalar state,
  upper bound, item use, and what the equivalent handwritten `while` lowering
  requires.

Backend tests should compare assembly shape against an equivalent handwritten
`while` fixture without freezing incidental register names, labels, stack
offsets, or the compiler's complete current instruction sequence. MIR tests
should own the stronger semantic operation counts because MIR identities and
operations are target-independent.

A documented `tests/benchmarks/range_loop` procedure should compile matched
range and `while` programs under the same compiler, runtime, toolchain,
trace setting, work count, and host conditions. It should record code size,
hot-loop instruction shape, and repeated successful wall time. The median
range time should be within 10% of the matched handwritten `while` median on
the reference procedure before the performance layer is considered complete.

Wall time is diagnostic acceptance evidence, not part of `make check`, because
host scheduling and frequency scaling are noisy. The repository's deterministic
gate should enforce MIR and assembly shape; the benchmark record explains the
measured result and any remaining fixed overhead outside the hot loop.

This milestone does not require a general inliner, devirtualizer, scalar
replacement pass, vectorizer, or optimizer framework. Those may later subsume
the narrow plan, but range performance should not wait for unrelated global
optimization work.

## RANGE10 — Diagnostics, determinism, and promotion

Coverage should follow the narrowest semantic owner:

- lexer and parser tests own `..` longest match, no-whitespace forms,
  precedence, non-associativity, nesting budget, malformed endpoints, recovery,
  and exact spans;
- module and resolution tests own compiler dependencies, canonical identity,
  replacement standard libraries, exact endpoint typing, bound selection,
  class opt-in, primitive evidence, and deterministic dumps and diagnostics;
- generic specialization tests own class-witness and primitive-successor
  realization, manual bound calls, capability failures, and closed
  `Range<T>` conformance;
- type/HIR tests own canonical construction, ordinary versus fused `for-in`
  selection, evaluation order, result typing, and eligibility exclusions;
- MIR and verifier tests own state/item epochs, compare-before-yield,
  increment-before-body, optional-free fused shape, normal/continue/break/return
  cleanup, and malicious plan mutations;
- static-lifecycle and backend tests own owner/call retention on the ordinary
  path, scalar fused legality, ABI neutrality, assembly shape, and absence of
  runtime range symbols;
- golden tests own explicit and concise primitive and `BigInteger` ranges,
  empty/equal/descending bounds, maximum integer endpoints, nesting, effects,
  failures, exits, panic attribution, and native output; and
- independent-process tests own identical syntax, resolved, HIR, preliminary
  MIR, planned MIR, final MIR, assembly, diagnostics, and artifacts under
  reordered provider and source discovery.

The complete repository gates remain `make check`, `make msrv-check` when Rust
targets or supported syntax change, and `git diff --check`. Performance
measurements remain the separate documented procedure from RANGE9.

Before implementation planning starts, the confirmed decisions should be
promoted into focused living contracts:

- `docs/language/RANGES.md` for source semantics and standard-library use;
- `docs/compiler/RANGES.md` for canonical identity, phase ownership, primitive
  realization, HIR plans, MIR, verification, and performance evidence;
- the grammar, iteration, generic-interface, operator, control-flow, status,
  testing, and debugging documents at their existing authoritative boundaries;
  and
- a PR-sized implementation roadmap ordered by explicit range foundations,
  syntax, complete ordinary execution, then fusion and performance evidence.

## Deliberate exclusions and later extensions

The initial range feature does not include:

- inclusive `..=` syntax;
- omitted or unbounded endpoints;
- automatic descending ranges;
- explicit positive, negative, heterogeneous, or runtime-selected steps;
- `RangeBy`, `StepRange`, `step_by`, or range adapters;
- floating-point ranges;
- character, enum, date, or other compiler-provided successor evidence;
- structural successor or range discovery;
- overloadable `..`, `OpRange`, implicit conversions, or general generic
  inference;
- mutable, borrowed, or consuming range items;
- fusion of stored ranges, class ranges, generic-bound ranges, arbitrary
  iterables, or user lookalikes;
- guaranteed inlining, devirtualization, vectorization, unrolling, or
  parallelization; or
- a runtime range object, public runtime symbol, target ABI change, or new
  backend instruction.

An explicit stepped range should be designed separately. It may use a
dedicated advance-by protocol or a carefully bounded additive contract, but it
must settle zero step, direction, overshoot, wrapping, heterogeneous step
types, and termination without changing the already frozen meaning of
half-open `lower .. upper`.

## Review questions before freezing

The core language direction is confirmed. Review should concentrate on the
remaining representation and acceptance details:

1. Should typed HIR represent canonical range construction as a dedicated
   expression or ordinary construction plus non-forgeable provenance?
2. Should the first fusion profile include direct explicit
   `Range<T>(lower, upper)` as proposed, or only `lower .. upper` until
   canonical-constructor semantic validation is sufficiently narrow?
3. Is the structural hot-loop contract plus a recorded 10% median threshold
   the right interpretation of “approach handwritten `while`,” or should the
   benchmark threshold be tightened after a baseline is measured?

These questions do not reopen `Successor<T>`, half-open ascending semantics,
class opt-in, exact endpoint typing, or the one-protocol `Iterable<T, T>`
direction.
