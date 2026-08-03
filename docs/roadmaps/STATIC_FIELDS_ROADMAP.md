# Zero-Default Static Fields Roadmap

Status: in progress; STF5 is next.

This roadmap adds class-owned mutable storage for values whose complete live
state can be established by zero-filled target storage before Skald entry
begins. The completed profile provides useful counters, flags, optional
caches, and inline-array registries without adding source initialization
expressions, module startup, shutdown ordering, garbage-collector roots, or a
new runtime service. Static storage remains distinct from object fields in
every semantic phase and becomes a verified program-owned place in MIR rather
than a special raw load/store escape from Skald's ownership machinery.

The implementation baseline has advanced since the first static-field audit.
The roadmap therefore composes with the complete primitive operator and cast
matrix, binary64 bit reinterpretation, conditional full-expression cleanup,
the split optional/shared/path-state verifiers, standard I/O buffer aliases,
cyclic modules, and runtime ABI version 8. It treats the sibling Niflheim
implementation as evidence for declaration separation, canonical ownership,
and deterministic symbols, but does not copy its nullable-reference, GC-root,
or runtime-shutdown model.

## Scope and invariants

- Accept `static name: T;` and `private static name: T;` as dedicated class
  members. `static` remains contextual: `static: i64;`, method and function
  names, parameters, locals, and other existing identifier positions retain
  their current meaning.
- Give every declaration one class-owned `StaticFieldId` allocated in direct
  static-field source order. Do not reuse `FieldId`, add a late `is_static`
  flag to instance fields, or renumber existing instance fields and methods.
- Keep static fields in the existing non-overloaded ordinary-member namespace.
  They collide with direct or inherited instance fields, static or instance
  methods, and other static fields. A derived class cannot hide or redeclare
  an inherited static field.
- Let `Class.name` and `module_binding::Class.name` select one static field by
  its declaring identity. Selection through a derived class aliases the exact
  base-owned slot; it never allocates derived storage. An unqualified local
  continues to shadow an unqualified class spelling.
- Apply the existing declaring-class privacy rule after selection. A private
  static field is accessible exactly from a callable lexically owned by its
  declaring class, including static methods and lifecycle bodies. Privacy is
  erased before HIR after resolution authorizes the use.
- Reject object-qualified static access and class-qualified instance-field or
  instance-method use with focused wrong-kind diagnostics. Calling a static
  field remains a non-callable-target error. Class selection itself has no
  runtime evaluation or receiver.
- Treat every static field as mutable program-owned storage. A static method
  needs no `mut` receiver to write it. Existing `ref` and `mut ref` call rules
  may borrow a compatible static place for one call; aliases still cannot
  escape or become stored values.
- Support exactly the source storage types with an all-zero complete live
  state: `i64`, `u64`, `u8`, `f64`, `bool`, primitive and exact-class `T?`,
  `shared? T` for every currently supported shared target, and inline `T[]`.
  Zero means numeric zero, positive floating zero, `false`, absent optional,
  absent optional shared owner, or the existing allocation-free empty inline
  array descriptor.
- Reject exact inline `T`, non-optional `shared T`, non-optional shared arrays,
  `Obj`, interfaces, `unit`, and every other type without an existing valid
  all-zero owning representation. A future extension must not reinterpret
  zero as a null non-optional handle or as an initialized exact object.
- Add no declaration initializer, static initializer block, static lifecycle
  member, lazy initialization, arbitrary constant evaluator, initialization
  dependency graph, or observable declaration/module initialization order.
  All supported slots are live simultaneously before the selected Skald entry
  function runs.
- Give static slots process lifetime. Ordinary replacement during execution
  retains, copies, adopts, releases, destroys, or frees the replaced value
  through the existing type-specific rules, but the final contents are not
  cleaned when the Skald entry function or host `main` returns. A final
  optional owner, present optional object, or inline-array backing therefore
  remains live until process termination and its destructor need not run.
- Keep normal local, argument, result, field, temporary, and full-expression
  cleanup unchanged. Static storage is never registered in a lexical cleanup
  scope and never receives `StorageLive` or `StorageDead`.
- Represent static storage as a typed program-owned MIR place root so ordinary
  loads, stores, optional operations, shared ownership, arrays, aliases,
  checked views, primitive operations, casts, logical branches, and I/O can
  reuse their current semantic operations. Do not introduce raw reference
  stores or a parallel unverified ownership path.
