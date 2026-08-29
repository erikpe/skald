# Generic-Range Compiler Contract

Status: implemented compiler contract through immediate primitive-loop fusion
and handwritten-`while` performance acceptance.
Resolution validates the canonical declaration bundle, closes exact integer
bounds to existing operations, and compiles explicit `Range<T>` values through
ordinary construction and general iteration. `..` has source AST,
compiler-dependency, exact resolved and HIR construction evidence,
diagnostics, ordinary lifecycle and native execution. Immediate `u8`, `u64`,
and `i64` syntax loops use the scalar fusion described below; deterministic
shape and recorded timings establish handwritten-`while` parity. This document owns those
identities and target/ABI constraints for the
[generic-range language contract](../language/RANGES.md).

The implemented grammar and pipeline include range expressions, the narrow
fusion profile below, and its completed structural and benchmark evidence.

## Canonical module and identities

The canonical `std::range` module imports only the foundational `std::iter`
and `std::ops` protocols. Explicit reachability or direct entry compilation
validates:

- one public `Successor<Output>` interface template;
- one parameter named `Output`; and
- one read-only zero-argument `successor() -> Output` requirement;
- one public `Range<T>` class template;
- its exact `OpLess<T>` and `Successor<T>` bounds;
- its exact `Iterable<T, T>` claim; and
- its public owning `init(start: T, end: T)` initializer.

The resulting request-local product retains the exact module, template,
parameter, requirement, declaration, and requiring-span identities. Resolved
dumps render that product and the three primitive realizations in stable
order. Same-named foreign declarations are ordinary unrelated source.

Later phases consume exact identities rather than rediscovering `std::range`,
`Successor`, `Range`, `successor`, or `init` by spelling.

Successfully parsed `..` supplies typed `CompilerDependencyKind` evidence for
`std::range` at the operator span without creating an import binding. Explicit
imports and direct canonical-module compilation remain equivalent validation
triggers. Provider collision, missing-module, and dependency-cycle errors
precede canonical declaration validation.

The installed class body remains ordinary source and must pass generic
specialization, conformance, lifecycle, HIR, MIR, static-effect, backend, and
native validation. Replacement standard libraries must preserve the frozen
declaration and semantic contract.

## Primitive successor realization

The compiler owns one closed static registry for:

```text
(u8,  Successor<u8>)  -> existing AddU8-by-one operation
(u64, Successor<u64>) -> existing AddU64-by-one operation
(i64, Successor<i64>) -> existing AddI64-by-one operation
```

The registry is mechanically validated against the canonical successor
identity and existing primitive operations. Missing, duplicate, wrong-type,
unsupported, or inconsistent cells are internal registry failures; source
cannot add, override, or orphan an entry.

This extends the existing canonical primitive-bound model without pretending
that a primitive has object conformance. Exact `Successor<T>` bounds close to
one of two realization kinds:

```text
SuccessorImplementation =
    ClassWitness { interface, requirement }
  | PrimitiveIntrinsic { add_one_operation }
```

Definition-site bound-member selection remains fixed. A class argument closes
to its ordinary exact witness call; `u8`, `u64`, or `i64` closes to the existing
primitive addition. Direct primitive member syntax remains unresolved. No
primitive interface view, witness metadata, box, cast, owner, reflection
record, runtime dispatch, or ABI representation is created.

The registry reuses one cohesive primitive-bound realization boundary with
canonical operators while keeping protocol keys and admissible cells
explicitly closed. Bound-selected primitive successor calls erase during
resolution to an ordinary same-typed addition of one. They therefore reach
typed HIR, MIR, verification, and the backend only as the existing addition
operation. Ordinary interface bounds are not generalized to arbitrary
primitive satisfaction.

## Explicit generic range pipeline

Status: implemented.

Explicit `Range<T>(start, end)` is ordinary generic class construction.
Template resolution, closed specialization, initializer selection, field
initialization, inferred capabilities, `Iterable<T, T>` conformance, method
bodies, witness metadata, receiver ownership, optional result handling, and
loop cleanup reuse their existing owners.

