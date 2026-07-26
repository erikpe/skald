# Arrays Implementation Roadmap

Status: in progress; AR6 is next.

This roadmap implements the frozen
[array language contract](../language/ARRAYS.md) and
[array compiler and runtime contract](../compiler/ARRAYS.md) from recursive
source syntax through verified MIR and Linux x86-64 native execution. It
preserves Skald's inline value semantics, deterministic lifecycle, explicit
shared ownership and dereference, optional-owner zero niche, call-scoped
aliases, minimal C runtime, and inspectable phase boundaries.

## Scope and invariants

- Implement built-in invariant `T[]`, `shared T[]`, and `shared? T[]` types,
  including grouped element ownership and recursively nested jagged arrays.
- Implement empty, default-length, and explicit-copy construction for inline
  and shared arrays. Lengths are `u64` and cannot exceed `i64::MAX`.
- Preserve deep copying for named inline sources and deterministic backing
  adoption for produced inline sources in initialization, assignment,
  arguments, results, fields, and temporaries.
- Default-initialize primitives, optionals, exact classes, inline arrays,
  concrete shared classes, and exact shared arrays according to the frozen
  capability rules. Each non-optional shared element receives one distinct
  allocation; optional shared elements default to absent.
- Implement fixed-size allocation identity, whole inline replacement with
  changing length, shallow shared-owner replacement, and rejection of
  whole-shared-pointee assignment.
- Implement immutable `len()`, signed negative-capable indexing, explicit
  shared `->[...]` projection, copied half-open slices, and checked in-place
  slice assignment.
- Implement deterministic increasing-index construction/copy/assignment and
  decreasing-index destruction, including exact-class, nested-array, shared,
  and optional element operations.
- Implement `ref T[]` and `mut ref T[]` call-scoped aliases plus exact-class
  and nested-array element alias sources. Preserve detached inline backing
  through verified hidden anchors.
- Keep array types outside class inheritance, interfaces, `Obj`, casts, type
  tests, structural indexing protocols, and external signatures.
- Keep the public C runtime array-unaware. Generated code reuses only
  `ska_rt_alloc` and `ska_rt_free`; this roadmap adds no public C symbol or
  runtime ABI version.
- Use one contiguous header-plus-elements allocation for the initial x86-64
  shared array representation. A nonempty inline backing likewise uses one
  contiguous allocation; an empty inline array requires no allocation.
- Preserve structured diagnostics, deterministic dumps, public phase facades,
  backend legality checks, and MIR verification before native lowering.
- Do not add fill/generator/literal initialization, inline optional array
  payloads, capacity or resizing of an existing allocation, slice views,
  reverse/strided slices, iteration, collection protocols, equality, external
  ABI, exceptions, concurrency, or atomic ownership.

## Repository gates

Every task runs its focused tests and `make check`. Every task that changes
Rust targets, manifests, accepted syntax, phase models, or public repository
facades also runs `make msrv-check`. The closing task additionally runs
`make robustness-long` and the full gates from an artifact-free snapshot or
clean checkout.

Use the existing test ownership:

- lexer, parser, resolution, type-checking, HIR/MIR dump, verification, and
  backend-unit coverage stay with their owning compiler modules;
- public cross-phase behavior belongs in `crates/skald-compiler/tests/`;
- reusable source corpora belong under `tests/compiler/`;
- complete success, compile-failure, and runtime-failure behavior belongs
  under `tests/golden/`; and
- the C runtime suite changes only if the public allocator contract changes,
  which this roadmap forbids.

## Progress