- Emit one target-private, writable, aligned, zero-filled slot per declared
  static field. Static slots do not affect instance layout, base prefixes,
  containment cycles, definite field initialization, synthesized class copy
  or destruction plans, dispatch metadata, string literal backing, external
  linkage, or callable ABI classification.
- Preserve deterministic declaration IDs, phase dumps, diagnostics, MIR,
  assembly, object symbols, and native behavior across module discovery and
  compiler processes.
- Add no public C symbol, allocator behavior, GC root, startup/shutdown call,
  process-wrapper lifecycle step, or runtime ABI revision. Runtime ABI version
  8 remains the compatibility boundary.
- Keep atomic access, synchronization, thread-local storage, reflection,
  top-level globals, `final`/constant statics, external statics, source-visible
  symbol export, exact-class/non-optional-owner initialization, and static
  destruction explicit non-goals.

## Representation and ownership boundaries

Syntax should use a distinct static-field class-member variant so the parser's
existing focused rejection becomes an accepted declaration without routing it
through instance `FieldDecl`. Resolution owns `StaticFieldId`, source names,
visibility, stored type syntax, inherited selection, module qualification, and
all source diagnostics. Resolved class declarations keep direct static fields
separate from instance fields while their shared hierarchy member map gains a
tagged static-field case.

HIR retains typed static declarations plus identity-only static reads and
places. The type checker owns the zero-default type restriction and selects
the existing primitive, optional, optional-shared, array, alias, and checked
view operations. A static array or optional owner is replaceable storage:
borrowed backing or pointee views must receive the same securing anchors that
protect a replaceable object field across later calls and argument effects.

MIR adds program-level static declarations and a static place root. Place APIs
must distinguish function-local storage from program storage instead of
assuming that every base has a `StorageId`. Verification resolves a static
root's type and mutable access from the program declaration table, treats it
as live and initialized at every callable entry, and still visits local
projection carriers such as normalized array indices. Lifetime dataflow must
ignore the root for `StorageLive`/`StorageDead`; optional, shared, array,
checked-view, call, I/O, and path-conditioned ownership analyses must track the
complete `MirPlace` where their existing invariants require it. The recent
private verifier splits and common path-state facade remain intact.

The x86-64 backend maps verified static roots to deterministic RIP-relative
symbols rather than frame homes. Target place addressing must support loads,
stores, aggregate projections, and address-taking for alias and I/O arguments
without making the frame planner own global slots. A cohesive private static
data owner should plan slot size, alignment, symbol, and emission; substantial
static-place addressing belongs in a descriptive lowering module while
`mod.rs` files remain facades.

## Progress

- [x] STF0 — Freeze the zero-default static-field contract
- [x] STF1 — Separate static declarations and inherited identity
- [x] STF2 — Establish typed static places and primitive behavior
- [x] STF3 — Add verified static MIR roots and x86-64 slots
- [x] STF4 — Extend inline optional storage to static roots
- [ ] STF5 — Extend optional shared ownership to static roots
- [ ] STF6 — Extend inline arrays, aliases, and I/O to static roots
- [ ] STF7 — Harden composition and publish the implemented contract

## PR-sized implementation sequence

### STF0 — Freeze the zero-default static-field contract

**Purpose:** Publish the exact source, lifetime, supported-type, and runtime
boundaries before representation work depends on them.

- [x] Add a clearly marked frozen future extension to the language grammar for
      `static name: T;` and `private static name: T;`, including contextual
      disambiguation from an instance field named `static`.
- [x] Specify class-qualified and module-qualified access, inherited aliasing,
      declaring-class privacy, shared namespace collisions, local shadowing,
      wrong-kind uses, and the absence of receiver evaluation.
- [x] Specify the complete supported zero-state type set and exact initial
      values, including positive-zero `f64`, absent optionals, and empty inline
      arrays. Record targeted rejection for every non-zero-valid storage type.
- [x] Specify program/process lifetime, ordinary replacement effects, and the
      observable absence of final static cleanup after normal entry return.
- [x] State that static fields add no initialization order, shutdown order,
      runtime service, panic reason, public symbol, or ABI-version change.
