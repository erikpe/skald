# Polymorphism Roadmap

Status: in progress; PM7 is next. The
[archived documentation overhaul](../archive/DOCUMENTATION_OVERHAUL_ROADMAP.md)
established the focused
[polymorphism design authority](../language/POLYMORPHISM.md) through DOC8.

This roadmap freezes and extends Skald's completed exact-class object-value
model with
single inheritance, base subobjects, opt-in virtual dispatch, interfaces,
`Obj` views, type tests, and checked narrowing. The implementation must extend
the existing place, lifecycle, return-storage, temporary, and cleanup models;
it must not create a second object-value pipeline.

The compiler-maintainability cleanup is now the implementation baseline. In
particular, new work should extend the private phase-model modules, concise
facades, shared typed-ID tables, responsibility-oriented verifier and lowering
modules, test-only MIR fixtures, and deterministic robustness harnesses already
present in the repository. The preparatory class-program orchestration work in
PM1 and PM2 is complete, so hierarchy implementation can extend focused owners.

## 1. Scope and invariants

The frozen profile includes:

- one direct class base through contextual `extends`, with acyclic hierarchies;
- explicit base-subobject initialization and stable semantic base projections;
- inherited fields and methods with deterministic lookup and diagnostics;
- lifecycle capability composition across the base subobject and direct fields;
- derived-body, derived-fields, then base-chain destruction under the existing
  exactly-once cleanup model;
- opt-in `virtual` methods and explicit, exactly compatible `override`
  declarations;
- interface signatures, explicit `implements`, and exact conformance checks;
- non-owning read-only and mutable class, `Obj`, and interface alias views;
- inline slicing, implicit non-slicing upcasts, `is` type tests, and explicit
  checked narrowing as distinct operations;
- deterministic dynamic-class metadata, dispatch tables, dumps, diagnostics,
  assembly, and native behavior.

The profile preserves these invariants:

1. Resolution is the only source-name selection phase. Base classes, inherited
   members, overrides, interface requirements, and conversions have stable
   typed identities before HIR.
2. HIR owns static types, selected declarations and conversions, receiver
   access, dispatch kinds, and lifecycle operations. It contains no target
   offsets, registers, symbols, or calling-convention locations.
3. MIR represents base, `Obj`, and interface views explicitly and verifies all
   metadata needed by a backend. A class object never becomes a scalar
   `MirValue`, and aggregate bytes never imply ownership.
4. Every inline derived object has one complete-object lifetime. Base
   construction, copying, assignment, return storage, temporary cleanup, and
   destruction extend the existing initialized-place state machine.
5. Backends consume verified hierarchy, dispatch, and conversion operations.
   They do not repeat name lookup, override selection, conformance checking,
   copy selection, or cleanup planning.
6. Receiver mutability is part of exact override and interface compatibility.
   Upcasts, interface conversions, and narrowing preserve or reduce access;
   they never grant mutable access.
7. Dynamic receiver information survives forwarding and calls made through
   `self`, so deep inherited overrides remain effective until an operation
   deliberately slices to an exact inline base value.
8. Hierarchy traversal, ID allocation, diagnostic precedence, metadata layout,
   phase dumps, assembly, and native observations are deterministic.
9. Phase implementation remains behind the established facades. New schema,
   verifier, lowering, and backend responsibilities receive cohesive private
   owners and focused tests rather than returning to broad central modules.

The frozen restricted profile excludes:

- multiple class inheritance and interface inheritance;
- access modifiers, `final`, abstract methods/classes, default interface
  bodies, interface fields, overload sets, and covariant overrides;
- standalone inline `Obj` or interface values;
- `shared`, allocation, reference counting, borrow anchors, and dynamic shared
  destruction;
- external polymorphic/object ABI and cross-module metadata coalescing;
- arrays, optionals, closures, generics, statics/globals, and reflection;
- exceptions, failed-construction unwinding, and partial-copy cleanup;
- unsafe pointer casts, user-visible dispatch tables, and user-defined
  conversion operators.

