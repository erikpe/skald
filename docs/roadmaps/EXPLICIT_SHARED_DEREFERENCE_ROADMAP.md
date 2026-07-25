# Explicit Shared Dereference Roadmap

Status: in progress; SD1 is complete and SD2 is next.

This roadmap makes the source boundary between a `shared T` owner handle and
the object behind that handle explicit. Prefix `*owner` selects a temporary
non-owning object place, while `owner->member` is convenient syntax for
selecting one member through exactly one shared edge. The implementation keeps
the current owner, anchor, `SharedPointee`, header, reference-count, cleanup,
dispatch, and ABI behavior rather than introducing another ownership or
runtime model.

The current source contract is
[Shared Ownership and Heap Allocation](../language/SHARED_OWNERSHIP.md), with
related conversion rules in [Object Casts](../language/OBJECT_CASTS.md).
The implementation boundary is
[Shared-Ownership Compiler and Runtime Contract](../compiler/SHARED_OWNERSHIP.md).
Those living documents remain authoritative for implemented behavior until
the corresponding roadmap tasks update them. Archived roadmaps and archived
design records remain historical and are not migration targets.

## Scope and invariants

- `shared T` remains a non-null owning value containing one strong handle. It
  is not itself an inline object place.
- Prefix `*source` requires `source` to have static type `shared T` and selects
  a temporary non-owning object place with static target `T`. It does not copy,
  move, slice, construct, destroy, or allocate an inline `T`.
- The dereferenced place retains the current ordinary mutable, non-exclusive
  pointee access. Sharing does not imply immutability or exclusivity.
- Postfix `source->member` is semantically equivalent to
  `(*source).member`. It evaluates `source` once and crosses exactly one
  shared edge. `.` never implicitly crosses a shared edge in the completed
  profile.
- Mixed paths expose storage structure directly:
  `root->inline_child.read()` crosses one shared edge, while
  `root->shared_child->read()` crosses two.
- Dereference is required whenever source code consumes the object behind a
  handle: field access or mutation, class or interface method receivers,
  `ref`/`mut ref` arguments, plain object casts, type tests, and owning inline
  copy consumers including explicit shared copy allocation.
- Shared-owner operations continue to consume the handle without
  dereferencing: local and field initialization, shared assignment, shared
  value parameters and results, compatible shared up-views, and
  `(shared T) source`.
- A stable shared local or value parameter continues to borrow without an
  extra retain. A replaceable shared place continues to copy an owner into a
  hidden anchor, and a produced owner continues to supply or be adopted into
  the full-expression anchor. Receiver-before-argument order, left-to-right
  argument order, result securing, and reverse anchor cleanup remain
  unchanged.
- Dereferenced class, interface, and `Obj` places remain non-storable,
  non-returnable as aliases, and bounded by their existing immediate consumer
  or full-expression lifetime. No first-class reference type is introduced.
- Whole-pointee assignment such as `*owner = source` remains invalid.
  Potential future in-place assignment must separately define behavior for a
  dynamically derived allocation viewed through `shared Base`; this roadmap
  neither reserves that syntax for execution nor invents dynamic lifecycle
  assignment.
- Field assignment through a dereferenced receiver remains supported. It may
  mutate a primitive field, copy-assign an inline subobject field, or replace a
  shared field handle according to the field's existing type and lifecycle
  rules.
- The completed compiler rejects every implicit owner-to-pointee use with a
  deterministic diagnostic that identifies the handle and points to `*` or
  `->` as appropriate. There is no permanent compatibility mode.
- MIR ownership operations, verifier lifetime invariants, x86-64 handle/header
  layout, internal calling convention, runtime ABI version, allocation
  boundary, and generated retain/release behavior remain unchanged unless an
  implementation discovery proves a small target-independent representation
  adjustment necessary to record explicit provenance.
- Current source fixtures, samples, native and compile-failure goldens, phase
  dumps, and living documentation migrate with the semantic cutover. Files
  under `docs/archive/` are deliberately not rewritten.
- Optional/nullable or weak handles, raw pointers, explicit release or count
  inspection, local aliases, whole-pointee assignment, dynamic cloning,
  external shared ABI, atomic counts, and exceptional cleanup remain outside
  this roadmap.

The sibling Niflheim draft helped establish the existing stable-owner,
replaceable-place, and produced-owner anchor categories. It still models
implicit shared-backed borrowing, so it supplies no source-syntax precedent
for this change. Skald's explicit handle-to-place boundary and existing
verified MIR remain authoritative.

