# Shared Ownership and Heap Allocation Roadmap

Status: planned; prerequisites complete, SO0 is next.

This roadmap implements the frozen non-null `shared T` object model, explicit
heap allocation, deterministic reference-counted lifetime, and shared-backed
borrowing. It extends the existing exact-class lifecycle, checked-place cast,
polymorphic metadata, full-expression cleanup, and verified MIR pipelines
rather than introducing parallel object or lifetime systems.

The source contract is
[Shared Ownership and Heap Allocation](../language/SHARED_OWNERSHIP.md), the
compiler/runtime contract is
[Shared-Ownership Compiler and Runtime Contract](../compiler/SHARED_OWNERSHIP.md),
and the complete cast direction matrix is
[Object Casts](../language/OBJECT_CASTS.md).

## Current baseline

- Inline exact-class values, selected copy operations, deterministic normal
  cleanup, value parameters/results, and full-expression temporaries execute
  through explicit HIR and verified MIR.
- The completed constructor-semantics roadmap provides ordinary initializer
  overload selection, the distinct `copy` lifecycle declaration, and
  target-directed `T(copy source)`.
- Class/interface/`Obj` views, virtual and interface dispatch, type tests, and
  plain checked-place casts preserve complete-object and metadata provenance.
- `(shared T) source` is parsed only far enough to issue an unsupported-feature
  diagnostic. Storage types do not yet accept `shared T`, and `new` has no
  allocation meaning.
- No phase product represents shared types, owners, allocation, anchors, or
  release. The x86-64 backend has no shared handle or allocation path.
- Runtime ABI version 4 exposes scalar output only; it has no allocation or
  deallocation entry points.

## Scope and invariants

- Every live `shared T` is one non-null strong owner of one live allocation.
  No source-visible moved-from, empty, null, raw-pointer, or count state exists.
- Only `new C(arguments)` and `new T(copy source)` allocate. Owner copying,
  assignment, results, upcasts, casts, and anchors preserve an existing
  allocation.
- The class named by `new` is concrete and determines the complete dynamic
  class. Exact-class copy allocation may slice and never implies
  dynamic-type-preserving cloning.
- Named shared values copy an owner; produced shared values transfer their
  existing owner; every normal lifetime end releases exactly one owner.
- Shared assignment secures the incoming owner before releasing the old owner,
  including direct and indirect self-assignment.
- The last release dynamically selects the complete most-derived destruction
  sequence exactly once, then deallocates the original header exactly once.
- Ownership and access remain separate. Shared pointees are mutable,
  read-only access to an enclosing inline object is shallow around a shared
  field, and aliases remain deliberately non-exclusive.
- A non-owning view reached through replaceable or produced shared storage is
  covered by an explicit owner or hidden anchor until its consumer completes.
- HIR records type, owner provenance, selected operations, and anchor
  requirements without target layout. MIR makes allocation, copy, adopt,
  release, anchoring, and lifetime order executable and verifies them before
  any backend consumes them.
- The x86-64 backend owns the one-word handle, 16-byte header, non-atomic
  `u64` count, metadata/finalizer layout, internal ABI, and generated count
  operations. The C runtime remains a checked `malloc`/`free` boundary.
- Receiver-before-argument order, left-to-right arguments, result securing,
  reverse temporary cleanup, stable IDs, diagnostics, phase dumps, assembly,
  and native observations remain deterministic.
- Strong cycles are permitted to leak. They are not collected and do not
  weaken memory safety.
- Optional and weak values, explicit release/count inspection, raw pointers,
  custom allocators, dynamic cloning, external shared ABI, atomic counts,
  concurrency guarantees, arrays, statics, exceptional cleanup, and
  recoverable allocation or cast failure are non-goals.

## Progress

