# Generic Interfaces Roadmap

Status: in progress; I0 is complete and I1 is next.

Implement the frozen [generic-interface language contract](../language/GENERIC_INTERFACES.md)
and [compiler specialization contract](../compiler/GENERIC_INTERFACES.md).
The finished compiler will accept explicit generic interface declarations and
applications, specialize every requested closed application into an ordinary
exact interface before HIR, enforce exact nominal bounds and conformance, and
execute ordinary witness dispatch through the existing verified MIR and
x86-64 backend without a runtime generic ABI.

The archived [design record](../archive/GENERIC_INTERFACES_DESIGN_PROPOSAL.md)
preserves the confirmed decisions. This roadmap owns delivery order and must
not reopen those decisions during implementation.

## Scope and invariants

- Generic interfaces use `interface I<T, U>` with explicit complete
  applications `I<A, B>`; arguments are never inferred or omitted.
- Interface-level and class-level `where T: I<U>` bounds may contain generic
  interface applications.
- A template is not an executable interface. `InterfaceTemplateId` and
  `InterfaceTemplateRequirementId` remain distinct from `InterfaceId` and
  `InterfaceRequirementId`.
- `TypeParameterId` has an honest class-or-interface template owner; interface
  templates are not modeled as synthetic classes.
- Every accepted canonical closed key receives one deterministic ordinary
  `InterfaceId` and source-order ordinary requirement identities.
- Structural substitution and contextual validation reuse the existing
  generic-class, type-position, ownership, and lifecycle rules.
- Closed applications are invariant and conformance is exact and nominal.
- Multiple applications of one template are supported when ordinary
  non-overloaded method rules can satisfy every exact requirement.
- A bound call selects a template requirement at definition time, maps it to
  the closed requirement during specialization, and retains ordinary interface
  dispatch.
- Bare closed interfaces remain non-owning views; `shared` remains the owning
  interface form. Casts and tests target the exact closed application.
- One deterministic coordinator handles interleaved class/interface requests,
  identical recursion, transformed-expansion rejection, provenance, and
  cached failure.
- Ordinary resolved executable declarations, HIR, MIR, verification, and
  backends never carry an unresolved parameter, interface template, template
  requirement, dictionary, or runtime type argument.
- Interface representation, witnesses, calling convention, shared ownership,
  static effects, runtime traces, and public runtime ABI remain unchanged
  after closure.
- Primitive conformances, operator protocols and overloading, iteration
  protocols, range types, new loop syntax, generic methods, inference,
  variance, associated types, interface inheritance, erased generics, and
  separate-compilation template ABI remain out of scope.
- New Rust implementation belongs behind cohesive facade-oriented modules.
  Shared template scheduling and substitution may be factored for reuse, but
  interface declaration specialization must not be folded into class body or
  lifecycle owners.
- Tests land with each owning phase. The final hardening task broadens the
  matrix; it does not defer basic positive, negative, or determinism coverage
  from earlier tasks.

## Progress

- [x] I0 — Parse and preserve generic interface syntax
- [ ] I1 — Generalize template identities and module declarations
- [ ] I2 — Resolve generic interface templates and requirements
- [ ] I3 — Resolve parameterized interface applications and bounds
- [ ] I4 — Coordinate deterministic class and interface specialization
- [ ] I5 — Materialize ordinary closed interface declarations
- [ ] I6 — Integrate exact nominal conformance and inheritance
- [ ] I7 — Close generic bounds and bound-selected calls
- [ ] I8 — Integrate views, ownership, casts, tests, and structural calls
- [ ] I9 — Prove HIR, MIR, witness, backend, and native execution
- [ ] I10 — Harden modules, diagnostics, dumps, robustness, and determinism
- [ ] I11 — Complete the conformance matrix and close the feature

## PR-sized implementation sequence

### I0 — Parse and preserve generic interface syntax

**Purpose:** Establish the complete frozen source shape, exact spans, syntax
dumps, and deterministic recovery before semantic owners depend on it.

