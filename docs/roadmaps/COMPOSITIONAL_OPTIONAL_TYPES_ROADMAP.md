# Compositional Optional Types Roadmap

Status: in progress; CO6 is complete and CO7 is next.

This roadmap makes optionality a compositional operation over existing
storable values. It establishes `(shared T)?` as the canonical form of an
optional shared owner while retaining `shared? T` as an exact shorthand,
replaces the current flat optional families with recursive deterministic type
identities and lifecycle plans, executes arbitrarily nested optionals, and
adds optional inline arrays. It deliberately stops before shared boxes whose
pointee is an optional value.

## Scope and invariants

### Source type model

- Postfix `?` wraps exactly one complete preceding type expression. General
  type grouping is accepted, so `(shared T)?`, `(T[])?`, and `(T?)?` compose
  without feature-specific parentheses rules.
- `(shared T)?` is the canonical spelling and semantic model for an optional
  shared owner. Existing `shared? T` remains accepted as exact shorthand for
  the same type, with no conversion, distinct overload candidate, or distinct
  lower-phase identity.
- `T[]?` is the canonical compact spelling of `(T[])?`. It remains distinct
  from `T?[]`, which is an array whose elements are optional.
- Every finite optional nesting depth within the compiler's ordinary syntax
  budget is supported when its innermost payload is eligible. There is no
  language-level one- or two-layer maximum.
- An optional layer has exactly two states: absent, or present with one
  complete live payload. Nested optionals are not flattened. `T` wrapped in
  `N` optional layers therefore has `N + 1` observably distinct presence
  shapes.
- `none` constructs absence at the outer expected optional layer. A new
  explicit `some(expression)` form constructs exactly one present layer and
  supplies its payload as the expected type of `expression`.
- Existing implicit injection remains one layer only: exact `P` may satisfy
  expected `P?`, but the compiler never repeatedly lifts `T` through an
  arbitrary number of optional layers. Exact matches continue to outrank
  optional injection during overload selection.
- Postfix `!` checks and removes exactly one layer. Chained unwraps retain
  evaluation-once, left-to-right failure, payload-copy/place, presence-guard,
  anchor, and full-expression cleanup rules at every layer.
- Eligible inline optional payloads are primitives, exact inline classes,
  inline arrays, ordinary shared owners, and other supported optionals.
  `unit`, bare interface/`Obj` values, function values, and other non-storable
  categories remain ineligible.

### Representation and lifecycle

- Recursive optional identity uses small deterministic interned IDs rather
  than recursive heap-owned Rust enums in every phase or a combinatorial
  family such as `OptionalArray` and `OptionalOptionalClass`. Existing
  recursive array identities are the implementation precedent.
- Source-shaped syntax retains punctuation and span provenance for diagnostics.
  Resolved, HIR, and MIR products normalize `(shared T)?` and `shared? T` to
  one semantic identity and use deterministic canonical dumps.
- Plain `T` and `shared T` never acquire an absent, null, moved-from, or invalid
  source-visible state.
- Inline optionals use an explicit state plus the complete payload layout.
  Every non-niche nested layer has its own state and complete inner wrapper;
  distinct nesting states are never collapsed as an optimization.
- `Optional<Shared<T>>` may retain the existing one-word zero-handle niche and
  direct shared-owner calling convention. An outer optional around that value,
  such as `(shared T)??`, remains a distinct tagged wrapper.
- An optional array conditionally owns one complete inline array descriptor and
  its backing lifecycle. A present empty array is distinct from an absent
  optional array, so absence does not reuse the valid empty descriptor state.
- Optional initialization, copy, assignment, destruction, checked access,
  aliasing, calls, array-element operations, and static shutdown recursively
  apply the selected lifecycle of the immediate payload.
- Inline containment analysis traverses optional payloads. Optionality does not
  make `class Node { next: Node?; }` finite. Array descriptors and shared-owner
  edges retain their existing non-inline containment behavior.
