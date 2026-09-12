# Object-View Planning Design Proposal

Status: frozen decision record. The design was accepted on 2026-09-12 and its
delivery is tracked by the active
[Object-View Planning Roadmap](../roadmaps/OBJECT_VIEW_PLANNING_ROADMAP.md).

This proposal separates reusable object-view planning from alias-argument
checking in Skald's type checker. The change is internal: it preserves the
language, typed HIR, MIR, diagnostics, evaluation order, ownership operations,
and runtime behavior. Its purpose is to give source classification, view
compatibility, access selection, and borrow-lifetime planning one cohesive
owner while leaving each consuming expression context responsible for its own
result and diagnostics.

## Intended outcome

- Give checked object-view sources a private owner independent of alias
  parameter dispatch.
- Represent a successful non-owning view decision as one typed plan containing
  the checked source, target, granted access, and required retention scope.
- Reuse that plan for object alias arguments, method and field receivers, and
  protocol iteration where their semantics agree.
- Let object casts, type tests, and copy construction reuse the same source
  facts and closed-world relation query while retaining their distinct result
  types and failure behavior.
- Keep primitive aliases, shared-owner aliases, optional-owner aliases, array
  aliases, parameter-mode dispatch, and alias-specific diagnostics outside the
  object-view planner.
- Preserve the current HIR representation. Planning must lower into existing
  `HirObjectView`, `HirCheckedObjectView`, `HirObjectReceiver`, and
  `HirCallArgument` variants rather than creating a second semantic IR.
- Make future source categories or consumers require one explicit source,
  planning, and diagnostic decision instead of another boolean threaded
  through `alias.rs`.

## Current architecture and evidence

[`typeck/expression/alias.rs`](../../crates/skald-compiler/src/typeck/expression/alias.rs)
is 1,689 lines. Its name understates its responsibilities. It currently owns:

- dispatch among primitive, array, shared-owner, optional-owner, and object
  alias arguments;
- the `ViewSourceUse` diagnostic and source-admission context;
- the eight-variant `CheckedObjectViewSource` representation;
- classification of bindings, explicit shared dereferences, optional payloads,
  array elements, fields, produced objects, and grouped forms;
- conversion of source access and target compatibility into object alias
  arguments;
- iteration-view construction and loop-duration shared-owner anchoring;
- cast-to-alias adaptation and checked-view consumer metadata;
- ancestor projection and forwarded-view construction; and
- context-specific diagnostic text and secondary labels.

The file is large because real semantic families accumulated together, not
because all of its code is one abstraction. Primitive scalar aliases,
shared-owner handle aliases, optional storage aliases, array aliases, and
non-owning object views carry different HIR and lifetime obligations. A14 must
separate the repeated object-view responsibility without hiding those
differences.

Several useful seams already exist:

- [`object_view_relation`](../../crates/skald-compiler/src/typeck/expression/object_view_relation.rs)
  is a pure closed-world query over an exact or dynamic source and a requested
  class, interface, or `Obj` target. It distinguishes static success, static
  failure, and a runtime-dependent relation.
- [`shared_pointee`](../../crates/skald-compiler/src/typeck/expression/shared_pointee.rs)
  distinguishes stable shared bindings from sources that require a hidden
  strong-owner anchor. It can strengthen a stable binding into a loop-duration
  anchor.
- [`place`](../../crates/skald-compiler/src/typeck/expression/place.rs) builds
  receiver carriers for produced objects, static fields, shared dereferences,
  optional payloads, array elements, and ordinary inline places. Several of
  these reconstruct the same `HirObjectView` provenance assembled by
  `alias.rs`.
- [`type_operations`](../../crates/skald-compiler/src/typeck/expression/type_operations.rs)
  uses the source classification for copy construction, casts, and type tests,
  then correctly retains operation-specific checked results.
- [`function/iteration`](../../crates/skald-compiler/src/typeck/function/iteration.rs)
  asks `alias.rs` for a read-only view that remains safe for the whole loop.

The source-language contracts make the boundary ownership-sensitive. Aliases
are non-exclusive; overlapping mutable aliases remain valid. Shared fields and
produced owners sometimes require hidden strong-owner anchors. Optional
payloads require presence guards. Array elements may require an inline-backing
anchor. Produced objects remain full-expression temporaries. Receivers are
evaluated before arguments, arguments are evaluated left to right, and anchors
are released in reverse completion order only after the result is secured.

