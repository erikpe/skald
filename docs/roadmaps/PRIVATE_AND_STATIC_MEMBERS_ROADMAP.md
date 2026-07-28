# Private Members and Static Methods Roadmap

Status: in progress; PSM1 is next.

This roadmap adds declaring-class privacy and receiverless class-owned methods
without creating a second callable, member-lookup, ownership, or backend
pipeline. It leaves fields and methods public by default, composes `private`
and `static` into private static methods, and preserves exact typed identities
through the existing forward compiler phases. The completed result removes the
last class-member prerequisite from the
[string types design proposal](STRINGS_DESIGN_PROPOSAL.md).

## Current baseline

- Fields and instance methods share one non-overloaded namespace across each
  complete base chain. Resolution selects the nearest declaration and retains
  its exact `FieldId` or `MethodId` plus declaring `ClassId`.
- Every accepted field and method is source-accessible wherever its receiver
  is available. Member visibility metadata and member-access diagnostics do
  not exist.
- Every `MethodId` identifies an instance method with a read-only or mutable
  receiver. Class-owned callable identity, lexical class ownership, `self`
  availability, MIR receiver storage, and receiver-aware ABI classification
  are coupled.
- Direct, virtual, and interface method calls carry explicit receiver
  selections. Top-level direct calls have no receiver. There is no explicit
  receiverless class-owned call target.
- `private` and `static` remain ordinary identifier spellings rather than
  recognized class-member modifiers. Source-visible static fields, globals,
  module initialization, and shutdown behavior are not implemented.

## Scope and invariants

- Fields and methods remain public by default. `private` is the only new
  member-visibility spelling; `public` remains a top-level declaration
  modifier.
- `private` and `static` are contextual in their exact class-member forms and
  remain legal field, method, parameter, local, and top-level names elsewhere.
- A private field or method is source-accessible only from a callable
  lexically owned by its exact declaring `ClassId`. Same-module declarations,
  derived classes, and importers receive no access.
- Initializers, copy constructors, copy assignment, destructors, instance
  methods, and static methods all count as bodies of their declaring class for
  private access. Top-level functions and members of every other class do not.
- Visibility affects source selection only. Private fields keep ordinary
  layout, containment, synthesized lifecycle, ownership, and destruction
  behavior, and compiler-owned intrinsic construction may inspect or
  materialize them without creating a source-visible escape.
- A private instance method is direct and may be read-only or mutable. It
  cannot be `virtual` or `override`, and it cannot satisfy an interface
  requirement.
- A static method has one class-owned `MethodId`, no `self`, no receiver access
  mode, no virtual dispatch, and no interface-conformance role. It uses the
  existing parameter, result, evaluation-order, ownership, cleanup, and
  internal ABI rules.
- Static calls use `Class.method(arguments)` or
  `module_binding::Class.method(arguments)`. The selected class spelling is not
  evaluated; explicit arguments evaluate left to right. An unqualified local
  binding continues to shadow an unqualified class spelling.
- Static methods participate in the existing single inherited ordinary-member
  namespace. Selection through a derived class may retain a base-owned
  `MethodId`; privacy does not enable field hiding, method hiding, implicit
  overriding, or inherited-name reuse.
- Calling a static method through an object and calling an instance method
  through a class are deterministic source errors. Static methods are not
  first-class values.
- `private static fn` is the ordinary composition of member visibility and
  method kind. It is not a separate identity, declaration table, call
  convention, or backend symbol family.
- Resolution remains the sole owner of name selection and privacy decisions.
  HIR and lower phases carry selected typed identities and method/call kinds
  without repeating source access checks or comparing names.
- MIR explicitly represents and verifies receiver presence. A class-owned
  callable identity does not by itself imply a receiver, while every
  receiver-bearing callable still has exactly one valid receiver storage slot.