## 2. Progress

- [x] PM0 — Freeze the executable polymorphism profile
- [x] PM1 — Extract resolver class-body orchestration
- [x] PM2 — Extract type-checker class-program orchestration
- [x] PM3 — Parse and resolve class inheritance
- [x] PM4 — Build the canonical class hierarchy and inherited lookup
- [x] PM5 — Compose base initialization and lifecycle semantics
- [x] PM6 — Add typed static base views and slicing in HIR
- [ ] PM7 — Represent and verify static inheritance in MIR
- [ ] PM8 — Lower static inheritance on x86-64
- [ ] PM9 — Resolve virtual declarations and override families
- [ ] PM10 — Select virtual calls and receiver views in HIR
- [ ] PM11 — Represent and verify virtual dispatch in MIR
- [ ] PM12 — Lower virtual dispatch on x86-64
- [ ] PM13 — Parse and resolve interfaces and conformance declarations
- [ ] PM14 — Validate conformance and select interface views in HIR
- [ ] PM15 — Represent and verify interface dispatch in MIR
- [ ] PM16 — Lower interface dispatch on x86-64
- [ ] PM17 — Type-check type tests and checked narrowing
- [ ] PM18 — Represent and verify tests and narrowing in MIR
- [ ] PM19 — Lower tests and checked narrowing on x86-64
- [ ] PM20 — Harden, document, and publish polymorphism

## 3. PR-sized implementation sequence

### PM0 — Freeze the executable polymorphism profile

**Purpose:** Resolve source and representation choices before code depends on
them.

- [x] Freeze the contextual forms for `extends`, `super(...)`, `virtual`,
      `override`, `interface`, `implements`, `Obj`, `is`, and checked narrowing,
      including the scoped non-owning result of a successful narrowing.
- [x] Freeze inherited lookup, redeclaration and shadowing rules, virtual-root
      and override compatibility, interface conformance, receiver access, and
      deterministic diagnostic precedence.
- [x] Freeze whether `Obj` is a semantic root or a physical base, the
      target-independent complete-object/view model, dynamic-class metadata,
      and how polymorphic receiver information crosses the internal alias ABI.
- [x] Freeze base construction, copy, assignment, destruction, slicing,
      temporary, return-storage, and permitted-elision behavior.
- [x] Reconcile the [grammar](../language/GRAMMAR.md), focused
      [polymorphism document](../language/POLYMORPHISM.md),
      [status matrix](../language/STATUS.md), and the exclusions above so later
      tasks do not rely on provisional prose.

**Tests:** Add focused grammar/spec consistency cases where executable rules can
already be asserted; run `make check` and `git diff --check`.

**Exit criteria:** Every later task can implement its source form, semantic
identity, ownership behavior, metadata, and failure mode without another
profile-level choice.

### PM1 — Extract resolver class-body orchestration

**Purpose:** Give all resolved class member bodies one reusable coordination
path before inheritance adds `super(...)` and more member metadata.

- [x] Replace repeated initializer, copy-constructor, copy-assignment,
      destructor, and method body setup with a cohesive private class-body
      owner.
- [x] Preserve source-member lookup, declaration lookup, callable IDs, body
      environments, definition ordering, recovery, and diagnostics exactly.
- [x] Keep class declaration collection and body resolution as separate
      responsibilities behind the existing resolver facade.
- [x] Move or add focused tests beside the new owner; update architecture prose
      only if the stable responsibility boundary changes.

**Tests:** Run resolver class/lifecycle unit tests, resolver dump and diagnostic
tests, `make check`, and `git diff --check`.

**Exit criteria:** Adding another class callable or initializer-only statement
requires one explicit orchestration path rather than copying member-resolution
setup.

### PM2 — Extract type-checker class-program orchestration

**Purpose:** Give class declaration lowering and member-body checking a clear
program-level owner before base lifecycle and dispatch expand their context.

- [x] Move class declaration and definition orchestration out of the broad
      type-check program entry module behind a narrow private interface.