- [x] Update the status matrix only as a frozen planned contract; do not claim
      compiler availability before the final task.

**Tests:** Documentation link/index checks and a consistency review across the
grammar, classes/lifecycle, types/values, optional values, shared ownership,
arrays, modules/interoperation, errors, and status documents; then
`make docs-check` and `git diff --check`.

**Exit criteria:** Every later task can decide syntax, legality, initial state,
access, mutation, ownership, and exit behavior without another language-level
choice, while living documentation still distinguishes the frozen extension
from implemented syntax.

### STF1 — Separate static declarations and inherited identity

**Purpose:** Establish source and resolved identities that cannot leak into
instance layout or lifecycle before accepting static uses in typed HIR.

- [x] Add a dedicated syntax static-field declaration and class-member variant
      with visibility, `static` span, name, type, and complete declaration
      span. Keep syntax module exports selective and its facade concise.
- [x] Replace the parser's current unsupported-static-field branch with exact
      parsing for public and private static fields while preserving recovery
      for duplicate/misordered modifiers, lifecycle combinations, missing
      names/types/semicolons, and later class members.
- [x] Preserve contextual identifier behavior for `static: T`, `fn static`,
      parameters, locals, and top-level declarations in lexer/parser dumps and
      robustness coverage.
- [x] Add class-owned `StaticFieldId` allocation independent of `FieldId` and
      `MethodId`, plus resolved static declaration tables and program/class
      accessors with dense-ID validation.
- [x] Extend the canonical ordinary-member tag, hierarchy construction,
      collision diagnostics, virtual/override diagnostics, and member display
      helpers with static-field identity. Derived selection must retain the
      declaring base ID and all inherited redeclarations must fail.
- [x] Keep static declarations out of instance `fields`, containment graphs,
      initializer completeness, copy capabilities, destruction plans,
      interface conformance, virtual families, and string-language-item field
      matching.
- [x] Extend deterministic AST/resolved dumps and declaration tests across
      interleaved instance fields, statics, methods, modules, and cyclic module
      graphs. Use a cohesive resolver submodule if static collection would
      otherwise enlarge the class facade substantially.

**Tests:** Focused syntax static-member, recovery, nesting, and dump tests;
resolver declaration, hierarchy, virtual-family, module, cyclic-import,
privacy-ready identity, and exact dump tests; frontend robustness smoke tests;
then `make compiler-test`, `make msrv-check`, and `git diff --check`.

**Exit criteria:** Valid declarations have stable independent identities and
participate in the complete inherited namespace, while no lower phase or
object lifecycle can mistake them for instance fields and source uses remain
deliberately unavailable until typed place semantics exist.

### STF2 — Establish typed static places and primitive behavior

**Purpose:** Define the receiver-free source-to-HIR place contract using the
five primitive types before ownership-bearing static roots enter MIR.

- [x] Resolve class- and module-qualified static reads, assignment targets,
      call arguments, and non-callable uses directly to `StaticFieldId`.
      Reuse static-method class selection and module lookup rather than adding
      a second class-name parser or import path.
- [x] Diagnose object-selected statics, class-selected instance members,
      inaccessible private statics, unknown members, static calls on fields,
      bare static values where a place/value is unavailable, and local
      shadowing with deterministic existing-precedence rules.
- [x] Add typed static declarations and a small identity-based `HirStaticPlace`
      owned by the HIR facade. Extend existing primitive expression,
      assignment, and alias families with a static-place case instead of
      cloning their operation semantics.
- [x] Enforce the full zero-default type predicate centrally, but initially
      enable source use only for primitive statics while later tasks add the
      owning families. Unsupported declarations must receive one stable
      type-check diagnostic at their type span.
- [x] Treat statics as mutable without a receiver capability. Permit
      read-only and mutable primitive alias arguments under existing non-escape
      rules, with class selection contributing no runtime evaluation.
- [x] Preserve all current primitive consumers: arithmetic, division and
      remainder, bitwise and shift operations, comparisons, eager and
      short-circuit boolean expressions, the complete cast matrix, `f64` bit
      reinterpretation, calls, returns, control flow, and I/O scalar values.
- [x] Extend resolved/HIR dumps and focused tests without broadening public
      submodule namespaces or adding static-specific primitive algorithms.