- [ ] SO0 — Parse and resolve shared types and allocation forms
- [ ] SO1 — Establish typed shared-owner vocabulary
- [ ] SO2 — Represent and verify the first owner lifetime in MIR
- [ ] SO3 — Upgrade the runtime to the minimal allocation ABI
- [ ] SO4 — Execute exact-class allocation and last-owner destruction
- [ ] SO5 — Complete local, assignment, and temporary owner semantics
- [ ] SO6 — Carry shared owners across calls and results
- [ ] SO7 — Integrate shared fields with class lifecycle
- [ ] SO8 — Execute shared-field layout and lifecycle
- [ ] SO9 — Add polymorphic shared views and dispatch
- [ ] SO10 — Execute shared-owner casts
- [ ] SO11 — Anchor shared-backed calls
- [ ] SO12 — Anchor shared-backed checked places
- [ ] SO13 — Add explicit exact-class copy allocation
- [ ] SO14 — Harden and publish shared ownership

## PR-sized implementation sequence

### SO0 — Parse and resolve shared types and allocation forms

**Purpose:** Give all later phases one source-shaped, identity-based frontend
contract before ownership decisions enter typed IR.

- [ ] Extend storage and result type syntax with contextual `shared` followed
      by a class, interface, or `Obj` target, preserving exact spans and
      existing `ref`/`mut ref` parameter grammar.
- [ ] Parse contextual `new C(arguments)` at expression precedence compatible
      with current construction, postfix selection, calls, casts, and the
      shared syntax-nesting budget.
- [ ] Reuse the prerequisite construction-mode representation so
      `new T(copy source)` retains explicit copy-allocation mode while
      `new T(arguments)` retains ordinary overload mode; do not reconstruct
      the distinction from argument count or type below the frontend.
- [ ] Resolve each shared target and allocation class to existing typed
      identities. Record ordinary versus copy-allocation mode without
      repeating source-shape inspection below resolution.
- [ ] Reject unknown targets, `new` of interfaces/`Obj`, and malformed or
      non-constructible targets with deterministic source diagnostics while
      keeping semantic execution gated until typed support lands.
- [ ] Add focused lexer/contextual-word, parser recovery, AST/resolved identity,
      exact-span, precedence, nesting, and deterministic-dump tests.
- [ ] Update the implemented grammar and status wording for the newly accepted
      syntax without claiming executable shared ownership.

**Tests:** Focused syntax and resolution suites, frontend robustness cases,
exact AST/resolved dumps, `make check`, and `make robustness-long`.

**Exit criteria:** Every valid shared type and both `new` forms cross
resolution with stable target identity and allocation mode; lower phases never
need to recover either fact from names or argument shape.

### SO1 — Establish typed shared-owner vocabulary

**Purpose:** Define one readable, target-independent semantic vocabulary for
shared types, owner sources, and allocation before executable lifetime state
is added to MIR.

- [ ] Add canonical HIR shared targets covering classes, interfaces, and
      `Obj`, clearly separated from inline exact-class values and non-owning
      views.
- [ ] Type-check shared locals, value parameters, results, and fields while
      rejecting aliases-to-shared targets, external shared signatures, static
      storage, invalid interface storage assumptions, and all implicit
      inline/alias-to-shared conversions.
- [ ] Represent ordinary `new C(arguments)` as exact-class allocation plus the
      already overload-selected ordinary initializer and a produced owner; do
      not encode byte size, header offsets, runtime symbols, or count
      operations.
- [ ] Distinguish named shared places from produced owners and record
      copy-versus-adopt intent at each enabled consuming boundary.
- [ ] Centralize shared compatibility and implicit up-view checks so locals,
      arguments, results, fields, casts, and later anchors do not grow
      independent conversion rules.
- [ ] Keep source execution explicitly gated where MIR support is not yet
      present, using structured diagnostics or lowering errors rather than
      panics.
- [ ] Give shared semantic models cohesive private modules behind the existing
      HIR/type-check facades; split broad expression or statement owners only
      where the new responsibility demonstrates the need.
- [ ] Add type-check diagnostics, HIR construction/provenance tests, and
      deterministic HIR dumps; update phase documentation for the new typed
      boundary.

**Tests:** Focused declaration, construction, type-compatibility, external
exclusion, diagnostic, and HIR-dump tests, followed by `make check`.