The sibling Niflheim repository has general semantic and lowering refactor
plans but no object-view planning boundary that matches Skald's inline values,
checked optional payloads, shared-owner anchors, and non-exclusive aliases.
Skald's current HIR provenance and ownership contracts therefore remain the
authority for this design.

## Scope and invariants

- The design is private to type checking. No public Rust path or source syntax
  changes.
- Source classification evaluates and checks a source exactly once.
- A plan never changes source order, inserts speculative checking, or causes a
  consumer to recheck an expression.
- A plan grants no more access than the source provides. Mutable access remains
  context-required and non-exclusive.
- Direct view planning accepts only statically compatible target relations.
  A runtime-dependent narrowing still requires an explicit checked cast or
  type test.
- An exact class-to-base view retains the existing ordered ancestor
  projections. Interface and `Obj` views retain complete-object provenance and
  dynamic dispatch metadata.
- Existing shared bindings borrow directly for an immediate consumer when
  currently safe. Replaceable or produced shared sources retain their hidden
  owner. A loop-duration consumer strengthens even a stable shared binding to
  an explicit retained owner.
- Produced inline sources retain their producer and projections as one bounded
  full-expression value. They never become mutable alias sources.
- Optional payload presence guards, optional-box owner anchors, and inline
  array backing anchors remain explicit in the existing HIR.
- Alias arguments and receivers remain bounded places; the planner does not
  create first-class references, escaping borrows, captures, or returnable
  aliases.
- Diagnostic codes, messages, primary and secondary labels, notes, order, and
  recovery behavior remain exact during migration.
- Typed HIR dumps, MIR, verification, assembly, runtime traces, destruction
  order, and failure behavior remain unchanged.

## Non-goals

- No borrow checker, exclusivity rule, region inference, lexical lifetime
  system, or data-race guarantee.
- No language-level view, reference, or lifetime type.
- No unification of owning values with non-owning views.
- No universal alias representation covering primitives, arrays, shared-owner
  handles, optionals, and objects.
- No type-erased source or consumer callbacks.
- No change to object-cast syntax, implicit conversion rules, dynamic type
  tests, copy construction, or method dispatch.
- No anchor elision, retain/release optimization, or cleanup rescheduling.
- No HIR or MIR variant consolidation as part of A14.
- No conversion of every `CallableChecker` diagnostic into a new error-return
  framework.
- No public extension or registration API for new view sources or consumers.

## Selected design

### A private object-view facade owns the shared vocabulary

Create a private recursive module under `typeck::expression` with a concise
facade and cohesive implementation files:

```text
expression/object_view/
├── mod.rs
├── source.rs
├── plan.rs
├── relation.rs
└── tests.rs          # when focused private tests justify a separate file
```

The facade selectively exposes private source, request, plan, problem, and
relation types to sibling expression modules. The existing relation code and
tests move under this owner without widening visibility. Shared-pointee
classification may move into `source.rs` or remain a sibling during migration;
the final layout should have one owner and should not leave a one-function
pass-through module.

`alias.rs` remains the entry for parameter-mode dispatch and alias-specific
families. `place.rs` remains responsible for object-place and field-place
checking. `type_operations.rs` remains responsible for casts and type tests.
`function::iteration` remains responsible for protocol selection and loop
construction.

### Source classification produces checked source facts

Rename and relocate the role currently served by `CheckedObjectViewSource` to
an object-view-owned source product. Its variants preserve the checked HIR
carrier needed to build the final view; receiver migration adds the existing
static-object carrier to the shared vocabulary:

```text
ObjectViewSource
├── inline class place + complete-object origin
├── static object source + complete-object origin
├── forwarded Obj binding
├── forwarded interface binding
├── checked shared pointee + anchor strategy
├── produced inline object + projections
├── checked optional payload + projections
├── checked optional-box payload + projections
└── checked array element + backing strategy
```

The source product exposes only semantic facts used by planners and typed
consumers: source span, available access, static target, optional exact dynamic
class, relation source, and conversion into an existing HIR view. Variant
payloads remain private so consumers cannot construct a source that skipped
checking.

