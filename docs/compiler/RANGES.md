# Generic-Range Compiler Contract

Status: implemented compiler contract through immediate primitive-loop fusion
and handwritten-`while` performance acceptance.
Resolution validates the canonical declaration bundle, closes exact integer
bounds to existing operations, and compiles explicit `Range<T>` values through
ordinary construction and general iteration. `..` has source AST,
compiler-dependency, exact structural resolved-source evidence, diagnostics,
ordinary lifecycle and native execution. Immediate `u8`, `u64`,
and `i64` syntax loops use the scalar fusion described below; deterministic
shape and recorded timings establish handwritten-`while` parity. This document
owns those identities and target/ABI constraints for the
[generic-range language contract](../language/RANGES.md).

The implemented grammar accepts concise ranges only as direct `for-in`
sources. The pipeline includes the narrow fusion profile below and its
completed structural and benchmark evidence.

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

Successfully parsed direct `..` sources supply typed
`CompilerDependencyKind::RangeForSource` evidence for `std::range` at the
operator span without creating an import binding. Invalid out-of-context
punctuation supplies no dependency evidence. Explicit imports and direct
canonical-module compilation remain equivalent validation triggers. Provider
collision, missing-module, and dependency-cycle errors precede canonical
declaration validation.

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
literal and member-access tokenization. Parsing represents the direct source
as `ForInSource::Range`, containing both endpoint expressions, the operator
span, and the complete span. General `Expression` has no range variant.
Out-of-context punctuation, a grouped complete range, missing endpoints, and
chains receive one `PAR017` diagnostic with bounded recovery into the normal
statement or `for`-header boundary.

Generic template source scanning, logical-depth accounting, AST traversal,
and dumps visit the endpoints from the `for-in` source owner. General
expression consumers require no range case and retain their existing syntax
budget.

Specialization request discovery keeps explicit generic type applications in
the source scanner. Concise ranges use a separate semantic probe after
ordinary callable signatures, class declarations, interface claims, and the
ordinary hierarchy are available. The probe reuses ordinary expression,
method, and operator selection with isolated diagnostics, function-reference
state, and compound-type interning; it records exact `Range<T>` keys at each
structurally visited direct range source and repeats while newly materialized
specializations add requests. It filters callable and class work by traversing
their statement trees; there is no global range-span registry or
span-containment test. Specialized declarations and real bodies are then
materialized and resolved once from the completed request set. Thus method and
overloaded operator results can select `T` without a second source-level type
system.

Resolution evaluates neither endpoint. It resolves both in source order,
requires one exact static type `T`, requests and validates canonical
`Range<T>`, closes its bounds, and selects its canonical `init(T, T)`. The
resolved `ResolvedForInSource::Range` retains:

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

The range evidence belongs to the loop source, never to an expression or
ordinary construction. Type checking revalidates its complete canonical
identity correspondence, including definition-site endpoint provenance,
before selecting an execution plan. Invalid structural evidence is rejected
before HIR is created.

## Typed HIR representation

Type checking consumes the resolved source distinction directly. An eligible
integer source creates `HirPrimitiveRangeIterationPlan` from the two endpoint
expressions. A class or otherwise ineligible direct source synthesizes the
canonical initializer construction as the ordinary protocol receiver, then
reuses exact construction, argument, ownership, lifecycle, and general
iteration planning.

Ordinary `ResolvedConstructExpr` and `HirConstruction` are origin-free.
Explicit `Range<T>(lower, upper)` therefore remains an ordinary expression by
construction rather than by a source tag, and cannot acquire fusion
eligibility through shape recognition. Grouping either endpoint is ordinary
endpoint syntax; grouping the complete range is rejected before resolution.

The fused plan retains only the exact loop evidence required before MIR:
operator span, canonical range template, closed class and initializer,
ordering and successor applications, and iterable application. Endpoint type
is carried by the scalar plan itself. Definition-site provenance is consumed
when eligibility is decided and does not decorate ordinary HIR construction.

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

- `ResolvedForInSource::Range` is the direct source of that exact loop;
- both endpoints carry specialization-independent semantic provenance;
- the endpoint, item, and state type is exactly `u8`, `u64`, or `i64`;
- ordering and successor are the compiler-provided canonical primitive
  realizations;
- the iterable application is the exact canonical `Range<T>` claim of
  `Iterable<T, T>`; and
- its exact canonical class, initializer, bounds, and iterable evidence all
  validate against the resolved program.

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
optimization. Classes, generic parameters, interface views, inherited claims,
and lookalikes likewise use the ordinary path; concise syntax cannot produce a
stored value.

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

Resolved-source and HIR validation must reject wrong canonical identities,
endpoint/result types, initializer mappings, bound realizations, inconsistent
endpoint provenance, and fusion across an observable boundary. Ordinary
constructions contain no field that could be mislabeled as concise syntax.
Fused-plan construction additionally requires both endpoints to have been
classified specialization-independent.

Preliminary and final MIR verification sees only ordinary operations. Focused
mutation tests must reject wrong scalar types, missing endpoint initialization,
compare/update mismatch, update after body, missing item epoch, incorrect
continue or break targets, skipped cleanup, extra optional or interface
traffic in a fused-plan fixture, and unbalanced storage.

Determinism tests compare tokens, AST, module graph, resolved program, HIR,
preliminary MIR, planned MIR, final MIR, assembly, diagnostics, metadata, and
artifacts across reordered source discovery, provider roots, equivalent
imports, and processes. Resolved dumps expose structural `RangeSource`
evidence; HIR dumps expose `RangeLoopEvidence` only for the fused plan and
ordinary construction for protocol execution. MIR dumps require no
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

- lexer/parser for punctuation, the direct-source boundary, recovery, nesting,
  and spans;
- module/resolution for canonical identities, dependencies, bound closure,
  exact endpoint typing, diagnostics, and dumps;
- specialization for class-witness and primitive-successor realization;
- type/HIR for ordinary construction, structural direct-source evidence, immediate-use
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