**Tests:** Resolver and type-check matrices for qualified access, privacy,
inheritance, wrong-kind selection, shadowing, primitive assignment, aliases,
and all five primitive types; HIR shape/dump and evaluation-order tests;
representative operator, checked-cast, logical-selection, bit-round-trip, and
static-method compositions; then `make compiler-test`, `make msrv-check`, and
`git diff --check`.

**Exit criteria:** Primitive static programs are fully selected and typed by
stable identity with no receiver or instance-field representation, and HIR
contains all information needed for target-independent static roots without
claiming native execution.

### STF3 — Add verified static MIR roots and x86-64 slots

**Purpose:** Make primitive static storage executable through a general
program-owned place foundation before adding optional and array ownership.

- [x] Add typed, dense `MirStaticFieldDeclaration` storage to `MirProgram` and
      lower the HIR declarations without target size, alignment, symbol, or
      section decisions.
- [x] Add a static `MirPlaceBase`/root and replace unconditional
      `base.storage()` assumptions with narrow root-aware APIs. Preserve local
      carrier identities for aliases, checked views, path activations, array
      indices, allocations, and temporaries.
- [x] Teach structural place verification to resolve static identity, type,
      and mutable access from `MirProgram`; reject missing, duplicate,
      wrong-type, and projected-through-nonaggregate uses, plus attempts to
      apply function-local storage-lifetime operations to a static root.
      Static roots are always live.
- [x] Update generic place visitors, argument checks, primitive stores/loads,
      lifetime analysis, dumps, test fixtures, public mutation tests, and
      backend legality so malformed static MIR fails before layout or
      instruction selection.
- [x] Generalize target place addressing so frame-backed, indirect, and
      symbol-backed places share checked projection/layout logic without
      assigning static slots fake frame homes.
- [x] Add deterministic collision-proof static symbols and a private
      ID-indexed slot plan using existing target type layouts. Extend the
      assembly model and emitter with aligned, writable, zero-filled local
      objects and correct byte, integer, and floating loads/stores/addressing.
- [x] Keep literal backings, dispatch metadata, panic messages, functions, and
      static slots as distinct assembly object families. Preserve the existing
      host-entry wrapper and runtime ABI marker exactly.
- [x] Place substantial slot planning/emission and static-place lowering in
      descriptive private modules, with explicit facade re-exports only where
      a cross-owner API is required.

**Tests:** MIR declaration density, place typing/access, always-live behavior,
foreign identity, lifetime walker, argument, dump, and mutation tests; backend
layout, symbol collision, section/alignment, byte/floating access,
RIP-relative address, legality, deterministic assembly, assembler acceptance,
and primitive native execution tests; then `make check`, `make msrv-check`,
and `git diff --check`.

**Exit criteria:** Public and private primitive statics execute through one
verified program-owned place model, emit deterministic zero-filled symbols,
compose with aliases and every primitive operation, and change neither object
layout nor the runtime interface.

### STF4 — Extend inline optional storage to static roots

**Purpose:** Add absent-by-default primitive and exact-class optionals while
preserving checked-view, mutation, copying, and conditional lifecycle rules.

- [x] Enable primitive `T?` and exact-class `T?` static declarations as
      initialized absent containers at every callable entry. Zero payload
      bytes carry no value while absent.
- [x] Extend typed optional source/destination/place families with static
      roots for presence tests, unwrap, copy, assignment, clearing,
      exact-class construction/copy/assignment, aliases to inline optional
      containers, and checked payload views.
- [x] Seed and preserve static optional initialization in the split optional
      verifier without coupling it to local `StorageLive` epochs. Integrate
      complete static places with existing path-conditioned state, guard
      counts, and mutation rejection.
- [x] Ensure static optional-class storage does not add a class-containment
      edge or instance bytes. Its target slot still uses the existing checked
      optional-class layout for its reserved payload.
- [x] Reuse ordinary replacement behavior: replacing a present exact-class
      payload runs its current assignment or destruction path, while the final
      present payload remains live at process termination and receives no
      generated cleanup.
- [x] Extend x86-64 static addressing through optional state and payload
      projections without a separate optional representation or runtime call.