- [x] Centralize construction of shared member-check context while keeping
      lifecycle-specific body kinds and receiver access explicit.
- [x] Preserve HIR declaration/definition table order, optional member slots,
      diagnostics, and the public type-check facade.
- [x] Keep callable body rules in their existing responsibility modules and add
      focused orchestration tests.

**Tests:** Run type-check class, lifecycle/copy, receiver, dump, and diagnostic
tests, `make check`, and `git diff --check`.

**Exit criteria:** New class member categories can be wired into one readable
class-program owner without duplicating `MemberCheckContext` assembly.

### PM3 — Parse and resolve class inheritance

**Purpose:** Establish stable direct-base identity without yet enabling
inherited access or polymorphic conversions.

- [x] Parse contextual `extends` on classes with focused malformed-input
      recovery and no global keyword regressions.
- [x] Resolve the base name to `ClassId`, rejecting unknown, duplicate,
      self-referential, and wrong-kind bases in deterministic order.
- [x] Extend class declaration models and typed tables through resolved IR
      while keeping the source spelling only where dumps and diagnostics need
      it.
- [x] Extend AST and resolved dumps, grammar snapshots, and hostile frontend
      mutation coverage.

**Tests:** Add parser recovery, resolution identity/diagnostic, exact-dump, and
generative frontend cases; run `make check` and `git diff --check`.

**Exit criteria:** Every accepted class has zero or one resolved direct base;
inherited member use and cyclic hierarchies still fail before HIR.

### PM4 — Build the canonical class hierarchy and inherited lookup

**Purpose:** Create the one target-independent hierarchy service used by all
later lifecycle, typing, verification, and layout work.

- [x] Reject direct and indirect cycles before HIR or layout with stable
      source-order diagnostics.
- [x] Define deterministic base-chain traversal, subtype queries, inherited
      field/method lookup, collision handling, and declaration-owner recovery.
- [x] Keep the hierarchy keyed by typed IDs and prevent later phases from
      reconstructing relationships from names or declaration order.
- [x] Cover forward references, deep chains, inherited redeclarations,
      containment interaction, and deterministic first-error paths.

**Tests:** Add hierarchy service unit tests plus resolver/type-check diagnostics
for cycles, wrong kinds, deep lookup, and collisions; run `make check` and
`git diff --check`.

**Exit criteria:** All consumers can answer ancestry and inherited-member
questions through one canonical identity-based model.

### PM5 — Compose base initialization and lifecycle semantics

**Purpose:** Extend complete-object ownership through base subobjects before any
dynamic dispatch is executable.

- [x] Parse and resolve the frozen `super(...)` form only in the permitted
      initializer position and select the base initializer by stable identity.
- [x] Type-check base initialization before derived fields and track base
      liveness explicitly during construction.
- [x] Include the base first in copy-construction and copy-assignment capability
      computation; retain exact selected operations in HIR.
- [x] Extend destruction planning to derived body, derived fields in reverse,
      then the complete base sequence, with no implicit failed-construction
      cleanup.
- [x] Keep diagnostics source-ordered through base and field paths and document
      the executable lifecycle contract.

**Tests:** Add lifecycle/capability unit tests and exact diagnostics for missing,
duplicate, misplaced, or unavailable base operations, including empty bases,
deep chains, user/synthesized combinations, returns, and temporaries; run
`make check` and `git diff --check`.

**Exit criteria:** HIR describes one complete derived-object lifecycle with an
explicit, ordered base contribution and no backend-owned lifecycle choice.

### PM6 — Add typed static base views and slicing in HIR

**Purpose:** Make static inherited access and exact base-object production
explicit at the typed semantic boundary.

- [x] Add HIR base projections carrying selected declarations, terminal class,
      and preserved receiver access.
- [x] Type-check inherited fields, direct non-virtual methods, read-only and
      mutable alias upcasts, and `Obj` upcasts without slicing.
- [x] Represent inline derived-to-base value conversion as selected base copy
      construction into exact base storage, never as raw prefix bytes.
