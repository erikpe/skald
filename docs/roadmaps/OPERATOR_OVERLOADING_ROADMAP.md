# Interface-Based Operator Overloading Roadmap

Status: **in progress**. OO0 is complete; OO1 is next.

This roadmap implements the frozen
[language contract](../language/OPERATOR_OVERLOADING.md) and
[compiler lowering contract](../compiler/OPERATOR_OVERLOADING.md). The
[archived design record](../archive/OPERATOR_OVERLOADING_DESIGN_PROPOSAL.md)
preserves rejected alternatives and rationale; this roadmap owns delivery
order and acceptance and does not reopen those decisions.

## Scope and invariants

- The complete dependency-free `std::ops` bundle is ordinary standard-library
  source with compiler-validated canonical identities. Explicit protocol
  references or selecting `std::ops` as the entry make the bundle reachable;
  operator punctuation never adds an implicit module dependency.
- Existing exact primitive operations always win before protocol selection and
  preserve their current types, checked failures, wrapping behavior, eager or
  short-circuit behavior, and IEEE-754 semantics.
- Every non-primitive overload selects exactly one effective canonical
  application from the static left type or definition-site generic bounds.
  RHS compatibility is ordinary read-only alias compatibility; no expected
  result, conversion, specificity, inheritance-depth, or specialization-time
  ranking participates.
- Source punctuation is erased before completed HIR. A class realization is an
  ordinary `HirExpressionKind::InterfaceCall`; a primitive realization is an
  existing primitive HIR operation. No operator-specific MIR, backend
  instruction, witness kind, runtime service, allocation, or ABI revision is
  introduced.
- Primitive protocol evidence is a closed compiler-owned static mapping. It can
  satisfy only canonical operator bounds and never turns a primitive into an
  object, interface view, witness, cast target, or user-replaceable
  implementation.
- Generic operator meaning is fixed at template definition. Specialization may
  realize that selection as an ordinary class witness call or primitive
  intrinsic, but may not search the concrete type for another meaning.
- Overloaded `!=` invokes `OpEq.op_eq` exactly once and negates its exact `bool`
  result. All four ordering predicates are direct. Prefix `!`, `&&`, and `||`
  remain non-overloadable.
- Ordinary methods remain non-overloaded. Existing conformance rules continue
  to reject incompatible multiple applications; operator syntax creates no
  hidden method-overload set.
- Read-only primitive `ref` parameters may borrow a checked produced primitive
  expression through one caller-owned scalar temporary. Existing places still
  borrow directly and `mut ref` remains place-only.
- Each task updates the living grammar, status, language, compiler, standard-
  library, and test documentation for only the behavior it actually delivers.
  Basic positive, negative, ownership, and determinism coverage lands with the
  owning task rather than being deferred to final hardening.
- Range types, new range syntax, iterator protocols, compound assignment,
  increment/decrement, conversion operators, truthiness, general method
  overloading, associated types, implicit numeric conversion, and new
  primitive operations are out of scope.

The implemented generic-interface, ordinary interface-call, object-view,
ownership, full-expression cleanup, primitive-operation, and general-iteration
pipelines are the baseline. No other planned feature blocks OO0.

## Progress

- [x] OO0 — Produced primitive read-only alias materialization
- [ ] OO1 — Canonical `std::ops` bundle and identity validation
- [ ] OO2 — Value-producing operator selection and HIR erasure
- [ ] OO3 — Typed equality, ordering, and complete operator surface
- [ ] OO4 — Receiver, ownership, evaluation, and effect integration
- [ ] OO5 — Compiler-provided primitive protocol evidence
- [ ] OO6 — Generic definition-site selection and specialization
- [ ] OO7 — Diagnostics, verification, and determinism hardening
- [ ] OO8 — Documentation, compatibility matrix, and release closure

## PR-sized implementation sequence

### OO0 — Produced primitive read-only alias materialization

