# Explicit Array Element-List Construction Roadmap

Status: in progress; EL0 through EL5 are complete and EL6 is next.

This roadmap implements the frozen
[explicit array element-list contract](../language/ARRAYS.md#frozen-explicit-element-list-construction)
and its archived
[design record](../archive/ARRAY_ELEMENT_LIST_CONSTRUCTION_DESIGN_PROPOSAL.md).
It adds typed `T[]{...}` and `new T[]{...}` construction from source through
verified MIR and native x86-64 while preserving the implemented array owner,
lifecycle, publication, and runtime boundaries.

## Scope and invariants

- Accept explicit typed inline and shared element lists, including empty,
  single-element, nested, optional, shared-owner, and ownership-grouped forms.
- Keep the explicit array identity authoritative; do not add element-type
  inference, array covariance, numeric promotion, or a universal empty-list
  type.
- Allocate unpublished outer backing before element effects, initialize
  exactly one increasing prefix from left to right, and publish only a complete
  array.
- Treat every element position as a previously uninitialized owning
  destination. Never implement the form as default construction followed by
  assignment.
- Reuse ordinary primitive, exact-class, optional, nested-array, shared-owner,
  and optional-owner initialization semantics, including named copy versus
  produced adoption.
- Preserve existing full-expression temporary and anchor lifetime; directly
  initialized slots belong to backing rather than to the temporary sequence.
- Keep the resulting array type's default, copy, assignment, and destruction
  capabilities independent from the operations used to construct one listed
  value.
- Preserve deterministic syntax, resolved, HIR, MIR, and assembly dumps plus
  structured non-panicking rejection at every staged implementation boundary.
- Keep target layout and helper choices private, and retain runtime ABI version
  8 with no new C entry point or metadata contract.
- Do not add trailing commas, inferred array literals, fill constructors,
  generators, comprehensions, spreads, repetition, rectangular shapes,
  mutable-length vectors, exceptional unwinding, or any other deferred
  collection feature.
- Maintain facade-oriented recursive Rust modules. Substantial new syntax,
  type-checking, MIR, verifier, or backend logic belongs behind the existing
  array facade rather than enlarging unrelated orchestration owners.

## Progress

- [x] EL0 — Retain explicit element-list source structure
- [x] EL1 — Select typed destination initialization plans
- [x] EL2 — Execute verified primitive element lists
- [x] EL3 — Execute exact-class destination placement and copying
- [x] EL4 — Execute inline optional element initialization
- [x] EL5 — Execute recursively nested inline-array elements
- [ ] EL6 — Execute shared and optional-shared owner elements
- [ ] EL7 — Harden and publish the complete element-list profile

## PR-sized implementation sequence

### EL0 — Retain explicit element-list source structure

**Purpose:** Establish the complete brace-list syntax and resolved source
contract before semantic or executable representations depend on it.

- [x] Extend array construction syntax with a distinct ordered element-list
      mode retaining `new`, exact type, both brace spans, every comma span,
      every element expression, and the complete construction span.
- [x] Recognize `array-inline-type { ... }` and
      `new array-inline-type { ... }` without confusing class blocks, ordinary
      call arguments, postfix indexing, or the existing `T[](length)` and
      `T[](copy source)` modes.
- [x] Accept empty, one-element, multiline, recursively nested, ownership-
      grouped, and postfix-consumed lists; reject untyped lists and trailing
      commas under the frozen grammar.
- [x] Extend syntax nesting/depth traversal and recovery so missing elements or
      braces preserve later elements, statements, and declarations when their
      boundaries are unambiguous.
- [x] Preserve the list and its exact array identity through resolution in
      deterministic source order without selecting type compatibility,
      lifecycle, or target behavior.
- [x] Extend AST and resolved dumps plus public phase accessors without exposing
      parser-private representation details.
- [x] Add one explicit, tested semantic availability gate so newly accepted
      source cannot panic or leak malformed HIR before typed plans land.
- [x] Promote the element-list production from the grammar's frozen-extension
      section into accepted syntax while keeping feature availability frozen in
      the status matrix.

**Tests:** Focused syntax and resolver array suites; empty/single/multiple and
nested list dumps; brace/comma span assertions; postfix parsing; trailing-comma,
missing-element, missing-brace, untyped-list, and recovery matrices; syntax
nesting and independent-process resolved-dump determinism; `make check`;
`make msrv-check`.

**Exit criteria:** Every frozen source shape has one deterministic syntax and
resolved representation, malformed lists recover without compiler panics,
later phases reject through one deliberate structured gate, and no semantic
owner must reconstruct commas, ordering, ownership grouping, or array identity
from source text.

### EL1 — Select typed destination initialization plans

**Purpose:** Resolve every element's exact owning operation in HIR before MIR
or the backend can depend on expression shape.

- [x] Add an ordered element-list construction mode to HIR, with one
      destination-directed initialization plan per source expression and
      separate inline versus shared outer ownership.
- [x] Centralize reusable stored-value initialization selection so an array
      element receives the same primitive, exact-class, optional, nested-array,
      shared-owner, or optional-owner compatibility decision as the
      corresponding owning destination without duplicating local/field rules.
- [x] Retain exact initializer/copy identities, access authorization, nested
      `ArrayTypeId`, shared target, named versus produced provenance, and every
      source span needed below HIR.
- [x] Require only the operation selected by each element source; do not
      request an array default plan or element assignment merely because the
      list is nonempty.
- [x] Preserve the completed array type's independently computed lifecycle
      table for later named copy, slice, assignment, and cleanup operations.
- [x] Diagnose the exact failing element for type mismatch, inaccessible
      initializer, unavailable copy, invalid ownership target, or other
      capability failure, while continuing to check recoverable later elements.
- [x] Extend HIR control-effect traversal and deterministic dumps for ordered
      listed expressions and selected initialization plans.
- [x] Replace the semantic availability gate with a deliberate executable-
      lowering gate that rejects unsupported list HIR structurally rather than
      panicking until verified MIR support lands.

**Tests:** Type compatibility and capability matrices for every legal element
family; no-default class construction; named versus produced provenance;
private initializer access; unavailable copy without false default/assignment
requirements; exact mismatch spans; deterministic HIR dumps; structured
lowering-gate tests; `make check`; `make msrv-check`.

**Exit criteria:** HIR contains one exact ordered destination plan for every
valid listed source, invalid sources fail at their owning expression, later
phases need no overload, access, provenance, or lifecycle selection, and
compiler-wide input remains non-panicking behind the explicit execution gate.

### EL2 — Execute verified primitive element lists

**Purpose:** Establish allocation-before-effects, heterogeneous ordered
initialization, initialized-prefix verification, publication, and the first
native vertical slice over trivial element lifecycle.

- [x] Extend target-independent MIR with the minimal construction vocabulary
      needed to allocate source-count backing, address the next uninitialized
      slot, initialize it, advance the prefix, and publish inline or shared
      outer arrays only after completion.
- [x] Lower listed expressions linearly or through explicit CFG without using
      the uniform default/copy array loop to invent live placeholder elements.
- [x] Integrate element evaluation with full-expression tracking so completed
      temporaries and anchors retain their existing enclosing lifetime and
      allocation failure precedes the first element effect.
- [x] Verify exact array and primitive element types, source order, position
      uniqueness, increasing prefix, no use before initialization, complete
      publication, backing consumption, and normal cleanup.
- [x] Add verifier mutations for missing, duplicate, out-of-order, wrong-type,
      post-publication, incomplete-publication, and leaked/duplicated backing
      operations.
- [x] Extend deterministic MIR dumps, storage/lifetime use accounting,
      cleanup verification, backend legality, and control-effect handling for
      the new mode.
- [x] Execute inline and shared-outer lists of `i64`, `u64`, `u8`, `f64`, and
      `bool` on x86-64 using existing layout, checked allocation, primitive
      stores, publication, and release machinery.
- [x] Remove the execution gate for primitive lists while retaining structured
      staging for lifecycle-bearing element families.
- [x] Prove the runtime header, symbol set, panic/allocator interface, and ABI
      version remain unchanged.

**Tests:** MIR lowering and exact dump tests; initialized-prefix, lifetime,
ownership, and cleanup mutations; backend legality and assembly tests; empty,
single, multiline, side-effect-ordered, inline/shared, postfix, local/field/
argument/result/assignment primitive cases; allocation-before-effect and
allocation-failure goldens; deterministic native output; ABI compatibility;
`make check`; `make msrv-check`.

**Exit criteria:** Primitive element lists execute through verified MIR for
both outer ownership modes, allocate before any listed effect, store every
value once in source order, publish only a complete prefix, adopt through all
ordinary array destinations, clean exactly once, and add no runtime ABI.

### EL3 — Execute exact-class destination placement and copying

**Purpose:** Implement the frozen class-specific destination extension and
observable lifecycle behavior after the prefix trust boundary is proven.

- [x] Supply an element slot as the final destination for an eligible
      ungrouped exact-class construction, invoking the selected ordinary
      initializer directly without a default value, copy, assignment, or
      temporary.
- [x] Supply the slot as the final result destination for an eligible
      exact-class-returning call while preserving callee cleanup and result
      completion order.
- [x] Copy-construct from named places and otherwise materialized sources using
      the selected exact operation and existing target-directed checked source
      rules.
- [x] Preserve grouping: a grouped fresh construction materializes, requires
      the applicable copy constructor, copy-constructs the slot, and destroys
      its temporary at the enclosing full-expression boundary.
- [x] Enforce declaring-class initializer privacy at the list call site and
      diagnose unavailable copy only for source shapes that require it.
- [x] Extend MIR lifetime and class-initialization verification so a slot
      advances the prefix only after initializer, result placement, or copy
      construction completes normally.
- [x] Reuse existing x86-64 initializer, object-result, copy, destructor, and
      aligned array-element place machinery without aggregate byte copying.
- [x] Preserve user-visible constructor, copy-constructor, destructor, and
      full-expression effect order in nested lists and every owning outer
      destination.

**Tests:** Fresh, named, grouped, call-result, private-initializer, explicit-
copy, ancestor/checked-source, no-default, unavailable-copy, user lifecycle,
alignment, field, parameter/result, and inline/shared-outer matrices; exact
HIR/MIR dumps; class/prefix verifier mutations; native lifecycle traces and
reverse array destruction; `make check`; `make msrv-check`.

**Exit criteria:** Exact-class slots implement precisely the frozen direct,
result, materialized, and named-source distinctions; lifecycle effects occur
once in the specified order; no default or assignment is introduced; and
completed arrays retain their independently computed copyability.

### EL4 — Execute inline optional element initialization

**Purpose:** Compose explicit list destinations with the existing absent,
present, conditional lifecycle, and payload-destination rules.

- [x] Initialize primitive and exact-class optional slots from `none` without
      reading or constructing payload bytes.
- [x] Inject ordinary primitive and exact-class sources into the expected
      optional element type without adding a universal `none` type or any
      implicit unwrap.
- [x] Direct an eligible ungrouped exact-class construction into the present
      payload destination; preserve named/materialized conditional copy and
      grouping behavior.
- [x] Lower presence publication only after its payload is complete, and
      advance the outer array prefix only after the complete optional value is
      live.
- [x] Verify conditional payload initialization, copy identities, absence,
      outer-prefix order, cleanup, and no read/destroy of absent payload bytes.
- [x] Reuse existing x86-64 optional layout, initialization, guard, copy, and
      conditional destruction machinery for inline and shared outer arrays.

**Tests:** Primitive/class absent-present matrices; mixed lists; fresh, named,
grouped, and call-result class payloads; unavailable payload copy; optional
state/prefix verifier mutations; exact HIR/MIR dumps; native lifecycle and
reverse cleanup traces for both outer ownership modes; `make check`;
`make msrv-check`.

**Exit criteria:** Every implemented inline optional element category can be
listed with exact existing absence/injection/lifecycle semantics, payload and
outer publication remain ordered, and no optional-array or implicit-unwrap
surface is added.

### EL5 — Execute recursively nested inline-array elements

**Purpose:** Make element lists compose recursively as jagged owning values
while preserving exact nested copy and produced-backing transfer semantics.

- [x] Deep-copy each named nested inline-array source into distinct inner
      backing through its exact element copy plan.
- [x] Adopt each produced nested source, including another element-list result,
      into its outer slot without a redundant deep copy or moved-from source
      value.
- [x] Preserve exact recursive `ArrayTypeId`, named/produced provenance,
      arbitrary jagged lengths, and ownership grouping at every nesting level.
- [x] Advance the outer initialized prefix only after the complete inner array
      owner is installed, and consume each produced inner backing exactly once.
- [x] Extend nested array ownership, cleanup, and verifier mutations for lost,
      duplicate, wrong-identity, named-as-produced, and produced-as-named
      transfers.
- [x] Reuse recursive x86-64 array copy, adoption, replacement, anchor, and
      reverse destruction machinery without flattening or rectangular layout.

**Tests:** Empty and nonempty inner lists; mixed jagged lengths; named deep
copy independence; produced adoption allocation/copy counts; two or more
nesting levels; recursive class/array graphs; ownership grouping; exact dumps;
malformed transfer mutations; native nested mutation and reverse cleanup for
inline/shared outers; `make check`; `make msrv-check`.

**Exit criteria:** Nested element lists produce independent jagged values,
named sources deep-copy, produced sources transfer once, every outer prefix
contains only complete inner owners, and no rectangular or inferred collection
semantics appear.

### EL6 — Execute shared and optional-shared owner elements

**Purpose:** Complete list initialization for the remaining legal one-word
owner categories without confusing element ownership with outer array
ownership.

- [ ] Copy/retain a named shared class or exact shared-array owner into each
      listed slot according to ordinary compatible target rules.
- [ ] Transfer/adopt a produced owner into its slot without a redundant retain
      and release, including produced calls, allocations, casts, and nested
      shared-array construction.
- [ ] Initialize optional shared-owner slots as absent or present through the
      existing zero niche and conditional owner operations.
- [ ] Preserve repeated named-owner sharing versus distinct allocations from
      separate `new` expressions; do not infer freshness from list position.
- [ ] Support legal exact class/interface/`Obj` target compatibility without
      array covariance, default-target selection, or dynamic cloning.
- [ ] Verify exact owner target, named/produced accounting, optional absence,
      slot publication, outer prefix, secure cleanup, and independent outer/
      inner strong counts.
- [ ] Reuse existing x86-64 one-word element layout, retain/adopt/release,
      optional-owner, finalization, and nested shared-array machinery for both
      inline and shared outer arrays.

**Tests:** Named-repeated and distinct-produced owners; class/interface/`Obj`
targets and explicit casts; ordinary/optional owner elements; shared-array
owners; all inline/shared outer combinations; owner counts and last-owner
destructor order; missing/duplicate retain/adopt/release verifier mutations;
exact dumps; strong-cycle specified leak cases; native lifecycle goldens;
`make check`; `make msrv-check`.

**Exit criteria:** Every legal shared or optional-shared element list has exact
ordinary owner semantics, repeated named sources share while separate
producers remain distinct, outer and element ownership accounts never alias in
verification, and runtime ABI version 8 remains unchanged.

### EL7 — Harden and publish the complete element-list profile

**Purpose:** Close cross-category, recovery, robustness, determinism, and
documentation gaps before changing the feature from frozen to implemented.

- [ ] Complete syntax, resolution, type, capability, access, ownership,
      evaluation, prefix, publication, lifecycle, and consuming-context
      matrices across mixed and recursively nested element families.
- [ ] Cover empty equivalence, maximum semantic length, trailing-comma
      rejection, malformed nested recovery, postfix consumers, allocation
      failure before effects, and every supported local/field/argument/result/
      assignment destination.
- [ ] Add complete native golden coverage for left-to-right effects, named
      copy versus produced adoption, direct class placement, optional state,
      nested arrays, shared owner counts, reverse cleanup, and both outer
      ownership forms.
- [ ] Extend hostile frontend, accepted nesting-budget, malformed-MIR,
      independent-process determinism, ABI-pressure, helper-collision, and
      system-assembler coverage.
- [ ] Audit new and touched Rust modules by responsibility; keep syntax,
      resolution, typed-plan, MIR lowering, verifier, and backend logic behind
      their established facades and split any new multi-responsibility hotspot.
- [ ] Remove all staging gates, temporary unsupported diagnostics, stale frozen
      rollout prose, and feature codes from living code, tests, dumps, and
      general documentation.
- [ ] Promote grammar, arrays, class lifecycle, optionals, shared ownership,
      evaluation, errors, status, phase/IR, backend, runtime ABI, indexes, and
      test guidance to current implemented behavior without requiring the
      archived proposal or roadmap.
- [ ] Reconfirm that inferred lists, trailing commas, fill/generator syntax,
      comprehensions, spreads, rectangular shapes, vectors, exceptional
      cleanup, covariance, and every other deferred feature remain rejected.
- [ ] Record any lower-priority discovery in a separately indexed element-list
      discoveries document rather than expanding the frozen scope.

**Tests:** All focused array, object-result, optional, shared-owner,
full-expression, MIR verifier, backend legality, assembler, and native suites;
new golden feature spec and syntax-failure cases; `make check`;
`make msrv-check`; `make robustness-long`; `make golden-determinism-test`;
documentation link/index validation; artifact-free final `make check`.

**Exit criteria:** Every frozen element-list form executes through verified MIR
on x86-64 with deterministic diagnostics, dumps, ownership, effects, and
cleanup; all exclusions remain rejected; living documentation describes only
current behavior; the complete repository and extended gates pass; no runtime
ABI change occurred; and no high-priority implementation discovery remains.

## Ordering and dependencies

EL0 establishes exact source retention before any semantic representation
depends on list shape. EL1 settles all destination and lifecycle choices in HIR
before executable IR is extended. EL2 proves the allocation, prefix,
publication, full-expression, verifier, target, and ABI foundation with
primitive values. EL3 then adds the only new class destination-placement rule.
EL4, EL5, and EL6 compose independent owning families over that proven
foundation; they are ordered to establish inline conditional payloads before
recursive array transfer and to complete shared-owner accounting last. EL7
removes staging only after every category executes together and owns the sole
frozen-to-implemented documentation transition.

The roadmap depends on the implemented array, class lifecycle, optional,
shared-ownership, panic, full-expression, MIR verifier, x86-64 backend, and
runtime ABI version 8 contracts. It has no dependency on another active
roadmap. Fill, generator, inferred literal, vector, or recoverable-exception
work must proceed through separate design proposals and must not alter this
sequence opportunistically.