**Tests:** Absent defaults for every primitive and representative exact
classes; presence, unwrap failure, assign/clear/replace, checked-view guard,
alias, private/inherited/module, conditional-expression, loop, copy lifecycle,
and final-no-cleanup native tests; optional verifier mutations over static
roots and projection types; layout and assembler coverage; then `make check`,
`make msrv-check`, and `git diff --check`.

**Exit criteria:** Inline optionals in static storage begin absent, obey every
existing dynamic guard and replacement rule, never participate in lexical
cleanup, and leave their final state live when entry returns.

### STF5 — Extend optional shared ownership to static roots

**Purpose:** Add process-lifetime optional strong owners without allowing zero
to enter the non-null shared-handle machinery or weakening ownership proof.

- [ ] Enable `shared? T` statics for current class, interface, `Obj`, and array
      targets with the existing zero absent niche. Continue rejecting every
      non-optional `shared T` static.
- [ ] Extend optional-shared typed places, copy/adopt/move assignment,
      presence, secured unwrap, owner casts, call arguments/results, and
      replaceable borrowed-source anchoring with static destinations and
      sources.
- [ ] Generalize shared and optional initialization state where it assumes a
      function-local root. A static optional owner is an initialized container
      at every function entry, contributes an owner only when dynamically
      present, and is never consumed merely by returning from a callable.
- [ ] Preserve secure self-assignment and replacement: retain or secure the
      incoming owner before conditionally releasing the old static owner, and
      never pass zero to retain, release, finalization, or allocation helpers.
- [ ] Prove that replacing the last old owner performs ordinary dynamic
      finalization, while the final owner stored at process exit is not
      released and its destructor need not run.
- [ ] Keep the pending produced optional shared-array result-unwrap discovery
      outside this task unless static-root support directly exposes the same
      invariant. Do not weaken its verifier as a workaround.

**Tests:** Class/interface/`Obj`/array target declarations; absent read,
present assignment, copy, produced adoption, self-assignment, secured unwrap,
casts and views, call and conditional paths, private/inherited/module access,
old-owner last-release, final-owner no-release, cycles, malformed MIR, and
assembly/native determinism; run the complete shared and optional verifier
suites plus `make check`, `make msrv-check`, and `git diff --check`.

**Exit criteria:** Optional shared statics maintain exact strong-owner counts
during all executable operations, preserve zero as absence, and intentionally
retain their final process-lifetime owner without any GC or runtime-root API.

### STF6 — Extend inline arrays, aliases, and I/O to static roots

**Purpose:** Complete the supported zero-state type set with empty inline
arrays and prove that replaceable static backing composes with aliases,
slices, ownership, and the current byte-I/O boundary.

- [ ] Enable every legal inline `T[]` static declaration with the existing
      zero empty descriptor, independent of element default construction. Keep
      non-optional shared-array statics rejected; optional shared-array owners
      remain covered by the preceding task.
- [ ] Extend array owner places, whole-array replacement, indexing, element
      assignment, slices, copied slices, length, nested arrays, and checked
      failure edges with static roots through existing typed operations.
- [ ] Treat a static array as replaceable storage. Secure a detached backing
      anchor whenever an alias, slice, element place, or later call effect
      could outlive the source selection within its full expression.
- [ ] Generalize array ownership, alias-dependency, legality, and cleanup
      verification away from unconditional local-storage roots while retaining
      local normalized-index, range-offset, slice-check, backing, anchor, and
      path-activation storage identities.
- [ ] Permit compatible `ref` and `mut ref` static array arguments and compose
      static byte arrays with the implemented standard-I/O read/write buffer
      intrinsics, including empty buffers, partial transfers, and later
      argument effects.
- [ ] Reuse ordinary replacement release for displaced nonempty backing. Do
      not emit exit cleanup for the final backing, recursively stored elements,
      or their destructors.
- [ ] Extend target symbol addressing through descriptors and array operations
      while keeping generated backing, clone, element, anchor, and release
      helpers unchanged.

**Tests:** Empty default, primitive/class/optional/shared/nested element
families, replacement and old-backing release, indexing, slices, overlap,
aliases, later mutation anchoring, loops and short-circuit paths, I/O read/write
buffers, private/inherited/module access, final-backing no-release, verifier
mutations, assembly acceptance, and native goldens; run focused array, I/O,
logical shared/array lifetime, and path-state suites plus `make check`,
`make msrv-check`, and `git diff --check`.

