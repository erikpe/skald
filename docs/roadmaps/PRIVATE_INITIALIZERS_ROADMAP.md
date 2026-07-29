# Private Ordinary Initializers Roadmap

Status: in progress; PRI1 is next.

This roadmap adds per-overload declaring-class privacy to ordinary `init`
declarations without turning lifecycle declarations into ordinary named
members or changing construction, copying, ownership, or target execution.
It extends the implemented private-member rule to the initializer-selection
phase that already owns static overload choice, then uses the result to give
classes a safe source-level factory boundary.

In this roadmap, “private constructor” means a private ordinary initializer.
The distinct `copy` constructor and the `assign` and `destroy` lifecycle slots
remain unchanged.

## Scope and invariants

- Ordinary initializers remain public by default and may instead use the exact
  source form `private init(parameters) { ... }`.
- `private` remains contextual. An explicit `public` class-member modifier is
  not added, and `private` remains a valid identifier outside its established
  modifier position.
- Visibility belongs to each initializer overload, not to the class or the
  complete overload set.
- A selected private initializer is accessible exactly from a callable
  lexically owned by its declaring `ClassId`. Receiver spelling, module,
  import, inheritance, and same-package relationships grant no access.
- Instance methods, static methods, ordinary initializers, copy construction,
  copy assignment, and destruction bodies of the exact declaring class are
  authorized caller contexts. Top-level functions and bodies owned by any
  other class are not.
- A derived initializer cannot call a private base initializer through
  `super(...)`. A base with no accessible initializer therefore cannot be
  initialized by that derived class; protected construction is a separate
  future feature.
- Direct `T(arguments)`, shared `new T(arguments)`, direct-base
  `super(arguments)`, and class-element default-length array construction all
  enforce the same rule.
- Overload applicability and unique-most-specific selection consider every
  initializer before access is checked. Selecting an inaccessible private
  overload is an access error and never falls back to a less-specific public
  overload.
- Existing no-match and ambiguity outcomes retain precedence when overload
  selection does not produce one initializer. Privacy is diagnosed only for a
  uniquely selected initializer.
- Default element capability remains a stable type plan. A plan containing a
  private zero-argument class initializer is usable only at a source call site
  authorized for that initializer. Empty and explicit-copy arrays do not
  inspect ordinary initializer visibility.
- Resolution retains visibility and its source span through overload
  selection. HIR and all lower phases contain only authorized
  `InitializerId`s and deliberately erase visibility.
- Initializer identity allocation, parameter compatibility, argument
  evaluation order, definite field initialization, receiver presence,
  ownership, cleanup, construction elision, MIR, symbols, internal ABI,
  runtime ABI, and target layout remain unchanged.
- Private initializers are not inherited, do not enter the ordinary
  field/method namespace, and do not affect virtual families or interface
  conformance.
- Private copy constructors, copy assignments, destructors, protected or
  package constructor access, friend access, delegating initializers, static
  initializers, implicit constructors, static fields, and reflection are
  explicit non-goals.
- Diagnostics, phase dumps, source-order initializer ordinals, module-provider
  behavior, assembly, and native observations remain deterministic.

## Progress

- [x] PRI0 — Represent and enforce initializer privacy without source exposure
- [ ] PRI1 — Expose private ordinary initializers through the source language
- [ ] PRI2 — Adopt, harden, document, and close private initializer support

## PR-sized implementation sequence

### PRI0 — Represent and enforce initializer privacy without source exposure

**Purpose:** Establish the complete internal metadata and authorization
boundary before the parser accepts a source form whose privacy must hold at
every construction site.

- [x] Add public-by-default member visibility to syntax and resolved ordinary
      initializer declarations, preserving modifier, introducer, signature,
      and complete declaration spans. Keep `private init` rejected by the
      parser during this task.
- [x] Preserve visibility in deterministic syntax and resolved dumps and
      through stable source-ordered `InitializerId` allocation. Do not carry
      it into HIR, MIR, or backend declarations.
- [x] Centralize selected-initializer authorization beside ordinary overload
      selection. Compare the initializer's declaring class with
      `CallableChecker::class_owner`; do not duplicate module or receiver
      access rules.
- [x] Apply authorization after unique-most-specific selection for direct
      inline construction, shared allocation, and direct-base initialization.
      Preserve no-match, ambiguity, and argument-analysis behavior.
- [x] Authorize default-length inline and shared arrays at the source call
      site when their selected element plan names a private zero-argument
      initializer. Reuse the same checker for exact class and `shared` class
      elements without making the global array capability table
      caller-dependent.
- [x] Add one stable type-check diagnostic for an inaccessible selected
      initializer, with the construction site as the primary label, the
      private modifier as a secondary label, and the exact declaring-class
      rule as guidance.
- [x] Provide narrow test-only fixture support for changing initializer
      visibility after ordinary parsing so every authorization path can be
      tested before source exposure.
- [x] Confirm that authorized HIR is byte-for-byte visibility-free and lower
      phases, runtime headers, ABI versions, and generated symbols are
      unchanged.

**Tests:** Syntax/resolved metadata and exact-dump tests; type-check unit tests
using private-initializer fixtures for same-class and foreign caller contexts,
mixed-overload selection, direct construction, `new`, `super`, inline and
shared default-length arrays, no-match and ambiguity precedence, and HIR
privacy erasure; public API regressions; then `make check`, `make msrv-check`,
and `git diff --check`.

