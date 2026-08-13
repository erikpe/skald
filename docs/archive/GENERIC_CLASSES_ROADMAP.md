# Generic Classes Roadmap

Status: complete.

Implement the frozen [generic-class language contract](../language/GENERIC_CLASSES.md)
and [compiler specialization contract](../compiler/GENERIC_CLASSES.md). The
finished compiler will accept explicit closed generic class applications,
infer contextual storage and lifecycle requirements, enforce explicit nominal
interface bounds, specialize each unique application into an ordinary exact
class, and execute the result through existing typed HIR, verified MIR, and
x86-64 lowering without a runtime generic ABI.

The archived [design record](GENERIC_CLASSES_DESIGN_PROPOSAL.md)
preserves the confirmed decisions. This roadmap owns delivery order and must
not reopen those decisions while implementing them.

## Scope and invariants

- Generic class declarations use `class C<T, U>` and explicit applications use
  `C<A, B>`; arguments are never inferred or omitted.
- Class-level `where T: Interface` is the sole initial written constraint.
- Mechanical storage, optional, array, shared-target, alias, default, copy,
  assignment, and destruction requirements are inferred from actual template
  uses.
- A template is not an executable class. `ClassTemplateId` and
  `TypeParameterId` remain distinct from `ClassId`.
- Every accepted closed key receives one deterministic ordinary `ClassId` and
  reuses existing canonical array, optional, optional-box, shared-target, and
  class type identities.
- Substitution is structural and literal. It never reparses generated source,
  flattens optionals, or moves optionality across a shared owner.
- Every specialization validates the complete class. Members are not lazily
  removed or deferred until call sites.
- Generic applications are invariant.
- Definition-site lookup, module visibility, and declaring-class access remain
  authoritative; application-site lookup resolves only the argument spellings.
- Identical specialization recursion may close a graph; transformed recursive
  re-entry is rejected deterministically before it can produce an infinite
  family.
- Ordinary resolved classes, HIR, MIR, verification, and backends never carry
  an unresolved type parameter or runtime generic dictionary.
- Generated statics, hierarchy, conformance, lifecycle, dispatch, layout, and
  symbols behave as those of equivalent hand-written exact classes.
- The feature adds no public C runtime surface and does not change runtime ABI
  version 9.
- Generic functions, independent generic methods or constructors, generic
  interfaces, member-level bounds, inference, defaults, variance, partial
  specialization, parameter construction, erasure, reflection, and separate-
  compilation specialization ownership remain out of scope.
- New Rust implementation belongs behind cohesive facade-oriented modules;
  substantial template, requirement, and specialization algorithms must not
  accumulate in `resolve/resolver/program/resolver.rs` or a monolithic
  `mod.rs`.

## Progress

- [x] G0 — Parse and preserve generic class syntax
- [x] G1 — Establish template identities and module declarations
- [x] G2 — Resolve parameter-bearing type terms and nominal bounds
- [x] G3 — Infer and evaluate contextual generic requirements
- [x] G4 — Build deterministic closed-specialization identities
- [x] G5 — Specialize class declarations into ordinary closed classes
- [x] G6 — Specialize executable bodies and operation selections
- [x] G7 — Integrate lifecycle, optionals, arrays, and ownership
- [x] G8 — Integrate nominal bounds, inheritance, conformance, and dispatch
- [x] G9 — Integrate per-specialization statics and whole-program effects
- [x] G10 — Harden diagnostics, dumps, modules, and determinism
- [x] G11 — Execute generic classes through MIR and x86-64
- [x] G12 — Deliver generic `Vec<T>` and close the feature

## PR-sized implementation sequence

### G0 — Parse and preserve generic class syntax

**Purpose:** Establish the complete frozen source shape, spans, recovery, and
unambiguous angle-bracket parsing before semantic code depends on it.

- [x] Add source-shaped AST nodes for ordered class type parameters, closed
  generic argument lists on named types, and ordered class-level `where`
  requirements.
- [x] Extend class parsing for `class C<T, U>` and the `where T: Interface`
  clause after optional `extends` and `implements` clauses.