- Existing evaluation order, direct destination construction, selected-path
  cleanup, guard invalidation failures, ownership accounting, and the public C
  runtime ABI remain unchanged.

### Explicit boundary

- `shared T?`, meaning a non-null shared box whose pointee is `T?`, is not part
  of this roadmap.
- `shared? T?`, meaning shorthand for an optional owner of such a box, is also
  not part of this roadmap.
- The compositional parser may preserve those source shapes, but semantic
  analysis continues to reject them with focused shared-box diagnostics. No
  provisional box target, allocation operation, metadata, finalizer,
  dereference, or mutation hook is introduced here.
- A future shared-box roadmap may depend on the completed recursive optional
  identity and lifecycle model, then add `Shared<Optional<P>>` as a new heap
  allocation target without reopening this roadmap's inline semantics.
- Optional references, `ref?`, first-class or escaping references, optional
  function values, external optional ABI mappings, lifted operators, optional
  chaining, coalescing, propagation, recoverable failures, concurrency or
  atomic guards, generics, and standard-library collection families are
  additional non-goals.

## Progress

- [x] CO0 — Freeze compositional optional contracts
- [x] CO1 — Introduce compositional type syntax and canonical shared optionals
- [x] CO2 — Canonicalize recursive optional identities
- [x] CO3 — Generalize typed optional lifecycle planning
- [x] CO4 — Generalize executable optional MIR and target realization
- [x] CO5 — Execute arbitrarily nested optional lifecycle
- [x] CO6 — Complete nested-optional access and callable integration
- [ ] CO7 — Execute optional inline arrays
- [ ] CO8 — Integrate optional arrays across stored and aggregate boundaries
- [ ] CO9 — Harden and publish compositional optionals

## PR-sized implementation sequence

### CO0 — Freeze compositional optional contracts

**Purpose:** Resolve every source and representation decision used by later
tasks while keeping the implemented grammar and availability unchanged.

- [x] Update the optional-values language contract with general postfix and
      grouping semantics, canonical and shorthand shared-owner spellings,
      arbitrary nesting, optional arrays, `none`, `some(expression)`,
      one-layer injection, one-layer unwrap, overload ranking, and eligible
      declaration positions.
- [x] Freeze an exact precedence and equivalence table for `T?[]`, `T[]?`,
      `(T[])?`, `(shared T)?`, `shared? T`, `(shared T)??`, `shared T?`, and
      `shared? T?`.
- [x] Specify all nested presence shapes, recursive lifecycle transitions,
      checked payload access, guard and anchor interaction, containment, failure
      order, and eligible/ineligible payload categories.
- [x] Update the optional-values compiler contract with recursive interned
      identities, phase ownership, lifecycle capability plans, HIR/MIR
      direction, tagged and niche layouts, internal ABI behavior, verifier
      obligations, and the unchanged C runtime boundary.
- [x] Preserve shared optional boxes as a focused exclusion in optional and
      shared-ownership contracts without reserving implementation structures
      for them.
- [x] Mark compositional optionals as a frozen design in the status matrix
      while leaving the implemented grammar and current support claims exact.
- [x] Define positive, compile-failure, runtime-failure, malformed-MIR,
      determinism, nesting-budget, layout-overflow, ABI-pressure, and lifecycle
      test matrices for the remaining tasks.

**Tests:** `make docs-check`, focused contract review against the current
grammar and implementation, `git diff --check`, and `make check`.

**Exit criteria:** Every included spelling, presence state, conversion,
lifecycle transition, layout category, and exclusion is unambiguous; the
future box boundary is explicit; and no living document claims planned forms
are already executable.

### CO1 — Introduce compositional type syntax and canonical shared optionals

**Purpose:** Establish one recursive source grammar and make the canonical
optional-owner spelling executable using only behavior already implemented.