- Static lowering reuses existing method symbols and the receiverless internal
  call layout. No public C runtime symbol, runtime ABI version, allocation
  operation, or target data layout is added.
- Source-visible static fields, static properties, globals, module
  initialization and destruction, lifecycle-member visibility, protected or
  package visibility, friend access, method overloads, abstract/final members,
  reflection, method values, and external static methods are explicit
  non-goals.
- `static name: T` and `private static name: T` receive focused unsupported
  static-field diagnostics; this roadmap introduces no placeholder static
  storage identity or initialization semantics.
- Phase dumps, diagnostics, identity allocation, inherited selection,
  interface analysis, assembly symbols, and native observations remain
  deterministic.

## Progress

- [x] PSM0 — Separate lexical class ownership from receiver presence
- [ ] PSM1 — Implement declaring-class member privacy
- [ ] PSM2 — Establish receiverless static-method IR and execution
- [ ] PSM3 — Expose static methods through the complete source pipeline
- [ ] PSM4 — Confirm and promote the unblocked string design

## PR-sized implementation sequence

### PSM0 — Separate lexical class ownership from receiver presence

**Purpose:** Remove the internal assumption that every class-owned callable has
a receiver before static declarations or calls depend on a receiverless member
model.

- [x] Split callable context into lexical class ownership and optional receiver
      availability. Preserve `self`, base-initialization, lifecycle, and
      receiver-access behavior for every currently accepted member.
- [x] Make member-body checking accept explicit optional receiver context while
      retaining initializer completeness, missing-return, local ownership, and
      cleanup behavior.
- [x] Change MIR member definitions and shared definition views to record an
      optional receiver storage slot. Keep every existing initializer, copy
      operation, destructor, and instance method receiver-bearing.
- [x] Make body lowering receive explicit optional receiver class information
      instead of deriving receiver existence from `CallableId::class()`.
- [x] Update MIR verification so current receiver-bearing declarations still
      require exactly one correctly owned `MirStorageKind::Receiver` slot, and
      receiverless definitions reject any such slot.
- [x] Derive frame layout, incoming parameter spilling, outgoing ABI
      classification, receiver-origin homes, legality checks, and cleanup rules
      from verified receiver presence rather than callable category.
- [x] Update test-only MIR and backend fixtures through cohesive constructors
      so later static-method tests do not duplicate raw optional-receiver
      assembly.
- [x] Keep public phase facades narrow and explicit; extract a focused private
      owner only where optional-receiver branching would otherwise spread
      substantial implementation logic across a facade.
- [x] Update [compiler phases and IR](../compiler/PHASES_AND_IR.md) and
      [backend](../compiler/BACKEND.md) only for the durable explicit-receiver
      representation. Do not claim static source support.

**Tests:** Focused callable-context, member-body, MIR lowering, receiver
mutation/verifier, frame, ABI, legality, cleanup, dump, and native regression
tests, followed by `make check`, `make msrv-check`, and `git diff --check`.

**Exit criteria:** Class ownership and receiver presence are independent
explicit facts from body resolution through target lowering, every current
program produces byte-for-byte-equivalent observable behavior, and no accepted
source form is added.

### PSM1 — Implement declaring-class member privacy

**Purpose:** Add the complete source access boundary needed by strings while
keeping privacy out of layout, lifecycle, and lower-phase execution.

- [ ] Add source-shaped member visibility to fields and instance methods with
      exact modifier and declaration spans. Keep `private` contextual, fields
      and methods public by default, and lifecycle declarations unmodified.
- [ ] Accept the canonical instance forms `private fn` and
      `private mut fn`. Reject private virtual/override declarations,
      duplicate or misplaced visibility, private lifecycle declarations, and
      malformed forms with focused recovery to later members.
- [ ] Preserve member visibility on resolved field and method declarations and
      in deterministic syntax/resolved dumps. Do not carry privacy into HIR or
      MIR after access has been decided.