**Exit criteria:** HIR can state what allocation and owner operation source
semantics require without target details, implicit ownership inference, or
duplicated compatibility policy.

### SO2 — Represent and verify the first owner lifetime in MIR

**Purpose:** Establish explicit owner accounting and verification for one
exact-class allocated local before any backend or broader value context relies
on it.

- [ ] Add target-independent shared storage and semantic operations for exact
      allocation, produced-owner adoption, named-owner copy, release, and
      full-expression ownership boundaries.
- [ ] Lower `var value: shared C = new C(arguments);` in source evaluation
      order: evaluate arguments, allocate unpublished exact storage, run the
      selected initializer, publish count-one ownership, adopt into the local,
      and release it at normal scope exit.
- [ ] Model an allocation under construction separately from a live published
      owner so no release, view, or call can observe a partially initialized
      allocation.
- [ ] Extend initialized-storage and cleanup planning for shared owners without
      treating a handle as an inline class place or an ordinary unowned scalar.
- [ ] Add responsibility-specific MIR verification for compatible owner
      storage, publication, single adopt/release paths, normal-exit cleanup,
      non-null provenance, exact allocation class, and the rule that allocation
      originates only from `new`.
- [ ] Make backend rejection of otherwise valid shared MIR structured until
      target support lands.
- [ ] Add malformed-MIR fixtures and mutations for duplicate adoption,
      release-before-publication, missing release, use-after-release,
      wrong-target storage, non-`new` allocation, and invalid control-flow
      joins.
- [ ] Add deterministic MIR dumps and update phase documentation with the
      explicit ownership state machine.

**Tests:** Focused HIR-to-MIR lowering, ownership-verifier mutation, cleanup,
pass-pipeline, backend-rejection, and exact MIR-dump tests, followed by
`make check`.

**Exit criteria:** Verified MIR can prove the complete normal lifetime of one
exact allocated owner, and no backend needs to infer retain/release policy from
storage uses.

### SO3 — Upgrade the runtime to the minimal allocation ABI

**Purpose:** Publish and test the deliberately small version-5 C boundary
before generated code depends on it.

- [ ] Change the runtime ABI version and link marker from version 4 to version
      5 across the public header, C implementation, generated process entry,
      direct harnesses, mismatch tests, and documentation.
- [ ] Add `ska_rt_alloc(uint64_t)` as a checked nonzero allocation wrapper that
      rejects unrepresentable `size_t`, terminates unsuccessfully on failure,
      and returns suitably aligned non-null storage.
- [ ] Add `ska_rt_free(void *)` as the exact-base deallocation wrapper with no
      knowledge of counts, headers, metadata, payloads, or finalizers.
- [ ] Keep failure machinery implementation-private and retain the existing
      scalar output behavior unchanged.
- [ ] Extend direct C harnesses for version compatibility, successful
      allocation/write/free, nonzero and representability preconditions,
      allocation failure, and link-marker mismatch.
- [ ] Update the runtime ABI authority and runtime test guide in the same
      change.

**Tests:** `make runtime-test`, focused driver link-mismatch tests,
`make golden-test`, and `make check`.

**Exit criteria:** The checked byte allocator and deallocator are independently
verified behind ABI version 5, while all reference counting and object policy
remain absent from C.

### SO4 — Execute exact-class allocation and last-owner destruction

**Purpose:** Complete the first source-to-native shared lifetime with the
frozen handle/header representation and dynamic finalization.

- [ ] Extend x86-64 legality, data layout, frames, and internal type
      classification with one non-null, eight-byte shared handle pointing to a
      16-byte allocation header.
- [ ] Check header-plus-payload size, alignment, addressability, and overflow
      before emitting a runtime allocation call; store count and exact dynamic
      metadata only at the verified publication point.
- [ ] Add one compiler-generated complete finalizer entry to every executable
      class descriptor without making target offsets visible in HIR or MIR.