## Source contract

The completed expression grammar adds unary dereference and a dereferencing
member suffix:

```text
unary-expression = "-" unary-expression
                 | "*" unary-expression
                 | object-cast-expression
                 | postfix-expression

postfix-expression = primary-expression
                     {member-suffix | dereference-member-suffix | call-suffix}
member-suffix = "." identifier
dereference-member-suffix = "->" identifier
```

Postfix `.` and `->` bind more tightly than prefix `*`, matching existing
postfix-versus-unary precedence. Thus `*owner.field` means
`*(owner.field)` and is not an implicit spelling of `(*owner).field`; source
should use the grouped form or `owner->field`. Binary multiplication remains
unambiguous by operator position, including expressions such as
`value * *owner_field`.

Representative completed forms are:

```ska
var owner: shared Leaf = new Leaf();
var replacement: shared Leaf = new Leaf();

owner->mutate();
var value: i64 = owner->value;
inspect(*owner);
var copied: Leaf = Leaf(copy *owner);
var allocated: shared Leaf = new Leaf(copy *owner);
var matches: bool = *owner is Leaf;

var narrowed_owner: shared Leaf = (shared Leaf) erased_owner;
var copied_view: Leaf = (Leaf) *erased_owner;

owner = replacement;
```

The first group operates through a borrowed pointee place. The two casts
deliberately distinguish an owner-preserving handle operation from a checked
borrowed view. The final assignment replaces the owner handle; it does not
invoke `Leaf` copy assignment on either allocation.

## Progress

- [x] SD0 — Centralize the shared handle-to-place semantic boundary
- [x] SD1 — Add explicit dereference syntax and direct object access
- [ ] SD2 — Integrate dereference with every object-place consumer
- [ ] SD3 — Require explicit dereference and publish the completed profile

## PR-sized implementation sequence

### SD0 — Centralize the shared handle-to-place semantic boundary

**Purpose:** Give the current implicit shared-backed uses one explicit
target-independent semantic owner before new syntax and diagnostics depend on
it, without changing accepted source behavior.

- [x] Extract one type-checking operation that converts a checked shared-owner
      source into a non-owning class/interface/`Obj` view with its static
      target, mutable access, complete-object origin, projections, source
      provenance, span, and anchor requirement.
- [x] Route class receivers, interface receivers, alias arguments, plain
      checked-place sources, type tests, field access, and owning inline copy
      sources through that operation instead of independently recognizing
      shared expressions.
- [x] Keep named-place versus produced-owner classification in the existing
      shared-source checker and preserve stable, copied-anchor, and
      adopted-produced behavior exactly.
- [x] Make the HIR distinction between an owner value and a borrowed pointee
      view readable enough that later phases never need source-shape or
      expected-type inference to discover a dereference.
- [x] Reuse `HirObjectView`, shared origins, checked-view carriers,
      `MirPlaceBase::SharedPointee`, and `SharedAnchor` where they already state
      the required invariant. Do not add a parallel MIR place or ownership
      state machine.
- [x] Preserve source diagnostics, evaluation and cleanup order, object IDs,
      HIR/MIR verification, backend behavior, native observations, and
      deterministic dumps except for intentional vocabulary that exposes the
      centralized boundary.
- [x] Update
      [Compiler Phases and Intermediate Representations](../compiler/PHASES_AND_IR.md)
      and the
      [shared-ownership implementation contract](../compiler/SHARED_OWNERSHIP.md)
      in this task if the target-independent representation or dump vocabulary
      changes; do not claim new source syntax yet.
- [x] Add focused type-check and MIR tests for class/interface/`Obj`, stable
      locals and parameters, replaceable and nested shared fields, produced
      allocations and calls, inline subobjects, casts, type tests, and every
      anchor category.

**Tests:** Focused shared-ownership type-check and MIR suites, deterministic HIR
and MIR dumps, verifier mutations, backend shared-ownership tests, then
`make check`.

**Exit criteria:** Every current implicit pointee use passes through one
reviewable handle-to-place semantic operation, while accepted source and
source-to-native behavior remain unchanged.

### SD1 — Add explicit dereference syntax and direct object access

**Purpose:** Implement `*` and `->` as a vertical slice for direct field,
method, interface, and type-test consumers before changing the remaining
object-source contexts.