- [x] Extend `InterfaceDecl` with the existing generic parameter-list and
  generic `where`-clause AST shapes.
- [x] Parse `interface I<T, U> [where ...]` while retaining `where` as a
  contextual word outside the confirmed header form.
- [x] Generalize `generic-requirement` parsing so its right side is a named
  type application rather than only a declaration path.
- [x] Admit named type applications in `implements` clauses while retaining
  semantic wrong-kind validation for non-interface targets.
- [x] Preserve nested closers in interface declarations, requirement
  signatures, bounds, and claims without changing expression comparison or
  shift tokenization.
- [x] Diagnose empty or trailing parameter/argument lists, missing commas and
  closers, malformed bounds, misplaced `where`, and broken `implements`
  applications with stable recovery into later requirements and declarations.
- [x] Extend syntax dumps with interface parameters, bounds, applied claims,
  punctuation spans, and grouping provenance.
- [x] Update the implemented grammar only for syntax actually accepted by this
  task, with an explicit semantic staging note until later tasks complete.

**Tests:** Focused parser and AST tests in the existing generic and interface
owners; syntax-dump goldens for single/multiple parameters, interface-level
bounds, generic `implements`, nested class/interface applications, and module-
qualified names; malformed recovery cases for every delimiter; adjacent
`<`, `>`, `>=`, and `>>` expression regressions; parser generative mutations;
`cargo fmt --check`, focused `cargo test --locked -p skald-compiler syntax`,
`make docs-check`, and `git diff --check`.

**Exit criteria:** The AST preserves every frozen generic-interface source
form and diagnostic span, ordinary generic-class and expression parsing is
unchanged, malformed input recovers deterministically, and unresolved generic
interfaces are explicitly gated rather than assigned false ordinary identity.

### I1 — Generalize template identities and module declarations

**Purpose:** Give interface templates and their requirements honest stable
identity while safely migrating the class-only type-parameter owner.

- [ ] Add `InterfaceTemplateId`, `InterfaceTemplateRequirementId`, and a
  class-or-interface `GenericTemplateId` owner to shared identity vocabulary
  with deterministic display and crate-private construction.
- [ ] Change `TypeParameterId` from class-template ownership to
  `GenericTemplateId`; adapt class-template tables and callers without
  weakening their typed accessors.
- [ ] Distinguish ordinary interface, interface template, class template, and
  other declaration kinds in top-level symbols, resolved module declarations,
  imports, qualification, visibility, collisions, and wrong-kind diagnostics.
- [ ] Allocate interface template and template requirement identities in
  canonical module, declaration, and requirement source order without
  consuming placeholder ordinary `InterfaceId` values.
- [ ] Collect interface type parameters, reject duplicates, and establish
  scope over the interface `where` clause and every requirement signature.
- [ ] Preserve generic-class identity, specialization key, dump, and module
  behavior through focused migration tests.
- [ ] Extend public phase facades only with stable inspectable identity/table
  products; keep allocation and collection implementation private.

**Tests:** Identity construction/display and owner distinction; dense table
lookup; mixed ordinary/generic class/interface declaration order; duplicate
parameters and cross-kind collisions; private/public selective and qualified
imports; raw names, arguments on non-generic declarations, and wrong-kind
uses; module-order and graph-permutation determinism; the complete existing
generic-class resolver suite; focused module tests; `make compiler-test`,
`make docs-check`, `make msrv-check`, and `git diff --check`.

**Exit criteria:** Every generic interface and source requirement has one
stable non-executable identity, parameter identity represents its real owner,
module lookup distinguishes every declaration kind, and no existing generic
class changes semantic identity or output.

### I2 — Resolve generic interface templates and requirements

**Purpose:** Build one complete definition-site-resolved semantic product for
a generic interface without adding unresolved parameters to ordinary types.

- [ ] Add cohesive generic-interface template declaration, requirement,
  parameter, bound, type-use, and table models behind the resolve facade.
- [ ] Extend structural template types with parameter-bearing interface
  applications while retaining separate closed ordinary interface terms.