The explicit range implementation adds no syntax, range HIR node, MIR
operation, backend branch, runtime symbol, or ABI change. It provides:

- primitive specializations realize both `OpLess<T>` and `Successor<T>`
  statically;
- class specializations use ordinary nominal witnesses;
- state and item copy/assignment capabilities fail at their ordinary concrete
  use sites;
- direct, stored, copied, nested, argument/result, and generic-bound range
  values retain exact class behavior; and
- ordinary `for-in` executes through the existing `HirForIn` protocol plan and
  verified MIR.

This ordinary path remains the implemented semantic reference for concise
range syntax and immediate primitive fusion.

## Range syntax and resolution

Status: implemented through resolved IR, typed HIR, and both ordinary and
fused loop execution plans.

Lexing adds a longest-match `..` token before `.` while preserving numeric
literal and member-access tokenization. Parsing adds a source-shaped,
lowest-precedence, non-associative range expression containing both operands,
the operator span, and the complete span. Recovery consumes a malformed
right endpoint or remaining invalid chain once so later statements resume at
their normal boundary.

Generic template source scanning, logical-depth accounting, AST traversal,
dumps, and every expression consumer must visit both endpoints without adding
a recursive parser path that weakens the existing syntax budget.

Specialization request discovery keeps explicit generic type applications in
the source scanner. Concise ranges use a separate semantic probe after
ordinary callable signatures, class declarations, interface claims, and the
ordinary hierarchy are available. The probe reuses ordinary expression,
method, and operator selection with isolated diagnostics, function-reference
state, and compound-type interning; it records exact `Range<T>` keys at each
`..` span and repeats only while an inner request exposes a new enclosing
endpoint type. Specialized declarations and real bodies are then materialized
and resolved once from the completed request set. Thus method and overloaded
operator results can select `T` without a second source-level type system.

Resolution evaluates neither endpoint. It resolves both in source order,
requires one exact static type `T`, requests and validates canonical
`Range<T>`, closes its bounds, and selects its canonical `init(T, T)`. The
resolved expression retains:

- lower, upper, operator, and complete spans;
- exact endpoint and result types;
- exact range class template, closed class, and initializer identities;
- selected `OpLess<T>` and `Successor<T>` applications;
- class-witness or primitive realization evidence; and
- compiler-dependency provenance separate from source imports.

Selection performs no expected-result filtering, implicit conversion,
promotion, common-base inference, structural lookup, constructor search on
`T`, or overload protocol search. Candidate, bound, diagnostic, request, and
dump order remains deterministic.

After successful resolution, type checking verifies the complete canonical
identity correspondence and lowers the result as ordinary class
construction. Invalid or forged provenance is rejected before HIR is
created.

## Typed HIR representation

Range syntax erases to the existing exact class-construction HIR rather than a
dedicated `HirRangeConstruction`. The construction retains all ordinary
destination, argument, evaluation, ownership, initializer, result, effect,
and cleanup plans.

One compiler-owned, non-forgeable construction origin records:

```text
HirConstructionOrigin::CanonicalRangeSyntax {
    operator_span,
    range_template,
    range_class,
    initializer,
    endpoint_type,
    endpoint_provenance: [lower, upper],
    ordering,
    successor,
}
```

Exact identity fields, not names or source spelling, authorize the origin.
Type checking validates correspondence between the origin, construction
arguments, selected closed class, initializer, bounds, endpoint type, and
result. Ordinary explicit `Range<T>(lower, upper)` has the normal construction
origin and is not upgraded by shape recognition.

Non-loop consumers lower the canonical range-syntax construction through the
ordinary class path. `HirForIn` selects a structured primitive-range plan only
for an immediately consumed eligible expression; all other consumers and
loops use the ordinary protocol plan.
Grouping that remains part of the same immediate expression may preserve the
origin; storage, copying, arguments, results, aliases, optionals, owners,
interface views, calls, or other independently observable boundaries erase
fusion eligibility while preserving the ordinary exact range value.