- [x] Extend named-type parsing so applications compose with grouping,
  postfix `?`/`[]`, leading `shared`, constructor heads, allocation heads,
  static selection, casts, type tests, bases, fields, and signatures.
- [x] Keep `where` contextual and preserve its existing use as an identifier
  outside the class-header form.
- [x] Parse nested closers such as `Outer<Inner<Str>>` in type context without
  changing expression `>`, `>=`, or `>>` token meaning.
- [x] Diagnose empty lists, missing separators or closers, trailing commas,
  malformed requirements, misplaced `where`, and argument syntax on recovery
  paths without losing later declarations.
- [x] Extend syntax dumps with parameters, arguments, bounds, punctuation
  spans, and grouping provenance.
- [x] Promote the frozen grammar extension into
  [`GRAMMAR.md`](../language/GRAMMAR.md) as accepted syntax while stating that
  semantic specialization remains gated by later tasks.

**Tests:** Focused lexer/parser and syntax-dump tests for every accepted
position; nested-closer tests beside shifts and comparisons; malformed-source
recovery tests; exact syntax goldens for representative positive and negative
forms; `cargo fmt --check`, `cargo test --locked -p skald-compiler syntax`,
`make docs-check`, and `git diff --check`.

**Exit criteria:** The AST preserves every frozen generic source form and exact
span, ordinary expression operators remain unchanged, malformed forms recover
deterministically, and semantic phases reject or gate generic nodes without
panicking or assigning false ordinary class meaning.

### G1 — Establish template identities and module declarations

**Purpose:** Give generic declarations honest stable identities and namespace
behavior before resolving parameter-bearing types or allocating closed class
instances.

- [x] Add `ClassTemplateId` and `TypeParameterId` to the shared identity
  vocabulary with deterministic display and crate-private construction.
- [x] Distinguish ordinary class and generic-template symbols in top-level,
  module-declaration, ordinary-binding, import, qualification, and visibility
  tables.
- [x] Allocate template identities in canonical module/declaration order while
  retaining existing dense `ClassId` ordering for hand-written non-generic
  classes.
- [x] Collect parameter declarations in source order, reject duplicate names,
  and establish their frozen scope across header clauses and all members.
- [x] Implement parameter shadowing of unqualified declarations while
  preserving qualified access to module declarations.
- [x] Diagnose raw generic names, arguments on non-generic declarations,
  generic wrong-kind uses, and exact arity at all initially discoverable type
  sites.
- [x] Preserve public/private import behavior without instantiating a template
  merely because it is imported or qualified.
- [x] Add template and parameter tables behind the `resolve` facade with
  explicit selective re-exports; keep collection and table implementation in
  cohesive private submodules.
- [x] Extend module and declaration dumps with stable template identities and
  parameter lists.

**Tests:** Identity formatting and table tests; single- and multi-module
collection tests; duplicate, shadowing, privacy, selective/qualified import,
wrong-kind, raw-name, and arity diagnostics; declaration-order and module-
permutation determinism; focused `resolve` and module tests plus
`make docs-check` and `git diff --check`.

**Exit criteria:** Every generic declaration has one stable non-executable
template identity, module lookup distinguishes it from an ordinary class, no
generic declaration consumes a placeholder `ClassId`, and namespace behavior
is deterministic across module graphs.

### G2 — Resolve parameter-bearing type terms and nominal bounds

**Purpose:** Create the template semantic product that can describe complete
generic declarations without contaminating ordinary resolved types.

- [x] Introduce a private facade-oriented generic-template subsystem under
  resolution, with cohesive model, type-resolution, bound-resolution, dump,
  and test ownership rather than adding parameter variants to
  `ResolvedTypeKind`.
- [x] Represent parameter leaves, definition-site named identities, nested
  template applications, shared targets, optional payloads, and array elements
  as structural template type terms with source spans.
- [x] Resolve nondependent declaration names and qualified paths once in the
  definition module; retain parameter-dependent applications without
  synthesizing names or parsing text.