- [ ] Resolve requirement parameter modes/types and results structurally,
  including primitives, parameters, functions, shared, optionals, arrays,
  nested generic classes, and nested generic interfaces.
- [ ] Resolve nondependent names in the template definition module and retain
  dependent applications as semantic terms rather than reparsed source.
- [ ] Assign and preserve `InterfaceTemplateRequirementId` through duplicate-
  name checking, lookup, dumps, and diagnostics.
- [ ] Infer contextual parameter, result, alias, shared-target, optional,
  array, and nested-application requirements using existing generic
  requirement vocabulary and ordinary capability owners.
- [ ] Validate all definition-independent interface errors once, including
  duplicate requirements, invalid modes, inaccessible names, raw declarations,
  and forbidden ordinary interface constructs.
- [ ] Add deterministic template semantic dumps with exact spans and
  requirement origins.

**Tests:** Focused template-resolution tests for every supported type tree and
parameter mode; nested generic class/interface type terms; definition-
site versus caller-site shadowing; qualified cyclic-module names; duplicate
requirements and parameters; invalid bare result versus valid alias target;
marker parameter with no inferred requirements; stable template requirement
identity and dumps; unchanged ordinary interface tests; focused resolve tests,
`make docs-check`, and `git diff --check`.

**Exit criteria:** Every generic interface has a complete inspectable template
product with no fake ordinary interface identity, all definition-independent
errors are settled, and ordinary resolved type tables still contain no
parameter or interface-template kind.

### I3 — Resolve parameterized interface applications and bounds

**Purpose:** Make generic interface applications first-class structural terms
through all pre-specialization type, claim, and constraint positions.

- [ ] Resolve closed applications at ordinary use sites and parameter-bearing
  applications inside class or interface templates with exact arity,
  visibility, and declaration-kind checking.
- [ ] Generalize class `implements` template claims from ordinary
  `InterfaceId` to ordinary-or-structural interface applications.
- [ ] Generalize class bounds and add interface bounds whose right side may be
  an ordinary interface or generic interface application.
- [ ] Validate bound subjects, duplicate exact bounds, inaccessible targets,
  nested application arity, and wrong declaration kinds with complete source
  origins.
- [ ] Propagate nested generic-class and generic-interface obligations through
  enclosing bounds rather than accepting unconstrained dependent applications.
- [ ] Discover closed interface requests from signatures, claims, bounds,
  aliases, shared targets, casts, tests, and outer generic arguments without
  allocating the same request twice.
- [ ] Preserve contextual legality: bare interface arguments may survive where
  actual uses are alias-only or unused and fail only where substituted owning
  roles require it.
- [ ] Extend resolved template dumps with structural interface applications,
  claims, bounds, and application origins.

**Tests:** Ordinary and template use-site matrices; class and interface
`where T: I<U>`; generic `implements`; nested class/interface applications;
raw name, arity, visibility, unknown subject, wrong kind, and duplicate bound
diagnostics; exact application-site argument lookup across modules; bare
interface alias-only/marker acceptance and owning-position rejection;
application discovery order and dump determinism; focused resolve/type-
requirement tests and `git diff --check`.

**Exit criteria:** Every frozen source position produces either one exact
ordinary request or one structural application with explicit obligations and
origins, and no consumer relies on source text or a class-only interface claim
representation.

### I4 — Coordinate deterministic class and interface specialization

**Purpose:** Establish canonical closed interface identities, a cross-kind
worklist, recursion handling, and failure caching before publishing generated
interface declarations.

- [ ] Add `GenericInterfaceInstanceKey` from one template identity and ordered
  canonical closed `ResolvedTypeKind` arguments, excluding spans and spelling.
- [ ] Refactor the specialization owner into a small coordinator that can
  schedule class and interface keys while retaining declaration-specific
  caches and realization logic.
- [ ] Implement requested, in-progress, complete, and failed interface cache
  states with one early reserved `InterfaceId` per unique key.
- [ ] Reuse equivalent requests across signatures, claims, bounds, modules,
  casts, tests, and nested applications.
