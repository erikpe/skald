# Shared Optional Boxes Roadmap

Status: planned; SB0 is next.

This roadmap makes an implemented optional value eligible as the exact payload
of a non-null shared allocation. It adds `shared T?` as
`Shared<Optional<T>>` and makes `shared? T?` the existing shorthand-derived
`Optional<Shared<Optional<T>>>`, without weakening ordinary shared handles or
flattening either optional layer. The completed recursive optional identity
and lifecycle model remains authoritative for the boxed payload; this roadmap
adds the allocation, pointee-place, metadata, finalization, and mutation
semantics needed to own that payload through a shared handle.

## Scope and invariants

### Source and type model

- `shared P?` is a non-null owner of one exact shared box whose complete
  payload has canonical optional identity `P?`. The box exists independently
  of whether that payload is absent or present.
- `shared? P?` remains syntax shorthand applied outside the complete
  `shared P?` type. It is exactly `(shared P?)?`, with no distinct conversion,
  overload candidate, identity, layout, or lifecycle.
- Optional nesting is compositional on both sides of the shared boundary.
  `shared P???` is `Shared<Optional<Optional<Optional<P>>>>`, while
  `(shared P??)???` wraps three additional optional layers around an owner of
  a box containing `P??`. There is no language-level nesting maximum other
  than the existing syntax and target-layout budgets.
- Every already-supported optional payload identity is eligible as a box
  target, including optional primitives, exact inline classes, inline arrays,
  ordinary shared owners, optional shared owners, and recursively nested
  optionals. Eligibility is recursive: introducing a shared-box edge does not
  make an otherwise invalid optional payload valid.
- Shared optional boxes are exact and invariant. They do not participate in
  class inheritance, interface conformance, `Obj` views, dynamic object casts,
  type tests, or array covariance. Two box-owner types are compatible only
  when their complete boxed `OptionalTypeId` is equal.
- `new P?()` allocates a box whose outer boxed optional layer is absent.
  `new P?(expression)` allocates a box and initializes that layer using the
  ordinary expected-`P?` rules: exact optional values remain exact,
  `some(expression)` constructs one present layer, `none` constructs one
  absent layer, and implicit injection adds at most one layer. Grouped and
  nested targets such as `new (shared P)?(...)`, `new P[]?(...)`, and
  `new P??(...)` follow the same rule.
- Box construction has no `copy` mode. A named optional argument is copied and
  a produced optional argument is consumed according to the already-selected
  optional lifecycle; the dedicated class and array meanings of `copy` remain
  unchanged.
- Prefix `*owner` selects the complete boxed optional as an exact bounded
  place. It can be read as an owning optional value, tested with `is some` or
  `is none`, unwrapped one layer with postfix `!`, and passed to a compatible
  `ref` or `mut ref` alias under the existing optional-alias rules.
- Whole-pointee assignment `*owner = expression` is supported for this exact
  non-polymorphic box target. It secures the incoming optional value before
  conditionally destroying the displaced payload and publishing the new
  state, so direct and indirect self-assignment are safe. This does not enable
  whole-pointee assignment for class, interface, `Obj`, or shared-array
  targets.
- A checked view into a boxed optional payload keeps the shared allocation
  alive and guards the selected optional layer against replacement for the
  complete bounded use. Stable owners borrow directly; replaceable or
  produced owner sources use the existing hidden-owner anchor discipline.

### Representation and lifecycle

- Resolution, HIR, and MIR extend their shared-target vocabularies with one
  exact optional-box case naming an existing canonical `OptionalTypeId`. No
  parallel one-level `OptionalBoxTypeId` family or source-spelling identity is
  introduced.
- Shared-box allocations use a distinct auditable allocation origin and typed
  construction/publication operations. They do not impersonate `new C(...)`,
  name a fake `ClassId`, enter class dispatch tables, or reuse class
  initializer/copy-constructor selection.
- The shared handle remains one non-null strong-owner word. The x86-64
  allocation retains the common strong-count and metadata header, followed by
  one aligned complete optional payload. Metadata is exact and specialized by
  `OptionalTypeId`; it supplies the box finalizer but no class hierarchy,
  method dispatch, cast, or object identity.
