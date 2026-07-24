# Object Casts and Narrow Removal Roadmap

Status: in progress; **OC2** is next.

This roadmap replaces the implemented scoped `narrow` statement with
expression-level C-style object casts. It establishes the checked-place
operation needed by the frozen shared-ownership design before shared types,
allocation, or reference counting enter the compiler.

The source contract is frozen in
[Object Casts](../language/OBJECT_CASTS.md). Current checked-narrowing behavior
remains authoritative in [Polymorphism](../language/POLYMORPHISM.md) until the
final migration task completes.

## Scope and invariants

- `(T) source` selects a checked non-owning class/interface/`Obj` place,
  preserving complete-object identity, dynamic metadata, and source access.
- Same-type and guaranteed upcasts are static; possible downcasts and
  cross-casts check metadata; impossible casts are compile-time errors.
- A checked class place may feed existing receiver, alias-argument, field,
  copy-construction, value-parameter, result, slicing, and assignment paths.
- Cast failure terminates unsuccessfully and does not return an invalid or null
  view.
- Cast sources evaluate once and participate in existing receiver, argument,
  temporary, result, and cleanup order.
- The final compiler no longer recognizes `narrow`, allocate narrowed-alias
  identities, or carry narrowed-alias HIR/MIR storage.
- The parser represents the frozen `(shared T) source` target shape well enough
  to diagnose it as unsupported until shared types are implemented. This
  roadmap does not add shared types or execute shared-owner casts.
- No cast allocates. The later shared implementation may allocate only from
  the two forms headed by `new`: ordinary `new T(arguments)` and explicit
  copy allocation `new T((T) source)`.
- This roadmap establishes the checked-place and exact-class copy source
  consumed by future copy allocation, but it does not parse or execute either
  shared `new` form.
- Local aliases, first-class references, optional casts, primitive casts,
  unsafe reinterpretation, user-defined conversions, and recoverable cast
  failure are non-goals.
- Dynamic cloning that preserves an arbitrary source dynamic class is deferred
  and is not inferred from casts or exact-class copying.
- Removing `narrow` deliberately removes its evaluate-once multi-statement
  binding. A cast view is valid only for its consuming full expression.
- Existing hierarchy, conformance, copy, cleanup, ABI, diagnostic,
  deterministic-dump, and backend trust boundaries remain intact.

Niflheim demonstrates that `(Type) expression` can support explicit checked
class and interface casts. Its nullable reference representation and runtime
cast helpers are not Skald contracts: Skald retains non-null values,
compiler-owned metadata checks, target-independent verified views, and no
runtime cast service.

## Progress

- [x] OC0 — Extract reusable object-view relation and failure semantics
- [x] OC1 — Add checked-place cast syntax and direct view consumers
- [ ] OC2 — Integrate casts with owning inline operations
- [ ] OC3 — Remove `narrow` and publish the cast profile

## PR-sized implementation sequence

### OC0 — Extract reusable object-view relation and failure semantics

**Purpose:** Turn the current `is`/`narrow` policy into one reusable semantic
owner before a second source form depends on it.

- [x] Extract the static-success, static-failure, and runtime classification
      over exact class, class, interface, and `Obj` sources without changing
      current behavior.
- [x] Keep target selection identity-based and preserve the closed declared
      class-set test for possible cross-casts.
- [x] Centralize checked-operation access preservation, target-place
      projection, and unrecoverable failure selection behind the type-checker
      expression facade.
- [x] Separate reusable checked-view facts from narrowed-alias binding and
      lexical-body construction.
- [x] Keep current HIR/MIR dumps, diagnostics, `is`, `narrow`, native output,
      and failure behavior byte-for-byte stable where they are asserted.
- [x] Add focused classifier tests covering exact inline sources, class views,
      interface views, `Obj`, same-type casts, upcasts, possible downcasts,
      cross-casts, and impossible relations.

**Tests:** Focused type-operation unit tests and deterministic dumps, followed
by `make check`.

**Exit criteria:** Current source behavior is unchanged, while one
responsibility-oriented type-operation component can classify both type tests
and future checked-place casts without knowing about a scoped alias body.

### OC1 — Add checked-place cast syntax and direct view consumers

**Purpose:** Introduce the new expression as a complete vertical slice for
non-owning uses before removing the old statement form.

- [x] Extend the parser with unary-precedence `(T) source` cast candidates,
      including exact spans, nesting-budget use, deterministic recovery, and
      the frozen precedence over grouped callable spelling.
- [x] Represent `T` by resolved class/interface/`Obj` identity; retain the
      `shared T` target mode syntactically for a focused unsupported-feature
      diagnostic without introducing shared semantic types.
- [x] Add source-shaped AST/resolved nodes and a typed HIR checked-place
      expression carrying source view, target, access, complete-object origin,
      and static/runtime classification.
- [x] Accept existing inline places and aliases as sources. Materialize
      supported produced inline objects through the existing full-expression
      temporary model rather than borrowing dead storage.
- [x] Support cast places as direct and virtual/interface method receivers,
      `ref`/`mut ref` arguments, field reads, and supported field mutation while
      preserving access and non-exclusivity.
- [x] Lower static casts as verified view projections and runtime casts through
      explicit success/failure control flow with a bounded temporary view
      carrier.