- [ ] Generate finalizers that accept the complete payload address and reuse
      the existing user-body, reverse-field, and base destruction plan.
- [ ] Lower release so count one becomes zero, dynamically calls the exact
      complete finalizer once, and then calls `ska_rt_free` once; keep the
      allocation live throughout finalization.
- [ ] Add generated count-overflow and invalid-state termination machinery in
      a cohesive ownership-lowering owner, even if retain first becomes
      source-reachable in the next task.
- [ ] Keep metadata symbols, finalizer symbols, descriptor entries, and
      assembly ordering deterministic and verify malformed metadata before
      instruction selection.
- [ ] Add assembler, static layout, runtime-call, exact/derived finalizer,
      allocation failure, count-overflow fixture, and native lifetime tests.
- [ ] Update backend and status documentation for the first executable
      exact-class allocation boundary.

**Tests:** Focused backend legality/layout/metadata/lowering tests, assembler
acceptance, native exact and derived destruction tests, runtime tests, and
`make check`.

**Exit criteria:** An exact `shared C` local can be allocated, initialized,
published, destroyed through dynamic metadata, and freed exactly once on
x86-64 with no ownership policy in the runtime.

### SO5 — Complete local, assignment, and temporary owner semantics

**Purpose:** Implement the core copy/adopt/release value rules before owners
cross callable or object-field boundaries.

- [ ] Enable same-target shared local initialization from named and produced
      sources, copying named owners and adopting produced owners without
      redundant retain/release pairs; broader polymorphic compatibility lands
      with shared views.
- [ ] Implement shared local assignment as
      evaluate-once, secure incoming, release old, store incoming, including
      direct and indirect self-assignment.
- [ ] Materialize unadopted produced owners as full-expression temporaries and
      release them in reverse completion order after the consuming result is
      secured.
- [ ] Extend HIR and MIR dumps so copy, adopt, release, assignment order, and
      temporary ownership are explicit and distinct.
- [ ] Extend ownership verification across blocks and normal returns for
      exactly one owner per live storage or temporary and no use after release.
- [ ] Lower retain with checked non-atomic `u64` overflow and lower assignment
      mechanically from verified MIR order.
- [ ] Preserve source-visible destruction timing; do not eliminate or merge
      owner operations as an optimization in this roadmap.
- [ ] Add named/produced initialization, chained temporaries, self-assignment,
      aliasing assignment, scope/return cleanup, overflow, diagnostic, dump,
      and native tests.

**Tests:** Focused type-check, MIR lowering/verifier, backend, cleanup-order,
and native tests, followed by `make check`.

**Exit criteria:** Every local or temporary owner follows explicit,
verified copy/adopt/release semantics and assignment never exposes an empty or
dangling destination.

### SO6 — Carry shared owners across calls and results

**Purpose:** Extend the established owner state machine across internal
callable boundaries without creating a second call or result pipeline.

- [ ] Enable shared value parameters and results for internal functions,
      initializers, methods, and interface requirements while preserving the
      external-signature exclusion.
- [ ] Copy named arguments at their source position, transfer produced
      arguments, and make the callee adopt and normally release each incoming
      parameter owner.
- [ ] Copy named returns, transfer produced returns, and secure the caller's
      result before callee or caller-side temporary cleanup.
- [ ] Permit assignment to a live shared value parameter using the same
      secure-incoming, release-old, store-incoming operation as a local.
- [ ] Integrate shared arguments with existing receiver-first and
      left-to-right mixed scalar, inline-copy, view, and shared evaluation.
- [ ] Extend MIR call arguments/results and verification with explicit owner
      handoff, callee parameter cleanup, result path agreement, and no
      double-release on normal return.
- [ ] Realize the one-word integer-class internal ABI, including register
      exhaustion, stack arguments, `rax` results, recursion, and call pressure.
- [ ] Keep shared call logic behind existing call facades and factor common
      copy/adopt handling rather than branching independently in every
      callable kind.
- [ ] Add mixed-ABI, named/produced argument, forwarding, recursion, result,
      cleanup-order, verifier-corruption, assembly, and native tests.