**Purpose:** Land the independently useful ordinary-call prerequisite before
operator lowering depends on it.

- [x] Extend primitive alias argument checking so any successfully checked
  produced scalar expression of the exact parameter type can satisfy a
  read-only primitive `ref` parameter.
- [x] Keep bindings, static fields, and groupings around existing places on the
  current direct `HirPrimitivePlace` path without creating a temporary.
- [x] Add one explicit HIR call-argument form for produced primitive alias
  storage, retaining the checked value, exact primitive type, source span, and
  read-only access contract.
- [x] Lower that form into one caller-owned MIR argument storage initialized at
  the argument's ordinary left-to-right evaluation position and borrowed until
  the call result is secured.
- [x] End the storage exactly once at the enclosing full-expression boundary in
  reverse cleanup order; prove no mutation, escape, early end, double end, or
  use before initialization.
- [x] Continue to reject every produced expression for `mut ref`, even when the
  expression is a literal or otherwise assignable to the parameter type.
- [x] Route ordinary direct, method, interface, indirect, constructor, and
  intrinsic-call argument checking through the same capability rather than
  adding an operator-only exception.

**Primary implementation areas:**
`typeck/expression/alias.rs`, `hir/ir/expression.rs`, `hir/dump.rs`,
`mir/lower/call.rs`, full-expression storage tracking, MIR call/lifetime
verification, and alias goldens.

**Tests:** Literals and produced arithmetic, comparison, call, and grouped
expressions for all five primitive types; existing binding/static direct
borrows; nested and multiple alias arguments proving evaluation order; calls
that return owning and primitive results; discard and failure cleanup; every
produced `mut ref` rejection; HIR/MIR mutation tests for missing initialization,
early storage end, mutation, escape, double end, and wrong type; native alias
goldens and independent-process determinism.

**Gates:** Focused compiler and alias goldens, `make compiler-test`,
`make golden-filter GOLDEN_FILTER='aliases/**'`, `make docs-check`,
`make msrv-check`, and `git diff --check`.

**Exit criteria:** Every ordinary read-only primitive alias parameter accepts an
exact produced scalar through verified bounded temporary storage, existing
places retain their direct path, and `mut ref` still requires a mutable place.

### OO1 — Canonical `std::ops` bundle and identity validation

**Purpose:** Establish the source-defined protocol vocabulary and one trusted
compiler product before punctuation can select it.

- [ ] Add dependency-free `std/std/ops.ska` containing exactly the frozen
  public interface templates, type parameters, requirement names, receiver
  mutability, parameter modes/types, and result types.
- [ ] Collect evidence only when an ordinary explicit import, qualified
  reference, signature/bound/claim use, or direct `std::ops` entry already
  loads the module; do not add a `CompilerDependencyKind` for operator tokens.
- [ ] Validate the reachable module once as one complete bundle after module
  provider resolution and before expression type checking.
- [ ] Record exact `InterfaceTemplateId` and
  `InterfaceTemplateRequirementId` values in a resolved operator-language-item
  product; make later consumers use identities rather than path or spelling
  lookup.
- [ ] Diagnose missing, duplicate, private, wrong-kind, wrong-arity,
  wrong-parameter-order, bounded, malformed-receiver, wrong-mode, wrong-type,
  wrong-result, and extra/missing-requirement declarations at their owning
  spans in stable canonical order.
- [ ] Support replacement standard libraries with the same exact contract and
  preserve ordinary module cycle/provider diagnostics ahead of bundle errors.
- [ ] Prove that primitive-only programs with operator punctuation still work
  under `--no-stdlib` and create no `std::ops` graph edge.
- [ ] Permit explicit ordinary class implementations, bounds, imports, and
  manual method calls against the declarations while punctuation remains gated
  to the currently implemented primitive profile until OO2.

**Primary implementation areas:** `std/std/ops.ska`, resolved program language-
item tables/dumps, `resolve/resolver/program`, module graph evidence tests,
standard-library replacement fixtures, and standard-library documentation.