- [x] Replace the closed syntax `OptionalPayloadKind` representation with a
      recursive optional type node retaining payload, grouping, and punctuation
      spans.
- [x] Parse a bounded postfix type chain containing `?` and `[]`, accept general
      parenthesized type grouping, and preserve focused recovery for missing
      targets, delimiters, repeated punctuation, and excessive nesting.
- [x] Accept `(shared T)?` everywhere `shared? T` is currently valid, and lower
      both spellings through the existing optional-shared semantics and
      zero-handle representation.
- [x] Treat `shared? T` as exact syntax shorthand while retaining enough source
      provenance for useful diagnostics and syntax inspection.
- [x] Parse nested optional, optional-array, and shared-box shapes
      compositionally, but keep their current semantic diagnostics until their
      responsible task completes; box forms remain rejected after this roadmap.
- [x] Make language documentation use `(shared T)?` canonically and describe
      `shared? T` as a supported alias. Keep representative existing shorthand
      fixtures rather than mechanically rewriting all source code.

**Tests:** Lexer and parser AST tests; grouping, precedence, alias, trivia,
recovery, and nesting matrices; syntax dumps; both spellings through current
optional-shared type checking, MIR, backend, and native execution; focused
rejections for deferred forms; `make check`; and `make msrv-check`.

**Exit criteria:** `(shared T)?` and `shared? T` are exactly one executable
type with unchanged ownership and ABI behavior, future compositions reach the
correct semantic gates, and no new payload category otherwise executes.

### CO2 — Canonicalize recursive optional identities

**Purpose:** Give resolution and all later semantic phases a deterministic,
scalable type graph before nested behavior depends on it.

- [x] Add an `OptionalTypeId` or equivalent small identity and an interned table
      whose entries name one complete resolved payload type, including prior
      optional and array identities.
- [x] Normalize canonical and shorthand shared-owner spellings to one
      optional-of-shared identity while keeping source spans outside equality
      and interning keys.
- [x] Replace flat resolved optional payload categories with canonical optional
      identity queries and preserve narrow public resolution facades.
- [x] Define deterministic bottom-up interning, equality, hashing, names, and
      dump order across repeated spellings, modules, arrays, and deep nesting.
- [x] Extend inline-containment cycle detection through arbitrary optional
      layers while retaining array descriptor and shared-edge boundaries.
- [x] Move payload eligibility and unsupported shared-box decisions to focused
      semantic validation so invalid types never enter executable HIR.

**Tests:** Resolution identity and interning tests; canonical/alias equality;
repeated cross-module spellings; optional and class-containment cycles; deep
valid and excessive nesting; optional/array combinations; focused box
diagnostics; deterministic resolved dumps across processes; `make check`; and
`make msrv-check`.

**Exit criteria:** Every parsed optional has one deterministic identity,
aliases do not create conversion edges or duplicate IDs, recursive containment
is correct, and lower phases never reconstruct type meaning from source shape.

### CO3 — Generalize typed optional lifecycle planning

**Purpose:** Make type checking and HIR describe recursive optional semantics
without forcing a simultaneous executable-IR rewrite.

- [x] Add a typed optional table or equivalent ID-indexed metadata carrying the
      immediate payload type, storage category, copy/assignment/destruction
      capability, checked-access category, and representation class.
- [x] Replace primitive/class/optional-shared HIR type families with canonical
      optional identities where their distinction is only payload category.
- [x] Select recursive initialization, injection, copy, assignment,
      destruction, presence-test, unwrap, checked-view, argument/result, static,
      and array-element lifecycle plans in type checking.
- [x] Generalize optional compatibility and overload ranking over exact
      identities while retaining current one-layer injection and existing
      optional-shared target compatibility.
- [x] Keep an explicit adapter from generalized typed HIR to the current
      executable MIR operations for the already supported primitive, class,
      and shared-owner cases; all new recursive cases remain gated.