- [x] Extend MIR verification before backend consumption: target relation,
      access, provenance, single definition, full-expression liveness,
      termination, and consumer compatibility must all be checked.
- [x] Reuse backend metadata membership checks and target-address derivation;
      emit the existing unrecoverable trap on failure without adding a runtime
      cast helper.
- [x] Add syntax, resolution, HIR, MIR, verifier-corruption, backend,
      deterministic-dump, assembly-acceptance, native-success, and
      native-failure coverage.
- [x] Keep `narrow` executable during this task and update living support prose
      only for the newly implemented direct-consumer boundary.

**Tests:** Focused syntax through backend type-operation suites, new native and
compile-fail goldens, `make check`, and `make robustness-long`.

**Exit criteria:** `((Leaf) value).read()` and corresponding receiver,
alias-argument, and field uses execute on x86-64 with verified place lifetime
and failure behavior, while existing `narrow` programs remain unchanged.

### OC2 — Integrate casts with owning inline operations

**Purpose:** Complete the plain cast matrix by feeding checked class places
through the established exact-class lifecycle pipeline.

- [ ] Permit a checked class place as the source for local and field
      initialization, exact-class value parameters, exact-class results, and
      whole-object assignment to a mutable owning destination.
- [ ] Select the target class's existing copy constructor or copy-assignment
      operation after the cast succeeds; reject missing capabilities through
      the ordinary diagnostics.
- [ ] Preserve exact target identity and slicing when the complete dynamic
      object is more derived than the cast class.
- [ ] Preserve destination-before-source assignment order, source-once
      evaluation, result-before-cleanup ordering, temporary registration, and
      exactly-once destruction.
- [ ] Reject interface and `Obj` cast places in standalone inline destinations.
- [ ] Reject cast places as whole-object replacement destinations and continue
      to allow only supported field mutation or method calls through mutable
      views.
- [ ] Prove produced inline sources remain live through the check and copy
      without introducing local aliases or path-dependent ownership at joins.
- [ ] Keep the checked-place copy boundary reusable by the later
      `new T((T) source)` shared copy-allocation consumer without adding
      allocation or shared-owner operations in this task.
- [ ] Add focused user/synthesized-copy, self-assignment, slicing, result,
      parameter, temporary, cleanup-order, diagnostic, dump, and native tests.

**Tests:** Focused lifecycle, copy, type-operation, MIR verification, backend,
and golden suites, followed by `make check`.

**Exit criteria:** Every allowed inline/alias-to-inline row in the frozen cast
matrix executes through existing selected lifecycle operations with verified
source lifetime and deterministic cleanup.

### OC3 — Remove `narrow` and publish the cast profile

**Purpose:** Delete the superseded statement-specific vertical slice only after
all retained use cases have a checked expression path.

- [ ] Migrate native, compile-fail, phase, verifier, dump, robustness, and
      public-facade tests from scoped narrowing to expression casts.
- [ ] Remove the contextual `narrow` grammar, parser statement/binding nodes,
      resolved narrowed aliases, `NarrowedAliasId`, HIR narrowing body/kind,
      MIR narrowed-alias storage and bind/end instructions, checked-narrow
      terminator vocabulary, verifier branches, frame homes, and backend
      statement lowering.
- [ ] Retain reusable dynamic-membership, view projection, explicit failure
      control flow, and trap lowering under cast-oriented names.
- [ ] Make former `narrow` source fail as unsupported syntax rather than
      retaining a compatibility alias or deprecation period.
- [ ] Audit diagnostics and deterministic dumps so living vocabulary contains
      casts, checked places, and cast failure rather than roadmap codes or
      narrowed aliases.
- [ ] Update the implemented grammar, status matrix, language overview,
      polymorphism, aliases, errors, phases, backend, debugging/testing
      guidance, and roadmap/archive indexes to make casts current and
      `narrow` historical.
- [ ] Confirm `(shared T) source`, shared sources, allocation, reference
      counting, both `new` allocation forms, and hidden shared anchors remain
      rejected pending a separate shared-ownership implementation roadmap.
- [ ] Run the complete repository, MSRV, long robustness, documentation-link,
      deterministic-process, native, and runtime gates.

**Tests:** `make check`, `make msrv-check`, and `make robustness-long`, plus
focused searches proving no active code, test, or living-document `narrow`
vocabulary remains outside historical archives.

**Exit criteria:** C-style checked-place casts are the only implemented
down/cross-cast operation; `narrow` has no accepted syntax or compiler-specific
representation; all current documentation and tests describe the cast
profile; and shared ownership can plan against the frozen cast boundary.

## Ordering and dependencies

OC0 is behavior-preserving preparation. OC1 establishes a verified executable
cast before any removal. OC2 composes that view with existing lifecycle
operations rather than adding a second copy pipeline. OC3 removes the old
statement only after every retained consumer is covered and then publishes the
new current contract.

This roadmap must complete before a shared-ownership implementation roadmap is
created. Shared work may then extend the cast target and source representation
with explicit owner copy/adopt, hidden anchors, ordinary allocation, and
copy allocation from a checked exact-class place without revisiting plain
place-cast semantics. Dynamic cloning, checked exceptions, and optional casts
remain later, independent designs.