**Tests:** Exact identity and source-order independence; direct and selective
imports; qualified references; complete valid replacement bundle; one focused
failure for every canonical field plus multiple simultaneous defects proving
stable ordering; missing/ambiguous providers and cycles; unused unreachable
malformed `std::ops`; direct `std::ops` entry; `--no-stdlib` primitive programs;
ordinary conformance and manual call execution with operator punctuation still
rejected for classes.

**Gates:** Focused resolve/module/standard-library tests,
`make golden-filter GOLDEN_FILTER='standard_library/**'`,
`make compiler-test`, `make docs-check`, `make msrv-check`, and
`git diff --check`.

**Exit criteria:** Every reachable canonical bundle has one fully validated
identity product, malformed replacements fail deterministically, explicit
protocol use works as ordinary generic-interface source, and punctuation has
not created hidden reachability.

### OO2 — Value-producing operator selection and HIR erasure

**Purpose:** Deliver the first end-to-end overloaded punctuation slice for
unary and binary value-producing protocols while preserving primitive
precedence.

- [ ] Centralize the frozen mapping from unary `-`, unary `~`, arithmetic,
  remainder, bitwise, and shift syntax to canonical template/requirement
  identities; retain source operator and operand spans independently from
  interface method spelling.
- [ ] Check the current exact primitive matrix first and preserve its existing
  HIR operation without requiring `std::ops` reachability.
- [ ] Otherwise enumerate effective exact applications from an exact class,
  inherited class conformance, specialized generic class, or exact canonical
  operator-interface view and deduplicate identical applications.
- [ ] Filter binary candidates solely through ordinary read-only alias
  applicability for the static RHS source and require exactly one remaining
  application without expected-result or specificity ranking.
- [ ] Diagnose unsupported and ambiguous selections separately with ordered
  application origins and both operand types/spans.
- [ ] Type the expression from the selected `Output` structural term and apply
  the existing result-capability checks for its actual consumption context.
- [ ] Erase a class realization to the existing exact
  `HirExpressionKind::InterfaceCall`; retain existing primitive operations for
  primitive realizations and reject any unresolved selection before HIR
  completion.
- [ ] Preserve ordinary method visibility, conformance, inheritance, and
  incompatible-multiple-application diagnostics; private or same-named
  structural methods never authorize punctuation.

**Primary implementation areas:** resolved expression/operator selection
records, generic-interface application lookup, `typeck/expression.rs` and a
focused operator-selection facade, `typeck/expression/call.rs`, HIR dumps, and
operator goldens.

**Tests:** `MyStr + MyStr`; every value-producing protocol; exact, inherited,
closed-generic, and exact-interface-view receivers; class and primitive output
types; unsupported operands; absent explicit import/reachability; same-named
nonconforming methods; inaccessible implementations; duplicate identical
claims versus incompatible/multiple applications; exact versus view-compatible
RHS ambiguity; expected-result nonselection; explicit-cast selection; unchanged
primitive HIR dumps and primitive `--no-stdlib` execution.

**Gates:** Focused resolver/type-check/operator tests,
`make golden-filter GOLDEN_FILTER='operators/**'`, `make compiler-test`,
`make docs-check`, `make msrv-check`, and `git diff --check`.

**Exit criteria:** Every supported value-producing class operator selects one
canonical application and reaches HIR only as an ordinary interface call,
while every exact primitive expression retains its pre-feature semantics and
representation.

### OO3 — Typed equality, ordering, and complete operator surface

**Purpose:** Complete source-visible protocol coverage with the frozen typed
predicate model and prove that non-overloadable logical syntax remains closed.

- [ ] Add `==` and `!=` selection through `OpEq<Rhs>` with exact `bool` result;
  lower overloaded `!=` to one secured `op_eq` result followed by the existing
  exact boolean negation.