- [x] AR0 — Parse the complete array source surface
- [x] AR1 — Establish canonical recursive array identities
- [x] AR2 — Type array storage, construction, and lifecycle capabilities
- [x] AR3 — Type array places, projections, assignment, slices, and aliases
- [x] AR4 — Establish verified target-independent array MIR
- [x] AR5 — Execute primitive inline construction, lifetime, and length
- [ ] AR6 — Execute checked primitive indexing and mutation
- [ ] AR7 — Carry inline array values across owning boundaries
- [ ] AR8 — Execute nontrivial and nested inline element lifecycle
- [ ] AR9 — Execute shared and optional-shared outer arrays
- [ ] AR10 — Execute shared and optional-shared element lifecycle
- [ ] AR11 — Execute copied slices and checked slice assignment
- [ ] AR12 — Execute array aliases and detached-backing anchors
- [ ] AR13 — Harden and publish the complete array profile

## PR-sized implementation sequence

### AR0 — Parse the complete array source surface

**Purpose:** Establish lossless source shape and recovery before later phases
depend on array syntax or ownership grouping.

- [x] Add `[` and `]` tokens, punctuation dumps, lexical coverage, and
      malformed-bracket recovery without changing existing token boundaries.
- [x] Extend type syntax with recursive postfix `[]` and grouping that
      distinguishes `shared T[]`, `(shared T)[]`, `shared? T[]`, and nested
      combinations under the ordinary nesting budget.
- [x] Add empty, default-length, and dedicated `copy` array construction
      syntax for inline and `new` forms without treating `copy` as an ordinary
      argument.
- [x] Parse element indexing and all four slice-bound shapes as postfix
      operations, preserving bracket and colon spans.
- [x] Parse `owner->[index]` and `owner->[start:end]` distinctly from ordinary
      `owner[index]`, `owner->member`, and explicit `(*owner)[...]`.
- [x] Add a structured later-phase unsupported diagnostic so the complete
      compiler rejects parsed arrays without a panic until resolution support
      lands.
- [x] Update the implemented grammar and status boundary to describe accepted
      syntax without claiming typed or executable arrays.

**Tests:** Lexer punctuation and malformed-token tests; syntax AST/dump,
precedence, grouping, nesting-budget, and recovery tests; focused robustness
cases for unmatched brackets, missing colons, malformed omitted bounds, and
ambiguous `shared` grouping; `make check`; `make msrv-check`.

**Exit criteria:** Every frozen array source form has one lossless AST shape
and deterministic dump, malformed forms recover structurally, existing
expressions retain their parse, and the full pipeline rejects arrays only at
the deliberate post-syntax gate.

### AR1 — Establish canonical recursive array identities

**Purpose:** Give every later phase stable exact array identities without
embedding recursive heap-owned type trees or re-resolving ownership grouping.

- [x] Add a dense `ArrayTypeId` and canonical interner/table keyed by exact
      element type, with deterministic insertion and dump order.
- [x] Extend resolved types and exact shared targets with inline, shared, and
      optional-shared array cases while keeping phase type values cheap and
      stable.
- [x] Resolve recursive array types in locals, fields, parameters, results,
      constructors, and alias signatures, preserving exact source spans and
      outer-versus-element ownership.
- [x] Preserve every resolvable element identity for type checking, while
      syntactically unsupported inline optional-array spellings continue to
      fail in the parser; do not make resolution own semantic element
      eligibility.
- [x] Keep arrays out of hierarchy, interface, object-view, cast, and type-test
      tables; compatibility remains exact recursive identity.
- [x] Extend resolved dumps, public phase accessors, and independent-process
      determinism coverage for canonical nested identities.
- [x] Replace the temporary resolution gate with a structured type-checking
      gate.

**Tests:** Resolution identity/interning tests; every ownership-grouping
permutation; repeated/deep nested types; declaration-position coverage;
resolved interface/`Obj` and `unit` element identities for later rejection;
syntactically excluded inline optional-array diagnostics; resolved dump and
cross-process determinism tests; `make check`; `make msrv-check`.

**Exit criteria:** Every legal array type resolves to one canonical
`ArrayTypeId`, exact repeated spellings share identity deterministically,
illegal shapes fail before HIR, and no lower phase must inspect source grouping
or names to recover array meaning.

