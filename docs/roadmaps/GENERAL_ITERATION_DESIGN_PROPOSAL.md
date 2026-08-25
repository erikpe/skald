# General Iteration Design Proposal

Status: draft design proposal. ITER1 through ITER16 record the proposed
direction and remain open until confirmed together. The implemented grammar
and living language and compiler contracts remain authoritative until this
design is frozen, promoted, and implemented.

This proposal adds nominal state-based iteration and a general
`for (item in iterable)` statement to Skald. Iteration is expressed by the
closed generic interface `std::iter::Iterable<Item, State>`. The compiler
evaluates and secures the iterable once, creates one hidden owning state, and
repeatedly asks for an optional next item. The source loop retains structured
identity and lifetime information through typed HIR before lowering to the
existing cyclic MIR and cleanup machinery used by `while`.

The scope is general iteration. Operator overloading, primitive and array
intrinsic interface conformance, numeric `Range<T>`, and `start .. end` syntax
are later consumers rather than prerequisites for this feature. A future
range can implement the same interface without changing `for` semantics.

The [status matrix](../language/STATUS.md) remains authoritative for compiler
availability, and the
[implemented grammar](../language/GRAMMAR.md) remains the exact accepted
syntax. This proposal does not make `for`, `in`, or `std::iter` executable.

## Intended outcome

The initial general-iteration feature should provide:

- a canonical library-declared
  `std::iter::Iterable<Item, State>` generic interface;
- nominal opt-in by ordinary and generic classes;
- `for (item in expression) { ... }` with narrow item-type inference;
- an optional explicit item annotation for documentation and ambiguity
  resolution;
- one evaluation of the iterable expression and one initialization of its
  hidden state;
- one `iter_next` call per attempted iteration and termination at the first
  absent result;
- support for primitive, owning class, array, optional, and shared-owner items
  whenever ordinary optional-result extraction can initialize the loop
  binding;
- correct distinction between end-of-iteration and an optional item whose
  value is absent;
- ordinary exact, inherited, generic-bound, and interface dispatch through
  the selected closed interface application;
- fresh item lifetime and ordinary reverse cleanup on normal completion,
  `continue`, `break`, and return;
- deterministic syntax, resolved, HIR, MIR, diagnostic, and native behavior;
- ordinary standard-library adoption by `Vec<T>` without compiler-intrinsic
  vector behavior; and
- no iterator allocation requirement, erased generic representation, target
  operation, or public runtime ABI extension.

A representative source shape is:

```ska
from std::iter import Iterable;

class Countdown implements Iterable<i64, i64> {
    private _start: i64;

    init(start: i64) {
        self._start = start;
    }

    fn iter_state() -> i64 {
        return self._start;
    }

    fn iter_next(mut ref state: i64) -> i64? {
        if (state <= 0) {
            return none;
        }

        var value: i64 = state;
        state = state - 1;
        return value;
    }
}

fn consume(ref values: Countdown) -> i64 {
    var total: i64 = 0;
    for (value in values) {
        total = total + value;
    }
    return total;
}
```

`value` has inferred type `i64`. The returned `value` is implicitly injected
as a present `i64?`, while `return none;` terminates iteration. The loop does
not expose its state or create a source-visible iterator object.

## Current boundary and architectural evidence

Skald already has most of the lower-level machinery required by state-based
iteration:

- generic interfaces close every requested application to an ordinary exact
  `InterfaceId` before HIR;
- generic class bounds retain definition-site requirement selection and close
  to ordinary interface dispatch;
- exact and generic classes can claim closed generic interface applications;
- interface requirements support owning results and mutable alias parameters;
- inline optionals distinguish one outer absence layer from a present payload
  that may itself be optional;
- produced optional call results support presence tests and checked payload
  extraction;
- exact-class and interface receivers already retain dispatch, owner, checked
  view, and anchor provenance;
- `while` assigns stable loop identities and implements fresh body lifetimes,
  nearest-loop `break` and `continue`, cyclic MIR, and deterministic cleanup;
- MIR verification already reasons about storage, owners, checked views,
  arrays, anchors, optional initialization, and cleanup across loops; and