- [ ] Update callable, phase, and backend documentation for the implemented
      internal shared ABI.

**Tests:** Focused call/type-check/MIR/verifier/ABI suites, assembler
acceptance, native and golden calls/results, and `make check`.

**Exit criteria:** Shared parameters and results transfer one verified owner
through every internal callable kind and preserve evaluation and cleanup order
under register and stack pressure.

### SO7 — Integrate shared fields with class lifecycle

**Purpose:** Make shared edges first-class owning fields in the
target-independent lifecycle before assigning them target layout.

- [ ] Require every shared field to be initialized exactly once from a
      compatible shared expression during ordinary initialization.
- [ ] Permit shared field replacement through mutable owning roots and
      `self`, securing the incoming owner before releasing the old field.
- [ ] Apply shallow read-only access: enclosing read-only access prevents
      handle replacement but does not make the separately allocated pointee
      read-only.
- [ ] Exclude shared edges from finite inline-containment analysis while
      retaining normal class/interface target validation.
- [ ] Extend user and synthesized copy construction, assignment, capability
      computation, and destruction with declaration-ordered copies/assignments
      and reverse-declaration releases.
- [ ] Extend MIR field projections, owner verification, target-independent
      destruction plans with explicit shared owner operations rather than
      inline payload containment.
- [ ] Preserve inherited lifecycle composition and dynamic last-owner
      destruction for graphs containing shared and inline fields.
- [ ] Keep backend rejection structured until shared-field target layout lands.
- [ ] Add nested field, replacement, unavailable capability, shallow access,
      base composition, exact HIR/MIR lifecycle order, and verifier-corruption
      tests.
- [ ] Update class/lifecycle and phase documentation without duplicating the
      shared authority.

**Tests:** Focused containment, initialization, capability, lifecycle,
HIR/MIR dump, cleanup, verifier-mutation, and backend-rejection tests, followed
by `make check`.

**Exit criteria:** Shared fields behave as owning edges in every user and
synthesized target-independent lifecycle operation, with explicit order and
owner balance verified before target layout.

### SO8 — Execute shared-field layout and lifecycle

**Purpose:** Realize verified shared owning edges on x86-64 independently of
the later polymorphic and borrow surfaces.

- [ ] Lay out every shared field as one eight-byte, eight-aligned header handle
      while retaining existing inline base-prefix and field layout rules.
- [ ] Lower field initialization, owner copying, replacement, and reverse-order
      release mechanically from verified MIR.
- [ ] Generate complete finalizers that recursively destroy inline fields and
      release shared fields in the already selected derived-to-base order.
- [ ] Preserve original header identity and dynamic metadata through nested
      shared-field loads and stores without embedding ownership policy in place
      addressing.
- [ ] Reject invalid shared field layouts, handle widths, projections, or
      incomplete finalizer metadata before instruction selection.
- [ ] Add layout, assembly, native cascade-destruction, inheritance,
      self-assignment-through-fields, and mixed inline/shared graph tests.
- [ ] Update backend documentation for shared fields and finalizer recursion.

**Tests:** Focused layout, place, cleanup, finalizer, legality, assembler,
native graph, and golden tests, followed by `make check`.

**Exit criteria:** Verified shared fields execute as one-word owning edges,
and mixed object graphs finalize in language order.

### SO9 — Add polymorphic shared views and dispatch

**Purpose:** Preserve allocation identity and dynamic metadata while shared
owners are viewed as ancestors, interfaces, or `Obj`.

- [ ] Enable implicit same-class, ancestor, guaranteed-interface, and `Obj`
      shared up-views without allocation, payload copying, slicing, or metadata
      replacement.
- [ ] Represent a shared pointee place with static target, access, canonical
      header identity, complete payload address, and dynamic metadata
      provenance in HIR and MIR.
- [ ] Support direct, virtual, and interface calls plus inherited field/base
      projections through stable existing shared locals and value parameters.