- [x] Resolve every `where` target to one exact `InterfaceId`, reject other
  declaration kinds, duplicates, inaccessible interfaces, and bounds on
  unknown parameters.
- [x] Retain parameter-bearing direct bases, fields, statics, lifecycle
  signatures, callable signatures, casts, tests, construction heads, and other
  type-bearing body shapes required by later substitution.
- [x] Reject definition-level operations that cannot be delayed or justified,
  including construction directly through `T` and member selection on an
  unconstrained parameter.
- [x] Represent bound-authorized member lookup by exact interface requirement
  identity while deferring the closed receiver selection needed by ordinary
  resolved bodies.
- [x] Add deterministic template semantic dumps for type terms, bounds,
  definition-site selections, and delayed dependent selections.

**Tests:** Focused template-resolution tests for every type constructor and
body position; definition-site versus application-site shadowing; qualified
and cyclic-module names; valid and invalid interface bounds; bound member
identity; unconstrained member and `T()`/`new T()` diagnostics; deterministic
semantic dumps; focused resolution suite, `make docs-check`, and
`git diff --check`.

**Exit criteria:** A generic declaration has one complete, inspectable,
definition-site-resolved template product, ordinary resolved type tables still
contain no parameter kind, and every unresolved decision is explicitly
classified as argument-dependent rather than encoded with a fake identity.

### G3 — Infer and evaluate contextual generic requirements

**Purpose:** Implement the frozen rule that argument legality follows actual
template uses, not a universal generic-argument whitelist.

- [x] Add structural `GenericRequirement` records containing a type term,
  role-specific capability, origin span, and stable reason.
- [x] Infer field/static storage, value parameter/result, alias access,
  optional payload, array element, and shared-target requirements from
  declarations and type construction.
- [x] Infer default, copy construction, assignment, and destruction
  requirements from delayed body operations and lifecycle synthesis only when
  those operations actually need them.
- [x] Build a narrow capability-query facade over existing optional, array,
  shared, stored-value, alias, and exact-class lifecycle owners; do not clone
  their eligibility or transition logic in the generic subsystem.
- [x] Preserve structural requirements over terms such as `T`, `T?`, and
  `T?[]` rather than reducing all requirements to flags on a parameter.
- [x] Implement recursive evaluation against closed substituted types,
  including optional absence defaulting, nested optional copying and
  assignment, array lifecycle, and shared-owner operations independent of the
  pointee's copy capability.
- [x] Keep unavailable class/aggregate copy operations as capabilities rather
  than eager errors until a recorded operation requires them.
- [x] Render inferred requirements and their origins in template dumps.

**Tests:** Requirement-inference unit tests for every context and operation;
positive and negative matrices for primitive, exact class, optional, nested
optional, array, shared exact/interface/`Obj`, bare interface/`Obj`, `unit`,
and non-copyable classes; explicit proof that `Observer<Interface>` and
`Owner<Interface>` may pass while `Vec<Interface>` fails; optional-array
default without payload default; nested lifecycle failure paths; focused
type-check/capability tests and `git diff --check`.

**Exit criteria:** The compiler can derive and evaluate the effective
mechanical contract for a closed argument list with exact source attribution,
and every answer agrees with the ordinary non-generic validator or lifecycle
planner for the same closed type.

### G4 — Build deterministic closed-specialization identities

**Purpose:** Establish canonical closed application keys, stable `ClassId`
allocation, caching, worklist discovery, and finite recursion before emitting
class declarations.

- [x] Add canonical instance keys containing one `ClassTemplateId` and ordered
  closed `ResolvedTypeKind` arguments; exclude spans and spelling provenance
  from equality.
- [x] Resolve application-site argument spellings under ordinary module and
  visibility rules, recursively close nested applications, and intern ordinary
  compound types bottom-up.
- [x] Implement a deterministic specialization owner with requested,
  in-progress, complete, and failed cache states.
- [x] Allocate one ordinary `ClassId` when a unique key enters the in-progress
  state and reuse it for identical recursive and repeated requests.