- [ ] Centralize member-access validation beside ordinary member selection.
      Compare the selected member's declaring `ClassId` with the current
      callable's lexical class owner, never with module, name, receiver, base,
      or dynamic-class spellings.
- [ ] Apply the centralized check to field reads and writes, nested object
      places, method calls, aliases, casts and checked views, shared pointee
      access, initializer and lifecycle bodies, and every other existing
      member consumer.
- [ ] Preserve the complete inherited ordinary-member namespace and collision
      rules. Private inherited declarations remain selected identities that
      produce privacy diagnostics rather than enabling hiding or
      redeclaration.
- [ ] Exclude private methods from interface conformance and virtual-family
      formation with declaration-linked diagnostics. Preserve exact signature
      checking for eligible public instance methods.
- [ ] Prove that synthesized copy construction, copy assignment, destruction,
      containment, target layout, and compiler-owned metadata inspection treat
      private fields exactly like public stored fields.
- [ ] Add one stable resolver diagnostic code for inaccessible private members
      and define deterministic precedence against unknown member, wrong member
      kind, receiver access, and type errors.
- [ ] Update the implemented
      [grammar](../language/GRAMMAR.md),
      [classes and lifecycle](../language/CLASSES_AND_LIFECYCLE.md),
      [polymorphism](../language/POLYMORPHISM.md),
      [modules and interoperation](../language/MODULES_AND_INTEROP.md),
      [compiler phases and IR](../compiler/PHASES_AND_IR.md), and
      [status matrix](../language/STATUS.md) when privacy is complete. Keep one
      authoritative declaring-class access rule and link to it rather than
      duplicating it.

**Tests:** Parser modifier/contextual-name/span/dump/recovery tests; resolver
tests for every field and method consumer; same-class lifecycle/static-ready
context, other-class, same-module, derived-class, inherited-collision, and
cross-module diagnostics; interface and virtual-family tests; HIR privacy
erasure and lifecycle/layout regressions; compile-fail goldens; then
`make check`, `make msrv-check`, `make robustness-long`, and
`git diff --check`.

**Exit criteria:** All supported source access to a private field or instance
method succeeds exactly inside its declaring class and fails everywhere else,
lower phases receive only already-authorized identities, and living language
and compiler documentation describes the implemented privacy contract.

### PSM2 — Establish receiverless static-method IR and execution

**Purpose:** Make receiverless class-owned declarations, bodies, calls, and ABI
behavior explicit and verifiable before the parser and resolver expose them to
ordinary source.

- [ ] Replace independent method booleans or mandatory receiver fields with
      phase-appropriate method-kind enums that make instance and static
      metadata mutually exclusive. Existing source methods initially lower as
      instance methods.
- [ ] Add explicit receiverless static call forms to HIR object/scalar
      producers and MIR call targets while retaining the selected class-owned
      `MethodId`.
- [ ] Teach member-body checking and lowering to process a static method body
      with lexical class ownership but no receiver context, `self` binding,
      receiver storage, receiver access, base initialization, or receiver
      cleanup.
- [ ] Extend class-result destinations, primitive/unit results, shared and
      optional-shared results, inline optionals, arrays, alias/value
      arguments, temporaries, ownership transfer, full-expression cleanup, and
      control-effect classification with receiverless static-call variants.
- [ ] Lower static calls to `MirCallTarget::Static(MethodId)` with no
      `MirCallReceiver`, preserving left-to-right explicit argument evaluation
      and all existing destination/result conventions.
- [ ] Verify declaration/definition kind agreement, receiver absence, callable
      and parameter ownership, result storage, static call target kind,
      argument modes and types, and exclusion from virtual families and
      interface conformance maps.
- [ ] Reuse the existing collision-proof class method symbol for static
      methods. Select it as a direct `CallableId::Method` call using the
      receiverless internal call layout.
- [ ] Generalize backend member-target legality to derive receiver
      classification from the declared method kind rather than assuming every
      `MethodId` has a receiver.