- [ ] Extend `is` to shared class/interface/`Obj` sources without changing
      ownership and reuse the canonical closed-world relation classifier.
- [ ] Preserve ordinary mutable pointee access and non-exclusive aliasing while
      retaining static target restrictions.
- [ ] Verify shared view targets, projections, origins, metadata compatibility,
      owner liveness, and dispatch selection before backend lowering.
- [ ] Derive payload and metadata from the header and reuse existing checked
      layout and dispatch tables; do not add runtime view or type-test helpers.
- [ ] Add same/up/interface/`Obj` view, deep virtual/interface dispatch,
      type-test, access, malformed-MIR, assembly, and native tests.
- [ ] Add a source-level strong cycle using a temporary acyclic seed and a
      later shared-`Obj` field replacement; prove its destructors do not run
      after external owners end while non-cyclic controls finalize normally.
- [ ] Update polymorphism and phase documentation for shared sources.

**Tests:** Focused view-relation, receiver, interface, type-operation, MIR
verification, backend dispatch, native polymorphism/cycle, and golden suites,
followed by `make check`.

**Exit criteria:** Every non-narrowing shared view preserves one allocation and
dynamic class while executing through verified existing dispatch machinery,
and an intentional strong cycle leaks by policy rather than by an unbalanced
lowering path.

### SO10 — Execute shared-owner casts

**Purpose:** Complete the owner-preserving cast direction without conflating it
with plain borrowed casts or allocation.

- [ ] Type-check `(shared T) source` only from compatible shared
      class/interface/`Obj` sources using the existing static-success,
      runtime-check, and static-failure relation.
- [ ] Copy a named source owner and transfer a produced source owner after any
      required check; preserve the original header, payload, and dynamic
      metadata.
- [ ] Reject inline and alias sources through focused ownership diagnostics and
      prove no cast path allocates or invokes a payload copy operation.
- [ ] Represent static and runtime shared casts with explicit result ownership,
      success/failure control flow, and source lifetime through result securing.
- [ ] Extend MIR verification for source target, owner provenance,
      copy-versus-adopt, result compatibility, terminating failure, and absence
      of allocation.
- [ ] Reuse backend metadata membership checks and unrecoverable failure
      lowering, then perform the verified retain or transfer on success.
- [ ] Add same/up/down/cross class/interface/`Obj`, named/produced,
      static-impossible, invalid-source, count, failure-order, dump, assembly,
      and native tests.
- [ ] Update the object-cast authority and status matrix for the implemented
      shared-owner column.

**Tests:** Focused type-operation tests from resolution through backend,
verifier mutations, native success/failure goldens, and `make check`.

**Exit criteria:** `(shared T) source` is an owner-producing view of the same
allocation with verified copy/adopt behavior, and no cast can manufacture heap
storage.

### SO11 — Anchor shared-backed calls

**Purpose:** Guarantee that direct non-owning receivers and alias arguments
reached through shared storage outlive the complete call.

- [ ] Classify each shared-backed borrow in HIR as covered by a stable existing
      owner, a produced owner whose lifetime is extended, a hidden copy of a
      replaceable owner place, or an inherited outer alias lifetime.
- [ ] Support direct `ref`/`mut ref` arguments and method receivers reached
      through shared locals, parameters, fields, nested places, produced
      owners, and inline field/base subobjects in shared payloads.
- [ ] Create hidden owner storage at the receiver or argument evaluation
      position, before later user code can replace the source place.
- [ ] Keep receiver anchors before explicit argument anchors, preserve
      left-to-right argument order, secure call results first, and release
      anchors with other full-expression temporaries in reverse completion
      order.
- [ ] Make a complete allocation's anchor cover every inline base or field
      subobject within its payload; do not search the object graph or infer
      ownership from an existing alias.
- [ ] Extend MIR with explicit anchor storage/lifetimes and verify source
      category, retain/adopt operation, view-before-release ordering,
      projection containment, call coverage, and all normal exits.
- [ ] Keep anchor selection and lowering in cohesive provenance/lifetime
      owners rather than scattering special cases through call, field, and
      cleanup code.