- [ ] Add independent direct selection for `OpLess`, `OpLessEq`, `OpGreater`,
  and `OpGreaterEq`; do not derive either greater predicate from a reversed or
  negated lesser predicate.
- [ ] Reuse OO2's candidate enumeration, RHS alias compatibility,
  deduplication, ambiguity, and static-left eligibility without adding
  predicate-specific ranking.
- [ ] Keep exact primitive equality and ordering on their existing path,
  including IEEE-754 NaN, zero, infinity, and comparison-result behavior for
  `f64`.
- [ ] Keep `Equatable.equals(ref Obj)` a separate explicit dynamic comparison
  API with no satisfaction, selection, or fallback relation to `OpEq`.
- [ ] Explicitly retain exact-`bool` prefix `!`, short-circuit `&&` and `||`,
  eager primitive boolean bitwise operators, and all other excluded syntax
  without protocol lookup.
- [ ] Cover the entire frozen punctuation-to-protocol table from one exhaustive
  compiler mapping so a future syntax addition cannot silently acquire an
  overload meaning.

**Primary implementation areas:** comparison and boolean type checking,
operator selection mapping, HIR boolean/unary composition, primitive operation
tables, resolved/HIR dumps, and comparison goldens.

**Tests:** Class equality and one-call `!=` effect counting; heterogeneous
`Rhs`; all four direct orderings with intentionally non-complementary methods;
ambiguous and unsupported predicates; subclass/base and interface RHS views;
`Equatable`-only and `OpEq`-only classes; exact primitive parity for integers,
booleans, all `f64` special forms and `BoxF64.equals`; compile failures proving
`!`, `&&`, and `||` never consult user protocols; exhaustive mapping unit test.

**Gates:** Focused comparison/type-check tests,
`make golden-filter GOLDEN_FILTER='operators/**'`, `make compiler-test`,
`make docs-check`, `make msrv-check`, and `git diff --check`.

**Exit criteria:** The entire frozen non-generic operator surface is selectable,
`!=` performs exactly one equality call, all orderings are direct, primitive
comparisons remain identical to direct syntax, and excluded logical operators
remain non-overloadable.

### OO4 — Receiver, ownership, evaluation, and effect integration

**Purpose:** Make protocol punctuation exactly call-equivalent across Skald's
complete object, argument, result, lifetime, and dispatch matrix.

- [ ] Reuse ordinary receiver checking for locals, fields, statics, `self`,
  aliases, produced exact-class values, checked views, explicit dereference,
  explicit optional unwrap, and exact interface views; reject raw shared,
  unrelated interface, `Obj`, optional, array, and function left types.
- [ ] Preserve direct, inherited, virtual override, and interface witness
  dispatch, including ordinary access and shared-anchor behavior.
- [ ] Secure the left receiver once, then evaluate and secure the RHS once,
  bind its read-only alias, issue one call, secure the result, and clean up in
  reverse full-expression order. Unary calls secure one receiver once.
- [ ] Exercise direct RHS places, OO0 produced primitive storage, class/view
  aliases, produced exact-class carriers, inheritance projections, and checked
  view anchors through the existing call argument machinery.
- [ ] Exercise every supported primitive, class, shared, optional, array,
  function, and specialized-generic `Output` capability through assignment,
  nesting, argument passing, discard, return, and failure paths.
- [ ] Preserve ordinary target-directed copies, moves/adoption, owner retention,
  alias lifetime, produced receiver destruction, call-result securing, panic
  traces, and reverse cleanup without operator-specific ownership rules.
- [ ] Feed interface-call targets into existing static-effect, reachable-target,
  body-retention, devirtualization, and artifact-order owners exactly as an
  equivalent explicit call.

**Primary implementation areas:** receiver and object-view type checking,
alias/call checking, HIR interface-call construction, MIR call and
full-expression lowering, lifetime verification, static effects, reachability,
and ownership/native goldens.