- [ ] Extend resolved/HIR/MIR/backend dumps and public phase facade re-exports
      with explicit, deterministic static kinds and targets.
- [ ] Update [compiler phases and IR](../compiler/PHASES_AND_IR.md) and
      [backend](../compiler/BACKEND.md) for receiverless class callables and
      their verifier/ABI invariants without yet claiming accepted static source
      syntax.

**Tests:** Test-only resolved/HIR/MIR fixtures for public and private-ready
static declarations, primitive/unit/class/shared/optional/array parameters and
results, receiver absence, ownership and cleanup, every malformed
declaration/call/receiver pairing, ABI register/stack boundaries, symbols,
dumps, assembly, and native execution, followed by `make check`,
`make msrv-check`, and `git diff --check`.

**Exit criteria:** Internally constructed typed programs can define, verify,
emit, and execute receiverless class-owned methods across every supported
parameter and result category, while ordinary source still has no static
method spelling.

### PSM3 — Expose static methods through the complete source pipeline

**Purpose:** Add the public static and private static source contract on top of
the already verified receiverless callable model.

- [ ] Accept `static fn` and `private static fn` in that exact order with
      complete modifier, name, signature, body, and declaration spans. Keep
      `static` contextual and preserve `static` and `private` as ordinary names
      outside modifier position.
- [ ] Reject `static mut fn`, virtual/override static combinations, repeated or
      misplaced modifiers, static lifecycle members, and source-visible
      `static name: T` or `private static name: T` with deterministic recovery.
      Static-field diagnostics must state that static fields are a separate
      future feature.
- [ ] Collect static methods in the existing class-owned `MethodId` sequence
      and ordinary member namespace. Preserve source/member ordering,
      inherited lookup, declaring-class identity, collision diagnostics, and
      non-overloading.
- [ ] Resolve `Class.method(arguments)` and
      `module_binding::Class.method(arguments)` before object-receiver
      selection when the class spelling is not shadowed by a local binding.
      Reuse ordinary module visibility and qualified-declaration lookup.
- [ ] Produce focused diagnostics for unknown class members, class-selected
      instance methods, object-selected static methods, fields used as static
      callables, static methods used without a call, and inaccessible private
      static methods.
- [ ] Give static bodies their enclosing `ClassId` for private access while
      rejecting `self`. Permit private field/method access through explicit
      object values and private static helper calls only under the same exact
      declaring-class rule.
- [ ] Type-check static call parameters and results through the shared call and
      ownership machinery. Do not add static-specific conversions, overload
      selection, elision, lifetime, cleanup, or failure rules.
- [ ] Exclude static methods from virtual roots, overrides, interface
      implementations, receiver-access checks, dispatch metadata, and
      receiver-before-argument evaluation.
- [ ] Exercise inherited public and private static selection, cross-module
      public class access, selective imports and aliases, unqualified local
      shadowing, recursive static helpers, and private static factories.
- [ ] Update the implemented
      [grammar](../language/GRAMMAR.md),
      [classes and lifecycle](../language/CLASSES_AND_LIFECYCLE.md),
      [functions and control flow](../language/FUNCTIONS_AND_CONTROL_FLOW.md),
      [polymorphism](../language/POLYMORPHISM.md),
      [modules and interoperation](../language/MODULES_AND_INTEROP.md),
      [status matrix](../language/STATUS.md),
      [compiler phases and IR](../compiler/PHASES_AND_IR.md), and
      [backend](../compiler/BACKEND.md) in the same task. Document private
      static methods as composition, not a separate capability.
- [ ] Update the repository overview only as needed to keep its concise current
      feature summary accurate. Do not add runtime ABI or static-storage
      documentation because neither contract changes.