- Allocation size, header padding, payload alignment, and total addressability
  are checked from the existing recursive optional layout. Publication writes
  exact box metadata and count one only after the complete optional payload is
  initialized.
- Last-owner release invokes a generated deterministic box finalizer. The
  finalizer applies the optional identity's existing recursive destruction
  plan exactly once, destroying a present payload and doing nothing for an
  absent payload, before the generic shared release path frees the exact
  allocation base.
- Copy, adopt, move, assignment, argument/result transfer, field and array
  storage, static initialization/shutdown, and full-expression cleanup of the
  box owner reuse ordinary non-null shared-owner accounting. They never infer
  boxed-payload lifecycle from a handle operation.
- Reads and writes of the pointee use explicit typed optional-place operations.
  Payload copy, initialization, assignment, destruction, checked access,
  guard invalidation, anchors, and selected-path cleanup recursively reuse the
  canonical optional lifecycle plan.
- A shared-box edge is indirect for inline-containment analysis. Recursive
  shapes such as an optional class containing a shared box do not recursively
  expand inline layout, while cycles wholly inside inline optional payloads
  remain rejected under the existing rules.
- Existing source evaluation order, one-layer injection and unwrap, presence
  shapes, direct destination construction, abrupt non-unwinding failure
  policy, ownership-count checks, and public C runtime ABI remain unchanged.

### Explicit boundary

- This roadmap does not add general `shared P` boxes for primitives or other
  inline values. Its only new shared allocation target is a supported
  `OptionalTypeId`.
- It does not add box identity comparison, hashing, object methods, interface
  conformance, `Obj` erasure, polymorphic casts, dynamic cloning, custom
  finalizers, or user-visible allocation metadata.
- Optional references, first-class or escaping references, optional function
  values, external shared or optional ABI mappings, lifted operators,
  optional chaining, coalescing, propagation, recoverable failures, weak
  owners, concurrency, atomic guards/counts, generics, and standard-library
  collection work remain out of scope.
- Existing `shared T?[]` continues to mean a shared array whose elements are
  `T?`; it is not a shared box containing `T?[]`. Existing shared arrays keep
  their array metadata, length, element storage, projection, and finalization
  rules.

## Progress

- [ ] SB0 — Freeze shared optional-box contracts
- [ ] SB1 — Resolve exact optional-box targets and construction syntax
- [ ] SB2 — Select typed box construction and owner semantics
- [ ] SB3 — Make box allocation and ownership executable in MIR
- [ ] SB4 — Verify box allocation, publication, and owner accounting
- [ ] SB5 — Model boxed optional access and bounded aliases
- [ ] SB6 — Model guarded boxed optional replacement
- [ ] SB7 — Realize x86-64 box layout, metadata, and finalization
- [ ] SB8 — Integrate stored, callable, aggregate, and nested compositions
- [ ] SB9 — Harden and publish shared optional boxes

## PR-sized implementation sequence

### SB0 — Freeze shared optional-box contracts

**Purpose:** Settle the source, identity, allocation, access, mutation, layout,
and failure contracts before any executable phase grows a new shared target.

- [ ] Update the optional-values and shared-ownership language contracts with
      exact meanings for `shared P?`, `shared? P?`, arbitrary inner and outer
      nesting, construction, dereference, reads, aliases, replacement, and
      exact target compatibility.
- [ ] Freeze a precedence and identity table covering `shared P?`,
      `(shared P)?`, `shared? P?`, `(shared P?)?`, `shared P??`,
      `(shared P??)??`, `shared P?[]`, `shared P[]?`, and
      `shared (shared P)?`.
- [ ] Specify `new P?()` and `new P?(expression)`, expected-type behavior,
      nested presence shapes, evaluation/publication order, self-assignment,
      checked-view guards, anchor lifetime, and abrupt-failure behavior.
- [ ] Update compiler contracts with the optional shared-target identity,
      target-independent operation direction, verifier obligations, x86-64
      header/payload/metadata direction, finalizer responsibilities, and
      unchanged runtime ABI.
- [ ] Record exact supported storage and callable positions plus all exclusions
      in the status matrix without claiming execution before it exists.
