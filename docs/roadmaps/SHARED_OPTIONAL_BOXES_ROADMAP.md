# Shared Optional Boxes Roadmap

Status: in progress; BX0-BX5 are complete and BX6 is next.

This roadmap implements the frozen
[shared optional box language contract](../language/OPTIONAL_VALUES.md#shared-optional-boxes)
and
[compiler representation](../compiler/OPTIONAL_VALUES.md#frozen-shared-optional-box-representation).
It makes `shared P?` a non-null owner of one immutable published optional
wrapper, derives `shared? P?` as the existing optional-owner shorthand, and
preserves class/base/interface/`Obj` polymorphism for object boxes. It builds
on the completed compositional optional, shared ownership, explicit shared
dereference, object cast, array, and static-field profiles rather than adding
parallel type, lifecycle, ownership, or allocation systems.

The archived
[design record](../archive/SHARED_OPTIONAL_BOXES_DESIGN_PROPOSAL.md) preserves
the confirmed SB1 through SB13 discussion. This roadmap schedules that frozen
outcome; it does not reopen those decisions.

## Scope and invariants

### Source and type model

- `shared P?` is `Shared<Optional<P>>`: one non-null strong owner of an
  allocation containing a complete canonical `P?` wrapper.
- `(shared P?)?` is `Optional<Shared<Optional<P>>>`; `shared? P?` is exact
  source shorthand for the same type. No conversion, overload distinction,
  layout distinction, or second identity is introduced.
- Optional layers remain literal and arbitrarily compositional within the
  existing syntax budget. `(shared P)??`, `(shared P?)??`, `shared P??`, and
  `shared? P??` remain distinct.
- Exact primitive, inline-array, shared-owner, nested optional, and other
  non-object box targets are invariant. Object boxes fix one exact concrete
  dynamic class and permit compatible class/base/interface/`Obj` static views.
  Bare owning interface and `Obj` optionals remain invalid.
- `new P?()` constructs an absent exact wrapper. `new P?(expression)` accepts
  exactly one wrapper initializer and reuses the existing absent, one-layer
  injection, `some`, copy, transfer, or direct-payload plan. It is not an
  ordinary `P` initializer argument list and has no `copy` construction mode.
- Failure-capable target-directed source checks precede destination allocation
  where the selected source plan permits. Publication occurs only after the
  complete exact wrapper is initialized.

### Ownership, access, and lifecycle

- Named box owners copy/retain one allocation; produced owners transfer/adopt;
  assignment secures a replacement owner before release; last release runs the
  exact recursive optional finalizer and frees the original base once.
- `new P?(*box)` creates an independent allocation through the ordinary
  optional-copy plan when available. No implicit operation combines owner copy
  with payload copy, and there is no copy-on-write behavior.
- Prefix `*` is required before presence, owning wrapper copy, read-only
  eligible aliasing, or checked unwrap. `is`, `!`, `.`, and `->` do not forward
  through a box owner.
- The published wrapper is shallowly immutable. Whole-pointee assignment and
  mutable whole-wrapper aliases are always rejected, including through exact
  object-box views. Owner variables, fields, statics, and array slots may be
  repointed to another compatible box without changing the old allocation.
- An absent box remains absent for its allocation lifetime. A present
  contained object or mutable aggregate retains its ordinary internal mutation
  operations; this feature is not a shared mutable optional cell.
- Non-owning access composes an existing owner or hidden owner anchor with the
  existing optional guard. Atomic guards, threads, data-race safety, escaping
  pointers, first-class references, and optional references remain excluded.
- Object-box unwrap retains exact dynamic metadata for virtual/interface
  dispatch, type tests, and checked casts. Static up-views and successful
  downcasts preserve the allocation; casts never allocate, copy the wrapper,
  or change presence.

### Representation, ABI, and quality

- Exact optional allocation identity and static polymorphic box-view identity
  remain distinct wherever their information differs. Deterministic interned
  IDs and canonical dumps are required; exact implementation-private Rust
  names remain flexible.
- Shared-target consumers use explicit capability queries for owner, object,
  array, and optional-place operations. No code may assume every non-array
  target is an object.
- HIR records typed box construction, selected optional plans, owner
  provenance, static view, exact target knowledge, access, anchors, and spans.
  MIR records a distinct optional-box allocation origin and explicit
  allocate/initialize/publish/adopt transitions.
- The x86-64 owner remains one word. The existing 16-byte header precedes the
  target-layout placement of `P?` at offset 16. Deterministic exact box
  descriptors select optional finalization and, for object boxes, preserve
  dynamic class and view membership.
- `(shared P?)?` uses the existing one-word zero niche outside the allocation;
  it is independent from the inner optional wrapper state.
- Runtime ABI version 9, the public header, allocation/free entry points, and
  common failure reporter remain unchanged. There is no checked box-store
  failure or new runtime service.
- Diagnostics preserve exact `shared`, shorthand `?`, target `?`, grouping,
  `new`, dereference, assignment, and initializer spans. Syntax, resolved, HIR,
  MIR, assembly, and native observations remain deterministic.
- General non-optional boxes, mutable shared optional cells, external box ABI,
  weak ownership, custom allocators, dynamic cloning, concurrency, recoverable
  failures, generics, and standard-library collection redesign are non-goals.
- Substantial Rust work follows the repository's facade-oriented recursive
  module structure. New identity, construction, verification, descriptor, or
  access responsibilities belong behind focused existing facades.

## Progress

- [x] BX0 — Retain box allocation syntax and canonical resolved targets
- [x] BX1 — Select typed box ownership and construction plans
- [x] BX2 — Verify optional-box allocation and owner lifetimes in MIR
- [x] BX3 — Execute primitive optional boxes on x86-64
- [x] BX4 — Execute exact lifecycle-bearing box targets
- [x] BX5 — Add explicit immutable box-pointee access
- [ ] BX6 — Execute polymorphic object-box views and dispatch
- [ ] BX7 — Integrate box owners with stored and callable positions
- [ ] BX8 — Integrate box owners with arrays and default elements
- [ ] BX9 — Harden and publish shared optional boxes

## PR-sized implementation sequence

### BX0 — Retain box allocation syntax and canonical resolved targets

**Purpose:** Establish one source-shaped frontend and deterministic identity
boundary before type checking or executable phases depend on box meaning.

- [x] Extend allocation-expression parsing with a complete grouped/postfix
      optional target for `new P?()` and `new P?(expression)`, distinct from
      class and array construction and limited to zero or one expression.
- [x] Preserve `new`, grouping, every relevant `?`, parentheses, initializer,
      comma/recovery, and complete-expression spans through AST and resolution.
- [x] Resolve the class named at the leaf of an object-box allocation exactly;
      reject interface/`Obj` allocation targets while retaining them as static
      owner views.
- [x] Extend resolved shared targets with exact optional allocation targets and
      a deterministic optional-object view identity carrying optional depth and
      class/interface/`Obj` leaf where `OptionalTypeId` lacks that information.
- [x] Normalize `shared? P?` to `(shared P?)?` without losing shorthand
      provenance; preserve arbitrary outer optional layers and canonical dump
      spelling.
- [x] Centralize resolved target capabilities so object, array, optional-place,
      and generic owner consumers are exhaustive and cannot silently classify
      an optional box as an ordinary object.
- [x] Replace the current resolution exclusion with one deliberate
      type-checking availability gate; valid new resolved forms must not reach
      incomplete HIR or panic lower phases.
- [x] Update grammar/current-availability wording as parser and resolution
      support changes without claiming typed or executable box support.

**Tests:** Lexer/parser allocation matrices; type grouping and precedence;
zero/one/multiple initializer recovery; exact spans; syntax and resolved dumps;
canonical/shorthand and nested identity interning across modules; concrete
versus interface/`Obj` allocation diagnostics; target-capability exhaustiveness;
frontend robustness mutations; `make check`; `make msrv-check`; and
`make robustness-long`.

**Exit criteria:** Every frozen type and allocation spelling reaches one
deterministic resolved identity and construction node, malformed forms recover
without panics, all lower phases remain protected by one structured gate, and
no consumer reconstructs target meaning from source syntax.

### BX1 — Select typed box ownership and construction plans

**Purpose:** Give HIR a complete target-independent account of box ownership,
compatibility, and exact wrapper construction before MIR owns lifetime state.

- [x] Extend HIR shared targets and type rendering with exact optional targets
      and optional object views, preserving the distinction between static view
      and known exact allocation target.
- [x] Generalize the existing shared compatibility facade for exact invariant
      boxes and class/base/interface/`Obj` object-box up-views, impossible
      relations, and later checked downcasts.
- [x] Add a typed optional-box producer carrying exact target, static result
      view, selected optional initialization/copy/transfer plan, source order,
      owner provenance, and allocation/publication spans.
- [x] Select zero-expression absent construction and one-expression ordinary
      optional initialization without requesting unavailable assignment or
      mutable-container capabilities.
- [x] For exact object allocation, enforce `new Derived?(Derived())` versus
      invalid `new Derived?(Base())`; perform possible target-directed source
      checks before allocation and retain direct destination construction where
      the existing optional plan allows it.
- [x] Enable local box owners, named copy, produced adoption, compatible owner
      assignment, and independent `new P?(*source)` payload copying in HIR.
- [x] Reject whole-pointee assignment, mutable whole-wrapper aliases, implicit
      forwarding, external signatures, invariant target conversions, and
      unavailable construction/copy lifecycle with focused spans.
- [x] Retain structured gates for dereference, polymorphic execution, broader
      stored positions, MIR, and backend work owned by later tasks.

**Tests:** Type-check compatibility and diagnostics; primitive/class/array/
shared-owner/nested optional plan selection; absent/injection/`some`/copy/
produced sources; exact object construction and pre-allocation checks; owner
copy versus independent box construction; forbidden assignment/alias/
forwarding/external cases; deterministic HIR dumps; `make check`; and
`make msrv-check`.

**Exit criteria:** HIR states every enabled local owner and construction
operation without source-shape inference or target layout, invalid operations
fail at their semantic owner, and executable phases remain deliberately gated.

### BX2 — Verify optional-box allocation and owner lifetimes in MIR

**Purpose:** Establish explicit allocation, initialization, publication, and
owner accounting before a backend can realize any optional box.

- [x] Extend MIR shared targets and metadata lowering deterministically from
      HIR, preserving exact optional targets and static optional-object views.
- [x] Add a distinct optional-box allocation origin and exact unpublished
      allocation storage whose `SharedAllocationPayload` place has the
      canonical `OptionalTypeId`.
- [x] Lower box construction in source order as source evaluation/checks,
      allocate, exact optional initialization, publication, produced-owner
      adoption, and full-expression cleanup.
- [x] Reuse existing optional initialization/publication instructions and
      recursive lifecycle plans at the unpublished payload instead of adding
      payload-category-specific box instruction families.
- [x] Execute target-compatible local owner copy, move, secure-before-release
      assignment, temporary cleanup, and last-owner obligation in MIR while
      keeping pointee access gated.
- [x] Extend shared structural/ownership/lifetime/place/cleanup and optional
      initialization verification for exact target, allocation origin,
      pre-publication access, single publication/adoption, owner balance,
      wrapper completion, and CFG agreement.
- [x] Extend deterministic MIR dumps, use accounting, static-lifecycle
      discovery hooks, and backend legality with a deliberate unsupported-box
      target gate.
- [x] Add mutation helpers and malformed-MIR cases without exposing private
      verifier state through public compiler APIs.

**Tests:** Exact MIR dumps and ordering assertions; named copy and produced
adoption; owner replacement/self-assignment; nested source temporaries;
mutations for wrong target/origin/place, initialization before allocation,
publication before completion, duplicate/missing publication/adoption/release,
pre-publication observation, owner loss, and CFG mismatches; backend-gate tests;
`make check`; and `make msrv-check`.

**Exit criteria:** Verified MIR proves the complete normal lifetime of local
optional-box owners and unpublished payloads, malformed states are rejected
before every backend, and no target-dependent offset or helper enters MIR.

### BX3 — Execute primitive optional boxes on x86-64

**Purpose:** Establish the descriptor, layout, finalizer, allocation, and
one-word ABI model with the smallest payload lifecycle before aggregates and
objects depend on it.

- [x] Extend target layout with checked header-plus-optional placement,
      alignment, addressability, and size-overflow calculations for primitive
      optional targets.
- [x] Emit deterministic exact optional-box descriptor symbols and compatible
      no-op primitive finalizer entries without merging descriptor identity.
- [x] Lower verified allocate/initialize/publish/adopt, owner copy/move/release,
      secure assignment, and last-owner exact-base free for primitive boxes.
- [x] Classify `shared P?` parameters/results as one integer word in backend
      legality even while broader callable source support remains gated.
- [x] Preserve checked allocation failure and common runtime trace attribution;
      add no runtime symbol, header declaration, or ABI marker change.
- [x] Keep aggregate payloads, pointee access, and object views behind explicit
      structured backend gates.
- [x] Add narrow source-to-native or MIR-to-native observations proving
      allocation identity, owner replacement, alias retention of the old box,
      and exactly-once deallocation without requiring unpublished access syntax.

**Tests:** Layout and overflow unit tests; descriptor/finalizer identity and
assembly tests; ownership-count and allocation-failure injection; register/
stack classifier tests; exact-base deallocation and owner-alias native traces;
runtime header/symbol/version checks; assembly acceptance and determinism;
`make check`; and `make msrv-check`.

**Exit criteria:** Primitive optional boxes complete their verified native
owner lifetime with deterministic metadata and unchanged runtime ABI, while
every unsupported payload or access form still fails structurally.

### BX4 — Execute exact lifecycle-bearing box targets

**Purpose:** Reuse recursive optional lifecycle for every invariant exact box
target before polymorphic object views add a second identity dimension.

- [x] Generalize box layout, descriptor, construction, copy, and finalization
      over exact class, inline-array, shared-owner, nested optional, and
      optional-array targets using canonical optional metadata.
- [x] Generate one deterministic finalizer per referenced exact optional box
      target that conditionally destroys inline objects/arrays, releases inner
      owners, and recursively handles nested wrappers before freeing the box.
- [x] Preserve allocation-before-initialization, publication-after-completion,
      direct exact-class payload construction, named optional copy, produced
      transfer, and source checks before destination allocation.
- [x] Make `new P?(*source)` create independent wrapper/payload storage and
      require only the selected optional copy capability.
- [x] Integrate box lifecycle dependencies with closed-world finalizer and
      static-lifecycle planning without confusing object, array, and exact
      optional allocation descriptors.
- [x] Reject layout/alignment/addressability overflow and unavailable recursive
      lifecycle operations deterministically before invalid native emission.
- [x] Retain exact-only object box compatibility and the explicit pointee-access
      gate until the responsible tasks complete.

**Tests:** Side-effect-visible exact-class destruction; optional arrays and
present-empty arrays; inner shared-owner retain/release; depth-two and
depth-five wrappers; absent and every present nesting shape; independent copy
allocation; construction-check-before-allocation; descriptor/finalizer order;
malformed metadata and cleanup; layout overflow; native lifecycle traces;
`make check`; and `make msrv-check`.

**Exit criteria:** Every exact eligible optional target can be boxed, copied
into an independent allocation when capable, and finalized exactly once on
x86-64 without payload-specific ownership systems or runtime ABI changes.

### BX5 — Add explicit immutable box-pointee access

**Purpose:** Expose the frozen observation and unwrap model while proving that
published wrapper mutation cannot enter HIR, MIR, aliases, or target code.

- [x] Extend explicit shared dereference type checking so `*box` yields an
      exact optional pointee place with stable, copied-field, or
      adopted-producer owner provenance.
- [x] Enable presence tests, owning wrapper copies, one-layer checked unwrap,
      primitive extraction, optional-array access, and ordinary present-object
      or mutable-aggregate consumers after explicit dereference.
- [x] Permit call-scoped read-only `ref P?` aliases for exact box wrappers;
      reject `mut ref P?`, whole-wrapper aliases from polymorphic views,
      aliases to shared owners, optional references, and escaping access.
- [x] Compose hidden owner anchors with existing optional guards through the
      complete immediate consumer, including replaceable fields, produced
      owners, and outer optional box owners once available.
- [x] Reject `*box = source`, `box!`, direct presence tests, `box.member`,
      `box->member`, and other implicit forwarding with focused operator and
      target spans.
- [x] Extend HIR/MIR places, guard and anchor verification, backend address
      lowering, termination paths, dumps, and cleanup without adding a
      published optional assignment instruction.
- [x] Preserve shallow mutability of an already-present contained object or
      aggregate while keeping every wrapper layer fixed.

**Tests:** Stable/field/produced owners; presence and primitive unwrap; exact
wrapper copy; read-only aliases; optional arrays; contained object mutation;
outer/inner failure order; guard overflow; anchor-before-guard and
guard-before-release mutations; whole-pointee/mutable-alias/forwarding
rejections; absent unwrap native failures; deterministic dumps; `make check`;
and `make msrv-check`.

**Exit criteria:** Every frozen exact-box observer executes through explicit
verified owner and guard lifetimes, no source or malformed MIR can mutate the
published wrapper, and no implicit operation crosses the shared edge.

### BX6 — Execute polymorphic object-box views and dispatch

**Purpose:** Restore the defining polymorphic behavior of shared objects while
preserving one immutable exact optional allocation.

- [ ] Enable class/base/interface/`Obj` optional-box owner up-views through the
      centralized compatibility relation for named and produced sources.
- [ ] Retain exact dynamic class and optional target in allocation descriptors
      even while absent; validate deterministic class and interface membership
      evidence separately from the static owner view.
- [ ] Make checked unwrap through a static box view produce the corresponding
      guarded object view with complete address, access, owner anchor, and
      dynamic metadata.
- [ ] Execute base fields, virtual calls, interface calls, `Obj` consumers, type
      tests, static upcasts, possible checked downcasts, and impossible-relation
      diagnostics without allocation or wrapper copying.
- [ ] Preserve `Implementation` dispatch through
      `shared Interface? = new Implementation?(...)` and `Derived` dispatch
      through `shared Base? = new Derived?(...)`, both absent and present.
- [ ] Support target-directed owning copy from a polymorphic object-box wrapper
      only into an eligible exact inline optional destination, with deliberate
      slicing; keep interface/`Obj` bare optional destinations invalid.
- [ ] Verify static view, exact descriptor, complete-object origin, cast result
      ownership, checked-view/guard/anchor order, and dispatch metadata through
      MIR and backend legality.
- [ ] Prove whole-pointee assignment remains a compile-time error through both
      exact and up-viewed owners; add no dynamic checked store or failure.

**Tests:** Class/base/interface/`Obj` owner transfers; absent and present
views; virtual and interface dispatch; fields and mutable methods; type tests;
static/runtime casts; impossible casts; named/produced cast ownership;
exact-inline optional slicing; interface/`Obj` copy rejection; metadata and
view verifier mutations; stack/recursion dispatch pressure; native success and
cast/unwrap failures; `make check`; and `make msrv-check`.

**Exit criteria:** Optional object boxes have the same owner-view and dynamic
dispatch behavior as ordinary shared objects, every allocation keeps one exact
class and immutable wrapper, and covariance introduces no store path.

### BX7 — Integrate box owners with stored and callable positions

**Purpose:** Carry the completed owner and access model across ordinary stored
and internal callable boundaries before arrays add generated multiplicity.

- [ ] Enable `shared P?` locals, fields, explicitly initialized statics,
      temporaries, internal value parameters/results, methods, interfaces,
      overrides, and initializer overloads for every eligible exact or object
      box view.
- [ ] Execute `(shared P?)?` and `shared? P?` through the existing
      optional-owner zero niche, including arbitrary additional outer optional
      layers, `none`, `some`, copy/adopt/release, and secured unwrap.
- [ ] Integrate box-owner field initialization/replacement, synthesized class
      copy/assignment/destruction, inheritance, containment, and strong-cycle
      behavior with ordinary shared edges.
- [ ] Integrate explicit static initialization, publication dependencies,
      replacement, normal reverse shutdown, and box finalizer reachability;
      initializer-free plain box statics remain invalid because the owner is
      non-null.
- [ ] Carry one-word owners through internal calls/results, recursion,
      virtual/interface signatures, register/stack pressure, caller/callee
      cleanup, and result securing without an aggregate hidden destination.
- [ ] Preserve owner anchors for replaceable fields, statics, produced values,
      and outer optional-owner unwrap before inner optional access.
- [ ] Continue rejecting external box signatures, aliases whose designated
      type is a box owner, module/top-level globals, and unsupported static
      defaults.

**Tests:** Every stored/callable position; named/produced arguments and
results; fields and inheritance; synthesized lifecycle; owner replacement with
old aliases; outer and inner absence combinations at multiple nesting depths;
static dependency/order/shutdown; recursive and mixed ABI pressure; virtual/
interface signatures; strong cycles; malformed call/static/field MIR;
deterministic HIR/MIR/native traces; `make check`; and `make msrv-check`.

**Exit criteria:** Box owners and optional box owners obey ordinary ownership,
stored-value, callable, and static lifecycle rules everywhere except the
deliberately deferred array and external boundaries.

### BX8 — Integrate box owners with arrays and default elements

**Purpose:** Complete aggregate storage and generated default construction
without sharing synthesized boxes or weakening array invariance.

- [ ] Add `shared P?` and `(shared P?)?` as array element categories for
      inline and shared-outer arrays, including fields, calls, slices, copy,
      assignment, replacement, and reverse cleanup.
- [ ] Reuse destination-directed element-list plans for named owner copy,
      produced owner adoption, optional-owner absence/presence, and compatible
      object-box views.
- [ ] Make requested nonempty default construction of `(shared P?)[]` allocate
      one distinct absent exact box per element in increasing prefix order;
      never reuse one synthesized owner across slots.
- [ ] Preserve allocation/publication discipline for the outer array and every
      inner box, initialized-prefix verification, allocation-before-element
      effects, and exact reverse cleanup.
- [ ] Keep `shared P?[]` as one shared outer array of inline `P?` elements and
      `(shared P?)[]` as an inline array of box owners; canonical identities,
      dumps, and operations must never conflate them.
- [ ] Extend array capability, ownership, lifecycle, anchor, static-dependency,
      helper-generation, layout, and backend legality owners exhaustively.
- [ ] Preserve invariant non-object box elements and ordinary compatible
      object-box view rules without introducing array covariance.

**Tests:** Empty/dynamic/default arrays; distinct default box identity; absent
and present optional owners; named and produced element lists; class/base/
interface/`Obj` box elements; outer shared arrays; copy/assignment/slices;
fields/calls/statics; allocation failures at outer and inner steps;
initialized-prefix and cleanup mutations; helper/dump determinism; native
reverse lifecycle; `make check`; and `make msrv-check`.

**Exit criteria:** Every frozen array position executes with one owner per slot,
requested defaults allocate distinct absent boxes, outer-array and inner-box
identities remain distinct, and malformed prefix/ownership states are rejected.

### BX9 — Harden and publish shared optional boxes

**Purpose:** Remove staging gates, prove the complete cross-phase contract, and
leave living documentation describing only the implemented profile.

- [ ] Audit every `ResolvedSharedTarget`, `HirSharedTarget`, `MirSharedTarget`,
      target conversion, object assumption, owner lifecycle expansion, static
      dependency, dump renderer, verifier, layout classifier, and backend match
      for explicit optional-box capability handling.
- [ ] Complete diagnostics for invalid payloads, construction arity,
      unavailable lifecycle, invariant mismatches, impossible object
      relations, implicit forwarding, whole-pointee assignment, mutable
      aliases, external signatures, layout overflow, and staged malformed IR.
- [ ] Complete deterministic syntax/resolved/HIR/MIR/assembly dumps and
      independent-process audits for exact targets, polymorphic views,
      descriptors, finalizers, nested owners, and generated array defaults.
- [ ] Complete positive, compile-failure, and runtime-failure goldens for the
      entire frozen source matrix, including interface dispatch and owner
      replacement that leaves aliases on old allocations.
- [ ] Prove runtime header, symbol set, common reporter, ABI marker, allocation
      failure, guard failure, exact-base free, and non-unwinding behavior remain
      version 9 with no box-store reason.
- [ ] Remove obsolete reserved-box diagnostics, availability gates, roadmap
      codes from living tests/comments/docs, and stale “frozen/not implemented”
      wording only after every enabled surface executes.
- [ ] Update grammar, optional/shared language and compiler contracts,
      phase/IR, backend, runtime, status, testing, debugging, and relevant test
      READMEs to the implemented profile; archive this completed roadmap.
- [ ] Audit touched Rust module responsibilities and move substantial logic
      behind cohesive facades; record only genuine out-of-scope follow-ups in
      a separately indexed discoveries document.

**Tests:** Full focused phase and native matrices; compiler public API tests;
golden default and determinism modes; runtime C harnesses; assembly acceptance;
allocation/count/guard boundary injection; malformed-MIR mutation suites;
independent-process identity/dump/assembly determinism; `make check`;
`make msrv-check`; `make robustness-long`; and the complete `make check-long`
release gate from an artifact-free snapshot or clean checkout.

**Exit criteria:** Every frozen box form and behavior is implemented and
documented, every exclusion fails deliberately, runtime ABI version 9 is
proved unchanged, no staging gate or stale rollout language remains, all
quality gates pass, and the roadmap is archived as complete.

## Ordering and dependencies

BX0 established distinct box-allocation syntax, canonical resolved targets,
and the deliberate type-check gate before semantic work. BX1 establishes
typed compatibility and selected
optional plans before BX2 commits executable ownership state. BX2 establishes
verified target-independent allocation and owner invariants before BX3 adds a
target realization.

BX3 uses primitive wrappers to settle header, descriptor, finalizer, one-word
ABI, and allocation behavior with minimal lifecycle. BX4 then generalizes that
proven target path over existing recursive optional lifecycle. BX5 exposes the
immutable pointee only after allocation/finalization work is complete. BX6
adds the separate static-view/exact-target dimension after exact access is
verified, avoiding simultaneous uncertainty in wrapper lifetime and dynamic
dispatch.

BX7 carries the completed scalar owner/access model across fields, statics, and
internal calls. BX8 follows because arrays multiply owner and construction
state and depend on stable stored-value, finalizer, and default plans. BX9 is
the only task that removes all staging language and marks the feature
implemented.

The completed compositional optional types roadmap supplies recursive
`OptionalTypeId` metadata, lifecycle plans, guards, optional arrays, arbitrary
outer nesting, and the optional-owner zero niche. Completed shared ownership
supplies non-null handles, allocation publication, owner operations, anchors,
metadata/finalizers, polymorphic casts/dispatch, and last-owner cleanup.
Explicit shared dereference supplies the access boundary; arrays and static
fields supply aggregate/default and program-lifetime protocols. These are
dependencies to reuse, not behavior to duplicate or redesign.