- [x] Extend unary parsing with prefix `*` and postfix parsing with `->member`,
      reusing the existing `Star` and `Arrow` tokens while preserving exact
      operator/member spans, nesting-budget accounting, deterministic
      recovery, and source-shaped AST dumps.
- [x] Preserve `.` versus `->` in source AST. Normalize `->member` to one
      explicit dereference plus member selection only after the source-shaped
      boundary, without evaluating the receiver twice or inventing a source
      `*` span.
- [x] Add resolved explicit-dereference vocabulary carrying the resolved
      shared source and class/interface/`Obj` static target. Remove ad hoc
      call/allocation/cast shape inspection from direct receiver selection
      where the explicit node supplies that fact.
- [x] Accept `(*owner).field`, `owner->field`, class methods,
      virtual/interface dispatch, inherited and inline-field projection,
      supported field mutation, nested `.`/`->` paths, produced receivers, and
      `*owner is T`.
- [x] Require each dereference operand to have a shared target and report a
      focused diagnostic for primitives, inline objects, aliases, `unit`, and
      other non-handle expressions. Dereference never performs an inline copy
      or shared-owner transfer by itself.
- [x] Keep existing implicit shared member and type-test forms temporarily
      executable during this staged task so unrelated source fixtures need not
      migrate before the complete consumer matrix exists.
- [x] Prove explicit stable, replaceable, nested, and produced receivers lower
      to the same direct borrow or hidden-anchor lifetimes as their temporary
      implicit equivalents, including receiver-before-argument evaluation and
      result-before-anchor-release order.
- [x] Update the implemented grammar and focused living language/compiler
      documents for the newly accepted explicit direct-access forms while
      accurately retaining the temporary implicit compatibility boundary.
- [x] Add lexer/parser precedence, recovery, AST/resolved dump, resolution,
      type-check, diagnostic, HIR/MIR, verifier, backend, and native coverage,
      including `value * *owner_field`, `make()->method()`, shared interface
      calls, and paths that cross multiple shared edges.

**Tests:** Focused syntax, resolution, shared type-check, MIR, backend, and
golden suites, followed by `make check`, `make msrv-check`, and
`make robustness-long`.

**Exit criteria:** Explicit dereference executes for every direct member and
type-test use on x86-64 with current lifetime behavior, exact precedence, and
verified one-edge `->` semantics.

### SD2 — Integrate dereference with every object-place consumer

**Purpose:** Complete the explicit handle-to-place matrix so the final task can
remove implicit conversions without losing any supported source capability.

- [ ] Accept explicit dereferenced places as `ref` and `mut ref` arguments,
      preserving call-scoped access restriction, forwarding rules,
      non-exclusivity, and hidden anchor selection.
- [ ] Require plain checked casts over a shared allocation to consume the
      dereferenced place, as in `(Dog) *animal_owner`, while keeping
      `(shared Dog) animal_owner` an owner operation over the handle.
- [ ] Accept dereferenced class places in the existing target-directed inline
      local/field initialization, value-parameter, result, copy-construction,
      slicing, and owning-destination copy-assignment source paths.
- [ ] Accept dereferenced checked sources in `T(copy *owner)` and
      `new T(copy *owner)`, preserving source-once evaluation, dynamic check
      before destination allocation, selected exact-`T` copy construction,
      source lifetime through completion, and result securing before cleanup.
- [ ] Cover shared fields, nested replaceable places, shared up-views,
      produced calls and allocations, and class/interface/`Obj` targets across
      each compatible consumer. Interface and `Obj` dereferences remain valid
      only in view-consuming contexts without standalone inline storage.
- [ ] Keep shared local/field initialization, shared assignment, shared
      arguments/results, implicit compatible owner up-views, and shared-owner
      casts operating directly on the handle. Reject unnecessary dereference
      where a shared owner is required rather than silently re-owning a
      borrowed place.
- [ ] Reject `*owner = source` with a dedicated whole-pointee-assignment
      diagnostic. Continue to accept `owner->field = source` wherever the
      selected field's existing mutation and lifecycle policy permits it.
- [ ] Keep legacy implicit owner-to-pointee forms temporarily accepted only
      until the final cutover, and add equivalence tests proving each explicit
      form produces the same ownership, anchoring, failure, dispatch, copy,
      cleanup, and native observation.
- [ ] Update affected focused living documentation for the completed explicit
      consumer matrix and the deferred whole-pointee-assignment boundary.