- [x] Update HIR dumps, compiler architecture, phase documentation, debugging,
      and testing ownership without exposing implementation-private tables as
      public compiler APIs.

**Tests:** Complete existing optional type-check and HIR suites; lifecycle
capability and containment tests; overload and conversion matrices; fields,
statics, calls, interfaces, arrays, and aliases; deterministic HIR dumps;
adapter exhaustiveness; `make check`; and `make msrv-check`.

**Exit criteria:** Typed HIR has one recursive optional vocabulary and complete
payload lifecycle plans, all previously accepted programs lower through the
compatibility adapter unchanged, and no new source behavior reaches MIR.

### CO4 — Generalize executable optional MIR and target realization

**Purpose:** Replace the closed executable optional families while preserving
the full current runtime profile before enabling recursive payloads.

- [x] Add MIR optional identities and lifecycle metadata lowered deterministically
      from HIR, including payload storage, ownership, guarded-view, cleanup,
      argument, result, static, and array-element requirements.
- [x] Generalize optional initialization, assignment, publication, cleanup,
      presence tests, unwraps, views, and source categories over those
      identities without erasing real primitive, owning aggregate, or shared
      owner operations.
- [x] Generalize structural, initialization, ownership, lifetime, place, call,
      cleanup, guard, static-lifecycle, and array verification over recursive
      optional metadata.
- [x] Generalize x86-64 layout, frames, calling convention classification,
      target legality, cleanup, instruction lowering, and dumps while retaining
      current tagged layouts and the one-word optional-shared niche exactly.
- [x] Remove the temporary HIR-to-legacy-MIR adapter only after every current
      optional program and malformed-MIR test uses the generalized path.
- [x] Keep nested optional and optional-array source gates closed throughout
      this behavior-preserving migration.

**Tests:** Complete optional, shared-owner, array-element, static, callable,
logical-cleanup, guard, verifier-mutation, ABI-pressure, backend, native, and
cross-process determinism suites; exact layout assertions; before/after dump
review; `make check`; and `make msrv-check`.

**Exit criteria:** Every currently supported optional executes through one
generalized verified MIR path with unchanged output, failure order, lifecycle,
layout, and ABI, and a new payload category no longer requires a parallel MIR
type family.

### CO5 — Execute arbitrarily nested optional lifecycle

**Purpose:** Enable recursive owning storage and make every finite `T????`
presence shape constructible and lifecycle-correct.

- [x] Add `some(expression)` as an expected-type-directed expression that
      creates exactly one present optional layer; `none` continues to create
      outer absence.
- [x] Enforce exact or one-layer implicit injection without recursive lifting,
      including deterministic overload ambiguity and mismatch diagnostics.
- [x] Admit optional payload identities recursively and execute absent/present
      initialization, optional copy, assignment, self-assignment, conditional
      recursive cleanup, and direct final-destination construction.
- [x] Lay out every non-niche layer as its own state plus complete payload,
      preserve every distinct presence shape, and reject checked size or
      alignment overflow before code generation.
- [x] Carry nested optional fields through synthesized class copying,
      assignment, destruction, inline containment, static initialization, and
      reverse shutdown.
- [x] Permit nested optionals as default-absent array elements and explicit
      element-list destinations while preserving unpublished-prefix and
      reverse-cleanup invariants.

**Tests:** Primitive, exact-class, shared-owner, and mixed nesting at depths
two, five, and the supported stress boundary; `none`, `some(none)`, and every
presence depth; one-layer injection; overload failures; class lifecycle and
self-assignment traces; statics; array defaults and lists; layout overflow;
malformed MIR; `make check`; and `make msrv-check`.

**Exit criteria:** Arbitrarily deep supported optionals initialize, copy,
assign, and destroy every layer exactly once; every presence shape is
constructible; and absent payload bytes are never used as live values.

### CO6 — Complete nested-optional access and callable integration

**Purpose:** Carry nested storage through checked access, aliases, overloads,
and every supported internal callable boundary.