- native interface calls, internal optional results, aliases, and loop control
  already use the ordinary internal ABI and unchanged public runtime boundary.

The missing work is not an iterator runtime. It is a source-visible protocol,
nominal protocol selection, a loop-duration receiver and state plan, a
structured HIR statement, and lowering that composes those decisions with the
existing loop CFG and ownership verifier.

The current `while` contract is deliberately insufficient as an early source
rewrite target. Source-level expansion cannot express a hidden nonescaping
interface receiver whose produced owner or backing anchor remains valid across
multiple calls, and it would make diagnostic spans, item scope, `continue`
cleanup, and compiler-generated identities accidental properties of synthetic
source. General iteration should reuse `while` execution mechanics only after
those semantic decisions are explicit.

## Niflheim precedent

Niflheim implements `for value in collection` through a structural
indexability protocol: `iter_len() -> u64` plus `iter_get(i64) -> T`. It keeps
a dedicated semantic `ForIn` node containing selected dispatch, then lowers
that node to control flow and provides direct array fast paths and later
interface-call devirtualization.

That phase boundary is useful evidence: collection acquisition, element type,
dispatch, hidden locals, safepoint liveness, and loop exits remain explicit
until executable lowering. Skald should adopt that structured boundary rather
than a parser-level rewrite.

The protocol itself is not suitable for Skald's intended general feature. A
length/index pair assumes finite indexable data, cannot naturally represent a
generator, and couples iteration to random access. Skald instead uses one
opaque state selected by the iterable and one optional next-item operation.
Niflheim remains guidance rather than normative behavior; Skald's exact inline
values, nominal generic interfaces, deterministic destruction, optional
composition, and explicit ownership anchors control this proposal.

## Design principles

1. **Iteration is nominal.** `for` selects the canonical closed
   `Iterable<Item, State>` interface, not same-named methods discovered by
   structural lookup.
2. **State is explicit but hidden.** The iterable chooses one ordinary owning
   state type; source code using the loop cannot inspect or replace it.
3. **Termination is one optional layer.** Outer absence ends iteration; outer
   presence yields one complete `Item`, including when `Item` is itself
   optional.
4. **The iterable evaluates once.** Receiver effects, checked views, produced
   owners, and backing anchors are established once before state creation.
5. **The receiver remains read-only.** Iteration does not grant a mutable
   receiver or freeze mutation through independent aliases.
6. **Each item owns an ordinary body-local lifetime.** The binding is neither
   an escaping alias nor a reference into iterator storage.
7. **Loop exits reuse existing meaning.** `break`, `continue`, return, normal
   completion, and panic retain the established cleanup and non-unwinding
   boundaries.
8. **Selection is fixed before specialization.** A generic bound chooses one
   interface application and requirements at the template definition site;
   a later closed argument cannot redirect the loop.
9. **HIR preserves the construct.** Resolution and type checking expose the
   selected protocol, receiver, state, item, and loop identity before MIR
   becomes ordinary control flow.
10. **Optimization follows semantics.** Devirtualization, inlining, scalar
    replacement, and range or array fast paths may remove overhead only after
    proving equivalence to the protocol loop.

## Decision register