- [ ] Maintain one cross-kind active path; accept identical-key recursion and
  reject re-entry of the same template with changed arguments as deterministic
  non-terminating expansion.
- [ ] Cache failures and collate later origins without duplicate IDs or
  independently ordered cascades.
- [ ] Preserve deterministic source-derived queue order and explicit
  transition/provenance dumps; never expose hash iteration.
- [ ] Keep the existing generic-class key, class identity, static order, and
  recursion behavior stable during coordinator extraction.

**Tests:** Canonical key equality across grouping/import aliases/optional
shorthand; distinction across templates, argument order, nested types, shared
targets, and interfaces; repeated and cross-module reuse; self-recursive
`Chain<T>`; mutual class/interface cycles; transformed interface and mixed
class/interface expansion rejection; failed-cache reuse; queue, hash-seed,
module-permutation, and repeated-process determinism; complete existing class
specialization suite; focused resolver tests and `git diff --check`.

**Exit criteria:** Every closed interface request has one stable success or
failure entry and ordinary identity, mixed recursion terminates under the
frozen rule, and the coordinator can publish dependencies without yet
pretending incomplete declarations are valid.

### I5 — Materialize ordinary closed interface declarations

**Purpose:** Turn each valid interface key into a complete ordinary
declaration that existing type checking and lower phases can consume unchanged.

- [ ] Substitute every requirement parameter and result type structurally,
  close nested class/interface applications, and intern all ordinary compound
  types through existing owners.
- [ ] Allocate ordinary `InterfaceRequirementId` values from the reserved
  closed `InterfaceId` and template requirement source-order indexes.
- [ ] Retain an explicit template-to-closed requirement mapping for bound-call
  closure, diagnostics, and dumps.
- [ ] Evaluate every contextual signature and nested-application obligation;
  reject the complete application if any requirement is invalid.
- [ ] Publish only complete closed `ResolvedInterfaceDeclaration` entries and
  feed them through existing ordinary interface signature validation.
- [ ] Make ordinary closed type resolution use
  `ResolvedTypeKind::Interface(InterfaceId)` with no generic alternative.
- [ ] Diagnose failures at both the application and originating requirement
  type use; retain cached failure identity/provenance consistently.
- [ ] Render generated interfaces, canonical argument mappings, closed
  requirements, and origins deterministically.

**Tests:** Exact substitution for primitives, exact classes, shared owners,
bare interface aliases, functions, optionals, arrays, and nested generics;
marker interfaces; invalid owning results, parameters, shared targets, and
nested constrained applications; exact ordinary/template requirement ID
mapping; complete-not-lazy failure; repeated failure collation; generated
declaration and type-table order; resolved dump snapshots; focused resolver
and interface type-check tests, `make docs-check`, and `git diff --check`.

**Exit criteria:** A successful application is indistinguishable from an
equivalent hand-written ordinary interface to existing declaration consumers,
contains only closed ordinary types and IDs, and failures retain exact
application and template causes.

### I6 — Integrate exact nominal conformance and inheritance

**Purpose:** Check ordinary and generic classes against exact closed interface
applications and build trustworthy witness maps.

- [ ] Resolve ordinary class claims to closed generic interfaces and close
  structural claims independently for every generic class specialization.
- [ ] Order hierarchy/effective-method validation, interface materialization,
  and conformance so each consumes complete identities and signatures.
- [ ] Reuse the existing exact conformance algorithm for name, arity, modes,
  types, result, receiver mutability, visibility, inherited methods, and
  overrides.
- [ ] Key conformance maps by exact closed `InterfaceId` and map every closed
  requirement to one concrete `MethodId`.
- [ ] Preserve inherited conformance as the same exact application and update
  witnesses through compatible effective overrides.
- [ ] Support multiple distinct applications of one template, including
  markers and shared exact implementations, while rejecting incompatible
  non-overloaded methods.
- [ ] Apply existing duplicate direct and redundant inherited conformance rules
  to the exact closed application rather than only the template name.