This representation maximizes construction reuse and keeps one explicit
optimization provenance. A dedicated range HIR expression should not be added
unless future evidence demonstrates an ownership or evaluation plan that the
ordinary construction cannot represent.

## Evaluation and ordinary lowering

The lower operand evaluates and is secured exactly once before the upper
operand evaluates and is secured exactly once. Both are ready before ordinary
range initialization. Construction, failures, produced values, alias
temporaries, lifecycle, static effects, panic traces, and cleanup remain the
existing exact-class rules.

The unfused `HirForIn` path remains unchanged:

```text
preheader: construct/acquire Range<T> -> iter_state -> own State
header:    iter_next(mut ref State) -> own T? -> test outer presence
absent:    clean result -> clean State -> release receiver -> exit
present:   initialize fresh T item -> clean result -> body
latch:     clean iteration scope -> header
```

Range method semantics make successor execute before body entry. Class
effects, dispatch, allocations, panic attribution, and lifecycle remain
observable on this ordinary path.

## Primitive range-loop fusion

The initial fused plan is eligible only when:

- `HirForIn` immediately consumes a construction whose origin is exactly
  `CanonicalRangeSyntax`;
- both endpoints carry specialization-independent semantic provenance;
- the endpoint, item, and state type is exactly `u8`, `u64`, or `i64`;
- ordering and successor are the compiler-provided canonical primitive
  realizations;
- the iterable application is the exact canonical `Range<T>` claim of
  `Iterable<T, T>`; and
- no storage, copy, argument, result, alias, optional, owner, view, call, or
  other observable boundary intervenes.

Generic-template analysis records endpoint provenance before substitution. A
closed endpoint is specialization-dependent when its type or value producer
depends on a template parameter, including transitive local bindings and
bound-selected operations whose concrete result type itself is fixed. Such a
range remains on the ordinary protocol path even if substitution later yields
an eligible integer. A range whose two endpoints are independently concrete,
such as literals or concrete parameters in the same specialized body, may
fuse under the remaining rules. Type checking validates this provenance
against the template semantic selection; it is never reconstructed from the
post-substitution type or source spelling.

Ordinary explicit `Range<T>(lower, upper)` is deliberately ineligible in the
initial profile. Skipping an ordinary constructor would require a separately
frozen side-effect-free semantic boundary or a general proof-producing
optimization. Stored syntax-produced ranges, classes, generic parameters,
interface views, inherited claims, and lookalikes likewise use the ordinary
path.

The selected structured HIR plan retains the loop and item identities,
ordered endpoint expressions, exact scalar type, primitive comparison and
addition operations, item initialization, body, exits, cleanup depths, and
source spans. HIR-to-preliminary-MIR lowering emits only existing operations:

```text
preheader: evaluate lower -> evaluate upper -> own current/end
header:    current < end -> present or exit
present:   initialize item = current -> current = current + 1 -> body
latch:     clean iteration scope -> header
exit:      clean scalar loop storage -> continue after loop
```

The update precedes body entry to match ordinary `iter_next`. `continue`
targets the latch after the update; `break` and return compose existing item,
body, and enclosing cleanup. Equal or descending bounds take the first exit.
Maximum endpoints remain safe because successor executes only for a strictly
smaller current value.

The fused path emits no range aggregate, receiver, witness, interface call,
`iter_state`, `iter_next`, optional result or branch, allocation, ownership
operation, runtime call, range MIR opcode, or backend intrinsic. It adds no
general MIR transformation pass: semantic eligibility and plan construction
belong before MIR, while MIR verification and backends remain source-loop
agnostic.

## Verification and deterministic evidence