### AR2 — Type array storage, construction, and lifecycle capabilities

**Purpose:** Freeze all type-directed ownership and lifecycle choices in HIR
before executable places and backend layout are introduced.

- [x] Extend HIR types, declarations, locals, fields, callable signatures, and
      shared targets with exact canonical array identities.
- [x] Enforce recursive invariance and element eligibility in every internal
      owning position while continuing to reject all external array
      signatures and static storage.
- [x] Reject `unit`, bare interface/`Obj`, alias, function, and every other
      non-owning or non-storable element category with type-checker-owned
      diagnostics.
- [x] Implement a terminating fixed-point lifecycle capability analysis for
      recursive class/array graphs and exclude array edges from finite inline
      containment rejection.
- [x] Select empty, default-length, and explicit-copy construction with exact
      element default/copy/destruction plans and `u64` length requirements.
- [x] Implement default capability for primitive, optional, exact-class,
      inline-array, concrete shared-class, and exact shared-array elements;
      reject shared interface/`Obj` targets and unavailable zero-argument
      initializers.
- [x] Record named deep-copy versus produced-backing provenance and inline
      versus shared allocation separately in HIR.
- [x] Diagnose length bounds known statically, unavailable construction
      capabilities, illegal ownership combinations, and unsupported implicit
      inline/shared conversions.
- [x] Extend HIR dumps and keep MIR lowering behind a deliberate array gate.

**Tests:** Type/capability matrices; recursive class/array containment and
fixed-point tests; constructor mode and overload-selection tests; primitive,
optional, class, nested, shared, and optional-shared defaults; interface/`Obj`
rejection; internal/external position diagnostics; exact HIR dumps;
`make check`; `make msrv-check`.

**Exit criteria:** HIR contains one explicit construction and lifecycle plan
for every valid array owner, every invalid element or capability fails
deterministically, recursive array edges terminate analysis, and no MIR or
backend phase needs to select semantic operations.

### AR3 — Type array places, projections, assignment, slices, and aliases

**Purpose:** Complete the source-visible array operation model in typed HIR so
lowering can consume checked operations rather than bracket syntax.

- [x] Add intrinsic immutable `len()`, element places, copied slice results,
      slice destinations, and whole-array assignment without desugaring to
      structural method names.
- [x] Require exact `i64` element indices and supplied slice bounds, preserve
      omitted bounds, and classify every runtime normalization or failure
      operation explicitly.
- [x] Normalize `->member` and `->[...]` to one evaluated shared owner plus one
      checked array-pointee projection, reusing explicit `*owner` provenance.
- [x] Require optional shared array unwrap before projection and retain the
      secured ordinary owner through the complete immediate operation.
- [x] Select primitive store, class copy assignment, nested whole-array
      assignment, and secure shared-owner replacement for element writes.
- [x] Type whole replacement independently from equal-length slice assignment;
      whole inline assignment may change length, while shared whole-pointee
      assignment is rejected.
- [x] Type copied half-open slices, omitted and negative bounds, exact
      equal-length slice assignment, and right-side slice temporary
      materialization.
- [x] Admit `ref T[]`, `mut ref T[]`, and exact-class or nested-array element
      alias sources with access propagation, while rejecting alias-root
      rebinding and optional reference types.
- [x] Record receiver/bound/right-side evaluation order, access, source
      provenance, and required inline/shared/optional anchor category in HIR
      dumps.

**Tests:** Type and access matrices for `len`, index, slice, assignment,
shared/optional projection, and aliases; exact wrong-type diagnostics;
read-only/mutable propagation; ownership-edge counting in nested expressions;
whole-versus-slice assignment; HIR dump determinism; `make check`;
`make msrv-check`.

**Exit criteria:** Every frozen array expression and destination is one fully
typed HIR operation with exact access, lifecycle, failure, evaluation, and
anchor requirements, and all deferred collection behavior remains rejected.