- [ ] Add replacement-during-call, produced receiver, nested shared field,
      inline subobject, forwarding, overlapping mutable alias, result-order,
      verifier-corruption, and destruction-timing tests.
- [ ] Update aliases, phases, and debugging documentation for explicit hidden
      call anchors.

**Tests:** Focused alias/call/type-check tests, exact HIR/MIR anchor dumps, MIR
lifetime mutations, backend and native destruction-order tests, golden
coverage, and `make check`.

**Exit criteria:** No direct shared-backed receiver or alias argument can
dangle when its source place is replaced during the call, and every required
call anchor is explicit and verified before code generation.

### SO12 — Anchor shared-backed checked places

**Purpose:** Extend the proven call-anchor state machine to plain checked-place
casts and their immediate non-owning or inline-copy consumers.

- [ ] Classify a shared-backed `(T) source` as stable-owner, produced-owner, or
      hidden-copy anchored before its static or runtime target selection.
- [ ] Cover checked places consumed by method receivers, `ref`/`mut ref`
      arguments, field access and mutation, exact-class inline initialization,
      value arguments/results, slicing, and owning assignment.
- [ ] Preserve one source evaluation, cast failure after lifetime safety,
      checked-view end before anchor release, and result/copy completion before
      full-expression cleanup.
- [ ] Reuse call-anchor storage and cleanup machinery while keeping checked
      view carriers and owner anchors distinct in HIR, MIR, dumps, and
      verification.
- [ ] Verify source category, access, target relation, contained projection,
      anchor coverage, success/failure edges, copy completion, and every normal
      cleanup path.
- [ ] Add stable local, replaceable field, produced owner, inline subobject,
      receiver, alias, inline-copy, slicing, mutation, failure-order,
      verifier-corruption, and destruction-timing tests.
- [ ] Update casts, aliases, phases, and debugging documentation for
      shared-backed checked-place anchors.

**Tests:** Focused cast/alias/copy/type-check tests, exact HIR/MIR dumps,
verifier mutations, backend and native success/failure order, goldens, and
`make check`.

**Exit criteria:** Every plain checked place reached through shared ownership
has an explicit verified owner covering its complete immediate consumer, with
the view ending before that owner can be released.

### SO13 — Add explicit exact-class copy allocation

**Purpose:** Compose the established checked-place, exact copy, allocation, and
anchor pipelines into `new T(copy source)` without adding cloning semantics.

- [ ] Require concrete copy-constructible `T` and the resolved explicit-copy
      construction mode; keep every `new T(arguments)` form in ordinary
      initializer overload resolution without copy fallback.
- [ ] Accept existing and produced inline objects, `ref`/`mut ref` aliases,
      and shared-backed places as target-directed checked sources under the
      complete object-view and anchor rules.
- [ ] Record the exact selected `T` copy constructor, checked target place,
      source provenance, anchor category, and produced exact `shared T` owner
      in HIR.
- [ ] Lower source evaluation and anchoring, dynamic check, exact-`T`
      allocation, one selected copy construction, publication, and result
      securing in that exact order.
- [ ] Verify that failure precedes destination allocation, the source place and
      anchor remain live through copy completion, publication occurs once, and
      the operation is never elided.
- [ ] Write metadata for the class named by `new`, never copy source dynamic
      metadata, and preserve deliberate ancestor slicing.
- [ ] Reject unavailable copy capability, interface/`Obj` allocation targets,
      statically impossible checked sources, ordinary-construction fallback,
      and any attempted dynamic-type-preserving clone.
- [ ] Add inline/alias/produced/shared sources, same/up/down checked selection,
      explicit inner-cast refinement, slicing,
      unavailable-copy, source-once, failure-before-allocation, cleanup-order,
      verifier-corruption, assembly, and native tests.
- [ ] Update shared construction, object-cast, lifecycle, phase, and backend
      documentation for executable copy allocation while retaining dynamic
      cloning as a deferred feature.