Resolved and HIR verification must reject wrong canonical identities,
endpoint/result types, initializer mappings, bound realizations, forged range
origins, explicit constructions mislabeled as syntax, and fusion across an
observable boundary. Fused-plan construction additionally rejects either
endpoint being marked specialization-dependent.

Preliminary and final MIR verification sees only ordinary operations. Focused
mutation tests must reject wrong scalar types, missing endpoint initialization,
compare/update mismatch, update after body, missing item epoch, incorrect
continue or break targets, skipped cleanup, extra optional or interface
traffic in a fused-plan fixture, and unbalanced storage.

Determinism tests compare tokens, AST, module graph, resolved program, HIR,
preliminary MIR, planned MIR, final MIR, assembly, diagnostics, metadata, and
artifacts across reordered source discovery, provider roots, equivalent
imports, and processes. Dumps expose the canonical range origin and selected
ordinary or fused execution plan before MIR erasure; MIR dumps require no
range-specific vocabulary.

## Performance acceptance

For matched tight accumulation loops over `u8`, `u64`, and `i64`, the fused
steady-state loop must contain:

- no direct or indirect call;
- no allocation, retain/release, interface, optional, or runtime operation;
- one termination comparison and conditional edge;
- one same-typed induction increment;
- no loop-carried range aggregate; and
- no additional loop-carried memory traffic beyond the hidden scalars, item
  use, and equivalent handwritten `while` lowering.

Target-independent MIR tests own exact semantic operation counts. Backend
tests compare assembly shape without freezing registers, labels, stack
offsets, or the complete incidental instruction sequence.

A documented
[range-loop performance procedure](../development/RANGE_LOOP_PERFORMANCE.md)
compiles matched syntax range and handwritten `while` programs under identical
compiler, runtime, toolchain, trace, work-count, and host conditions. It
records code size, hot-loop instruction shape, and repeated successful wall
time. The recorded medians for all three integer types are within 10% of their
matched `while` medians.

The structural requirements are the durable deterministic contract. Wall time
is recorded acceptance evidence and is not added to `make check`. The
threshold may be tightened by later compiler planning after stable evidence;
doing so does not change source semantics.

## Backend, runtime, and validation boundary

Backends receive only verified ordinary scalar, construction, call, optional,
lifecycle, and control-flow MIR. The unfused path uses existing exact class and
interface metadata; the fused path uses no range metadata at all. There is no
new target operation, calling convention, layout rule, public symbol, runtime
service, allocation requirement, or runtime ABI revision.

Published assembly applies the backend's general closed-world artifact
retention after instruction selection. A fused-only canonical range therefore
leaves its complete validation, HIR, and MIR evidence available to earlier
phase dumps, but its unreferenced `Range<T>` methods and metadata do not reach
textual assembly. Explicit construction, stored execution, callable addresses,
and protocol dispatch create ordinary symbol edges and retain the complete
transitive artifacts they use. The retention pass does not recognize ranges.

Coverage ownership is:

- lexer/parser for punctuation, precedence, recovery, nesting, and spans;
- module/resolution for canonical identities, dependencies, bound closure,
  exact endpoint typing, diagnostics, and dumps;
- specialization for class-witness and primitive-successor realization;
- type/HIR for ordinary construction, non-forgeable origin, immediate-use
  eligibility, evaluation, capabilities, and execution-plan selection;
- MIR/verifier for ordinary and fused CFG, state/item epochs, update order,
  exits, cleanup, and malicious mutations;
- static lifecycle/backend for call retention, scalar legality, ABI neutrality,
  assembly shape, and absent runtime symbols;
- goldens for explicit and concise primitives, opted-in classes, boundaries,
  effects, failures, exits, panic attribution, and native output; and
- the separate benchmark for measured comparison with `while`.

The rationale is preserved in the
[design record](../archive/GENERIC_RANGES_DESIGN_PROPOSAL.md), and delivery is
recorded by the
[archived implementation roadmap](../archive/GENERIC_RANGES_ROADMAP.md).