- [x] Discover nested applications through substituted terms using stable
  module, declaration, member, expression, and argument order.
- [x] Diagnose transformed recursive re-entry of the same template with a
  different active argument sequence, while permitting identical-key re-entry
  to close a graph.
- [x] Cache failures and collate later use sites without duplicate IDs or
  independently ordered cascades.
- [x] Retain provenance from generated `ClassId` to key, template declaration,
  and ordered application origins.
- [x] Extend dumps with keys, state transitions, assigned class identities, and
  recursion paths without exposing hash iteration.

**Tests:** Canonical-key equality across grouping and optional shorthand;
distinct identities across templates, argument order, optionals, arrays,
shared targets, and nested classes; repeated and cross-module reuse;
identical-key recursive graphs; transformed recursion rejection; failed-cache
reuse; queue and module permutation determinism; focused specialization tests,
`make docs-check`, and `git diff --check`.

**Exit criteria:** Every requested closed application has exactly one stable
success or failure cache entry, recursion terminates under the frozen rule,
and class identity/order is reproducible without yet publishing incomplete
ordinary declarations.

### G5 — Specialize class declarations into ordinary closed classes

**Purpose:** Materialize complete closed headers and member declarations that
all existing class-level validators can consume unchanged.

- [x] Substitute the direct base, interface claims, fields, static fields,
  lifecycle signatures, initializer overloads, and method signatures for each
  in-progress key.
- [x] Reuse `ResolvedTypeInterner` for every resulting array, optional, and
  optional-box identity; preserve literal `Optional<Shared<T>>` versus
  `Shared<Optional<T>>` composition.
- [x] Allocate deterministic field, static, initializer, lifecycle, and method
  identities from the generated `ClassId` and template member order.
- [x] Validate contextual declaration requirements before publishing a
  generated class, with diagnostics at both application and originating
  template type use.
- [x] Insert only complete closed declarations into ordinary class tables and
  keep template records in their separate semantic owner.
- [x] Feed generated declarations through optional/array eligibility and
  finite-containment validation, including recursive graphs across existing
  legal indirection boundaries.
- [x] Preserve declaring-class privacy and generated member visibility without
  granting access to private members of argument types.
- [x] Extend resolved dumps to show the generated exact class, canonical
  parameter mapping, substituted members, and specialization origin.

**Tests:** Exact declaration substitution for `Vec<Str>`, `Vec<Str?>`,
`Vec<shared Str>`, and `Vec<shared Interface>`; rejection of bare interface,
`Obj`, or `unit` precisely where a template use forbids it; multi-parameter
order; recursive fields and containment; private types and members; member ID
stability; optional/array table order; focused resolver/type-check declaration
tests and deterministic resolved dumps.

**Exit criteria:** Generated declarations are indistinguishable from valid
hand-written closed class declarations to existing declaration validators,
contain no parameter-bearing type, and preserve exact contextual diagnostics.

### G6 — Specialize executable bodies and operation selections

**Purpose:** Close every generic initializer, lifecycle, method, and static
initializer body so complete-class validation and ordinary HIR can proceed.

- [x] Substitute type-bearing nodes in expressions, statements, locals,
  construction/allocation, casts, type tests, static selections, explicit copy
  construction, and calls.
- [x] Resolve delayed initializer overloads, exact callable compatibility,
  casts, object places, and argument/result operations against substituted
  closed types using existing selection logic.
- [x] Preserve source evaluation order, full-expression boundaries, access,
  produced-value provenance, alias anchors, optional guards, and ownership
  selection from the template body.
- [x] Generate complete ordinary resolved definitions under member identities
  allocated by the specialization.
- [x] Validate every body in the class even when its member is not called by
  reachable code; do not implement lazy method instantiation.
- [x] Map operation and requirement failures to the application plus the
  originating template expression or declaration.
- [x] Ensure nested applications discovered during body specialization rejoin
  the deterministic worklist and complete before ordinary type checking needs
  them.
- [x] Keep body-specialization algorithms in cohesive private modules behind a
  small specialization facade; avoid duplicating the existing general body
  resolver wholesale.