**Completion summary:** Typed HIR now represents intrinsic length, checked
indices and slices, whole/element/slice destinations, class and nested element
places, shared and optional-shared pointee projections, lifecycle-selected
writes, aliases, evaluation order, runtime failure reasons, provenance, and
anchor categories explicitly. Focused array matrices, the complete compiler
suite, `make check`, and `make msrv-check` pass; array-bearing programs still
stop at the deliberate HIR-to-MIR boundary owned by the next task.

### AR4 — Establish verified target-independent array MIR

**Purpose:** Create the executable ownership and trust boundary before any
backend relies on descriptor or allocation layout.

- [x] Add canonical array declarations/types, owning descriptor and produced
      temporary storage roles, unpublished backing storage, shared owners,
      slice temporaries, and inline/shared anchor storage to MIR.
- [x] Add target-independent operations for checked allocation arithmetic,
      construction prefixes, publication, deep copy, produced adoption,
      whole replacement, element operations, destruction, and owner release.
- [x] Add signed index/slice normalization, checked projection, bounds and
      length-mismatch failure, copied-slice, and slice-assignment vocabulary.
- [x] Represent generated array loops with explicit basic blocks and storage;
      do not depend on source loop syntax or an optimization pass.
- [x] Lower array declarations and supported HIR operations to explicit MIR
      while retaining a structured backend-unsupported result.
- [x] Extend verification with array table/type integrity, initialized-prefix
      state, named-copy/produced-consumption accounting, exact element
      capability, owner joins, anchor dependencies, checked projections,
      no-write-before-slice-checks, and terminating failure edges.
- [x] Split array structural, initialized-storage, ownership, projection, and
      anchor verification behind the MIR verifier facade rather than adding
      unrelated logic to the existing optional verifier hotspot.
- [x] Add deterministic MIR dumps, smallest-valid fixtures, and test-only
      mutation hooks for every invariant.

**Tests:** MIR lowering/dump tests for every operation family; verifier tests
that mutate one type, prefix, publication, transfer, cleanup, bound, slice, or
anchor invariant at a time; backend legality rejection; public MIR facade
tests; `make check`; `make msrv-check`.

**Exit criteria:** Complete array HIR lowers to target-independent MIR with no
layout facts, malformed array MIR is rejected before target lowering, and the
x86-64 backend fails structurally rather than panicking on every verified array
operation it does not yet execute.

### AR5 — Execute primitive inline construction, lifetime, and length

**Purpose:** Establish the first native array allocation and cleanup vertical
slice over the simplest element lifecycle.

- [x] Add the initial x86-64 inline descriptor and one-block nonempty backing
      layout, including immutable length, owner/anchor state, alignment, and a
      valid allocation-free empty representation.
- [x] Add checked header, stride, padding, element-count, and total-byte
      calculations with the frozen `i64::MAX` length ceiling.
- [x] Add deterministic generated helper identity and emission infrastructure
      specialized by `ArrayTypeId`, behind cohesive backend facades.
- [x] Execute `T[]()`, `T[](length)`, and `len()` for `i64`, `u64`, `u8`,
      `f64`, and `bool`, including exact zero/false initialization in
      increasing order.
- [x] Publish only fully initialized backing, destroy primitive arrays in the
      verified lifetime order, and free each nonempty backing exactly once.
- [x] Reuse `ska_rt_alloc` and `ska_rt_free` without changing the runtime
      header, marker, symbols, or ABI version.
- [x] Keep non-local owning boundaries and indexing behind their next
      deliberate execution gates.

**Tests:** Layout and checked-arithmetic unit tests; empty and representative
runtime lengths; primitive zero/false observation; allocation/free and normal
cleanup assembly tests; overflow, maximum-length, and allocation-failure
goldens; runtime ABI compatibility test; `make check`; `make msrv-check`.

**Exit criteria:** Primitive inline array locals can be empty or dynamically
sized, report exact length, clean their backing once, and fail safely on every
size/allocation violation without any runtime ABI addition.