- [ ] Define focused positive, compile-failure, runtime-failure, malformed-MIR,
      determinism, deep-nesting, layout-overflow, ABI-pressure, lifecycle, and
      guard-invalidation test matrices for the remaining tasks.

**Tests:** `make docs-check`, focused contract review against the implemented
grammar and compiler, `git diff --check`, and `make check`.

**Exit criteria:** Every spelling, nesting boundary, construction form,
compatibility relation, pointee operation, allocation transition, metadata
role, lifecycle action, failure edge, and exclusion is unambiguous, while
living documentation still distinguishes planned behavior from executable
behavior.

### SB1 — Resolve exact optional-box targets and construction syntax

**Purpose:** Establish deterministic semantic identities and source-shaped
construction before type checking or lower phases consume the new target.

- [ ] Replace the focused resolution rejection with an exact
      `ResolvedSharedTarget::Optional(OptionalTypeId)` or equivalent, reusing
      bottom-up optional interning for arbitrary boxed nesting.
- [ ] Extend `new` parsing and resolved expressions to preserve a complete
      optional type target, zero- versus one-argument box construction,
      grouping, punctuation spans, and malformed or excessive nesting.
- [ ] Normalize `shared? P?` to `Optional<Shared<Optional<P>>>` and prove it is
      identical to `(shared P?)?`, while retaining shorthand provenance only
      in source-shaped syntax.
- [ ] Give canonical resolved names and deterministic dumps to optional box
      targets across repeated spellings, modules, arrays, and nested wrappers.
- [ ] Diagnose invalid box targets, wrong construction arity, reserved `copy`,
      invalid optional payloads, and attempts to treat an optional box as a
      class/interface/`Obj` target at their narrow semantic owners.
- [ ] Preserve existing meanings for shared optional arrays and arrays of
      optional elements; add no provisional executable HIR operation yet.

**Tests:** Lexer/parser AST and recovery tests; resolution identity, interning,
grouping, precedence, cross-module, dump-determinism, nesting-budget, and
diagnostic tests; focused existing array/optional resolution suites;
`make check`; and `make msrv-check`.

**Exit criteria:** Every valid shared optional-box spelling resolves to one
canonical exact target at any supported nesting depth, construction source
shape is retained without ambiguity, invalid forms fail before HIR, and no
runtime behavior is yet inferred from syntax.

### SB2 — Select typed box construction and owner semantics

**Purpose:** Make HIR state all target compatibility, payload lifecycle, and
owner-transfer decisions before MIR introduces allocation state.

- [ ] Extend HIR shared targets, types, places, sources, transfers, fields,
      statics, arrays, calls, and deterministic dumps with the exact optional
      box target.
- [ ] Type-check zero-argument absent construction and one-argument optional
      construction through the canonical optional initialization plan,
      preserving one-layer injection, exact-match ranking, named copy versus
      produced consumption, and direct-destination eligibility.
- [ ] Introduce a typed box-allocation producer carrying its
      `OptionalTypeId`, selected initialization plan, payload source, owner
      provenance, and source spans without class construction metadata.
- [ ] Make ordinary shared owner initialization, copy/adopt transfer,
      assignment, fields, array elements, arguments/results, and cleanup accept
      exact-compatible box targets while rejecting class up-views and casts.
- [ ] Propagate recursive containment and lifecycle capability failures to
      focused source diagnostics rather than constructing partial HIR.
- [ ] Keep box pointee read, alias, unwrap, and mutation operations behind the
      later access tasks while retaining their resolved source shapes.

**Tests:** Type and lifecycle selection tests; exact and mismatched target
compatibility; constructor arity and expected-type cases; absent/present/deep
payload plans; owner copy/adopt classification; field, array-element,
callable, and static HIR dumps; cast/`Obj` exclusions; deterministic HIR;
`make check`; and `make msrv-check`.

**Exit criteria:** HIR contains a complete target-independent box allocation
and owner plan for every accepted construction and owning boundary, rejects
every unsupported relation explicitly, and leaves no lifecycle or transfer
choice for MIR to infer.

### SB3 — Make box allocation and ownership executable in MIR

**Purpose:** Introduce explicit unpublished-payload and owner state transitions
before verification or target layout depends on them.