**Exit criteria:** Inline array statics begin as valid empty owners and support
the complete implemented array and byte-I/O surface with verified backing
stability, ordinary replacement release, and deliberate process-lifetime
retention of their final backing.

### STF7 — Harden composition and publish the implemented contract

**Purpose:** Close cross-phase gaps, prove determinism and exclusions, and
replace future-static wording with one authoritative implemented profile.

- [ ] Audit every exhaustive member, declaration, expression, statement,
      place, ownership, lifetime, dump, legality, layout, symbol, and test
      helper match for the static-field cases. Resolve responsibility hotspots
      in touched modules; record unrelated maintainability work separately.
- [ ] Prove cross-module and cyclic-module public/private access, inherited
      canonical identity, local shadowing, collision ordering, source-order ID
      allocation, and independence from module discovery order.
- [ ] Prove composition with static and instance methods, lifecycle bodies,
      initializer overloads, primitive operators and casts, binary64 bit
      conversion, optionals, shared casts/views, arrays, loops, logical
      selection, panic isolation, strings where the supported type boundary
      permits them, standard I/O, and both alias modes.
- [ ] Add exact compile-failure goldens for unsupported declaration types,
      malformed modifiers, privacy, wrong selection kind, collisions, and
      invalid uses; add native goldens for zero defaults, mutation,
      inheritance/module aliasing, ownership replacement, and intentional
      absence of exit cleanup.
- [ ] Extend cross-process pipeline determinism through AST, resolved, HIR,
      MIR, diagnostics, static symbols, assembly, stdout/stderr/status, and
      module-source permutations. Ensure unrelated primitive and standard
      library fixtures do not need duplicated static setup.
- [ ] Update the implemented grammar, classes/lifecycle, types/values,
      optional, shared ownership, arrays, functions/control flow, modules,
      errors, status, compiler phases/IR, backend, runtime ABI, debugging, and
      testing documentation. Keep the process-lifetime rule authoritative in
      one language location and link to it elsewhere.
- [ ] State explicitly that runtime ABI version 8, the public C header, the
      process entry wrapper, object layouts, dispatch metadata, and external
      ABI remain unchanged. Remove the old blanket statements that all static
      storage is unsupported while retaining exact-class/non-optional-owner
      initialization and shutdown as future work.
- [ ] Review the planned produced-object-alias roadmap against the completed
      static alias behavior. Preserve its independent produced-temporary scope
      and update only material shared call/full-expression assumptions.
- [ ] Run the full repository gate from an artifact-free snapshot, the MSRV
      gate, extended robustness, documentation/link checks, and diff hygiene.

**Tests:** All focused suites above plus `make check`, `make msrv-check`,
`make robustness-long`, `make docs-check`, and `git diff --check`; inspect a
clean generated assembly artifact for section, symbol, relocation, and runtime
marker hygiene.

**Exit criteria:** Every documented supported static field compiles and
executes deterministically across the current language surface, every excluded
form fails at its owning phase, no final static cleanup or runtime integration
is synthesized, all living documentation describes current behavior without
roadmap codes, and no high-priority responsibility problem remains unrecorded.

## Ordering and dependencies

The contract comes first because zero validity and process lifetime decide the
entire initialization, ownership, and runtime boundary. Declaration identity
then lands without contaminating object layout. Primitive HIR establishes the
source place contract before MIR is generalized, and the primitive native
slice proves static root verification, target addressing, symbols, and data
emission before ownership-bearing categories depend on them. Inline
optionals, optional shared owners, and arrays follow separately because each
has a distinct verifier/dataflow responsibility and independently observable
replacement behavior. Broad composition and documentation wait until every
supported category is executable.

This roadmap depends on the implemented module/privacy/static-method,
primitive/cast, optional, shared-ownership, array, logical full-expression,
path-state, standard-I/O, x86-64 backend, and runtime-version-8 contracts. It
has no semantic dependency on the planned produced-object-alias roadmap; the
two may proceed independently, but whichever lands second must re-run and, if
needed, adapt shared call-argument and full-expression coverage. The pending
optional shared-array result-unwrap and standard-I/O test-organization
discoveries remain separately owned and do not expand this roadmap.