- [x] Preserve receiver-before-argument evaluation, exact object-value
      ownership, return-storage, temporary, and elision rules.
- [x] Extend HIR dumps and place/access diagnostics without leaking target
      layout.

**Tests:** Add focused HIR/type-check tests for deep projections, access
restriction, static calls, alias forwarding, slicing, grouping, object results,
and invalid conversions; run `make check` and `git diff --check`.

**Exit criteria:** Every static inheritance operation is identity-selected in
HIR and distinguishable as projection, alias view, or sliced value.

### PM7 — Represent and verify static inheritance in MIR

**Purpose:** Extend target-independent places and lifecycle instructions with
verified base semantics.

- [ ] Add MIR base projections and explicit selected base copy/lifecycle
      operations using the existing model facades and responsibility modules.
- [ ] Lower HIR base views for locals, receivers, parameters, return storage,
      arguments, temporaries, aliases, and nested inline fields.
- [ ] Verify ancestry, projection owner/terminal types, access, overlap,
      liveness, selected copy capabilities, and destruction-plan consistency.
- [ ] Extend deterministic MIR dumps, test-only fixtures, and structured MIR
      mutations for corrupt hierarchy and base-place metadata.

**Tests:** Run focused MIR lowering, dump, place, call, cleanup, and mutation
tests for static inheritance, then `make check` and `git diff --check`.

**Exit criteria:** Invalid base metadata is rejected before the backend, and
valid MIR contains everything required to execute static inheritance.

### PM8 — Lower static inheritance on x86-64

**Purpose:** Execute verified base layout and lifecycle behavior without
exposing target layout above the backend.

- [ ] Lay out the direct base according to the frozen ABI, then derived fields,
      with checked offsets, padding, alignment, and total-size arithmetic.
- [ ] Lower every verified base projection and selected construction, copy,
      assignment, slicing, and destruction operation.
- [ ] Preserve scalar/object argument classes, hidden results, aliases,
      temporaries, cleanup, and mixed register/stack pressure.
- [ ] Return structured backend errors for corrupt metadata or displacement
      overflow; do not add an implicit aggregate-copy path.

**Tests:** Add layout and legality unit tests plus native traces for empty and
padded bases, deep chains, slicing, returns, temporaries, cleanup, and mixed
call pressure; run `make check` and `git diff --check`.

**Exit criteria:** Static single inheritance and its complete lifecycle execute
deterministically on x86-64 with target choices confined to the backend.

### PM9 — Resolve virtual declarations and override families

**Purpose:** Establish stable virtual identities and compatibility before
representing dynamic calls.

- [ ] Parse contextual `virtual` and `override` method modifiers in the frozen
      order with focused recovery.
- [ ] Resolve virtual roots and explicit overrides from canonical inherited
      lookup; reject missing roots, non-virtual redeclarations, and invalid
      modifier combinations.
- [ ] Validate exact parameters, result, receiver mutability, and all other
      frozen signature rules.
- [ ] Assign deterministic override-family and slot identities without target
      offsets or symbols; extend AST, resolved, and HIR declaration dumps.

**Tests:** Add parser, resolution, type-check, and exact-dump tests for deep
families, inherited non-overrides, signature/access mismatches, forward
declarations, and stable diagnostic precedence; run `make check` and
`git diff --check`.

**Exit criteria:** Every virtual declaration belongs to one canonical family,
and every override is fully validated before executable call selection.

### PM10 — Select virtual calls and receiver views in HIR

**Purpose:** Make dynamic versus direct dispatch and receiver metadata explicit
in typed executable semantics.

- [ ] Distinguish direct and virtual method targets in HIR using selected method
      and override-family identities.
- [ ] Represent the frozen complete-object pointer/view and dynamic-class
      metadata carried by polymorphic alias receivers.
- [ ] Preserve dynamic receiver information through parameter forwarding,
      nested calls, and base methods calling virtual methods through `self`.