- [ ] Add focused negative coverage for owner/pointee confusion, non-shared
      dereference, access restriction, impossible checked relations,
      unsupported interface/`Obj` owning destinations, and whole-pointee
      assignment.

**Tests:** Focused alias, object-cast, lifecycle-copy, shared copy-allocation,
type-check, MIR verification, backend, deterministic-dump, compile-failure,
and native suites, followed by `make check`, `make msrv-check`, and
`make robustness-long`.

**Exit criteria:** Every currently supported way to borrow, inspect, mutate, or
copy from a shared pointee has an explicit `*` or `->` spelling with verified
behavior equivalent to the existing implementation, while handle operations
remain syntactically and semantically distinct.

### SD3 — Require explicit dereference and publish the completed profile

**Purpose:** Make the explicit boundary the sole accepted source contract,
remove superseded inference, migrate the repository, and leave all living
documentation authoritative.

- [ ] Reject `.member`, method/interface receiver, alias argument, plain cast,
      type test, and inline-copy uses that attempt to consume a raw `shared T`
      handle as an object place. Diagnostics must identify the owner type and
      recommend `->` for member selection or `*` for a general place consumer.
- [ ] Remove resolver and type-checker fallback branches that manufacture a
      shared-backed object receiver or view without an explicit dereference.
      Rename residual `SharedExpression`-style vocabulary where it obscures
      the now-explicit source boundary.
- [ ] Preserve direct handle operations and verify diagnostics do not
      recommend dereference for shared assignment, shared parameters/results,
      compatible shared up-views, or `(shared T) source`.
- [ ] Migrate all current Rust source fixtures, top-level `.ska` corpora,
      samples, run and compile-failure goldens, public-facade fixtures,
      deterministic phase dumps, and robustness seeds. Do not edit historical
      examples or milestone wording under `docs/archive/`.
- [ ] Add exact compile-failure goldens for every removed implicit category,
      ambiguous `.`/`->` paths, non-shared `*`, and deferred
      whole-pointee assignment. Keep diagnostics deterministic across repeated
      compiler processes.
- [ ] Update all affected living source authorities, including `README.md`,
      the language overview, grammar, types/values, functions/control flow,
      classes/lifecycle, aliases/ownership, shared ownership, object casts,
      polymorphism, and status matrix. State that `.` stays within an inline
      place, `->` crosses one shared edge, and dereference yields a bounded
      non-owning place rather than inline ownership.
- [ ] Update all affected living implementation and development authorities,
      including compiler phases/IR, the shared-ownership compiler contract,
      debugging/dump vocabulary, and testing guidance. Audit backend and
      runtime documents, but do not revise representation or ABI claims when
      behavior is unchanged.
- [ ] Confirm the final source migration does not change allocation count,
      retain/release order, last-owner destruction, hidden-anchor count and
      order, dispatch target, cast failure timing, copy/slicing behavior,
      generated shared ABI, or native output.
- [ ] Remove temporary compatibility wording from living documentation and
      tests. Record whole-pointee assignment only as a deferred/open direction,
      not an implemented capability.
- [ ] Run documentation link/index validation, formatting and diff hygiene,
      the complete repository gate, the supported-toolchain gate, and the
      extended deterministic robustness suite.

**Tests:** Exact frontend diagnostics and phase dumps; focused syntax,
resolution, type-check, MIR verifier, backend, public API, and golden suites;
then `make check`, `make msrv-check`, `make robustness-long`, and
`git diff --check`.

**Exit criteria:** No valid source program relies on implicit shared
dereference; every living example and contract uses `*`/`->`; archived history
is untouched; all owner, anchor, MIR, backend, runtime, ABI, and native
invariants remain satisfied.

## Ordering and dependencies

This roadmap depends on the completed shared-ownership, object-cast,
polymorphism, constructor, alias-parameter, and deterministic-cleanup
profiles. It has no dependency on another active roadmap.

SD0 centralizes the existing semantic boundary without a source break, reducing
the risk that syntax work creates another view or anchor path. SD1 establishes
the parser, resolved identity, precedence, and direct vertical slice. SD2 then
completes every expected-type and copying consumer while implicit forms still
provide a comparison oracle. SD3 performs one deliberate breaking cutover,
migrates current sources and living documentation, and removes the obsolete
inference only after explicit syntax covers the complete retained profile.

No task changes runtime representation or ABI. Backend work is limited to
proving that the newly explicit source forms lower to the already verified
places and operations.