- [ ] Retain class/interface application and method-signature origins in
  conformance diagnostics and resolved dumps.

**Tests:** Ordinary and generic class positive conformance; substituted
primitive/class/shared/optional signatures; inherited methods and overrides;
read-only/mutable receiver mismatch; private/static/wrong-signature failures;
multiple marker applications; one method satisfying multiple same-signature
applications; incompatible same-named requirements; duplicate exact versus
distinct applications; redundant inherited claims; generated generic bases;
cross-module visibility; focused resolve/type-check conformance suites and
deterministic dumps.

**Exit criteria:** Every claimed closed application has one exact proven
conformance or one source-attributed failure, inherited and multiple
applications obey the frozen rules, and witness maps contain only complete
ordinary identities.

### I7 — Close generic bounds and bound-selected calls

**Purpose:** Make parameterized interface bounds useful in generic bodies
without introducing application-dependent duck typing or altered dispatch.

- [ ] Resolve a member selected through a generic interface bound to one
  `InterfaceTemplateRequirementId` at the template definition site.
- [ ] Retain the bounded parameter, exact structural bound application,
  selected requirement, member spelling, and source origin in template
  selections.
- [ ] Detect ambiguous same-named requirements across ordinary and generic
  bounds before specialization.
- [ ] At class specialization, close each bound application and require the
  exact class argument's effective conformance to that exact interface.
- [ ] Map template requirement identity to the closed
  `InterfaceRequirementId` and generate an ordinary interface call selection.
- [ ] Preserve interface dispatch, receiver access, argument/result ownership,
  evaluation order, produced-result lifetime, and failure behavior.
- [ ] Reject bounds satisfied only structurally, through a bare interface,
  through an unlifted shared wrapper, or by another application of the same
  interface template.
- [ ] Extend template and generated-body dumps with before/after bound
  selections and exact conformance evidence.

**Tests:** `where Source: Producer<T>` positive calls for primitive, class, and
shared-owner results; interface-level nested bounds; exact application
mismatch; missing nominal claim; bare/shared subject rejection; multiple
bounds, same-name ambiguity, and duplicate bound behavior; mutable requirement
receiver access; inherited argument conformance; produced exact-class result
use and cleanup; definition-site selection under module shadowing; resolved and
HIR dump identity; focused generic body, call, ownership, and type-check tests.

**Exit criteria:** Every accepted bound call has one definition-site template
requirement, one closed ordinary requirement, and ordinary dispatch semantics;
no specialization may reselect a member from the concrete class.

### I8 — Integrate views, ownership, casts, tests, and structural calls

**Purpose:** Prove that exact closed applications compose with the complete
existing interface object model rather than only conformance and direct calls.

- [ ] Admit closed generic interfaces in every ordinary interface alias and
  receiver position with unchanged access and call-scoped lifetime rules.
- [ ] Integrate `shared I<T>` and optional/array combinations through existing
  owner construction, copy, transfer, assignment, result, cleanup, and hidden
  anchor paths.
- [ ] Resolve class-to-interface views, object-place casts, shared casts, and
  type tests against the exact closed `InterfaceId`.
- [ ] Preserve exact dynamic metadata checks: implementing `I<A>` never
  satisfies a query for `I<B>`.
- [ ] Compose closed generic interfaces used as generic class arguments,
  including alias-only acceptance and owning-storage rejection for bare views.
- [ ] Route structural indexing and slicing through closed generic interface
  requirements when the exact specialized signature is eligible.
- [ ] Verify produced owning results, optionals, arrays, shared owners, access
  modes, checked anchors, and cleanup use ordinary closed paths.
- [ ] Retain current exclusions for interface method references, independently
  stored bare views, escaping borrows, boxing, and interface-to-interface
  implicit conversion.

**Tests:** Read-only/mutable aliases; inherited and forwarded views; shared
fields/locals/parameters/results/arrays/optionals; owner copy and transfer;
successful and failing exact casts/tests across two applications; checked
anchors and produced sources; `Vec<shared I<T>>` versus bare owning rejection;
structural get/set/slice calls with generic interface receivers; evaluation
and cleanup order; explicit method-reference exclusion; focused ownership,
object-view, cast, optional, array, and structural-indexing suites.