- [x] Make each postfix `!` check and remove one layer, with chained unwraps
      preserving evaluation once, source-order failures, and exact result type.
- [x] Generalize scalar extraction, owning wrapper copying, class payload
      places, shared-owner securing, nested guards, and shared-root anchors
      according to each immediate payload category.
- [x] Permit `ref value: P?` and `mut ref value: P?` for supported nested inline
      containers while continuing to reject optional references and aliases to
      unsupported designated categories.
- [x] Carry nested optionals through functions, methods, interfaces, virtual
      overrides, initializer overloads, value parameters/results, temporaries,
      recursion, register/stack pressure, and hidden aggregate destinations.
- [x] Complete exact-match, one-layer injection, `none`, and `some` overload
      ranking without introducing recursive conversion chains.
- [x] Preserve guard invalidation failures, re-entrant mutation protection,
      selected-path cleanup, and full-expression order across every consumer.

**Tests:** Chained unwrap success and failure; presence tests at every layer;
nested/overlapping guards; later-argument and re-entrant mutation; read-only
and mutable aliases; every callable and dispatch form; overload matrices;
recursion and ABI pressure; logical expressions; verifier mutations; native
failure goldens; `make check`; and `make msrv-check`.

**Exit criteria:** Nested optionals cross every supported internal boundary,
each unwrap exposes only one verified live layer for its bounded consumer, and
guards, anchors, temporaries, and ownership transfers balance on all normal
paths.

### CO7 — Execute optional inline arrays

**Purpose:** Add `T[]?` as a tagged optional owning one complete inline array
descriptor while keeping neighboring array and ownership forms distinct.

- [ ] Admit `T[]?` and `(T[])?` as one canonical optional identity for every
      currently legal inline array type, including recursive arrays and owning
      element categories.
- [ ] Initialize absence from `none`, inject exact array values one layer, and
      construct explicit presence through `some(expression)` without
      conflating absence and an empty array.
- [ ] Execute optional-array copy construction, assignment, self-assignment,
      produced-backing transfer where permitted, conditional backing cleanup,
      direct construction, presence tests, and checked one-layer unwrap.
- [ ] Reuse canonical array lifecycle capabilities and generated helpers rather
      than duplicating element copy, assignment, and destruction policy inside
      optional lowering.
- [ ] Implement tagged optional-array layout, frame storage, internal
      parameter/result behavior, verifier rules, failure edges, and native
      x86-64 lowering without changing the inline array descriptor or C runtime
      ABI.
- [ ] Preserve `shared? T[]` as shorthand for `(shared T[])?`, an optional
      shared array owner, and keep it distinct from `T[]?`.

**Tests:** Absent versus present-empty arrays; dynamic and element-list
construction; named copy and produced result; assignment/self-assignment;
nested arrays; every owning element lifecycle; presence and unwrap; layout and
allocation failures; MIR mutations; assembly helpers; destruction traces;
`make check`; and `make msrv-check`.

**Exit criteria:** `T[]?` executes as a conditional inline array value, every
present backing follows the existing array lifecycle, absence is never treated
as an empty descriptor, and all neighboring shared/element forms remain
unchanged.

### CO8 — Integrate optional arrays across stored and aggregate boundaries

**Purpose:** Complete optional arrays in every supported declaration, alias,
callable, and recursive aggregate position after core ownership is proven.

- [ ] Support optional arrays in class fields, static fields, internal
      parameters/results, methods, interfaces, overrides, initializer
      overloads, temporaries, and synthesized enclosing lifecycle.
- [ ] Permit read-only and mutable call-scoped aliases to optional-array
      containers with backing anchors and presence guards covering the complete
      immediate consumer.
- [ ] Permit optional arrays as array elements, including default outer
      absence, explicit element lists, indexing, slices, nested optional arrays,
      and reverse cleanup.
