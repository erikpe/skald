# Constructor Overloads and Explicit Copy Construction Roadmap

Status: in progress; CM2 complete, CM3 is next.

This roadmap replaces the transitional single-initializer and signature-
classified copy-constructor model with the frozen constructor contract:
one or more ordinary `init` overloads, one distinct `copy` lifecycle slot,
compile-time most-specific overload selection, and explicit target-directed
`T(copy source)` construction. It leaves every selected operation explicit in
typed IR so shared allocation can later reuse the same construction modes
without reinterpreting syntax or ownership.

The source contract is
[Classes and Lifecycle](../language/CLASSES_AND_LIFECYCLE.md#ordinary-initializer-overloads).
The current/frozen boundary is recorded in the
[status matrix](../language/STATUS.md), and the phase ownership boundary is
recorded in [Compiler Phases and Intermediate Representations](../compiler/PHASES_AND_IR.md).

## Current baseline

- The parser and resolver represent `copy(ref source: T)` as the distinct copy
  lifecycle slot. Every `init(...)` is ordinary, and source collection still
  rejects a second ordinary initializer.
- Resolved and HIR class declarations and definitions store dense,
  source-ordered ordinary-initializer vectors, and MIR lowers every entry.
  Source collection still rejects a second ordinary initializer. Calls
  preselect the sole accepted initializer during name resolution, before
  argument compatibility is known.
- `InitializerId` contains a class-local ordinal, backend symbols include it,
  and MIR stores ordinary initializer declarations in a vector. Copy
  construction now uses the distinct `CopyConstructorId` and callable-symbol
  namespace introduced by CM0. The singular assumptions above MIR still
  prevent ordinary initializer ordinals from being used source-side.
- Copy construction, checked object places, slicing, exact-class copy
  capability selection, deterministic cleanup, and verified MIR execution are
  implemented. The migration changes declaration and explicit-selection
  syntax, not their established lifetime guarantees.
- `new` and shared storage are not implemented. The planned shared roadmap
  currently depends on this roadmap for ordinary overload and explicit-copy
  selection.

## Scope and invariants

- Every class declares one or more ordinary `init` members. No implicit
  default initializer is synthesized.
- Ordinary initializers form one class-owned overload set; function and method
  overloading remain out of scope.
- Parameter names do not participate in signatures. Initializers with the same
  ordered parameter types are rejected when identical or when they differ
  only by binding mode.
- Applicability uses existing argument binding, access, view-conversion, and
  copy-capability rules. Constructor overloading adds no implicit numeric
  conversion, runtime downcast, or ownership conversion.
- The selected overload is the unique most-specific applicable ordered
  parameter-type sequence. Binding mode is never a specificity tiebreaker.
  Missing and ambiguous matches are compile-time errors with deterministic
  candidate diagnostics.
- Selection uses static source types. Runtime dynamic class never dispatches
  an ordinary initializer. An explicit checked cast may refine an argument
  before selection.
- Each derived ordinary initializer begins with one `super(arguments);`, which
  independently selects one direct-base overload using the same rules.
  Delegation between ordinary initializers remains unsupported.
- `copy(ref source: T)` is the sole explicit copy-constructor declaration for
  `T`. It is a distinct lifecycle identity and never an `init` overload.
  Missing declarations retain the existing synthesized-copy capability rules.
- `T(copy source)` selects copy construction explicitly and exactly once.
  `T(arguments)` selects only ordinary initialization and never falls back to
  copy construction. Existing implicit owning-copy contexts remain valid.
- Explicit copy construction evaluates its source once, applies the
  target-directed static/runtime/impossible object-view relation, keeps the
  checked source live through copy completion, and is never copy-elided.
  Slicing to exact `T` is deliberate; dynamic-type-preserving cloning is
  deferred.
- `copy` is contextual only in its direct lifecycle declaration and
  construction-marker positions. It remains an ordinary identifier elsewhere.
- Ordinary initializer identities are dense, class-owned, source-ordered, and
  distinct from the one copy-constructor identity. Resolved IR may retain
  candidates, but HIR and lower phases carry exactly one selected operation.
- Receiver-before-argument order, left-to-right arguments, destination
  selection, stable spans, deterministic dumps, normal cleanup, backend
  symbols, and native observations remain deterministic.
- Access modifiers, default arguments, variadic constructors, initializer
  delegation, user-defined conversions, runtime overload dispatch,
  function/method overloads, dynamic cloning, shared storage, and heap
  allocation are non-goals.

## Progress

- [x] CM0 — Give copy construction a distinct identity
- [x] CM1 — Generalize ordinary initializer storage
- [x] CM2 — Adopt the distinct copy-constructor declaration
- [ ] CM3 — Select ordinary initializer overloads
- [ ] CM4 — Select overloaded direct-base initialization
- [ ] CM5 — Execute explicit target-directed copy construction
- [ ] CM6 — Harden and publish the constructor model

## PR-sized implementation sequence

### CM0 — Give copy construction a distinct identity

**Purpose:** Remove the identity collision between ordinary and copy
construction before either declaration family expands.

- [x] Introduce a dedicated `CopyConstructorId` and `CallableId` variant,
      parallel to copy assignment and destruction, so a copy lifecycle
      operation cannot enter an ordinary initializer candidate set.
- [x] Migrate resolved, HIR, MIR, capability, verifier, backend, symbol, dump,
      local-owner, parameter, and test-fixture paths from copy-as-
      `InitializerId` to the dedicated identity.
- [x] Keep the current source behavior temporarily: one accepted ordinary
      initializer and the legacy signature-classified copy declaration still
      compile while copy construction receives an honest internal identity.
- [x] Verify declaration/definition ownership for the copy slot independently
      of ordinary initializer density before backend lowering.
- [x] Update phase dumps and internal documentation for the separated identity
      without claiming a source-level syntax change.
- [x] Add identity/verifier mutations, callable-symbol collision tests, and
      regressions for all existing copy capability and native lifecycle
      behavior.

**Tests:** Focused identity, resolution, HIR, MIR lowering/verifier, backend
symbol, copy/lifecycle, deterministic-dump, and native regression tests,
followed by `make check` and `make msrv-check`.

**Exit criteria:** Every phase represents and executes the copy constructor
with a type-distinct identity, while current accepted source programs retain
their behavior.

### CM1 — Generalize ordinary initializer storage

**Purpose:** Remove singular declaration and body storage before enabling an
overload set or changing source copy classification.

- [x] Store ordinary initializer declarations and executable bodies as dense
      source-ordered vectors in resolved IR and HIR; lower the complete vectors
      into the existing MIR collection.
- [x] Give class-body work items explicit initializer member/identity pairs so
      declaration recovery cannot desynchronize syntax, metadata, and bodies.
- [x] Generalize class/program lookup, parameter/local ownership, definition
      iteration, capability consumers, dumps, and test fixtures without yet
      accepting a second ordinary initializer.
- [x] Verify declaration/definition density, class ownership, parameter
      ownership, and body existence for every stored ordinary initializer.
- [x] Preserve the current preselected single-initializer call path behind a
      narrow helper so the later overload task can replace it without touching
      storage consumers again.
- [x] Keep backend code generation ordinal-driven and prove distinct
      initializer labels and frames through multi-entry HIR/MIR fixtures.
- [x] Update phase documentation for the generalized representation without
      claiming source-level overloading.

**Tests:** Focused resolved/HIR/MIR table, class-body, identity/density
verifier, dump, backend-symbol/frame, and regression tests, followed by
`make check` and `make msrv-check`.

**Exit criteria:** Every phase can store, find, dump, verify, and emit multiple
ordinary initializer declarations and bodies, while the accepted language
still exposes one.

### CM2 — Adopt the distinct copy-constructor declaration

**Purpose:** Make lifecycle intent source-explicit and remove signature-based
classification before `init` becomes an overload set.

- [x] Add a source-shaped `copy` class-member declaration with exact introducer,
      parameter, body, and recovery spans; keep `copy` contextual rather than
      reserving it lexically.
- [x] Resolve `copy(ref source: T)` directly into the dedicated copy slot and
      require exactly one read-only alias parameter designating the enclosing
      exact class.
- [x] Reject duplicate, malformed, modified, result-bearing, additional-
      parameter, wrong-mode, and wrong-target copy declarations without
      reclassifying them as ordinary initializers or methods.
- [x] Stop classifying any `init` signature as copy construction.
      `init(ref source: T)` becomes an ordinary initializer under the same
      rules as every other `init`.
- [x] Migrate all repository source, focused fixtures, goldens, dumps, and
      diagnostics from the legacy copy declaration to `copy`.
- [x] Preserve user and synthesized copy capability, base composition,
      definite-field initialization, cleanup, and native effects unchanged.
- [x] Update the implemented grammar and status boundary when the new
      declaration becomes accepted; remove the legacy source spelling from
      living behavior documentation.

**Tests:** Focused lexer/contextual-word, parser/recovery, resolution,
declaration-diagnostic, copy-capability, inheritance, HIR/MIR dump, backend,
golden, and native tests, followed by `make check`, `make msrv-check`, and
`make robustness-long`.

**Exit criteria:** `copy(ref source: T)` is the only explicit copy-constructor
declaration, `init(ref source: T)` is ordinary, and no compiler phase infers
lifecycle kind from an initializer signature.

### CM3 — Select ordinary initializer overloads

**Purpose:** Enable direct class construction through one reusable,
diagnostic-quality overload engine.

- [ ] Accept one or more ordinary initializers per class and require at least
      one even for an empty class; assign dense `InitializerId` ordinals in
      ordinary-initializer source order.
- [ ] Reject duplicate signatures and same-parameter-type sequences that
      differ only by binding mode. Parameter names remain irrelevant.
- [ ] Change resolved construction from one preselected initializer to the
      resolved class, source-ordered arguments, and stable candidate set or
      class-owned lookup needed by type checking.
- [ ] Analyze each argument once into reusable static type, access, place,
      provenance, and production facts. Probe candidate applicability without
      emitting final diagnostics or constructing candidate-specific HIR.
- [ ] Centralize applicability over the existing value, copy, `ref`, and
      `mut ref` binding rules. Do not add implicit downcasts, primitive
      conversions, or ownership conversions.
- [ ] Implement the unique most-specific static parameter-type relation using
      the canonical class/interface/`Obj` hierarchy. Never use binding mode or
      runtime dynamic class as a tiebreaker.
- [ ] After selection, check and lower the arguments exactly once against the
      selected parameters and record one initializer identity in HIR.
- [ ] Apply selection to every ordinary inline construction consumer,
      including locals, direct class fields, temporaries, value arguments,
      results, and the existing permitted elision destinations.
- [ ] Report deterministic no-match and ambiguity diagnostics with supplied
      static types, candidate signatures, declaration spans, and focused
      per-argument reasons where useful.
- [ ] Extend resolved/HIR/MIR dumps and verifier coverage for candidate
      ownership and selected identity without preserving unresolved overloads
      below type checking.

**Tests:** Exact/arity/type, primitive/class/interface/`Obj`, subtype,
unrelated-interface ambiguity, alias access, value-copy capability,
mode-only-rejection, source-once, elision, diagnostic-span, dump, verifier,
backend-symbol, and native overload tests, followed by `make check` and
`make msrv-check`.

**Exit criteria:** Every `T(arguments)` selects one statically determined
ordinary initializer or produces one complete deterministic diagnostic, and
HIR/lower phases contain no unresolved overload choice.

### CM4 — Select overloaded direct-base initialization

**Purpose:** Reuse the established overload engine for inheritance without
duplicating call policy or weakening incomplete-object rules.

- [ ] Resolve `super(arguments)` to the direct base and source-ordered
      arguments without selecting an initializer before their static types are
      known.
- [ ] Use the same applicability, mode-only, specificity, no-match, ambiguity,
      and selected-argument lowering machinery as direct construction.
- [ ] Record the selected base `InitializerId` in HIR and preserve the existing
      base-first lifecycle, incomplete-`self`, temporary, and statement-
      boundary cleanup rules.
- [ ] Check every derived ordinary initializer independently; overloads in the
      derived class neither inherit nor combine with the base overload set.
- [ ] Preserve the mandatory first-statement `super(...)`, no implicit
      zero-argument base call, no copy-body `super`, and no initializer
      delegation rules.
- [ ] Add deterministic candidate diagnostics and exact resolved/HIR/MIR dumps
      for base selection.
- [ ] Cover multiple derived overloads selecting different base overloads,
      deep inheritance, aliases, slicing/value arguments, ambiguous
      interfaces, source order, and native effects.

**Tests:** Focused resolution/type-check/inheritance/lifecycle suites,
diagnostic and phase dumps, MIR verification, backend/native constructor
chains, and goldens, followed by `make check`.

**Exit criteria:** Every valid `super(arguments)` records one overload-selected
direct-base initializer and preserves the established complete-object
construction order.

### CM5 — Execute explicit target-directed copy construction

**Purpose:** Give copy construction one unambiguous expression form that shared
allocation can later compose without overloading ordinary initialization.

- [ ] Parse contextual `copy` immediately after a construction argument list's
      opening parenthesis as a one-source construction mode, preserving exact
      spans, precedence, nesting, and recovery. `T(copy)` and
      `T(copy, other)` remain ordinary arguments that happen to name a binding.
- [ ] Resolve `T(copy source)` only when `T` names a concrete class and retain
      explicit copy mode rather than fabricating an ordinary argument.
      Functions, methods, interfaces, and `Obj` cannot consume the marker.
- [ ] Type-check the source once through the canonical object-view relation:
      statically select guaranteed exact/ancestor targets, emit a runtime check
      for dynamically possible forwarded sources, and reject statically
      impossible sources.
- [ ] Reuse checked-place access, provenance, slicing, produced-temporary, and
      copy-capability machinery. Keep an explicit inner cast as an optional
      additional refinement rather than requiring a matching cast.
- [ ] Record one selected copy constructor and checked source in HIR; lower
      its success/failure edges, source lifetime, destination construction,
      and cleanup through existing verified MIR operations.
- [ ] Keep implicit owning-copy contexts unchanged. Make `T(arguments)`
      ordinary-only, make `T(copy source)` copy-only, and reject fallback in
      either direction.
- [ ] Evaluate the source exactly once, keep it live until the exact `T`
      destination is complete, preserve deliberate slicing, and prohibit
      copy elision and dynamic-type-preserving cloning.
- [ ] Expose one construction-mode representation that the shared roadmap can
      reuse for `new T(arguments)` and `new T(copy source)` without source-
      shape inspection.
- [ ] Add same/up/down/cross, inline/produced/alias, read-only/mutable,
      inner-cast refinement, slicing, unavailable-copy, static-impossibility,
      runtime-failure, source-once, cleanup-order, parser-ambiguity, dump,
      verifier, assembly, and native tests.
- [ ] Update implemented grammar, lifecycle, casts, phases, status, debugging,
      and testing documentation for the executable explicit-copy form.

**Tests:** Focused syntax through backend tests, parser robustness,
checked-view and copy suites, verifier mutations, assembler acceptance, native
success/failure goldens, `make check`, `make msrv-check`, and
`make robustness-long`.

**Exit criteria:** `T(copy source)` invokes exact-`T` copy construction once
from one target-directed checked source, ordinary construction never selects
copy implicitly, and the reusable construction mode is ready for `new`.

### CM6 — Harden and publish the constructor model

**Purpose:** Audit the complete constructor matrix, remove migration
scaffolding, and leave a maintainable prerequisite for shared ownership.

- [ ] Exercise every ordinary parameter binding mode and type relation across
      locals, fields, value arguments, results, temporaries, elision,
      inheritance, and direct-base construction.
- [ ] Exercise implicit and explicit copy construction across existing places,
      produced objects, aliases, checked casts, slicing, inheritance, user and
      synthesized capabilities, and normal cleanup.
- [ ] Add malformed public-MIR and backend legality coverage for initializer
      density, declaration/definition mismatch, selected target/signature,
      copy identity, checked source lifetime, and call target existence.
- [ ] Audit constructor, call-argument, class-definition, copy, dump, verifier,
      and backend-symbol modules by responsibility. Resolve high-priority
      cohesion problems; record larger lower-priority findings in an indexed
      constructor discoveries document rather than expanding this task.
- [ ] Remove legacy signature classification, singular initializer fields,
      temporary semantic gates, stale source spellings, and roadmap codes from
      living code, tests, diagnostics, and general documentation.
- [ ] Make implemented grammar, status, language overview, lifecycle, aliases,
      casts, polymorphism, phases, backend, debugging, and testing documents
      concise and current, with one authority for each rule.
- [ ] Confirm exclusions remain rejected: mode-only overloads, zero ordinary
      initializers, initializer delegation, method/function overloading,
      default/variadic arguments, implicit downcasts, runtime overload
      dispatch, shared/new execution, and dynamic cloning.
- [ ] Run the complete repository, supported-toolchain, long robustness,
      documentation, deterministic-process, assembler, native, and diff-
      hygiene gates; then archive this roadmap and unblock shared ownership.

**Tests:** `make check`, `make msrv-check`, `make robustness-long`,
`git diff --check`, focused stale-vocabulary searches, and the complete
constructor/copy native and compile-failure golden matrix.

**Exit criteria:** The frozen constructor model is implemented and executable
through verified x86-64 lowering, living documentation describes only current
behavior, and the shared-ownership roadmap can begin without reopening
constructor syntax, selection, identity, or copy-source semantics.

## Ordering and dependencies

CM0 first separates lifecycle identities so copy construction cannot collide
with an expanding initializer set. CM1 removes singular storage behind the
current source boundary. CM2 then makes copy intent explicit before ordinary
`init` becomes overloadable. CM3 establishes one direct-construction overload
engine, and CM4 reuses it for base initialization without expanding the first
selection slice.

CM5 composes the already implemented checked-place and copy pipelines only
after ordinary construction can no longer fall back to copy by signature.
CM6 broadens coverage and removes transition scaffolding after every semantic
operation is executable.

The implemented class lifecycle, polymorphism, object-place cast, and
cast-relative receiver work are prerequisites and are complete. This roadmap
is itself a blocking prerequisite for
[Shared Ownership and Heap Allocation](SHARED_OWNERSHIP_ROADMAP.md); that
roadmap must not begin until CM6 is complete. Shared storage, `new`, owner
lifetimes, and hidden anchors remain in the dependent roadmap.