**Exit criteria:** Internal phase data can represent public or private ordinary
initializer overloads, every existing initializer consumer enforces one
central selected-identity access rule, no accepted source syntax has changed,
and lower phases remain visibility-independent.

### PRI1 — Expose private ordinary initializers through the source language

**Purpose:** Accept the complete source feature only after its internal access
boundary is already exhaustive and verified.

- [ ] Accept `private init(parameters) { ... }` while keeping `init` public by
      default and `private` contextual. Include the modifier in the complete
      declaration span.
- [ ] Preserve focused recovery for duplicate or misplaced `private`, and
      reject combinations with `static`, `mut`, `virtual`, or `override`
      without losing later class members.
- [ ] Continue rejecting `private copy`, `private assign`, and
      `private destroy` with diagnostics that distinguish those deferred
      lifecycle-visibility questions from supported private ordinary
      initializers.
- [ ] Exercise access from every exact-class body category, including
      receiverless static factories, and reject top-level, same-module,
      cross-module, unrelated-class, and derived `super(...)` callers.
- [ ] Prove per-overload behavior: a selected public overload succeeds, a
      selected private overload outside its owner fails without public
      fallback, and mixed public/private ambiguity and no-match diagnostics
      retain their defined precedence.
- [ ] Exercise direct construction, field construction, local initialization,
      produced results, shared allocation, and both inline and shared
      class-element default-length arrays from source.
- [ ] Update the implemented grammar, classes/lifecycle contract, array
      contract, compiler phase contract, language overview, and status matrix.
      Keep one authoritative exact-declaring-class rule and link to it rather
      than duplicating variants.
- [ ] Update string language/compiler documentation to remove the obsolete
      statement that lifecycle declarations cannot be private, without yet
      changing the canonical standard-library implementation.

**Tests:** Parser modifier/contextual-name/span/dump/recovery tests; resolved
visibility and deterministic identity/dump tests; type-check tests for every
caller body, overload outcome, construction form, module boundary, inheritance
case, and array element ownership form; exact compile-failure diagnostics and
successful native factory goldens; cross-process diagnostic/HIR determinism;
then `make check`, `make msrv-check`, `make robustness-long`, and
`git diff --check`.

**Exit criteria:** `private init` is accepted from source, each uniquely
selected private overload is callable only inside its exact declaring class
across every construction form, diagnostics and dumps are deterministic, and
living language/compiler documentation describes the implemented contract.

### PRI2 — Adopt, harden, document, and close private initializer support

**Purpose:** Prove the feature in its motivating standard-library boundary,
complete adversarial coverage, and remove rollout-only wording before
archival.

- [ ] Replace the canonical string library's trusted fresh-storage helper with
      a private ordinary initializer where that reduces indirection, while
      retaining its public empty initializer, exact descriptor representation,
      logical immutability, public API, literal semantics, and synthesized
      lifecycle.
- [ ] Prove that string literals still use compiler-owned intrinsic
      construction rather than calling any public or private initializer, and
      that ordinary dynamic string factories can call the private initializer
      only through exact-class lexical ownership.
- [ ] Complete malformed and adversarial matrices for visibility metadata,
      selected initializer identity, array default plans, module/provider
      permutations, and independently mutated resolved/HIR products.
- [ ] Add independent-process determinism coverage for mixed-visibility
      overload selection, rendered privacy diagnostics, HIR, assembly, and the
      canonical string module.
- [ ] Audit the parser, resolved declarations/dumps, overload selector, array
      capability consumer, standard library, and documentation owners by
      responsibility. Resolve small high-value maintainability issues directly
      and index larger follow-ups in a separate discoveries document.
- [ ] Update testing/debugging guidance and maintained examples for private
      factories, overload-selection inspection, inaccessible `super(...)`,
      and array-default authorization.
- [ ] Remove stale “lifecycle visibility unsupported” language from living
      code and documentation while preserving the explicit exclusions for
      private copy construction, assignment, and destruction.
- [ ] Confirm runtime ABI/public header stability and repository artifact
      cleanliness.
- [ ] Complete and archive this roadmap after all focused and repository gates
      pass.

**Tests:** Focused standard-library string, syntax, resolution, type-check,
array, HIR, MIR, backend, documentation checker, public API, determinism, and
golden suites; sample compilation; artifact-free `make check`,
`make msrv-check`, `make robustness-long`, `make docs-check`, and
`git diff --check`.

**Exit criteria:** Private ordinary initializers are a maintained,
deterministic source-to-native contract used by the canonical library without
changing lower-phase execution or runtime ABI, all living documentation is
current, and this roadmap is ready to archive.

## Ordering and dependencies

PRI0 keeps the new spelling unavailable while visibility metadata, overload
authorization, and the non-obvious array-default path become complete. PRI1
then exposes one source form on top of that already verified boundary and
updates the normative language/compiler contract in the same review. PRI2
uses the feature in `std::str::Str`, closes broad determinism and malformed
matrices, and performs promotion and archival only after native behavior is
observable.

The roadmap depends on implemented declaring-class privacy, lexical class
ownership for receiver-bearing and receiverless callables, ordinary
initializer overload selection, direct-base initialization, shared
allocation, class-element array default plans, the canonical string module,
and deterministic phase products. It does not depend on static fields,
exceptions, protected access, or changes to copy capability.