**Tests:** Exact/inherited/override/interface receivers in every supported
carrier; produced and effectful left/right expressions; explicit shared and
optional crossings versus implicit-crossing failures; class/interface/Obj RHS
view compatibility and ambiguity; output-family matrix; call-body failure and
cleanup counters; self-aliasing; receiver-before-RHS and one-evaluation proofs;
panic trace and static-effect equivalence between punctuation and explicit
calls; native direct/virtual/interface dispatch.

**Gates:** Focused call/receiver/lifetime/static-effect tests, representative
operator and ownership goldens, `make compiler-test`, `make golden-test`,
`make docs-check`, `make msrv-check`, and `git diff --check`.

**Exit criteria:** Every accepted non-generic overloaded operator is
observationally equivalent to its one selected ordinary interface call in
evaluation, dispatch, effects, result ownership, failure, and cleanup.

### OO5 — Compiler-provided primitive protocol evidence

**Purpose:** Represent supported primitive operations as canonical static bound
evidence without changing the object model or direct primitive expressions.

- [ ] Add one closed declarative registry keyed by receiver primitive type and
  exact canonical closed operator application, with an existing
  target-independent primitive semantic operation as each value.
- [ ] Generate or mechanically validate the registry against the frozen
  operator table, validated canonical identities, and implemented primitive
  matrix so missing, extra, duplicate, wrong-result, and unsupported cells
  cannot drift silently.
- [ ] Extend exact generic-bound satisfaction to accept registry evidence only
  for canonical operator templates; retain exact-class nominal satisfaction
  for every other interface bound.
- [ ] Store primitive evidence outside resolved object/interface conformance,
  witness metadata, complete-object layouts, cast/test relations, shared
  ownership, reflection, and runtime-visible tables.
- [ ] Prohibit source declarations from replacing, supplementing, or creating
  primitive applications and keep direct primitive member syntax invalid.
- [ ] Preserve exact direct-operation semantics for every mapped cell,
  including checked integer division/remainder, shift legality, wrapping
  arithmetic, comparison, and IEEE-754 behavior.
- [ ] Expose stable resolved dumps for canonical primitive evidence and focused
  validation failures without changing HIR for direct primitive expressions.

**Primary implementation areas:** canonical operator language-item product,
primitive operation definitions, generic bound-satisfaction queries,
specialization requirement diagnostics, resolved dumps, and primitive/operator
tests.

**Tests:** Complete table-driven positive matrix for all five primitives and
every supported exact operation; unsupported cells such as `f64` remainder;
wrong RHS/output applications; canonical versus same-named foreign interface;
unused primitive-satisfied bounds; non-operator primitive bounds still failing;
no primitive interface view/cast/test/member call; no witness or layout entry;
registry mutation tests for missing/duplicate/wrong cells; stable dumps and
direct primitive parity.

**Gates:** Focused primitive/generic-requirement/resolve tests, operator and
generic-interface goldens, `make compiler-test`, `make docs-check`,
`make msrv-check`, and `git diff --check`.

**Exit criteria:** The compiler can prove exactly the supported canonical
primitive operator applications at compile time, no unsupported or unrelated
interface gains primitive satisfaction, and no runtime/object representation
has changed.

### OO6 — Generic definition-site selection and specialization

**Purpose:** Make canonical operator bounds useful inside generic bodies while
freezing meaning before concrete type arguments are known.

- [ ] Extend generic-template body analysis with source-shaped unary/binary
  operator selections retaining operator/operand spans, bounded parameter,
  exact structural protocol application, template requirement identity,
  structural `Rhs`/`Output`, and bound origin.
- [ ] Select from exact declared bounds at template definition, applying the
  same RHS alias compatibility and unranked zero/multiple-candidate rules as
  closed selection; diagnose ambiguity before any specialization request.
- [ ] Keep exact primitive syntax in a generic body unavailable unless the
  static structural operands and declared bounds authorize the operation.