- [ ] Extend compatibility, overload, containment, capability, lifecycle,
      diagnostic, dump, and cross-process determinism coverage across every
      optional-array boundary.
- [ ] Prove that `T?[]`, `T[]?`, `T[][]?`, `(shared T[])[]`,
      `(shared T[])?`, and `shared? T[]` retain their intended canonical
      identities and operations.
- [ ] Update array, optional, lifecycle, alias, static, phase, backend, runtime
      ABI, debugging, and testing documentation as each boundary becomes
      executable.

**Tests:** Fields and statics; reverse shutdown; every call and dispatch form;
alias mutation and invalidation; array elements, lists, indexing, slices, and
reverse cleanup; spelling/identity matrices; ABI pressure; robustness and
determinism; `make check`; and `make msrv-check`.

**Exit criteria:** Optional arrays work in every supported owning and alias
position, recursive aggregate lifecycle is deterministic, and no array or
shared-owner spelling acquires an unintended identity or conversion.

### CO9 — Harden and publish compositional optionals

**Purpose:** Close cross-feature gaps, prove robustness, and promote the
bounded compositional optional profile to an implemented contract.

- [ ] Complete syntax, identity, eligibility, conversion, overload, lifecycle,
      alias, array, static, callable, ABI, failure, and diagnostic matrices
      across representative and stress nesting depths.
- [ ] Add hostile punctuation/grouping, excessive-depth, layout-overflow,
      malformed-MIR, generated-helper, cross-module, independent-process
      determinism, and native-failure coverage.
- [ ] Audit optional, array, lifecycle, verifier, and backend modules by
      responsibility; keep recursive identity, capability planning, storage
      state, guards, ownership, and target layout behind cohesive facades.
- [ ] Audit living language/compiler/runtime/debugging/testing documentation,
      update the implemented grammar and status matrix, and remove stale flat
      optional exclusions and rollout vocabulary.
- [ ] Prove `shared T?` and `shared? T?` remain focused shared-box exclusions,
      and prove all other non-goals remain rejected without provisional box
      IR, metadata, or backend hooks.
- [ ] Record lower-priority discoveries in a separately indexed document
      without expanding this roadmap; resolve every high-priority correctness
      or maintainability finding before closure.
- [ ] Remove roadmap task codes from living code, tests, dumps, diagnostics,
      and non-roadmap documentation, then archive the completed roadmap and
      update the active and archive indexes.

**Tests:** Complete focused compiler and golden suites; `cargo test --locked
--workspace`; `make check`; `make msrv-check`; `make robustness-long`;
documentation validation; `git diff --check`; artifact-free final validation;
and deterministic native execution across the compositional optional matrix.

**Exit criteria:** Canonical optional shared owners, arbitrary nested
optionals, and optional inline arrays execute through verified MIR on x86-64;
canonical and shorthand spellings share identities; boxes and all other
exclusions remain rejected; living documentation states only current behavior;
full repository gates pass; and no high-priority discovery remains unresolved.

## Ordering and dependencies

CO0 settles the source and compiler contracts before representations depend on
them. CO1 establishes compositional syntax and the canonical optional-owner
form using only currently executable behavior. CO2 creates recursive identities
before CO3 teaches type checking and HIR to select recursive lifecycle plans.
CO4 migrates executable MIR, verification, and the backend while all new source
gates remain closed, keeping representation churn separate from new behavior.

CO5 then proves recursive owning lifecycle before CO6 exposes nested payloads
through aliases and callable boundaries. CO7 reuses the complete recursive
wrapper over the existing array lifecycle; CO8 finishes aggregate and stored
integration. CO9 alone promotes the profile to implemented and closes the
roadmap.

The roadmap depends on the implemented optional-value, array,
shared-ownership, static-field, logical-cleanup, and panic contracts. It has no
dependency on another active roadmap. A future shared optional-box roadmap may
depend on this completed work, but no box-related implementation is a
dependency or parallel task here.