- [ ] Add distinct MIR box-allocation storage, allocation origin,
      payload-initialization, publication, and adoption operations keyed by
      exact `OptionalTypeId`; do not overload class allocation identities.
- [ ] Lower absent and value construction into checked allocation intent,
      direct optional payload initialization, publication, and produced-owner
      adoption with selected-path and full-expression cleanup made explicit.
- [ ] Generalize shared owner storage and transfer operations to exact optional
      targets without changing one-word copy, adopt, move, secure-before-
      release assignment, or release semantics.
- [ ] Carry canonical optional lifecycle plans and typed payload places across
      HIR-to-MIR lowering; preserve allocation-before-publication and forbid
      reads or releases of unpublished box storage.
- [ ] Integrate box owners with normal local/parameter/result cleanup and
      abrupt terminating paths under the existing non-unwinding policy.
- [ ] Extend deterministic MIR dumping and source-operation trace attribution
      with payload-neutral box terminology and canonical identities.

**Tests:** Focused HIR-to-MIR instruction-order, storage-role, owner-transfer,
cleanup, conditional-path, nested-payload, call, field, array-element, static,
dump, and independent-process determinism tests; `make check`; and
`make msrv-check`.

**Exit criteria:** Valid typed constructions lower to explicit deterministic
box allocation and owner state machines at every owning boundary, with no
target offsets or inferred lifecycle work and no unpublished allocation
escaping.

### SB4 — Verify box allocation, publication, and owner accounting

**Purpose:** Make malformed box MIR fail at the target-independent boundary
before layout, metadata, or code generation can trust it.

- [ ] Validate every optional box target, storage kind, payload type,
      lifecycle plan, allocation origin, initialization transition,
      publication, adoption, and ordinary owner operation.
- [ ] Prove exactly one complete initialized optional payload before
      publication, count-one ownership only after publication, and exactly one
      consume or release path for produced owners.
- [ ] Extend shared target compatibility and path-state joins with exact
      optional identities while rejecting class/interface/`Obj` relations and
      mismatched nested depths.
- [ ] Verify normal cleanup, call transfer, field/array/static ownership,
      conditional construction, failure isolation, and no use of unpublished,
      consumed, moved, or released storage.
- [ ] Add test-only MIR mutations for wrong IDs, wrong lifecycle plans,
      missing/duplicate/reordered transitions, bad origins, premature payload
      use, leaked construction storage, target mismatches, and owner imbalance.
- [ ] Keep the backend rejection boundary defensive even when public or
      test-mutated MIR bypasses ordinary verification.

**Tests:** Colocated shared and optional verifier suites, focused malformed-MIR
mutations, logical lifetime and path-state cases, deterministic verifier
diagnostics, backend rejection tests, `make check`, and `make msrv-check`.

**Exit criteria:** Verification proves the complete box allocation and owner
state machine independently of source shape, every intentional mutation fails
for its owning invariant, and verified MIR supplies every fact required by the
backend.

### SB5 — Model boxed optional access and bounded aliases

**Purpose:** Expose the box's optional payload through explicit, lifetime-safe
places without yet permitting replacement.

- [ ] Generalize shared dereference resolution and HIR from object-only places
      to an exact optional box-pointee place while retaining existing object
      and shared-array paths.
- [ ] Support value reads, `is some`/`is none`, one-layer postfix unwrap,
      chained nested unwrap, and compatible read-only or mutable alias binding
      from `*owner`.
- [ ] Copy named boxed values and conditionally secure owning payloads through
      the selected recursive optional lifecycle rather than exposing raw
      payload bytes.
- [ ] Reuse stable-owner and hidden-anchor rules so produced or replaceable
      owner sources keep the allocation live for the complete read, checked
      view, alias call, or nested payload consumer.
- [ ] Extend optional guards and MIR place provenance to name both the shared
      allocation and exact optional layer; end checked views before releasing
      hidden owner anchors.
- [ ] Verify access, identity, anchor, guard, evaluation-once, and cleanup
      invariants, including failure at each nested absent layer.