**Tests:** Resolved-body tests for every statement/expression family that can
carry a type or select a type-dependent operation; constructor overloads,
ordinary/copy allocation, calls, parameters/results, aliases, casts, tests,
arrays, optionals, and nested applications; unused-invalid-member rejection;
evaluation/provenance dump preservation; application-origin diagnostics;
focused resolver tests and `git diff --check`.

**Exit criteria:** Every completed specialization has ordinary closed resolved
bodies with all callable, type, access, ownership, and lifecycle selections
made, and no downstream checker needs to understand a template term or delayed
generic operation.

### G7 — Integrate lifecycle, optionals, arrays, and ownership

**Purpose:** Prove that specialization composes with Skald's complete stored-
value and deterministic-lifecycle matrix rather than supporting only scalar
or trivial classes.

- [x] Run exact-class copy construction and assignment analysis over generated
  fields, bases, arrays, and recursive optionals.
- [x] Select class destruction plans for specialized direct fields, nested
  optionals, arrays, shared owners, and bases in ordinary order.
- [x] Type-check generic value parameters/results, aliases, stored-value
  initialization, assignment, optional injection/unwrap, array construction,
  element access/replacement, and shared-owner operations through existing HIR
  plans.
- [x] Preserve the distinction between optional shared owners and shared
  optional boxes for parameter substitution and operation selection.
- [x] Verify capability-sensitive class-wide acceptance: a specialization may
  exist without copying until one member actually requires copy construction
  or assignment.
- [x] Add HIR assertions proving every specialized operation carries exact
  canonical closed types, selected lifecycle identities, guards, anchors, and
  cleanup plans.
- [x] Extend living optional, array, shared-ownership, class-lifecycle, and
  testing documents only where generic composition adds a new implemented
  boundary; retain their existing semantics as authoritative.

**Tests:** Type-check/HIR matrices for primitives, exact classes with user and
synthesized lifecycle, absent capabilities, arrays, nested arrays, recursive
optionals, shared exact/base/interface/`Obj`, optional owners, and shared
optional boxes; copy/assignment failure paths; arguments/results; aliases;
stored fields and statics; focused capability and HIR tests plus
`make docs-check` and `git diff --check`.

**Exit criteria:** Every frozen owning type family can participate in a generic
class exactly when its substituted declaration and operations are valid, and
typed HIR contains only the same closed lifecycle and ownership operations used
by non-generic classes.

### G8 — Integrate nominal bounds, inheritance, conformance, and dispatch

**Purpose:** Complete the generic object-model contract after closed
declarations and bodies are stable.

- [x] Evaluate each explicit interface bound against the closed exact class
  argument's effective nominal conformance.
- [x] Lower member use authorized by a bound through the selected interface
  requirement and ordinary interface dispatch semantics; reject duck-typed
  matching and shared-owner lifting.
- [x] Diagnose ambiguous bound member names and failed conjunctive bounds with
  definition and application evidence.
- [x] Materialize closed generic bases before ordinary hierarchy analysis and
  validate cycles, base initialization, inherited selection, privacy, slicing,
  and base lifecycle.
- [x] Support ordinary classes extending closed generic applications and
  generic classes implementing non-generic interfaces.
- [x] Compute overrides, virtual families, and conformance maps independently
  for every generated `ClassId` using exact substituted signatures.
- [x] Keep applications invariant even when arguments or generated classes
  participate in valid class/interface/shared view conversions.
- [x] Preserve existing safe devirtualization boundaries without treating
  specialization alone as proof of a different dispatch result.

**Tests:** Bound success/failure across direct and inherited conformance;
unconstrained and duck-typed member rejection; bound ambiguity; shared pointee
versus shared-owner constraints; generic base chains, cycles, overrides,
interfaces, `Obj`, checked casts, type tests, slicing, virtual/interface
dispatch, and invariance; focused hierarchy/interface/object tests and exact
HIR dumps.

**Exit criteria:** Closed generic classes participate in the complete existing
object model with ordinary identities and dispatch, explicit bounds provide
only nominal interface knowledge, and no application conversion or runtime
generic witness exists.