Source admission is an enum rather than a collection of booleans. The initial
closed policy is conceptually:

```text
ExistingObjectPlace
ExistingOrProducedObject
```

Mutable alias arguments select `ExistingObjectPlace`. Read-only aliases,
iteration, checked casts, and copy construction may select
`ExistingOrProducedObject` where current behavior permits it. Type tests keep
their existing place-only policy. Explicit shared dereference remains required;
an owning `shared T` expression is never silently classified as its pointee.

Classification still uses `CallableChecker` to check nested expressions,
places, optional guards, array projections, and producers. It does not perform
target compatibility or choose the access granted to the consumer.

### A typed request describes the consuming view

A non-owning view consumer submits one `ObjectViewRequest` containing three
orthogonal decisions:

```text
ObjectViewRequest
├── target: class, interface, or Obj
├── required access: read-only or mutable
└── retention: immediate consumer or loop body
```

These fields replace feature booleans. The request is created by a named
context adapter such as an alias argument, receiver, or iteration path. The
planner does not inspect parameter syntax, call declarations, method names, or
loop protocol declarations.

`ImmediateConsumer` means the view and any required source anchor live through
the already-defined immediate call, field operation, cast consumer, or
full-expression boundary. `LoopBody` strengthens a stable shared binding into
an owned anchor because the body may replace that binding. Retention is a
closed semantic enum, not an arbitrary duration or source span.

### The plan is a checked, consumable type-checking product

Planning consumes an `ObjectViewSource` and a request and returns either an
`ObjectViewPlan` or a structured `ObjectViewProblem`. A successful plan proves:

- the source access permits the requested access;
- the source statically provides the requested target;
- any class ancestor projections are ordered and complete;
- the source's existing field, optional, array, or produced projections are
  retained;
- the required anchor or guard strategy is present for the retention scope;
  and
- lowering the plan creates exactly one existing `HirObjectView`.

The plan is consumed when converted into HIR. It is not cloneable by default,
is never stored in resolved IR or HIR, and cannot outlive the current callable
check. It is a phase-local decision product rather than another compiler IR.

The initial planner returns direct views only for
`ObjectViewRelation::StaticSuccess`. Static failure produces an incompatible-
target problem. `Runtime` produces a checked-operation-required problem. This
preserves the rule that implicit alias, receiver, and iteration conversions do
not introduce runtime checks.

### Checked operations reuse facts without sharing the wrong result

Object casts and type tests reuse `ObjectViewSource::relation_source` and the
existing closed-world relation classifier. They do not pretend to be ordinary
direct-view consumers:

- a type test maps static success, static failure, or runtime dependence into
  its existing `HirTypeTestKind` and retains no reusable view plan;
- a checked cast builds its existing `HirCheckedObjectView`, including runtime-
  terminate behavior and consumer target/access metadata;
- copy construction may consume a direct or checked view through its existing
  owning-copy plan; and
- a checked cast used as an alias argument retains the current cast-specific
  compatibility and checked-view wrapper.

This boundary shares source truth and relation logic while keeping operation
authority explicit. A future implementation may add a small checked-view plan
if cast construction itself remains duplicated after migration, but A14 does
not require one universal result enum.

### Context adapters own diagnostics and final wrappers

The source and planner return small structured problems for decisions they own,
including:

- the expression is not an admitted object source;
- an owning shared value needs explicit dereference;
- a produced source is not allowed in this context;
- source access is insufficient;
- the requested target is statically incompatible; or
- the relation requires an explicit checked operation.

Each context adapter maps those problems to its existing diagnostic code,
wording, labels, notes, and recovery behavior. Alias parameters retain their
parameter declaration and type secondary labels. Iteration retains iterable-
protocol language. Casts, tests, copies, receivers, and field places retain
their own terminology.

Expression-local failures encountered while checking a nested producer,
optional payload, shared source, or array projection continue to report from
their established checker. A14 does not wrap or duplicate those diagnostics.
This keeps the planning API narrow and avoids a generic diagnostic framework.

### Alias checking keeps the non-view families explicit

The alias-argument entry point continues to dispatch by lowered parameter type
and binding mode. The following paths do not enter `ObjectViewPlan`:

- primitive scalar place and produced read-only alias arguments;
- array alias places and slices;
- shared-owner handle places;
- optional shared-owner storage;
- other optional storage; and
- cast-to-alias adaptation that requires a checked result.