- [ ] Close an already selected application to either an ordinary
  `ClassWitness { interface, requirement }` realization or OO5
  `PrimitiveIntrinsic { operation }` realization.
- [ ] Generate an existing HIR interface call for class specializations and an
  existing primitive HIR operation for primitive specializations; never leave
  a structural or protocol placeholder in completed HIR.
- [ ] Reuse the same realization for manual bound calls such as
  `left.op_add(right)`, including primitive specializations, while continuing
  to reject direct primitive member syntax outside bound-authorized templates.
- [ ] Preserve exact definition-module lookup, closed generic class
  conformances, inherited claims, requirement mapping, receiver/RHS aliases,
  result capabilities, static effects, and cleanup.
- [ ] Prove specialization never reselects based on concrete members,
  conformances, result context, or a more specific concrete RHS relation.

**Primary implementation areas:** generic-template expression analysis and
selection records, bound-member resolution, specialization closure and body
generation, primitive evidence realization, resolved/generated-body dumps,
type checking, and HIR construction.

**Tests:** The frozen `Adder<T> where T: OpAdd<T, T>` example with `u64` and a
class; distinct `Rhs` and `Output`; unary, algebraic, equality, and ordering
bounds; manual bound calls for class and primitive arguments; multiple
applicable bounds; missing and wrong applications; unsupported primitive cells;
foreign same-named protocols; definition-site shadowing; concrete types with
additional applications proving no reselection; class witness versus primitive
intrinsic dumps; produced literal arguments through OO0; result ownership,
failure, effects, native execution, and specialization-order determinism.

**Gates:** Focused generic-template/specialization/type-check tests,
`make golden-filter GOLDEN_FILTER='generic_interfaces/**'`, representative
operator goldens, `make compiler-test`, `make docs-check`, `make msrv-check`,
and `git diff --check`.

**Exit criteria:** Every accepted generic operator or manual canonical bound
call has one definition-site template requirement and closes deterministically
to an ordinary class witness call or existing primitive operation without
specialization-time semantic lookup.

### OO7 — Diagnostics, verification, and determinism hardening

**Purpose:** Audit every trust boundary and adversarial combination after the
complete semantic path exists, without adding a lower-level operator feature.

- [ ] Establish stable diagnostic precedence: module/provider failures,
  canonical-bundle validation, generic definition-site selection, closed
  selection/applicability, result capability, then ordinary call/lifecycle
  failures.
- [ ] Give malformed protocols, unsupported operands, ambiguous applications,
  RHS alias incompatibility, unsatisfied primitive bounds, invalid results, and
  internal mapping mismatches distinct diagnostics with exact ordered origins.
- [ ] Extend resolved dumps with canonical identities and selected primitive or
  protocol evidence; show only existing primitive operations or exact interface
  calls in HIR dumps.
- [ ] Add preliminary/final MIR mutation tests proving malformed injected
  calls, primitive operations, produced alias temporaries, cleanup, targets,
  and metadata are rejected by existing owners; add only narrowly missing
  invariants and no operator MIR node.
- [ ] Audit static effects, reachable targets, retained bodies, virtual and
  interface dispatch tables, panic traces, target legality, public symbols,
  runtime references, and assembly artifacts for explicit-call or intrinsic
  equivalence.
- [ ] Prove all declaration, candidate, bound, application, specialization,
  witness, diagnostic, dump, effect, target, and artifact order is independent
  of hash iteration, module import order, provider discovery, and process.
- [ ] Run bounded generative malformed-source and mutation coverage across
  operator chains, nested generics, aliases, comments, delimiters, and recovery.

**Primary implementation areas:** diagnostics catalog, resolve/HIR/MIR dumps,
MIR verifiers, static effects and target discovery, backend/artifact tests,
golden runner determinism, and generative robustness suites.

