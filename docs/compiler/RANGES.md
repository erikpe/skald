# Generic-Range Compiler Contract

Status: frozen compiler contract; not implemented. This document owns the
canonical identities, primitive successor realization, range-expression phase
flow, ordinary-construction HIR provenance, primitive loop-fusion boundary,
verification, performance evidence, target, and ABI constraints for the
[generic-range language contract](../language/RANGES.md).

The implemented grammar remains unchanged until the syntax task lands. No
range declaration, expression, HIR provenance, fusion plan, or benchmark
guarantee described here is currently compiler behavior.

## Canonical module and identities

The canonical `std::range` module is ordinary standard-library source that
imports `std::iter::Iterable` and `std::ops::OpLess`. Resolution validates:

- one public `Successor<Output>` interface template;
- one read-only zero-argument `successor() -> Output` requirement;
- one public `Range<T>` class template;
- its exact `OpLess<T>` and `Successor<T>` bounds;
- its exact `Iterable<T, T>` claim; and
- the ordinary public `init(T, T)` selected by range syntax.

The validated product retains exact module, interface template, requirement,
class template, initializer, bound, and conformance identities. Later phases
must consume those identities rather than rediscovering `std::range`,
`Successor`, `Range`, `successor`, or `init` by spelling. A lookalike module or
declaration is ordinary unrelated source.

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

The compiler adds one closed static registry for:

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

The range registry should reuse cohesive static-bound realization
infrastructure shared with canonical operators, while keeping protocol keys
and admissible primitive cells explicitly closed. It must not generalize all
ordinary interface bounds to arbitrary primitive satisfaction.

## Explicit generic range pipeline

Explicit `Range<T>(start, end)` is ordinary generic class construction.
Template resolution, closed specialization, initializer selection, field
initialization, inferred capabilities, `Iterable<T, T>` conformance, method
bodies, witness metadata, receiver ownership, optional result handling, and
loop cleanup reuse their existing owners.

The explicit range milestone adds no syntax, range HIR node, MIR operation,
backend branch, runtime symbol, or ABI change. It is complete only when:

- primitive specializations realize both `OpLess<T>` and `Successor<T>`
  statically;
- class specializations use ordinary nominal witnesses;
- state and item copy/assignment capabilities fail at their ordinary concrete
  use sites;
- direct, stored, copied, nested, argument/result, and generic-bound range
  values retain exact class behavior; and
- ordinary `for-in` executes through the existing `HirForIn` protocol plan and
  verified MIR.

This ordinary path is the semantic reference for range syntax and fusion.

## Range syntax and resolution

Lexing adds a longest-match `..` token before `.` while preserving numeric
literal and member-access tokenization. Parsing adds a source-shaped,
lowest-precedence, non-associative range expression containing both operands,
the operator span, and the complete span. Recovery consumes a malformed
right endpoint or remaining invalid chain once so later statements resume at
their normal boundary.

Generic template source scanning, logical-depth accounting, AST traversal,
dumps, and every expression consumer must visit both endpoints without adding
a recursive parser path that weakens the existing syntax budget.

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
ordinary class path. When `HirForIn` immediately consumes the expression,
type checking may use its origin to select the frozen primitive fusion plan.
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
- the endpoint, item, and state type is exactly `u8`, `u64`, or `i64`;
- ordering and successor are the compiler-provided canonical primitive
  realizations;
- the iterable application is the exact canonical `Range<T>` claim of
  `Iterable<T, T>`; and
- no storage, copy, argument, result, alias, optional, owner, view, call, or
  other observable boundary intervenes.

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
observable boundary.

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

A documented `tests/benchmarks/range_loop` procedure compiles matched syntax
range and handwritten `while` programs under identical compiler, runtime,
toolchain, trace, work-count, and host conditions. It records code size,
hot-loop instruction shape, and repeated successful wall time. The median
range time must be within 10% of the matched `while` median before the final
performance milestone is complete.

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
ordered by the [implementation roadmap](../roadmaps/GENERIC_RANGES_ROADMAP.md).