**Exit criteria:** Every currently supported ordinary interface consumer
accepts the equivalent valid closed generic interface and preserves its exact
ownership, access, failure, dispatch, and cleanup contract without a generic-
specific runtime representation.

### I9 — Prove HIR, MIR, witness, backend, and native execution

**Purpose:** Carry the closed feature through every lower trust boundary and
execute exact generic-interface dispatch without new lower-IR concepts.

- [ ] Lower generated closed interface declarations, conformances, views, and
  calls through existing HIR structures with ordinary interface identities.
- [ ] Assert at HIR construction that no template parameter, interface
  template, template requirement, or structural application survives.
- [ ] Lower exact conformance and requirement calls through existing MIR,
  including shared ownership, optional/array values, produced results, cleanup,
  and static-effect inference.
- [ ] Extend preliminary and final MIR verification tests for generated closed
  declarations, requirement ownership, exact signature maps, call targets,
  effects, and complete-object metadata.
- [ ] Add mutation tests that inject undeclared/mismatched closed interface IDs
  or forbidden template provenance below resolution and require rejection.
- [ ] Emit distinct deterministic witness metadata and symbols for distinct
  applications; allow one concrete method address to satisfy multiple exact
  entries.
- [ ] Verify x86-64 receiver/result ABI, dynamic lookup, casts/tests, shared
  handles, runtime trace identity, and unchanged runtime ABI version.
- [ ] Add native goldens for ordinary/generic classes, inherited overrides,
  multiple applications, bound calls, shared owners, structural calls, and
  dynamic failure.

**Tests:** Focused HIR and MIR dumps; static-effect and lifecycle planning;
MIR declaration/call/conformance mutation corpus; backend dispatch, object ABI,
layout, symbol, cast, and shared-owner tests; assembly snapshots where useful;
new `tests/golden/generic_interfaces/` positive, negative, and checked-failure
specs; exact stdout/stderr/exit observations; focused full-determinism golden
filter; `make compiler-test`,
`make golden-filter GOLDEN_FILTER='generic_interfaces/**'`, `make runtime-test`, and
`git diff --check`.

**Exit criteria:** Representative programs compile and execute through
verified MIR with ordinary exact witnesses, malformed lower products are
rejected independently, target output is deterministic, and runtime ABI
version and surface are unchanged.

### I10 — Harden modules, diagnostics, dumps, robustness, and determinism

**Purpose:** Make failures and inspectable products stable across large module
graphs, malformed source, recursive specialization, and independent processes.

- [ ] Audit every generic-interface diagnostic for one primary cause, precise
  application span, relevant template/requirement note, nested obligation
  path, and absence of duplicate cascades.
- [ ] Complete public/private, selective import, qualified import, alias,
  cyclic module, shadowing, and application-site argument lookup coverage.
- [ ] Stabilize syntax, resolved, HIR, MIR, assembly, metadata, and diagnostic
  dumps for mixed generic classes/interfaces and alternate module orderings.
- [ ] Add cross-process pipeline determinism cases containing nested
  applications, multiple conformances, bounds, mutual recursion, cached
  failure, casts/tests, and witness dispatch.
- [ ] Extend frontend robustness generation to mutate interface parameter and
  argument lists, nested closers, `where` applications, generic `implements`,
  requirement signatures, casts, and tests.
- [ ] Audit specialization and template modules for focused ownership, bounded
  functions, reusable test builders, and absence of new resolver god objects;
  split only demonstrated mixed responsibilities.
- [ ] Document debugging and testing inspection points for interface template
  identities, closed keys, requirement mappings, conformance, and witness
  metadata.
- [ ] Run focused determinism and extended robustness gates and retain every
  minimized discovered defect as an owner-local regression.