Only the ordinary object-view branch classifies a source, requests target and
access, renders alias-specific planning failures, and wraps the successful HIR
view in `HirCallArgument::View`. This leaves `alias.rs` smaller without making
unrelated alias kinds depend on object-view vocabulary.

### Receiver and iteration migration is semantic, not mechanical

Receiver migration applies only to carriers already represented by
`HirObjectView`: produced receivers, static object sources, shared pointees,
optional payloads, and array-element views. Ordinary `HirObjectPlace` and
checked place carriers retain their place-specific representation and
inspection evidence.

Iteration uses the same source and direct-view planner with read-only access
and `LoopBody` retention. Protocol lookup and `HirForIn` assembly remain in the
iteration checker. This makes the exceptional lifetime requirement visible at
the request site instead of mutating a shared-pointee source after generic
classification.

### HIR remains the lifetime and evaluation-order contract

The planner selects existing HIR provenance; it does not execute ownership
operations itself. Current lowering remains responsible for evaluating the
receiver before arguments, evaluating arguments left to right, materializing
anchors at each source position, retaining guards through the consumer, and
releasing temporaries in reverse completion order after result security.

Exact HIR dump equivalence is therefore a primary migration check. Any proposed
plan that requires new ordering state in the type checker must stop and revise
the design rather than relying on construction order inside a helper.

## Failure and diagnostic contract

- Existing invalid programs retain the same diagnostic codes and ordered
  diagnostics.
- Primary spans continue to identify the source expression or selected member;
  grouping continues to widen only the source span it currently widens.
- Alias diagnostics retain parameter declaration/type secondary labels and
  mutable-access notes.
- Implicit shared dereference remains rejected before target compatibility is
  considered.
- Produced mutable aliases remain rejected as temporary objects before an
  implicit view is constructed.
- Runtime-dependent direct views remain rejected with the existing advice to
  use an explicit cast or type test where applicable.
- A failed source check publishes no partial plan or HIR wrapper.
- Recovery continues after the same boundary; the planner must not emit a
  second diagnostic for a nested checker failure.

## Alternatives considered

### Split `alias.rs` into files without introducing a product boundary

This improves navigation but leaves receiver, iteration, casts, and aliases
reconstructing source and view decisions independently. File movement is a
useful first implementation step only when it establishes the selected source
and plan ownership.

### Put planning state in HIR

HIR already stores the selected source, provenance, access, target, guards,
and anchors needed by lower phases. Adding a second planning representation to
HIR would expose type-checking workflow and permit partially planned products
to cross the phase boundary. The plan remains private and lowers immediately
to current HIR.

### Use `CheckedObjectReceiver` as the universal plan

Receiver carriers include inspection places and place-specific variants that
alias arguments, iteration, casts, and tests do not need. Reusing that type
would invert ownership and make non-receiver operations depend on receiver
details.

### Use a generic visitor or callback for each source and consumer

The source set is closed and semantically distinct. Exhaustive enums make new
ownership cases visible to the compiler and reviewer. Type-erased callbacks
would hide missing anchor, access, or provenance handling.

### Keep `ViewSourceUse` and add more booleans

The current enum usefully names diagnostic contexts but also controls produced
source admission. Adding flags for mutable access, runtime checks, anchoring,
or loop duration would create invalid combinations. Closed source-admission and
retention enums plus a typed request make those decisions independent and
reviewable.

### Unify all alias kinds under one plan

Primitive, array, shared-owner, optional-owner, and object aliases encode
different storage and ownership operations. A universal alias plan would
replace explicit type distinctions with branching payloads and expand the
blast radius of every change. They remain separate.

## Validation strategy

### Focused source and plan tests

Add table-driven private tests for the pure decisions that are not already
better covered through complete type checking:

- exact class, base, interface, and `Obj` target relations;
- dynamic source relations requiring runtime checks;
- read-only and mutable access acceptance;
- produced-source admission;
- immediate versus loop-body shared anchoring;
- ancestor and nested field projection order; and
- conversion of every source family into the current HIR view shape.

Avoid tests that merely duplicate enum matches or private file layout.

### Type-checking equivalence