### G9 — Integrate per-specialization statics and whole-program effects

**Purpose:** Give each closed class independent class-owned state while
preserving eager lifecycle, dependency evidence, and deterministic shutdown.

- [x] Allocate distinct static field and initializer identities and backend
  storage for every closed specialization.
- [x] Specialize explicit static initializer bodies and infer their direct and
  transitive effects through existing callable and lifecycle operations.
- [x] Treat a static selection as a specialization request even when no object
  value or constructor otherwise requests the class.
- [x] Preserve zero-default versus explicit initialization validation after
  substitution, including parameter-dependent optional and array types.
- [x] Include generated statics in dependency planning, cycle diagnostics,
  activation order, publication, replacement, normal-return result
  preservation, and reverse shutdown.
- [x] Keep template records free of static runtime storage; only a closed
  `ClassId` owns slots and lifecycle bodies.
- [x] Extend planned/final MIR dumps with human-readable closed generic owner
  names while preserving identity-selected static effects and plan indices.

**Tests:** `Cache<Str>` versus `Cache<i64>` independence; zero/default and
explicit stored matrices; static-only specialization discovery; direct and
transitive dependencies across specializations; self/cross-specialization
cycles; replacement and destruction order; preliminary/planned/final MIR
tests, static goldens where available, and deterministic plan dumps.

**Exit criteria:** Generated statics behave exactly like statics on distinct
hand-written classes, participate in all existing lifecycle certificates, and
remain independent across argument keys.

### G10 — Harden diagnostics, dumps, modules, and determinism

**Purpose:** Make generic failures understandable and specialization products
reproducible before the backend and standard library depend on them broadly.

- [x] Give template-definition and application failures distinct stable
  diagnostics without exposing private compiler terms as the primary message.
- [x] Render application-site primary labels, template-origin secondary
  labels, nested generic/type-constructor paths, and existing lifecycle
  field/base paths in one coherent diagnostic.
- [x] Collate repeated failed-key uses and prevent duplicate cascades from
  worklist rediscovery.
- [x] Complete deterministic syntax, template, specialization, resolved, HIR,
  and MIR dumps with qualified semantic generic names.
- [x] Audit module cycles, selective and qualified imports, aliases, privacy,
  application-site argument lookup, and definition-site template lookup across
  multi-file graphs.
- [x] Add cross-process and graph-permutation tests for template IDs, closed
  class IDs, requirement order, cache order, diagnostics, phase dumps, static
  plans, and assembly-independent semantic products.
- [x] Extend bounded robustness mutation for angle brackets, `where`, nested
  applications, and malformed constraints while preserving termination and
  deterministic recovery.
- [x] Audit affected resolver and type-check modules for mixed
  responsibilities; extract substantial state machines behind private
  submodules and keep facade exports explicit and minimal.
- [x] Update debugging and testing guidance for template, specialization, and
  obligation dumps.

**Result:** Generic failures now distinguish definition and application
ownership, suppress the temporary execution gate when a more specific closed
application failure exists, collate repeated key origins, and retain inferred
lifecycle field/base evidence. Failed contextual graphs atomically restore the
ordinary class product and unpublish dependent definitions and virtual
families. Whole-program closed names use canonical module-qualified semantic
spellings through resolved, HIR, planned-MIR, final-MIR, and static-plan
products. Specialization declaration inputs and name rendering are cohesive
private components behind the existing facade.

**Tests:** Exact compile-failure goldens for definition, arity, wrong-kind,
constraint, contextual, lifecycle, recursion, module, and privacy failures;
pipeline determinism and graph permutation suites; bounded robustness; focused
golden filters; `cargo fmt --check`, Clippy for affected crates,
`make docs-check`, and `git diff --check`.

**Exit criteria:** Every frozen invalid program fails at a stable useful source
location, repeated and cross-module uses remain deterministic, all semantic
products are inspectable, and the new resolver subsystem has cohesive module
ownership.