- [ ] Enforce receiver access before dispatch selection and keep calls on sliced
      inline bases exact and static.
- [ ] Extend HIR dumps and diagnostics without introducing ABI fields.

**Tests:** Add HIR/type-check tests for base/derived receivers, deep overrides,
`self` redispatch, forwarding, mutable calls, recursion, and sliced bases; run
`make check` and `git diff --check`.

**Exit criteria:** HIR identifies the exact static family and dynamic receiver
view for every call, with no backend inference required.

### PM11 — Represent and verify virtual dispatch in MIR

**Purpose:** Define a target-independent, corruption-resistant virtual call
contract.

- [ ] Add explicit virtual call targets and polymorphic receiver operands to
      the MIR model behind the existing facades.
- [ ] Lower HIR virtual calls while retaining source evaluation order, argument
      modes, result destinations, and full-expression cleanup.
- [ ] Verify family/slot ownership, signature and access agreement, receiver
      view compatibility and liveness, and dynamic metadata provenance.
- [ ] Extend dumps, shared fixtures, and structured mutations for invalid
      families, receivers, signatures, and metadata.

**Tests:** Run focused MIR call, argument, place, cleanup, dump, and robustness
tests for virtual dispatch, then `make check` and `git diff --check`.

**Exit criteria:** Every virtual MIR call names one verified family and carries
one valid complete-object receiver view.

### PM12 — Lower virtual dispatch on x86-64

**Purpose:** Execute opt-in virtual methods through a checked internal ABI.

- [ ] Compute deterministic per-class virtual tables from stable identities in
      a backend analysis rather than embedding target slots in MIR.
- [ ] Map polymorphic receiver views to the frozen internal register/stack ABI
      and forward their dynamic metadata through nested calls.
- [ ] Lower virtual calls with existing scalar/object arguments, hidden results,
      temporaries, cleanup, recursion, and stack alignment.
- [ ] Reject malformed metadata and unsupported external signatures before
      instruction selection.

**Tests:** Add backend metadata/legality tests and native cases for deep
overrides, `self` redispatch, inherited non-overrides, mutable access, recursion,
mixed arguments, stack pressure, and sliced bases; run `make check` and
`git diff --check`.

**Exit criteria:** Calls through base aliases select the dynamic override while
direct and sliced calls retain exact static behavior.

### PM13 — Parse and resolve interfaces and conformance declarations

**Purpose:** Establish stable interface and requirement identities independently
of executable interface dispatch.

- [ ] Parse top-level interface signatures and contextual class `implements`
      lists with focused recovery and no standalone interface values.
- [ ] Add `InterfaceId` and interface-member identities using the shared typed
      identity/table patterns; resolve nominal type positions by declaration
      kind.
- [ ] Resolve class conformance lists in source order and reject unknown,
      duplicate, malformed, and wrong-kind entries deterministically.
- [ ] Extend AST and resolved models/dumps, grammar snapshots, public phase API
      tests, and frontend robustness mutations through cohesive private owners.

**Tests:** Add parser recovery, identity/table, resolution diagnostic, exact
dump, public API, and generative cases; run `make check` and `git diff --check`.

**Exit criteria:** Valid interface signatures and class conformance claims cross
resolution by typed identity while interface-typed calls remain disabled.

### PM14 — Validate conformance and select interface views in HIR

**Purpose:** Make interface compatibility and non-owning conversions explicit
before lowering them.

- [ ] Validate requirement uniqueness and exact class conformance, including
      inherited implementations, receiver mutability, parameters, and results.
- [ ] Define deterministic requirement-to-method maps for every valid
      class/interface pair and retain both identities in HIR.
- [ ] Type-check class-to-interface and interface-to-`Obj` alias conversions,
      forwarding, and interface method calls while preserving access and
      lifetime.
- [ ] Reject standalone interface/`Obj` inline storage and all unimplemented
      shared or external forms with focused diagnostics.
- [ ] Extend HIR dumps and the focused polymorphism and compiler phase
      documentation for the stable conformance boundary.