Run and, where gaps exist, extend the existing alias-parameter, produced-object
receiver, iteration, object-cast/type-test, explicit-copy, shared-ownership,
optional-box, optional-payload, array-element, static-field, final-field,
generic-object, and generic-interface suites. Pin:

- exact diagnostics and spans for rejected sources, access, and targets;
- HIR dumps for every source and consumer family;
- non-exclusive overlapping mutable aliases;
- checked-cast-to-alias behavior;
- optional presence guards and shared-owner anchors; and
- receiver-before-argument and left-to-right argument evaluation.

### Lowering and native behavior

Run the existing MIR alias, produced-alias, object-temporary, shared cast/view,
array, and logical lifetime suites. Golden and backend tests must preserve:

- final MIR and assembly for representative successful programs;
- exact destructor and last-owner timing;
- reverse cleanup after calls and copies;
- anchor survival when calls mutate or replace source owners;
- checked failure before destination allocation or mutation; and
- native output and panic traces.

### Repository gates

Each implementation step should run focused type-checking and MIR tests,
`cargo fmt --all -- --check`, and Clippy with warnings denied. The final step
must run `make check`, `make msrv-check`, the documentation checker, and
`git diff --check` from a clean checkout or artifact-free snapshot.

## Risks and mitigations

| Risk | Mitigation |
| --- | --- |
| A generic plan erases source-specific ownership | Keep exhaustive private source variants and lower them directly into existing typed HIR sources. |
| Planning changes evaluation or cleanup order | Evaluate each source once; preserve exact HIR dumps and native effect/destruction tests. |
| Iteration accidentally borrows a replaceable owner | Make loop-body retention an explicit request that strengthens stable shared bindings. |
| Mutable access is widened | Require the source access to permit requested access and keep produced sources read-only. |
| Runtime narrowing becomes implicit | Direct plans accept static success only; checked casts and tests retain separate owners. |
| Diagnostics drift during extraction | Return structured problems and keep context rendering with existing consumer tests until exact equivalence is proven. |
| The module split creates navigation overhead | Use one private facade with cohesive source, plan, and relation files; avoid per-variant modules and pass-through wrappers. |
| Scope expands into every alias or HIR family | Keep non-object alias paths and all HIR/MIR representations outside A14. |

## Roadmap boundary

An implementation roadmap should stage the work by stable ownership boundary:

1. Establish characterization coverage and the private object-view facade.
   Move the relation query and extract source classification behind the current
   API without migrating consumers.
2. Introduce the typed request, retention scope, plan, and structured planning
   problems. Migrate the ordinary object alias-argument branch while leaving
   primitive, array, shared-owner, optional-owner, and checked-cast alias paths
   intact.
3. Migrate view-producing receiver carriers and iteration. Prove exact HIR,
   evaluation-order, anchor, guard, and destruction equivalence.
4. Migrate cast, type-test, and copy-construction source/relation use where it
   removes duplication without merging their result types. Remove superseded
   helpers, audit module responsibility, update living documentation only if a
   current contract was missing, and close A14.

Each stage should be independently reviewable and behavior-preserving. If
source extraction reveals that two consumers require different provenance or
retention semantics, keep separate typed adapters rather than widening the plan
with an optional field or boolean.

## Decision summary

| Question | Decision |
| --- | --- |
| Where does shared planning live? | In a private `typeck::expression::object_view` facade. |
| What is shared? | Checked object source facts, target relation, access validation, retention selection, projections, and conversion to existing `HirObjectView`. |
| What remains context-specific? | Parameter dispatch, diagnostics, receiver carriers, iteration protocol selection, checked casts, type tests, copy construction, and final HIR wrappers. |
| Which alias kinds use the plan? | Ordinary non-owning object-view aliases only. Primitive, array, shared-owner, and optional-owner aliases remain separate. |
| How are lifetimes represented? | A closed immediate-consumer or loop-body retention request selecting existing HIR anchors and guards. |
| Are runtime checks implicit? | No. Direct planning requires static compatibility; casts and tests retain explicit checked behavior. |
| Does the design change HIR, MIR, or source behavior? | No. The plan is private and consumed into existing HIR. |
| Is this one implementation PR? | No. Source ownership, alias migration, receiver/iteration migration, and checked-operation consolidation are separate reviewable stages. |