**Completion summary:** The x86-64 backend now represents an inline array as
one descriptor word, uses an allocation-free zero descriptor for empty arrays,
and allocates one checked header-plus-payload block for nonempty primitive
arrays. Deterministic per-array initialization and release helpers execute
zero/false construction, publication, length, and exact cleanup through the
unchanged runtime allocator ABI. Target legality still rejects indexing and
non-local owning boundaries structurally. Focused layout, assembly, native,
driver, public API, and golden coverage passes together with `make check` and
`make msrv-check`.

### AR6 — Execute checked primitive indexing and mutation

**Purpose:** Make fixed-size primitive backing useful while proving the signed
normalization and checked-place model independently of copying and calls.

- [ ] Lower `array[index]` reads and writes through one evaluated receiver and
      one evaluated exact-`i64` index.
- [ ] Implement one-time negative-relative-to-end normalization without
      negating or overflowing the minimum `i64`.
- [ ] Check normalized indices before address calculation and emit one
      unrecoverable failure edge for out-of-range access.
- [ ] Execute exact primitive load/store with verified access and alignment;
      reject mutation through read-only roots.
- [ ] Preserve zero-length, `-1`, `-length`, `length`, and minimum-`i64`
      behavior exactly across all primitive element types.
- [ ] Extend backend legality, dumps, diagnostics, and native failure
      observation for checked primitive element places.

**Tests:** Focused normalization and address tests; every positive/negative
boundary; empty arrays; minimum `i64`; read-only mutation rejection;
single-evaluation fixtures; native get/set and bounds-failure goldens;
`make check`; `make msrv-check`.

**Exit criteria:** Every primitive element access is normalized and checked
before memory access, mutable stores update only the selected slot, and invalid
indices cannot reach address calculation or return normally.

### AR7 — Carry inline array values across owning boundaries

**Purpose:** Complete the distinctive named-deep-copy and
produced-backing-adoption semantics before adding nontrivial element effects.

- [ ] Implement explicit `T[](copy source)` and implicit named deep copy into
      distinct primitive backing.
- [ ] Implement produced backing adoption in local/field initialization,
      whole assignment, arguments, results, and temporaries without an
      observable moved-from source or redundant element copy.
- [ ] Implement whole inline assignment with arbitrary source length:
      complete and secure the named copy or producer, install it, then end the
      old owner and backing.
- [ ] Preserve direct and indirect self-assignment, including exact deep-copy
      behavior for named sources and no destination length restriction.
- [ ] Extend fields, internal value parameters, results, calls, returns,
      full-expression cleanup, and internal ABI lowering with exact
      named-copy/produced-transfer ownership.
- [ ] Reject arrays in external signatures and alias-root whole replacement.
- [ ] Verify one owner account per live inline value and exactly-once
      consumption of every produced backing.

**Tests:** Local/field/parameter/result/assignment matrices; named copy
independence; produced adoption allocation/copy counts; changing lengths;
self-assignment; recursion and register/stack/result pressure; verifier
mutations; native ownership and cleanup goldens; `make check`;
`make msrv-check`.

**Exit criteria:** Primitive inline arrays obey frozen value semantics across
every internal owning boundary, named sources always remain independent,
produced backings transfer exactly once, and normal cleanup loses or duplicates
no backing.

### AR8 — Execute nontrivial and nested inline element lifecycle

**Purpose:** Extend the proven inline owner model to every deterministic
non-shared element lifecycle and recursive jagged arrays.

- [ ] Generate increasing-index default and copy construction,
      increasing-index copy assignment, and decreasing-index destruction from
      the exact element lifecycle plan.
- [ ] Execute exact-class zero-argument initialization, user/synthesized copy
      construction and assignment, destructor effects, and unavailable
      capability diagnostics.
- [ ] Execute primitive and exact-class optional elements with absent default,
      conditional copy/assignment/destruction, and no implicit inline optional
      array payload.