**Tests:** Add conformance/type-check and HIR dump tests for inherited methods,
multiple interfaces, reordered requirements, mutable methods, forwarding,
wrong signatures, missing methods, and invalid storage; run `make check` and
`git diff --check`.

**Exit criteria:** HIR contains one verified conformance map and selected
non-owning view for every interface conversion and call.

### PM15 — Represent and verify interface dispatch in MIR

**Purpose:** Define interface views and calls without introducing interface
objects or target table layouts.

- [ ] Add explicit interface view operands, conversions, and call targets to
      MIR using interface and requirement identities.
- [ ] Lower HIR interface calls and forwarding with complete-object pointer,
      dynamic-class metadata, access, and lifetime intact.
- [ ] Verify conformance, requirement/method agreement, view provenance,
      liveness, access, call signatures, and non-ownership.
- [ ] Extend deterministic dumps, shared fixtures, and structured mutations for
      wrong conformance, requirements, views, and calls.

**Tests:** Run focused MIR call, argument, place, dump, verification, cleanup,
and mutation tests for interface views, then `make check` and
`git diff --check`.

**Exit criteria:** Valid MIR describes interface dispatch completely and all
source-reachable corruptions stop at verification.

### PM16 — Lower interface dispatch on x86-64

**Purpose:** Execute verified interface calls through deterministic backend-owned
tables.

- [ ] Compute stable interface requirement tables and class witnesses from
      typed identities without source-name ordering.
- [ ] Lower class/interface/`Obj` views under the frozen internal ABI and route
      calls to the selected implementing method.
- [ ] Preserve dynamic metadata through multiple interface conversions and
      inherited overrides, including calls made through `self`.
- [ ] Retain structured legality and overflow errors and reject external
      interface signatures before instruction selection.

**Tests:** Add backend table/legality tests and native cases for multiple
interfaces, deep inherited implementations and overrides, reordered
requirements, mutable access, forwarding, recursion, and mixed stack
arguments; run `make check` and `git diff --check`.

**Exit criteria:** Interface aliases dispatch to the conforming complete inline
object without standalone interface storage or ownership transfer.

### PM17 — Type-check type tests and checked narrowing

**Purpose:** Complete the source and HIR conversion model over the established
dynamic metadata.

- [ ] Parse the frozen `is` and explicit checked-narrowing forms with clear
      precedence, grouping, and recovery.
- [ ] Type-check class, base, `Obj`, and interface source/target combinations;
      distinguish static success, static impossibility, and runtime checks.
- [ ] Bind successful narrowing only through the frozen scoped alias form,
      preserving source access and lifetime and rejecting escape or access
      increase.
- [ ] Represent test/narrowing kind, target identity, selected view, and
      failure behavior explicitly in HIR.
- [ ] Extend the implemented grammar, focused polymorphism semantics, HIR
      dumps, diagnostics, and frontend robustness mutations.

**Tests:** Add parser/type-check/HIR tests for deep bases, interfaces, static and
dynamic outcomes, access, scope escape, grouping, nested calls, and invalid
casts; run `make check` and `git diff --check`.

**Exit criteria:** Every accepted type test or narrowing has one explicit HIR
semantic kind and can produce only a bounded non-owning view.

### PM18 — Represent and verify tests and narrowing in MIR

**Purpose:** Make runtime metadata checks and their control flow explicit before
target lowering.

- [ ] Add MIR type-test and checked-narrowing operations carrying source view,
      target identities, result view, and explicit failure behavior.
- [ ] Lower static outcomes without unnecessary runtime metadata work and lower
      dynamic success/failure control flow deterministically.
- [ ] Verify legal type relations, metadata provenance, target conformance,
      scoped result liveness/access, and failure edges.
- [ ] Extend dumps, shared fixtures, and mutations for invalid targets,
      metadata, views, and check results.

**Tests:** Run focused MIR lowering, CFG, place, cleanup, dump, verification, and
robustness tests for tests/narrowing, then `make check` and
`git diff --check`.