**Completion summary:** Added nine focused generic compile-failure goldens,
cyclic/aliased/selective multi-module resolver permutations, an independent-
process graph-to-final-MIR comparison, and deterministic bounded mutations for
generic punctuation and malformed constraints. `cargo test -p
skald-compiler`, affected-crate Clippy with warnings denied, focused goldens,
formatting, documentation checks, and whitespace validation pass. No frozen
language decision or runtime ABI changed. Both G9 discoveries assigned to G10
are resolved; no follow-up discovery remains from this task.

### G11 — Execute generic classes through MIR and x86-64

**Purpose:** Prove that closed specialization needs no generic lower-IR or
runtime protocol and executes through the existing backend trust boundary.

- [x] Lower specialized constructors, methods, lifecycle operations,
  inheritance, dispatch, arrays, optionals, shared owners, aliases, and statics
  through ordinary closed HIR-to-MIR paths.
- [x] Add verifier assertions or structural audits proving every generated
  class, member, type, place, operation, and helper identity is concrete and
  belongs to the closed program tables.
- [x] Generate deterministic collision-free private symbols that distinguish
  template identity and complete canonical argument identity while keeping
  user-facing dumps semantic.
- [x] Emit layouts, allocation metadata, finalizers, dispatch tables, statics,
  calls, and cleanup for generated classes exactly as for hand-written classes.
- [x] Confirm register/stack ABI behavior follows each substituted exact type
  and that external signature restrictions remain unchanged.
- [x] Add backend/native coverage for multiple specializations with identical
  layouts but distinct identities and for applications spanning modules.
- [x] Confirm runtime ABI version 9 and the public C header/archive remain
  unchanged.

**Result:** Valid closed applications now enter the public compiler pipeline
without a staging diagnostic. Final MIR applies a generic-agnostic closed-type
audit to declarations, signatures, fields, statics, storage, and values before
the existing operation/place/lifecycle verification. The x86-64 backend uses
ordinary substituted layout, register/stack/hidden-result classification,
allocation, dispatch, finalization, static, call, and cleanup paths. Private
symbols encode canonical semantic applications and retain the closed
`ClassId`; qualified runtime-trace owners avoid duplicating module paths.

**Completion summary:** Added verifier mutations for generated fields,
signatures, and body storage; public-driver multi-module emission coverage;
backend/native tests for scalar, SSE, stack, hidden-result, lifecycle,
optional-array/shared-owner, static, bound-dispatch, inheritance, equal-layout
identity, and assembly acceptance; and three source-to-process goldens for
normal execution, checked bounds failure, and cross-module applications.
Runtime ABI version 9, `skald_runtime.h`, and the archive surface are unchanged.
Focused generic tests and goldens, the full compiler suite, runtime suite,
Clippy, formatting, documentation, and diff gates pass. No follow-up discovery
was created.

**Tests:** MIR lowering and verifier mutation tests; backend layout/symbol/
dispatch/static tests; assembly acceptance; native construction, copy,
assignment, cleanup, array/optional/shared behavior, bounds failure, and
multi-module execution; focused golden tests; `make runtime-test`,
`make docs-check`, and `git diff --check`.

**Exit criteria:** Representative generic classes compile and execute natively
with verified ordinary MIR, distinct deterministic artifacts, exact ownership
and cleanup, and no new runtime generic or C ABI surface.

### G12 — Deliver generic `Vec<T>` and close the feature

**Purpose:** Exercise the complete frozen profile in its motivating standard-
library abstraction, finish documentation, and validate the repository from a
clean artifact-free state.

- [x] Implement an ordinary standard-library `Vec<T>` whose backing is `T?[]`
  and whose API establishes the intended capacity, growth, indexing, push,
  replacement, last, pop, clear, copy-independence, and prompt destruction
  semantics for every admitted `T`.
- [x] Let the implementation's actual operations infer the required copy and
  assignment capabilities; do not add source-visible lifecycle bounds or
  compiler-known vector methods.
- [x] Exercise `Vec<Str>`, `Vec<Str?>`, `Vec<shared Str>`,
  `Vec<shared Interface>`, primitives, exact inline classes, nested arrays,
  and deliberately unavailable lifecycle cases.