**Tests:** Focused construction/copy/cast/anchor tests through every compiler
phase, verifier mutations, assembler acceptance, native success/failure and
golden tests, followed by `make check`.

**Exit criteria:** `new T(copy source)` creates one exact `T` allocation by one
explicit target-directed checked copy operation, with no allocation on check
failure and no dynamic cloning.

### SO14 — Harden and publish shared ownership

**Purpose:** Audit the complete ownership matrix, remove rollout scaffolding,
and make shared ownership a dependable current contract.

- [ ] Exercise named and produced owners in every local, field, parameter,
      result, assignment, receiver, alias, cast, ordinary allocation, and copy
      allocation position, including mixed inline/shared object graphs.
- [ ] Audit all normal control-flow paths for owner balance, result-before-
      cleanup ordering, last-owner destruction, anchor coverage, and absence
      of use-after-release or double finalization.
- [ ] Add malformed public-MIR and backend legality coverage for every shared
      operation, header/finalizer invariant, target mismatch, invalid lifetime,
      and external/static exclusion.
- [ ] Add deterministic HIR/MIR/assembly/process tests and native goldens for
      count behavior observable through destruction, dynamic finalization,
      cascades, cycles, call pressure, cast failure, and allocation failure.
- [ ] Audit touched Rust modules by responsibility. Resolve high-priority
      ownership, call, cleanup-verifier, metadata, or lowering hotspots; record
      larger lower-priority findings in an indexed shared-ownership discoveries
      document rather than expanding the final task.
- [ ] Remove temporary unsupported-feature branches, stale “future shared”
      language, and roadmap codes from living code, tests, diagnostics, and
      general documentation.
- [ ] Update grammar, status, language overview, ownership, lifecycle, casts,
      polymorphism, errors, compiler architecture, phases, backend, runtime ABI,
      debugging, testing, and runtime guides so each fact has one crisp
      authority and current summaries link to it.
- [ ] Confirm all exclusions remain rejected: null/optional/weak handles, raw
      ownership construction, early release/count access, external shared ABI,
      statics, arrays, atomics, exceptions, and dynamic cloning.
- [ ] Run the complete repository, MSRV, long robustness, documentation,
      deterministic-process, assembler, native, runtime, and diff-hygiene
      gates; then archive this roadmap.

**Tests:** `make check`, `make msrv-check`, `make robustness-long`,
`git diff --check`, focused searches for stale rollout vocabulary, and all
shared-owner native/compile-failure goldens.

**Exit criteria:** The complete frozen shared-ownership profile is implemented,
verified, executable on x86-64, accurately documented as current behavior, and
contains no untracked ownership inference or unresolved implementation
decision.

## Ordering and dependencies

SO0 fixes source shape and identity ownership first. SO1 establishes typed
language decisions without target details, and SO2 makes the minimum owner
lifetime explicit and verifiable before code generation. SO3 publishes the
minimal runtime boundary independently; SO4 then completes the first native
allocation/finalization vertical slice.

SO5 completes intraprocedural owner semantics before SO6 carries owners across
calls. SO7 adds owning graph edges only after both storage and call transfer
are stable, and SO8 realizes those edges independently on x86-64. SO9
establishes ordinary polymorphic shared views before SO10 adds checked
owner-preserving casts.

SO11 introduces direct call anchors before SO12 composes them with checked
place carriers and owning inline consumers. SO13 comes last among semantic
slices because copy allocation depends on checked places, copy capabilities,
allocation, owner publication, and anchors. SO14 broadens coverage and
documentation only after every operation exists.

The archived constructor-semantics roadmap supplies overload-aware ordinary
construction, the separate copy-constructor identity and declaration, and the
shared `T(copy source)` construction-mode representation. The archived
object-cast roadmap and resolved cast-relative receiver follow-up are also
complete prerequisites. Runtime ABI version 5 must land before the backend
emits allocation calls. Dynamic cloning, weak ownership, optionals, and
checked exceptions remain independent future designs and must not be pulled
into this sequence.