| ID | Decision | Proposed direction | State |
|---|---|---|---|
| [ITER1](#iter1--source-syntax-and-binding) | Source form | Add `for (name [: Type] in expression) block`; reserve `for` and recognize `in` contextually in the header | **Proposed** |
| [ITER2](#iter2--canonical-iterable-interface) | Protocol | Use compiler-recognized, library-declared `std::iter::Iterable<Item, State>` with `iter_state` and `iter_next` | **Proposed** |
| [ITER3](#iter3--termination-and-optional-items) | Termination | Treat outer `none` as termination and outer presence as one item, preserving nested optionality | **Proposed** |
| [ITER4](#iter4--eligibility-selection-and-ambiguity) | Selection | Require one exact nominal application; use an explicit item annotation only as an exact candidate filter | **Proposed** |
| [ITER5](#iter5--generic-bounds-and-definition-site-selection) | Generics | Permit selection through an `Iterable<Item, State>` bound and freeze its requirement identities before specialization | **Proposed** |
| [ITER6](#iter6--iterable-evaluation-and-loop-duration-receiver) | Receiver | Evaluate once and retain one read-only loop-duration interface view with any required owner, guard, or anchor | **Proposed** |
| [ITER7](#iter7--state-item-and-result-lifetimes) | Ownership | Initialize one owning state; create one optional result attempt and one fresh owning item per entered iteration | **Proposed** |
| [ITER8](#iter8--evaluation-order-and-observable-effects) | Effects | Acquire receiver, initialize state, then call `iter_next` sequentially; perform no speculative or duplicate call | **Proposed** |
| [ITER9](#iter9--break-continue-return-and-cleanup) | Loop exits | Reuse nearest-loop identities and clean item/result/body before the next attempt or loop exit, then state and receiver | **Proposed** |
| [ITER10](#iter10--mutation-dispatch-and-source-stability) | Mutation | Keep a read-only, non-exclusive receiver; allow ordinary independent mutation while anchors and checked views remain valid | **Proposed** |
| [ITER11](#iter11--compiler-phase-and-ir-boundaries) | Representation | Retain source and HIR `for-in` forms, then lower to ordinary call, optional, lifetime, branch, and loop MIR | **Proposed** |
| [ITER12](#iter12--standard-library-adoption) | Library | Add dependency-free `std::iter` and make `Vec<T>` an ordinary `Iterable<T, u64>` implementation | **Proposed** |
| [ITER13](#iter13--diagnostics-dumps-and-testing) | Quality | Diagnose canonical-interface, candidate, item, state, receiver, and scope failures with deterministic identities and spans | **Proposed** |
| [ITER14](#iter14--performance-and-optimization-boundary) | Performance | Require allocation-free state when the implementation chooses an inline state; defer devirtualization and fast paths | **Proposed** |
| [ITER15](#iter15--initial-exclusions-and-future-consumers) | Boundary | Exclude ranges, operators, intrinsic arrays/primitives, borrowed or mutable items, generators, labels, and loop values | **Proposed** |
| [ITER16](#iter16--promotion-and-delivery-boundary) | Delivery | Confirm the register together, promote living contracts, then create a PR-sized implementation roadmap | **Proposed** |

## ITER1 — Source syntax and binding

The proposed grammar extension is:

```text
statement             = ...
                      | for-in-statement

for-in-statement      = "for" "(" identifier
                        [":" storage-type]
                        "in" expression ")" block
```

Parentheses and the body block are mandatory, matching `while`. `for` becomes
a reserved statement keyword. `in` is contextual only after the binding and
optional type annotation inside a `for` header; it remains an ordinary
identifier elsewhere.

The binding annotation is optional:

```ska
for (item in values) {
    // Item inferred from the selected interface application.
}

for (item: Str in values) {
    // The annotation must match the selected Item exactly.
}
```

The loop binding is an ordinary initialized owning local scoped to the body.
It is visible from the first body statement through the end of that dynamic
iteration. A fresh lifetime begins on every entered iteration. It may shadow
an enclosing value binding but cannot duplicate another declaration in the
body's outermost scope. The optional header annotation does not introduce
general local inference outside `for`.

The initial syntax has one identifier rather than a pattern. It has no `var`,
`ref`, `mut ref`, destructuring, index, counter, `else`, label, or loop-value
clause. `for` is a statement and cannot appear in expression position.

## ITER2 — Canonical iterable interface

The installed standard library declares:

```ska
public interface Iterable<Item, State> {
    fn iter_state() -> State;
    fn iter_next(mut ref state: State) -> Item?;
}
```

The logical identity is exactly `std::iter::Iterable`. The compiler recognizes
that template identity rather than the unqualified spelling or a structural
signature. It validates that the declaration is public, has exactly two type
parameters, and has exactly the two public read-only receiver requirements
shown above with their names, parameter modes, structural parameter uses, and
result shapes. A malformed or unavailable canonical module produces a focused
language-item diagnostic when `for-in` syntax requires it.

`std::iter` is a dependency-free standard-library module. A compilation that
contains `for-in` requests the canonical module through the same controlled
standard-library provider boundary used by other compiler-owned language
dependencies. Disabling or replacing the standard library remains supported,
but using `for-in` without a valid canonical declaration is rejected rather
than silently selecting a user interface with the same name.

The receiver of both requirements is read-only. Mutable iteration of the
source is a separate future protocol. `State` is passed as `mut ref` so an
inline primitive, class, optional, array, or owner state can advance without
allocating or returning a replacement state on every iteration. The state
cannot escape through the interface signatures.

The interface is ordinary source after canonical validation. User classes
implement it through normal nominal claims, conformance, visibility, and
generic specialization. The compiler does not inject methods into classes or
recognize `iter_state` and `iter_next` outside this exact interface.

## ITER3 — Termination and optional items

`iter_next` returns `Item?`. The outer optional layer is the complete
termination protocol:

- outer absence ends the loop;
- outer presence supplies one complete `Item`; and
- the compiler never calls `iter_next` again after observing absence.

Conceptually:

```text
none        -> end iteration
some(value) -> enter one iteration with value
```

`some(...)` in that notation describes optional state; it is not new source
syntax. Ordinary Skald present injection constructs the result. For example,
returning an `Item` expression from a function whose result is `Item?`
constructs outer presence, while `return none;` constructs outer absence.

If `Item` is itself optional, the result is a nested optional. Outer absence
still ends iteration, while an outer-present, inner-absent value yields a
genuine absent optional item. An implementation can make the distinction by
first constructing an `Item`-typed optional value and then returning that
value into the outer `Item?` result. The loop never flattens, coalesces, or
reinterprets optional layers.

An implementation may return absence on its first call. It may compute items
lazily, perform effects, and terminate earlier than another instance of the
same class. The protocol does not require an exact length, indexing, fused
post-termination behavior, rewindability, cloneability, or purity.

## ITER4 — Eligibility, selection, and ambiguity

The iterable expression is eligible when its static type or one declared
generic bound provides one usable exact application of the canonical
`Iterable<Item, State>` template.

For an exact class, selection examines its effective direct and inherited
nominal interface claims. For an interface-typed expression, the static type
must itself be one exact `Iterable<Item, State>` application. For a generic
type parameter, selection examines only declared bounds. Same-named methods,
another interface with the same requirements, `Obj`, and undeclared
closed-interface conformance do not participate.

Without an item annotation, exactly one application must be available. With
an annotation, the compiler filters applications by exact `Item` identity and
then still requires exactly one remaining application. The annotation does
not convert an item, infer `State`, rank candidates, or use an expected type:

```ska
for (entry: Str in source) {
    // Selects only an exact Iterable<Str, State> candidate.
}
```

No candidate reports that the static type does not implement the canonical
protocol. Multiple candidates report every candidate application and the
claims or bounds that introduced them. An annotation mismatch identifies the
declared item type and available applications.

The selected `Item` must be a legal owning body-local type and an optional
payload. The selected `State` must be a legal owning local, owning result, and
mutable alias target. Initializing the item from the checked optional payload
must satisfy ordinary copy, owner, array, optional, and lifecycle rules. These
are use-site obligations of the loop in addition to contextual validity of the
closed interface application.

Shared owners do not implicitly dereference for protocol selection. A shared
class or interface owner must cross its edge explicitly before iteration,
consistent with ordinary method access. Optional iterable values require an
explicit checked payload selection. Built-in arrays receive no intrinsic
Iterable claim in the initial design.

## ITER5 — Generic bounds and definition-site selection

Generic classes may iterate a parameter through an explicit generic-interface
bound:

```ska
class Consumer<Item, State, Source>
where Source: Iterable<Item, State>
{
    init() {}

    fn consume(ref source: Source) -> unit {
        for (item: Item in source) {
            // ...
        }
    }
}
```

Resolution selects the bound and the template-level identities of
`iter_state` and `iter_next` while validating the generic body. Specialization
closes that exact application and maps those requirements to its ordinary
closed identities. A later class argument cannot redirect the loop to a
same-shaped application, same-named methods, or a different bound.

If more than one bound on the source parameter provides a viable application,
the same annotation filtering and ambiguity rules apply at the definition
site. Specialization cannot resolve an ambiguity left in the template.

Calling the requirements retains ordinary interface semantics. Closed generic
knowledge does not itself promise devirtualization. Primitive, array, or other
compiler-provided conformance is outside this proposal, so the satisfying
closed source argument remains an exact class under the initial generic-bound
contract.

## ITER6 — Iterable evaluation and loop-duration receiver

The iterable expression evaluates exactly once before state initialization.
The compiler establishes one read-only view of the selected closed Iterable
application and retains the receiver carrier for the complete dynamic loop.
Every `iter_state` and `iter_next` call uses that same view; source syntax is
not reevaluated and does not copy the collection merely to make it stable.

The receiver plan generalizes existing read-only object-receiver machinery
from one call or full expression to a bounded loop duration:

- a stable exact-class or interface place remains borrowed directly;
- a produced exact-class value is materialized once in hidden owning storage;
- an explicitly dereferenced stable shared owner retains the required owner
  anchor;
- a produced or replaceable shared owner is adopted or copied into a hidden
  anchor;
- an array-backed exact-class element retains its backing anchor;
- a checked view or optional payload retains its guard and root anchor for the
  loop when the verifier can prove that the body does not invalidate it; and
- a source category that cannot provide stable loop-duration provenance is
  rejected rather than reevaluated or silently copied.

Hidden receiver storage and anchors are compiler-owned and inaccessible by
name. They begin only after successful iterable evaluation and end after the
state is destroyed. A receiver expression that fails or terminates before
completion begins no loop state or cleanup.

The loop-duration view is nonescaping. It does not add local reference types,
source borrow declarations, exclusivity, or a general facility for retaining
aliases between calls.

## ITER7 — State, item, and result lifetimes

After receiver acquisition, `iter_state` executes exactly once. Its result is
secured in one hidden owning `State` local before any `iter_next` call. No
default construction is required. The state remains initialized across every
iteration attempt and is destroyed once on normal loop exit after all
per-iteration values have ended.

Each attempt performs these semantic steps:

1. call `iter_next` with a mutable alias to the hidden state;
2. secure its complete `Item?` result;
3. test the outer presence layer;
4. on absence, end the result and exit the loop;
5. on presence, initialize a fresh owning item binding from the checked
   payload under ordinary type-specific rules;
6. end the optional result and its bounded payload view after the item is
   secure; and
7. execute the body with only the item, state, and receiver lifetimes still
   active.

The item binding is independent owning storage. Mutating or replacing it does
not mutate the state or iterable. Primitive extraction copies one scalar;
shared owners retain their ownership; arrays and exact classes use their
ordinary copy requirements; nested optionals preserve every presence layer.
The initial feature does not add destructive move-out from an optional result.

Consequently, an otherwise valid Iterable application may still be unusable
in a loop when its `Item` cannot initialize an owning local. The diagnostic
points to the loop binding and the lifecycle requirement introduced by
iteration.

## ITER8 — Evaluation order and observable effects

The complete order is:

```text
iterable expression
receiver acquisition
iter_state call
first iter_next call
first item initialization
first body
second iter_next call
...
```

Each operation runs at most once in its source-defined position. Receiver
effects complete before `iter_state`; state creation and its full-expression
cleanup complete before the first attempt; an attempt's `iter_next` effects
complete before its presence test and body; and an item is fully initialized
before the first body statement.

The compiler performs no speculative `iter_next`, prefetch, length query,
duplicate presence test, or call after termination. Optimizations must
preserve call count, order, panic behavior, runtime-trace attribution,
temporary completion, and cleanup.

State mutation performed before `iter_next` returns remains visible even when
the result is absent or the body exits early. The protocol treats the method
call as the advancement boundary; `continue` does not repeat the same item.

## ITER9 — Break, continue, return, and cleanup

`for-in` receives an ordinary stable `LoopId`. An unlabeled `break` or
`continue` selects the nearest enclosing `while` or `for-in` uniformly.

Normal body completion destroys body locals and the item in reverse order,
then begins the next `iter_next` attempt. `continue` performs the same
per-iteration cleanup before jumping to the next attempt. It does not call
`iter_state` again and does not skip directly into the body.

`break` destroys nested body locals, the item, and any live attempt storage,
then destroys the state and ends the retained receiver before continuing after
the loop. Natural termination at outer absence ends the absent result first,
then follows the same state-and-receiver exit sequence.

Return first secures its result, cleans all exited body and item storage, then
the iteration state and receiver, followed by enclosing callable storage under
the existing return contract. Panic and other compiler-known abrupt
termination remain non-unwinding and perform no newly promised cleanup.

An empty iteration still initializes and later destroys the state and retained
receiver. A failed `iter_state` initializes no state. A failed or panicking
`iter_next` enters no body for that attempt. Cleanup registration begins only
after the corresponding owner or state completes successfully.

Like `while`, every `for-in` has a conservative fallthrough path for definite
return because the first attempt may terminate. Neither a known iterable nor
an implementation body proves that the loop executes.

## ITER10 — Mutation, dispatch, and source stability

Iteration borrows its receiver read-only, but Skald aliases are non-exclusive.
The body may mutate the same logical collection through another permitted
path. The protocol implementation owns the observable policy for growth,
replacement, removal, cached length, invalidation, and whether later calls see
those changes. The language promises no snapshot, fail-fast behavior, or
concurrent-modification diagnostic.

Memory and checked-view safety remain compiler obligations. A retained shared
or array anchor keeps the selected allocation or backing alive even if another
owner slot is replaced. A loop body may not perform an operation that the
existing verifier proves would invalidate a live optional guard, checked view,
or inline root. Such a conflict is rejected or represented with a safe copied
owner; it never becomes unchecked dangling access.

Calls use the selected closed interface requirement and ordinary witness
dispatch. Inheritance and overrides affect conformance and witness selection
exactly as for an explicit interface call. An optimizer may devirtualize a
proven exact receiver but must preserve override, view, and runtime-trace
semantics.

Neither interface method is `mut`, so iteration syntax cannot directly call a
mutable source method. A class may still use existing private-cell behavior
from a read-only receiver, and state mutation occurs through its explicit
`mut ref` parameter.

## ITER11 — Compiler phase and IR boundaries

The proposed ownership boundary is:

- **Lexing and syntax** reserve `for`, recognize contextual header `in`, retain
  the optional annotation, iterable expression, body, punctuation, and exact
  spans in a dedicated statement node, and recover without manufacturing a
  `while` AST.
- **Resolution** assigns the loop and binding identities, resolves the
  iterable once, loads and validates the canonical interface template,
  selects one closed application or generic bound, records both requirement
  identities, resolves the body scope, and binds `break` and `continue` to the
  nearest loop.
- **Type checking and HIR** validate receiver access, item/state capabilities,
  result extraction, and body uses; construct a loop-duration receiver plan;
  and retain a structured `HirForIn` containing the selected application,
  calls, state, result, item, body, and cleanup-relevant origins.
- **MIR lowering** acquires the receiver and state in a preheader, emits an
  attempt header containing the interface call and optional test, initializes
  the item only on the present edge, lowers normal completion and `continue`
  to the next attempt, and lowers absence and `break` through one compatible
  exit cleanup sequence.
- **MIR representation** uses ordinary storage, interface calls, mutable alias
  arguments, optional results and guards, branches, jumps, cleanup, anchors,
  and loop backedges. It gains no target-independent iterator instruction.
- **Verification** proves single acquisition and state initialization,
  state-before-use, exact selected call signatures, present-only item
  initialization, per-iteration cleanup, valid cyclic lifetime state, and
  state/receiver cleanup on every normal exit.
- **Backends** consume only verified ordinary MIR. They add no `for`, iterator,
  optional-iteration, or range-specific target operation.

Resolved and HIR dumps expose source loop identity, binding type and identity,
selected closed Iterable application, state type, requirement identities,
receiver carrier, and body. MIR dumps expose the resulting calls, presence
branch, storage epochs, cleanup, and backedge without reconstructing source
protocol selection.

Exact Rust enum names, helper types, block numbering, hidden storage names,
and lowering algorithms remain implementation-private.

## ITER12 — Standard-library adoption

The first standard-library module is:

```text
std/std/iter.ska -> std::iter
```

It contains only the canonical interface and has no dependency on vectors,
ranges, strings, I/O, errors, or runtime intrinsics. This keeps the protocol
available to foundational generic classes without a dependency cycle.

`std::vec::Vec<T>` should explicitly import the interface and claim
`Iterable<T, u64>`. Its state is the next logical index. `iter_state` returns
zero, and `iter_next` returns outer absence at logical length or a present copy
of the occupied `T` slot before advancing the index. Because the state and
index arithmetic are concrete `u64`, this adoption does not depend on
operator overloading or primitive interface conformance.

When `T` is itself optional, the vector's outer occupancy optional naturally
provides the termination layer while its payload retains the inner optional
item. The implementation and native tests must distinguish an occupied absent
optional element from the end of the vector.

No built-in array, `Str`, map, process vector, or other class gains iteration
implicitly. Later library changes may implement the same interface through
ordinary source. Compiler-provided array conformance requires its own design
because arrays cannot write nominal `implements` claims.

## ITER13 — Diagnostics, dumps, and testing

Focused diagnostics should distinguish:

- missing or malformed canonical `std::iter::Iterable`;
- a static type with no canonical Iterable application;
- an explicit shared owner used without dereference;
- an unavailable or unstable loop-duration receiver source;
- multiple candidate applications, including their exact arguments and
  declaration origins;
- an item annotation that matches no candidate or leaves ambiguity;
- invalid Item optional-payload or owning-local capability;
- invalid State result, storage, mutable-alias, or lifecycle capability;
- item initialization that lacks required copy or ownership support;
- malformed implementation signatures through ordinary conformance
  diagnostics;
- invalid `break` or `continue` placement through existing loop diagnostics;
  and
- duplicate or out-of-scope loop bindings through ordinary lexical
  diagnostics.

Syntax tests own contextual `in`, reserved `for`, annotation parsing,
punctuation spans, nesting, recovery, and interaction with calls and type
applications. Resolution tests own canonical identity, application selection,
generic bounds, ambiguity, scopes, loop IDs, exits, modules, and deterministic
dumps. Type-check and HIR tests own receiver plans, exact Item/State types,
optional extraction, lifecycle requirements, and rejected carriers.

MIR and verifier tests should cover empty, single, multiple, nested, broken,
continued, and returning loops; successful and failed state construction;
present and absent attempts; primitive, exact-class, optional, array, shared,
and nested-optional items; produced and anchored receivers; mutation through
independent aliases; cleanup mutations; cyclic storage epochs; and exact
interface-call signatures. Native goldens should use destructor-visible item
and state classes to pin order and exactly-once cleanup, plus a nested optional
iterable that yields a genuine absent optional item before termination.

Standard-library goldens should iterate `Vec<T>` across primitive, `Str`,
optional, exact-class, shared-owner, nested-vector, and empty cases. Generic
goldens should iterate through an Iterable bound and an interface-typed alias.
Compile-failure goldens should pin missing, ambiguous, mismatched, shared-edge,
and lifecycle diagnostics. Cross-process determinism should compare resolved,
HIR, MIR, diagnostic, assembly, and native observations with permuted source
and provider order.

Implementation work should use focused owning-phase tests while iterating,
then run `make check` and `git diff --check`. A later implementation roadmap
must divide these obligations into reviewable phase-owned tasks rather than
placing every layer in one patch.

## ITER14 — Performance and optimization boundary

The semantic protocol requires no heap iterator. A class may choose a primitive
or other inline State, and the compiler stores it directly in hidden local
storage. The retained interface view and any owner or backing anchor use the
existing internal representations. Optional next results use the implemented
inline optional representation for their closed Item type.

The initial implementation may perform one interface dispatch and one optional
presence branch per attempt. That cost is visible in generated code but not a
language guarantee. General interface-call devirtualization, method inlining,
scalar replacement of optional carriers, and loop simplification may later
remove it for exact iterable types.

A future canonical primitive range or built-in array can receive a verified
fast path after profiling. Such an optimization must begin from the same
selected protocol and structured loop semantics; it cannot alter evaluation
count, mutation observation, overflow, termination, cleanup, dispatch, or
panic traces. The initial feature does not add a performance threshold or
benchmark timing gate.

## ITER15 — Initial exclusions and future consumers

The initial feature does not define:

- operator protocols or user-defined operator overloading;
- compiler-provided primitive or array interface conformance;
- `Range<T>`, range literals, `..`, inclusive ranges, descending ranges, or
  steps;
- generator functions, `yield`, coroutines, suspension, or resumable frames;
- shared iterator objects or an `Iterator<Item>` allocation protocol;
- structural `iter_len`, `iter_get`, `next`, or same-named-method discovery;
- mutable or consuming iterable receivers;
- borrowed `ref item` or `mut ref item` loop bindings;
- item patterns, destructuring, enumeration, filters, comprehensions, or
  adapters;
- labels, value-carrying `break`, loop expressions, or loop `else`;
- asynchronous, parallel, atomic, volatile, or concurrent iteration;
- implicit shared dereference, optional unwrap, numeric conversion, or item
  conversion;
- a source-visible iterator state or local alias facility;
- exact-size, double-ended, fused, cloneable, or rewindable iterator laws;
- exceptions or cleanup during the existing non-unwinding panic boundary; or
- guaranteed inlining, devirtualization, vectorization, or range fast paths.

These exclusions leave deliberate extension points. Operator interfaces and a
successor or explicit-step policy can later support ordinary
`Range<T> implements Iterable<T, T>`. Range syntax can then normalize to that
class, and `for` requires no change. Generator work can choose a State that
owns a future resumable frame without changing termination or loop cleanup.
Intrinsic arrays can later provide exact compiler-known Iterable applications
without introducing a second `for` protocol.

## ITER16 — Promotion and delivery boundary

Confirmation requires ITER1 through ITER16 to be accepted together. Partial
confirmation would leave source syntax, protocol shape, receiver lifetime,
optional termination, and lowering mutually underspecified.

After confirmation:

1. promote exact source-visible behavior into a focused general-iteration
   language contract and the implemented grammar as a frozen extension;
2. promote phase, receiver, HIR, MIR, verification, target, and ABI boundaries
   into the compiler documentation;
3. update the status matrix to `Frozen design` while implementation remains
   absent;
4. create and index a PR-sized implementation roadmap ordered by contracts,
   syntax and identity, protocol selection, structured HIR, MIR cleanup,
   standard-library adoption, native execution, and hardening; and
5. keep operator overloading and range syntax in separate proposals and
   roadmaps whose dependencies point to the implemented iteration contract.

Implementation completes only after every accepted receiver and item family,
generic-bound path, loop exit, cleanup edge, diagnostic, dump, standard-library
case, verifier mutation, and native observation passes the repository quality
gate. The completed proposal and roadmap then move to the archive, while
living documentation retains only current behavior.

## Alternatives rejected by the proposal

### Length plus indexing

`iter_len() -> u64` and `iter_get(i64) -> Item` are efficient for vectors but
exclude generators and make iteration policy depend on random access. They can
remain ordinary collection APIs and optimization evidence without defining
general `for` semantics.

### A shared iterator object

`Iterable<Item>` returning `shared Iterator<Item>` gives the state a nominal
runtime object but requires allocation or a more complex escape optimization
for ordinary loops. It also introduces another ownership and dispatch layer.
An explicit State keeps the common path inline while permitting a shared owner
as State when an implementation genuinely needs one.

### Parser-level expansion to `while`

Early expansion duplicates or hides source expressions, creates synthetic
bindings and spans, loses the selected protocol identity, and cannot express
loop-duration anchors safely. Structured HIR followed by ordinary MIR control
flow provides the useful simplicity at the correct phase boundary.

### Structural method discovery

Recognizing any class with `iter_state` and `iter_next` would avoid an explicit
claim but undermine exact generic-interface identity, make same-named methods
compiler-significant, and complicate generic bounds and ambiguity. Nominal
`Iterable<Item, State>` makes capability, Item, and State explicit.

### `has_next` plus `next`

Two calls introduce disagreement, duplicate effects, and time-of-check versus
time-of-use behavior. One optional-producing call advances state and reports
termination atomically at the language level.

### `none` as an Item sentinel

Treating an `Item`-typed `none` as termination would make it impossible to
iterate genuine absent optional items. The distinct outer `Item?` layer
preserves both cases without a second boolean or sentinel value.