- [x] Compare the generic vector's shared-object behavior with the implemented
  `VecObj` contract and decide documentation/library migration without
  silently removing or renaming the existing public class.
- [x] Add complete native, compile-failure, panic, lifecycle, module,
  determinism, and representative performance-measurement coverage without
  making timing a correctness gate.
- [x] Promote generic classes to **Implemented contract** in the status matrix
  and remove stale "not implemented" wording from living language, compiler,
  grammar, architecture, and testing documents.
- [x] Remove roadmap task codes or rollout language from living code, tests,
  dumps, diagnostics, and documentation while preserving them in this roadmap
  and the archived design record.
- [x] Audit all touched modules for facade clarity, narrow visibility,
  cohesive test placement, and remaining high-priority maintainability issues;
  record lower-priority follow-ups in an indexed discoveries document rather
  than expanding closeout.
- [x] Run the complete validation gates from an artifact-free snapshot and
  archive this roadmap only after every task and exit criterion is complete.

**Result:** The installed standard library now provides ordinary `Vec<T>`
with private `T?[]` occupancy storage, exact substituted element semantics,
geometric growth, signed logical indexing, structural copy independence, and
prompt removal cleanup. Its source operations infer storage, copy, assignment,
and destruction requirements without vector-specific compiler knowledge.
`VecObj` remains public and unchanged as the heterogeneous `shared Obj`
compatibility profile.

**Completion summary:** Added native and failure goldens for primitive, string,
nested-optional, exact-class, nested-array, shared exact/interface, lifecycle,
panic, and cross-module behavior; a reproducible non-gating performance
measurement; and focused type-check/MIR regressions. The vector acceptance
surface exposed and fixed generic-agnostic optional-class array clearing and
complete guarded-payload provenance defects in their owning MIR layers. Living
language, compiler, standard-library, debugging, and testing documents now
describe the implemented contract. The resolved discoveries record contains
no pending work. From an artifact-free snapshot, `make check`, `make
msrv-check`, `make robustness-long`, full golden determinism, documentation,
formatting, and diff gates pass; runtime ABI version 9 remains unchanged.

**Tests:** Focused `generic_classes/**` and `standard_vec/**` goldens during
development; full independent-process determinism; `make check`,
`make msrv-check`, `make robustness-long`, and `git diff --check` from a clean
or artifact-free checkout; final documentation link/index validation.

**Exit criteria:** The complete frozen generic-class profile and generic
vector execute on x86-64, all exclusions remain explicit, every repository
gate passes, the status matrix and living contracts describe current behavior,
and the completed roadmap is archived with no unowned actionable discovery.

## Ordering and dependencies

G0 freezes the parser product before semantic identities depend on it. G1 then
separates templates from executable classes in the module namespace. G2 owns
the parameter-bearing semantic representation; G3 builds the contextual
requirement vocabulary over that representation. G4 can then canonicalize and
cache applications without yet publishing incomplete ordinary classes.

G5 closes declarations first because hierarchy, lifecycle, conformance, and
body selection need complete signatures and member identities. G6 closes
bodies and feeds additional discovered applications back into the stable
worklist. G7 proves stored-value and lifecycle composition before G8 adds the
broader nominal object model. G9 follows once class identities, bodies, and
lifecycle effects are stable enough for whole-program static planning.

G10 hardens diagnostics, modules, determinism, and internal structure before
lower phases make specialization artifacts costly to change. G11 then proves
that ordinary MIR and x86-64 are sufficient. G12 uses `Vec<T>` as the complete
source-to-native acceptance surface and owns feature closeout.

Within these dependencies, narrow test utilities, dump rendering, and
documentation edits may be prepared alongside their owning task, but no task
may publish parameter-bearing types into ordinary resolved IR or weaken
existing validators to bypass an incomplete specialization. The runtime ABI
has no planned dependency; any discovered need for a new runtime generic
service is a design contradiction and must stop the roadmap for review rather
than expand a task.