**Tests:** Reordered equivalent module graphs and declarations; multiple-error
precedence; malformed replacement bundles; ambiguous claims/bounds with stable
origin ordering; corrupted HIR/MIR realization and alias lifetime cases;
primitive/class generic artifact equivalence; runtime-symbol and ABI snapshot;
independent-process full-output determinism; bounded parser/resolver/type-check
mutations and deep but supported operator/generic nesting.

**Gates:** Focused verifier/diagnostic/dump tests, `make compiler-test`,
`make golden-test`, `make golden-determinism-test`, `make robustness-long`,
`make docs-check`, `make msrv-check`, and `git diff --check`.

**Exit criteria:** Invalid states fail at their owning boundary, valid states
reach only existing call or primitive machinery, observable output is
deterministic, and no operator-specific backend/runtime surface exists.

### OO8 — Documentation, compatibility matrix, and release closure

**Purpose:** Prove the frozen contract is completely delivered, remove staging
language, and leave one maintainable source of truth.

- [ ] Audit the canonical interface/operator/primitive table mechanically
  against standard-library declarations, compiler selection, primitive
  evidence, tests, and documented surface.
- [ ] Complete the receiver, RHS alias, output category, direct/inherited/
  interface dispatch, generic class/primitive, replacement-library,
  `--no-stdlib`, equality/ordering, failure, cleanup, and unsupported-case
  matrix without duplicating tests already owned by earlier tasks.
- [ ] Promote the grammar, status matrix, language index, compiler phases/IR,
  module-system, generic-interface, alias, standard-library, and testing docs to
  implemented wording with exact cross-links and examples.
- [ ] Confirm prefix `!`, `&&`, `||`, `Equatable`, ordinary method overloading,
  unsupported primitive cells, implicit shared/optional crossings, direct
  primitive members, and out-of-scope range/iteration work remain unchanged.
- [ ] Record any real actionable follow-up discovered during delivery in a
  separate indexed discovery document rather than expanding this roadmap.
- [ ] Run the complete ordinary and extended repository gates, update progress
  after each task's actual merge, mark the feature implemented only when all
  acceptance criteria pass, then archive this completed roadmap and update both
  roadmap indexes.

**Primary implementation areas:** language/compiler/status/index docs,
standard-library and golden READMEs, cross-layer table tests, roadmap indexes,
and release validation.

**Tests and gates:** Complete `make check`, `make check-long`, explicit
`git diff --check`, documentation link/index validation, a clean independent
rerun of representative class and generic native examples, and comparison of
public runtime ABI/symbol artifacts with the pre-feature baseline.

**Exit criteria:** The complete frozen operator-overloading contract is
implemented, documented, deterministic, native-tested, ABI-neutral, and free
of staging gates; no roadmap checkbox remains open and the completed roadmap is
archived.

## Ordering and dependencies

```text
OO0 produced primitive ref storage
  \
   +--> OO2 class value operators --> OO3 predicates/full surface --> OO4 lifecycle
  /                                                                    |
OO1 canonical std::ops identities -------------------------------------+
                                                                       |
                                                                       v
OO5 primitive evidence --> OO6 generic selection/specialization --> OO7 hardening --> OO8 closure
```

- OO0 and OO1 are independent and may be implemented in either order, but both
  must be complete before OO2 consumes arbitrary protocol RHS expressions.
- OO2 establishes the one selector and HIR-erasure boundary. OO3 extends that
  boundary to predicates; OO4 proves the complete call-equivalence matrix.
- OO5 depends on OO1's trusted identities and the existing primitive matrix,
  but not on generic operator syntax. It must precede OO6.
- OO6 depends on OO0–OO5 because generic class and primitive realizations must
  share the already proven closed selection, alias, call, and intrinsic paths.
- OO7 audits the complete implementation. OO8 alone promotes the feature from
  staged implementation to the fully implemented language status.

Safe internal refactors that preserve these boundaries may land inside their
owning task. A change that alters the frozen language contract, adds lower IR
or runtime machinery, or expands scope requires a separately reviewed design
revision rather than an untracked roadmap adjustment.