**Tests:** Resolution/HIR/MIR place tests; stable, field, array-element,
static, optional-owner-unwrapped, call-produced, and conditionally produced
owners; read/copy/alias/presence/unwrap success; absent failures; chained depth;
guard and anchor mutations; deterministic HIR and MIR dumps; `make check`; and
`make msrv-check`.

**Exit criteria:** Every accepted read-only boxed-payload use has one explicit
verified MIR realization with exact optional semantics, required anchors and
guards cover the complete consumer, and absence or malformed provenance
cannot reach target lowering.

### SB6 — Model guarded boxed optional replacement

**Purpose:** Add exact whole-pointee mutation with self-assignment safety and
checked-view invalidation rules.

- [ ] Type-check `*owner = expression` only for a mutable exact optional-box
      place and select the recursive optional assignment plan under ordinary
      expected-type and one-layer injection rules.
- [ ] Evaluate and secure the owner and incoming value exactly once before
      conditionally destroying or releasing the displaced payload; publish the
      new presence state only after initialization completes.
- [ ] Make direct and allocation-alias self-assignment safe for primitive,
      exact-class, array, shared-owner, optional-owner, and nested optional
      payloads.
- [ ] Reject replacement while a checked view or alias guards the affected
      optional layer, including later-argument invalidation, nested guards, and
      mutation through a second owner of the same allocation.
- [ ] Add explicit HIR/MIR replacement operations and verifier transitions;
      do not reuse class whole-pointee assignment or infer optional lifecycle
      from stores.
- [ ] Preserve source order, selected-path cleanup, abrupt-failure behavior,
      and exact reverse destruction across old and secured incoming payloads.

**Tests:** Type/HIR/MIR assignment plans; every payload lifecycle category;
present/absent transitions at several depths; direct and indirect
self-assignment; aliases through two owners; guard invalidation compile/runtime
failures; malformed transition and cleanup mutations; deterministic HIR and
MIR lifecycle traces; `make check`; and `make msrv-check`.

**Exit criteria:** Exact box replacement is explicit and verified in MIR for
every supported optional lifecycle, never exposes partial or invalid state,
and cannot invalidate a live checked payload view.

### SB7 — Realize x86-64 box layout, metadata, and finalization

**Purpose:** Map only verified box semantics to one concrete target layout and
deterministic generated lifecycle code.

- [ ] Extend target layout with checked header-plus-optional-payload size,
      alignment, padding, payload offset, addressability, frame, argument, and
      result classifications for every canonical box target.
- [ ] Generate exact optional-box metadata and finalizer symbols in
      deterministic `OptionalTypeId` order, separate from class dispatch and
      shared-array metadata tables.
- [ ] Lower allocation, payload initialization, publication, retain, adopt,
      move, copy, assignment, dereference, replacement, and release using the
      common non-null shared handle and exact payload address.
- [ ] Generate a finalizer that recursively applies the selected optional
      destruction plan once before freeing the preserved allocation base;
      absent payloads perform no payload destruction.
- [ ] Preserve the one-word internal shared-owner ABI, hidden-owner anchors,
      ownership overflow reporting, compiler-defect traps, and runtime ABI
      version and symbols.
- [ ] Reject unknown identities, excessive size/alignment, unencodable
      offsets, inconsistent metadata, or illegal box operations with
      structured backend errors.

**Tests:** Target layout boundary and overflow tests; metadata/finalizer symbol
and order tests; assembly instruction-order and system-assembler tests; owner
copy/adopt/release, absent/present/nested finalization, recursive payload,
alignment, ABI register/stack pressure, static shutdown, and runtime failure
goldens; deterministic assembly; `make check`; and `make msrv-check`.

**Exit criteria:** Verified boxes execute on x86-64 with checked exact layout,
one-word owners, deterministic metadata and finalizers, recursively correct
last-owner destruction, and no C runtime contract change.

### SB8 — Integrate stored, callable, aggregate, and nested compositions

**Purpose:** Complete every promised composition after the core owner and
pointee mechanisms are executable in isolation.

- [ ] Exercise box owners and optional box owners in locals, parameters,
      results, fields, statics, inline and shared arrays, array element lists,
      slices, and class synthesized lifecycle.