- [ ] Execute nested inline arrays recursively: empty inner default, deep named
      copy, produced adoption, whole inner assignment, and decreasing recursive
      cleanup.
- [ ] Integrate array fields into class synthesized lifecycle and verify that
      array edges break finite layout containment while capability fixed
      points remain deterministic.
- [ ] Track and publish exact initialized prefixes without exposing partial or
      optional element states.
- [ ] Preserve lifecycle-visible operation count and order in dumps, assembly,
      and native execution.

**Tests:** User-effecting class lifecycle arrays; synthesized operations;
missing default/copy/assignment capabilities; optional absent/present
elements; `T[][]` and deeper jagged arrays; recursive `Node[]` fields;
containment/capability cycles; partial-prefix verifier mutations; reverse
destruction goldens; `make check`; `make msrv-check`.

**Exit criteria:** Inline arrays correctly own every legal non-shared element
category, nested arrays are independent jagged values, recursive type analysis
terminates, and each visible lifecycle operation occurs exactly in the frozen
order.

### AR9 — Execute shared and optional-shared outer arrays

**Purpose:** Add shallow array allocation ownership for the already executable
primitive, optional-inline, exact-class, and nested-inline element categories
after inline backing and nontrivial finalization are established.

- [ ] Extend shared and optional-shared targets, owner storage, calls, fields,
      results, and verifier accounting to exact `ArrayTypeId` targets without
      entering class/interface polymorphism.
- [ ] Implement one contiguous x86-64 shared array allocation containing
      strong count, exact finalizer/type metadata, immutable length, padding,
      and aligned element payload; shared empty arrays still allocate a
      distinct non-null header.
- [ ] Execute `new T[]()`, `new T[](length)`, and
      `new T[](copy source)` with publication only after complete element
      initialization or copying.
- [ ] Reuse ordinary named-owner copy, produced adoption, secure assignment,
      optional zero niche, parameters/results, fields, and last-owner release.
- [ ] Generate exact decreasing-index array finalizers and free the one outer
      allocation after its final element is destroyed.
- [ ] Execute `*owner`, `owner->len()`, `owner->[index]`, and optional
      `owner!` composition with one owner evaluation and existing
      stable/copied/adopted anchor classification.
- [ ] Ensure mutation is visible through every owner, owner reassignment leaves
      other owners on the old allocation, and whole-pointee assignment remains
      rejected.

**Tests:** Shared local/field/call/result matrices; empty and nonempty
one-block layout; named/produced owner counts; optional absent/present
behavior; `*`/`->` equivalence and raw-handle rejection; mutation visibility;
last-owner element order; self-assignment; count/allocation failure; malformed
MIR; native goldens; `make check`; `make msrv-check`.

**Exit criteria:** Shared array owners preserve one exact fixed-size allocation
through all ordinary ownership boundaries, optional owners never pass zero
into ordinary operations, explicit dereference is mandatory, and last release
finalizes and frees exactly once.

### AR10 — Execute shared and optional-shared element lifecycle

**Purpose:** Complete orthogonal outer-storage and element-ownership
combinations using the existing one-word handle machinery.

- [ ] Lay out ordinary and optional shared element slots as one eight-byte word
      on x86-64, with zero as absence only for the optional form.
- [ ] Default-initialize `shared C` elements by selecting exact concrete
      zero-argument `C()`, allocating one distinct `C`, publishing its owner,
      and adopting it into each increasing-index slot.
- [ ] Default-initialize exact `shared T[]` elements as distinct empty shared
      arrays; keep shared interface/`Obj` targets non-defaultable.
- [ ] Default-initialize `shared? C` and `shared? T[]` elements as absent
      without pointee allocation or ordinary owner operations.
- [ ] Implement element copy construction, secure-before-release assignment,
      conditional optional operations, and decreasing-index release for inline
      and shared outer arrays.
- [ ] Compose nested inline/shared/optional ownership edges without confusing
      outer array ownership with element owner accounts.