**Tests:** Parser and recovery coverage for every valid/invalid modifier and
contextual-name form; resolution/type-check tests for selection, privacy,
inheritance, interfaces, shadowing, qualified modules, diagnostics, dumps, and
all supported parameter/result ownership categories; MIR verification
regressions; target-specific ABI/assembly tests; complete compile-fail and
native goldens; public API and determinism tests; then `make check`,
`make msrv-check`, `make robustness-long`, and `git diff --check`.

**Exit criteria:** Public and private static methods compile from source
through native execution with no receiver, private static methods obey the one
declaring-class rule, static fields remain rejected and unrepresented, and all
living documentation describes the complete implemented contract.

### PSM4 — Confirm and promote the unblocked string design

**Purpose:** Complete the string proposal's required confirmation pass now
that its member prerequisites have stable implemented contracts, without
implementing string literals or string execution.

- [ ] Re-read the implemented modifier grammar, privacy boundary, static-call
      model, module visibility, ownership rules, and the complete
      [string proposal](STRINGS_DESIGN_PROPOSAL.md); record any discrepancy as
      a blocking design correction rather than silently changing the string
      representation.
- [ ] Replace illustrative string declarations with the exact implemented
      private-field, ordinary-method, static-method, and private-static-method
      spellings.
- [ ] Confirm that compiler language-item validation and future intrinsic
      descriptor materialization may inspect private field metadata without
      granting ordinary source access.
- [ ] Confirm that ordinary public static factories and private instance/static
      helpers can express all proposed dynamic string construction paths
      without compiler-selected method names.
- [ ] Promote source-visible string rules into a focused
      `docs/language/STRINGS.md` contract and phase, immortality, verification,
      layout, compiler/standard-library boundary, and runtime rules into
      `docs/compiler/STRINGS.md` plus the existing shared-ownership authorities
      where applicable.
- [ ] Update the documentation index, language overview, compiler overview,
      status matrix, and cross-links so one authoritative location owns each
      promoted fact. Mark strings as frozen design, not implemented behavior.
- [ ] Move the completed proposal to `docs/archive/`, add it to the archive
      index, remove its active design-proposal entry, and repair every incoming
      relative link without rewriting its historical content.
- [ ] Create and index a separate PR-sized string implementation roadmap that
      begins with language-item discovery and literal syntax; do not expand
      this roadmap into string implementation.
- [ ] Audit touched compiler modules by responsibility, preserve concise
      facades and cohesive tests, remove rollout vocabulary from living
      documentation, and place any unrelated actionable discovery in a
      separately indexed roadmap document.

**Tests:** `make docs-check`, focused documentation-checker tests, repository
link inspection, then an artifact-free `make check`, `make msrv-check`,
`make robustness-long`, and `git diff --check`.

**Exit criteria:** The implemented member features have no stale or
contradictory documentation, the string model is frozen in living language and
compiler authorities, its historical proposal is archived, a separate string
implementation roadmap is the indexed next action, and this roadmap is ready
to close and archive.

## Ordering and dependencies

PSM0 is behavior-preserving groundwork: later work must not encode static
methods by faking an instance receiver or by branching on class-owned callable
identity throughout the backend. PSM1 follows because privacy is
target-independent and can become a complete source feature before static
methods exist. Its exact `ClassId` access boundary is then reused unchanged by
private static methods.

PSM2 establishes one explicit receiverless declaration/call/verifier/ABI path
behind existing phase facades before source syntax can produce it. PSM3 then
adds source selection and complete semantic coverage without simultaneously
inventing lower representations. Privacy and receiverless static execution are
otherwise independent after PSM0, but keeping PSM1 before PSM3 ensures the
composed `private static fn` form is complete when static methods become
source-visible.

PSM4 depends on all implementation tasks and is deliberately documentation-
focused. It clears the existing string freeze gate and hands string execution
to a separate roadmap. Static fields remain independent future work
throughout; no task in this sequence may add their identities, storage,
initialization, destruction, or concurrency semantics.