- [ ] Execute arbitrary nested optional payloads inside boxes and arbitrary
      optional layers outside box owners, including boxed optional arrays,
      boxed optional shared owners, and optionals containing further exact box
      owners.
- [ ] Cover calls, methods, interfaces, recursion, argument register/stack
      pressure, hidden aggregate results, produced results, and receiver or
      alias anchors without granting boxes object dispatch themselves.
- [ ] Integrate checked box-payload places with existing array, class,
      optional, static-shutdown, short-circuit, and full-expression cleanup
      plans.
- [ ] Confirm exact incompatibility across different optional identities and
      retain focused failures for `Obj`, interface, cast, type-test, external
      ABI, and non-optional general-box attempts.
- [ ] Add source-to-native examples proving that an absent boxed payload is
      distinct from an absent optional box owner at several nesting depths.

**Tests:** Cross-phase and native matrices for all storage and callable
positions; nested depth-two and depth-five shapes; arrays, fields, statics,
recursion, dispatch around but not on boxes, ABI pressure, logical branches,
failure order, reverse cleanup, and compile-time exclusions; `make check`;
`make golden-determinism-test`; and `make msrv-check`.

**Exit criteria:** Every promised source position and finite supported nesting
shape composes through native execution with exact presence states, ownership,
ABI, guards, anchors, and cleanup, while excluded object and ABI relations
remain focused failures.

### SB9 — Harden and publish shared optional boxes

**Purpose:** Audit the completed feature as one coherent language and compiler
contract, remove rollout artifacts, and close the roadmap only after all
repository gates pass.

- [ ] Run hostile syntax and semantic nesting, layout overflow, malformed-MIR,
      ownership-count, guard-limit, failure-path, and independent-process
      determinism audits; fix defects within this roadmap's contract.
- [ ] Audit shared-target and optional-lifecycle matches for explicit box
      handling, narrow facades, cohesive module ownership, deterministic
      iteration, and absence of fake class/array identities or duplicated
      lifecycle policy.
- [ ] Update grammar, language, compiler, runtime-ABI, status, testing, and
      debugging documentation to describe only the final implemented surface;
      remove the former shared-box exclusion and stale rollout wording.
- [ ] Remove roadmap task codes from living source, tests, diagnostics, dumps,
      and documentation while retaining semantic names and historical roadmap
      vocabulary.
- [ ] Resolve or record out-of-scope discoveries separately without expanding
      this roadmap, then repair links and indexes for archival.
- [ ] Run the complete repository, deterministic golden, robustness, MSRV,
      documentation, formatting, and diff-hygiene gates from an artifact-free
      snapshot.

**Tests:** Focused regression suites from every prior task; `make check`;
`make golden-determinism-test`; `make robustness-long`; `make msrv-check`;
`git diff --check`; and artifact/status inspection.

**Exit criteria:** The implemented grammar, contracts, phase models,
verification, x86-64 target, tests, and status agree; every accepted shared
optional-box composition is deterministic and lifecycle-correct; no stale
exclusion or milestone vocabulary remains in living artifacts; and the
roadmap is ready to move to `docs/archive/`.

## Ordering and dependencies

This roadmap depends on the completed compositional optional types work:
recursive `OptionalTypeId` interning, lifecycle plans, arbitrary nested
optionals, optional arrays, one-layer construction/access, guards, aggregate
calling conventions, and deterministic target layouts must remain stable.
SB0 freezes the new allocation boundary. SB1 establishes canonical target and
source identities before SB2 selects lifecycle and ownership. SB3 makes those
decisions executable, and SB4 verifies them before any backend relies on them.
SB5 and SB6 add read and mutation behavior in that order so anchor/guard
invariants are established before replacement can invalidate a view. SB7 then
realizes the complete verified operation set on x86-64. SB8 broadens evidence
across existing consumers and arbitrary nesting without reopening core
semantics, and SB9 performs the final contract and determinism audit.

The sibling Niflheim implementation has no equivalent non-null shared box
containing Skald-style inline recursive optionals; its nullable object
references are therefore useful only as a syntax comparison, not as a
representation or lifecycle precedent. Skald's shared-array metadata and
generated-finalizer machinery, together with its recursive optional lifecycle,
are the closer architectural precedents.