- [ ] Verify each default non-optional slot adopts exactly one published owner
      and every copied/present slot contributes exactly one strong count.

**Tests:** All four inline/shared outer and ordinary/optional shared element
combinations; exact allocation counts and constructor order; one-word layout;
interface/`Obj` and missing-default rejection; owner-sharing versus backing
independence; element replacement and self-assignment; nested shared arrays;
strong-cycle specified leaks; reverse release/finalization goldens;
`make check`; `make msrv-check`.

**Exit criteria:** Every legal shared-element array is constructible under its
frozen capability rule, optional arrays allocate no default pointees, slot and
strong-count accounting are exact, and outer/inner ownership remain
independent.

### AR11 — Execute copied slices and checked slice assignment

**Purpose:** Add the frozen bulk-copy surface only after every element
construction and assignment category executes independently.

- [ ] Implement half-open slice normalization for present or omitted `i64`
      bounds, including negative-relative-to-end positions, empty ranges, and
      start-not-after-end validation.
- [ ] Execute slice reads as new inline arrays with increasing-index element
      copy construction, including reads from inline, shared, and
      optional-shared receivers.
- [ ] Execute slice assignment only after destination bounds, source
      completion/bounds, and exact length equality succeed; perform no earlier
      destination write.
- [ ] Materialize right-side slice temporaries before writes so overlapping
      ranges have snapshot semantics and lifecycle-visible
      copy/assignment/destruction order.
- [ ] Apply primitive store, class copy assignment, nested whole-array
      assignment, and secure shared-owner assignment in increasing destination
      order.
- [ ] Distinguish whole replacement from `destination[:] = source` in HIR,
      MIR, verifier state, backend lowering, diagnostics, and native behavior.
- [ ] Add only semantics-preserving trivial bulk operations; do not require
      slice fusion or a non-copying view.

**Tests:** Every omitted/positive/negative/empty/reversed bound; minimum `i64`;
length mismatch with unchanged destination; overlapping source/destination;
full-range versus whole assignment; primitive, class, nested, shared, and
optional element effects; slices from shared/optional owners; allocation and
bounds failures; native goldens; `make check`; `make msrv-check`.

**Exit criteria:** Slice reads always own distinct inline backing, slice writes
preserve destination allocation and length, every check precedes every write,
and overlap and nontrivial lifecycle effects match the frozen snapshot model.

### AR12 — Execute array aliases and detached-backing anchors

**Purpose:** Preserve nonexclusive call-scoped borrowing when inline
whole-array replacement can detach element storage.

- [ ] Execute `ref T[]` and `mut ref T[]` parameters over stable inline array
      places and explicitly dereferenced shared array places with exact access.
- [ ] Execute exact-class and nested-array element alias sources after checked
      indexing, preserving source backing and element identity for the
      complete call.
- [ ] Add compiler-generated inline backing owner and anchor accounting:
      replacement ends the source-visible owner, while element destruction and
      deallocation wait for the final dependent anchor.
- [ ] Distinguish a whole-array descriptor alias, which observes descriptor
      replacement, from an element/nested-backing alias tied to the detached
      backing selected at argument evaluation.
- [ ] Reuse stable/copied/adopted shared owner anchors and secured optional
      owners for shared-backed whole arrays and elements.
- [ ] Preserve nonexclusive overlap, receiver/argument source order, later
      invalidating argument behavior, and the prohibition on alias-root
      rebinding or escaping/local aliases.
- [ ] Extend MIR verification so every borrowed projection has one compatible
      live descriptor/backing/owner dependency and every anchor ends after its
      last consumer but before deferred destruction.

**Tests:** Read-only/mutable whole-array aliases; element/nested aliases;
stable local, field, shared, produced, and optional sources; replacement during
later argument evaluation and during the call; overlapping mutable aliases;
descriptor-observes-replacement versus element-retains-old-backing behavior;
deferred destructor order; leaked/early/reordered anchor MIR mutations; native
goldens; `make check`; `make msrv-check`.