**Exit criteria:** MIR fully describes every static or dynamic result and
rejects malformed checks before backend selection.

### PM19 — Lower tests and checked narrowing on x86-64

**Purpose:** Execute verified checks against deterministic class/interface
metadata without ownership or object-graph search.

- [ ] Lower static and runtime class/interface membership checks against the
      backend metadata established for dispatch.
- [ ] Materialize successful scoped views under the existing polymorphic alias
      ABI and implement the frozen unrecoverable failure behavior.
- [ ] Preserve complete-object address, receiver access, evaluation order,
      temporary cleanup, and stack alignment across success and failure paths.
- [ ] Reject corrupt metadata and unsupported forms through structured backend
      errors.

**Tests:** Add backend legality and native cases for success, failure, deep
bases, multiple interfaces, round trips through `Obj`, nested calls, and stack
pressure; assert stdout, stderr, and process status; run `make check` and
`git diff --check`.

**Exit criteria:** Type tests and checked narrowing execute deterministically
and produce only verified non-owning views.

### PM20 — Harden, document, and publish polymorphism

**Purpose:** Make the restricted profile dependable and leave a stable boundary
for shared ownership.

- [ ] Complete compile-failure and native goldens across hierarchy declarations,
      lifecycle, layout, static access, slicing, virtual/interface dispatch,
      `Obj`, conversions, tests, narrowing, aliases, object values, and cleanup.
- [ ] Add cross-process determinism coverage for every phase dump, dynamic
      metadata artifact, assembly, diagnostic, and native observation.
- [ ] Audit source-reachable assertions, hierarchy/table assumptions,
      complete-object views, initialized-place transitions, verifier mutations,
      and backend legality; retain every discovered regression as a focused
      test.
- [ ] Audit touched files and functions by responsibility, split any enlarged
      owner or test module, and record unrelated follow-ups in the indexed
      discoveries document.
- [ ] Update the focused grammar, status, polymorphism, class, alias, compiler
      phase, backend, README, debugging, sample, testing, and roadmap-index
      authorities with only current behavior.
- [ ] Complete all quality gates, mark this roadmap complete, archive it, repair
      links and indexes, and publish shared ownership as the next object-model
      direction.

**Tests:** Run focused golden/determinism/robustness tests, `make check`,
`make robustness-smoke`, `make msrv-check`, and `git diff --check` from an
artifact-free snapshot or clean checkout.

**Exit criteria:** Restricted polymorphism is explicit, deterministic, exactly
owned, structurally verified, maintainably organized, fully documented, and
the roadmap is archived.

## 4. Ordering and dependencies

The order is deliberate:

- The completed documentation overhaul established the language authority and
  focused polymorphism document. PM0 freezes its decisions there.
- PM1 and PM2 were source-design-independent preparation and removed the two
  known orchestration growth hazards. PM0 freezes contracts before PM3 adds
  hierarchy representations or member categories.
- Static inheritance proceeds declaration graph, lifecycle, HIR, MIR, then
  backend. Dynamic dispatch cannot obscure an incomplete base-object model.
- Virtual dispatch establishes the dynamic receiver and metadata contract used
  by interfaces. Interfaces establish the conformance metadata reused by type
  tests and narrowing.
- Each target-independent task precedes its backend consumer so x86-64 never
  becomes the semantic authority.
- PM20 is the only broad hardening PR. Every earlier task still updates the
  living documentation it changes and must pass its focused tests,
  `make check`, and `git diff --check`.

The repository contains no CI job for this roadmap. Existing external
infrastructure regularly runs `make check` on clean checkouts; the Makefile is
the common local and external automation interface. Run `make msrv-check` in a
task that changes manifests, the supported toolchain contract, or Rust syntax
compatibility, and always run it for final closeout.

The slice is complete only when PM0-PM20 and their exit criteria are complete,
the final quality gates pass, and shared ownership can reuse the resulting
complete-object metadata, dispatch, lifecycle, and alias-view contracts.