**Tests:** Module graph and provider permutation tests; exact diagnostic
goldens for every frozen rejection family; syntax/resolved/HIR/MIR dump
snapshots; `pipeline_determinism`; generic-interface golden determinism;
bounded and long robustness; `make compiler-test`, `make golden-determinism-test`,
`make robustness-long`, `make msrv-check`, `make docs-check`, and
`git diff --check`.

**Exit criteria:** Module behavior, failures, phase products, assembly, and
native observation are reproducible across independent processes and input
order permutations; malformed input never panics; remaining implementation
owners have cohesive responsibilities.

### I11 — Complete the conformance matrix and close the feature

**Purpose:** Audit the entire frozen contract, fill cross-feature coverage
gaps, promote implemented documentation, and close from an artifact-free
repository state.

- [ ] Build a traceability matrix from every language/compiler contract rule
  and exclusion to owner-local or golden tests; add missing positive, negative,
  boundary, and determinism cases rather than relying on prose assertions.
- [ ] Cover all five primitives as interface arguments, exact classes,
  inheritance, shared exact/interface/`Obj`, bare interface alias-only uses,
  optionals, arrays, nested generic classes/interfaces, multiple parameters,
  functions in signatures, and marker interfaces.
- [ ] Cover ordinary/generic implementing classes, exact mismatches, multiple
  applications, inherited conformance, overrides, generic bounds, views,
  casts, tests, structural calls, produced results, ownership, cleanup, and
  checked failure in complete source-to-native programs.
- [ ] Prove every initial exclusion remains rejected or unaffected, especially
  primitives as conforming subjects, operators, iteration, generic methods,
  variance, interface inheritance/defaults, structural conformance, method
  references, erasure, and runtime dictionaries.
- [ ] Audit public phase facades and substantial changed files/functions by
  responsibility; resolve high-priority maintainability issues and record any
  bounded lower-priority follow-up in an indexed discoveries document.
- [ ] Update `GRAMMAR.md`, language/compiler status, overview, architecture,
  phase, testing, debugging, backend, and runtime ABI documentation to describe
  only implemented behavior and remove stale rollout language from living
  docs.
- [ ] Remove roadmap codes from living tests, comments, diagnostics, and docs;
  retain them only in the historical roadmap.
- [ ] Run the full ordinary and extended repository gates from an artifact-free
  snapshot and inspect final diff/status hygiene.
- [ ] Mark the roadmap complete, move it and the frozen implementation history
  to the archive, update both indexes and every incoming link, and leave no
  unindexed actionable discovery.

**Tests:** The completed traceability matrix plus all focused suites; exact
generic-interface golden specs in default and full determinism modes;
`make check`, `make golden-determinism-test`, `make msrv-check`,
`make robustness-long`, `make docs-check`, and `git diff --check` from an
artifact-free snapshot.

**Exit criteria:** Every frozen rule and exclusion has executable evidence,
all supported combinations pass source-to-native execution, every repository
gate passes, living documentation describes the implemented feature without
milestone vocabulary, and the roadmap is archived with no unresolved work in
scope.

## Ordering and dependencies

I0 establishes source shape before semantic products depend on it. I1 then
creates honest identities and migrates shared parameter ownership before I2
builds interface template semantics. I3 threads structural applications
through claims and bounds, providing all request sources needed by I4's
coordinated specialization owner. I5 materializes complete ordinary
interfaces; conformance and calls must not consume them before that boundary.

I6 establishes exact conformance maps before I7 closes bound-selected calls.
I8 broadens the closed identity through existing object and ownership
consumers. I9 is deliberately later: lower phases should see only stable
ordinary products and therefore require integration and verification, not a
second generic design. I10 hardens observability and malformed/large-graph
behavior after the semantic paths exist. I11 audits the frozen matrix and
closes documentation and archives only after every focused owner has already
landed its own tests.

The implemented generic-class specialization, ordinary interface conformance
and dispatch, module system, shared ownership, object casts, optionals, arrays,
structural indexing, verified MIR, x86-64 backend, runtime ABI version 9, and
golden runner are prerequisites. No other active roadmap blocks I0.