**Exit criteria:** No array or element alias can outlive its selected storage,
replacement cannot destroy a borrowed old backing early, anchors remain
unobservable and exactly balanced, and all existing nonexclusive alias
semantics are preserved.

### AR13 — Harden and publish the complete array profile

**Purpose:** Close cross-feature gaps, prove robustness, and promote arrays
from frozen design to an implemented contract without absorbing deferred
collection work.

- [ ] Complete syntax/type/ownership/access/capability/operation diagnostic
      matrices, including malformed grouping, optional-array payloads,
      interface/`Obj` defaults, raw shared access, wrong index/bound types,
      root rebinding, and whole shared-pointee assignment.
- [ ] Add native failure coverage for every bounds, slice, count, size, and
      allocation failure; use only the contractually promised unsuccessful
      process result.
- [ ] Extend hostile frontend, nesting-budget, malformed-MIR mutation,
      independent-process determinism, internal ABI pressure, and generated
      helper collision coverage.
- [ ] Audit array modules by responsibility and keep syntax, identity,
      capability, HIR, MIR, verification, and backend implementations behind
      their established facades; split cohesive owners where required.
- [ ] Audit all non-archived documentation, update grammar/compiler/runtime/
      debugging/testing guidance, and promote arrays to **implemented
      contract** only after source-to-native support is complete.
- [ ] Prove every deferred extension remains rejected: inline optional arrays,
      rich initialization, resize/capacity, slice views, strides, equality,
      casts/tests, structural protocols, iteration, static/external arrays,
      exceptions, concurrency, and atomic ownership.
- [ ] Remove temporary gates, stale “future array” prose, rollout vocabulary,
      and roadmap task codes from living code, tests, dumps, diagnostics, and
      general documentation.
- [ ] Resolve or record lower-priority implementation discoveries in a
      separately indexed array discoveries document without expanding this
      roadmap's frozen scope.

**Tests:** Complete focused array suites; `make check`; `make msrv-check`;
`make robustness-long`; documentation link/index validation; artifact-free
full build, compiler, golden, CLI, runtime, determinism, and native execution
gates.

**Exit criteria:** Every frozen array form executes through verified MIR on
x86-64 with deterministic diagnostics and dumps, all exclusions remain
rejected, documentation describes current behavior without rollout language,
the full repository gates pass from an artifact-free snapshot, and no
high-priority array implementation discovery remains unresolved.

## Ordering and dependencies

AR0 and AR1 establish source shape and canonical identity before semantic
models can depend on arrays. AR2 settles recursive capabilities, ownership, and
construction before AR3 adds places and projections. AR4 creates one verified
target-independent execution vocabulary before any backend layout is trusted.

AR5 establishes the allocation/helper/lifetime baseline; AR6 adds checked
projection independently; AR7 then carries the proven primitive backing model
through all owning boundaries. AR8 depends on those boundaries and adds
observable element lifecycle plus nested arrays. AR9 reuses AR8 finalization
for shared outer allocation, and AR10 composes existing shared-owner machinery
inside both outer storage modes.

AR11 comes after all element assignment categories so slice semantics do not
temporarily rely on raw byte copying. AR12 follows whole replacement, nested
elements, shared owners, and checked projections because backing anchors must
cover all of them. AR13 closes diagnostics, robustness, exclusions,
documentation, and maintainability only after the complete profile executes.

The completed shared-ownership, explicit-dereference, optional-values,
constructor, lifecycle, polymorphism, and object-cast profiles are
implementation prerequisites already present in the repository. The pending
optional-verifier maintainability discovery is not a semantic dependency;
array verification must use separate cohesive owners and avoid worsening that
hotspot. Checked exceptions and source loops are not dependencies: array
prefix failure remains unrecoverable, and generated array loops use ordinary
target-independent basic blocks.
