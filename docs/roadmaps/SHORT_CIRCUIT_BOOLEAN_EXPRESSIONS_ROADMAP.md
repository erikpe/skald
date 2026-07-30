# Short-Circuit Boolean Expressions Roadmap

Status: in progress; SC0 is complete and SC1 is next.

This roadmap implements Skald's frozen exact-boolean `&&` and `||` profile.
The feature is complete only when arbitrary currently valid `bool`-producing
operands work through every expression consumer, including operands with
calls, mutation, allocation, ownership, optional unwrap, checked casts, array
access, panics, and full-expression cleanup.

The roadmap deliberately establishes path-dependent lifetime representation
before enabling source selection. A truth-table implementation over literals
or other pure primitive expressions is not an intermediate definition of the
feature and must not be advertised as support for `&&` or `||`.

The governing language contract is
[Short-circuit logical expressions](../language/FUNCTIONS_AND_CONTROL_FLOW.md#short-circuit-logical-expressions).
The target-independent representation contract is the
[frozen primitive operator representation](../compiler/PHASES_AND_IR.md#frozen-primitive-operator-representation),
and the source precedence is the
[frozen primitive operator expression extension](../language/GRAMMAR.md#frozen-primitive-operator-expression-extension).

## Scope and invariants

### Operator selection and source shape

- `left && right` and `left || right` require both operands to have the exact
  static type `bool` and produce a canonical `bool`.
- Type checking inserts no cast, promotion, narrowing, truthiness conversion,
  or expected-type reinterpretation.
- `&&` binds more tightly than `||`; both chains associate left to right.
  Comparison and contextual `is` expressions bind more tightly than `&&`.
- `&&` and `||` are distinct longest-match tokens. This roadmap does not add
  eager single-`&` or single-`|` boolean operators.
- AST and resolved IR retain source operator identity and operand structure.
  Typed HIR selects a dedicated structured logical operation, not an eager
  binary scalar operation.
- Both operands are type-checked in source order so diagnostics in an
  unreachable-at-runtime right operand are not suppressed.

### Evaluation and observable effects

- The left operand evaluates exactly once.
- For `&&`, a false left result skips the right operand and supplies `false`;
  otherwise the right operand evaluates exactly once and supplies the result.
- For `||`, a true left result skips the right operand and supplies `true`;
  otherwise the right operand evaluates exactly once and supplies the result.
- A skipped operand performs no call, mutation, allocation, construction,
  destruction, retain, release, optional unwrap, checked cast, bounds check,
  external operation, output, user panic, or compiler-known failure.
- Evaluation remains left to right when logical expressions are nested in
  eager unary or binary operations, calls, receivers, indices, assignments,
  and other larger expressions.
- A failure in the left operand always occurs before logical selection. A
  failure in the right operand is reachable only on the selected right path.
  A non-returning failure does not acquire an ordinary join successor.
- Panic and compiler-known failure remain non-unwinding. Once reporting
  begins, no remaining Skald cleanup is guaranteed; the conditional cleanup
  plan applies only to paths that reach the enclosing full-expression
  boundary.
- Constant folding and later optimization are not required by this roadmap.
  Any later transformation must retain exactly the same effects, failure
  reachability, ownership, cleanup, and evaluation count.

### Full-expression lifetime and cleanup

- A temporary completed while evaluating the left operand remains live while
  an evaluated right operand and the rest of the enclosing full expression
  execute.
- A temporary completed only on the right path becomes live only on that path
  and remains live until the enclosing full-expression boundary.
- The skipped path neither creates nor cleans a right-only temporary.
- All temporaries completed on the selected path are cleaned at the enclosing
  full-expression boundary in reverse completion order. Logical operands do
  not introduce nested full-expression boundaries.
- The selected scalar result is secured before cleanup can invalidate any
  storage, owner, view, or anchor used to compute it.
- Inline objects, class optionals, shared owners, optional shared owners,
  arrays, array anchors, checked object views, hidden receiver and argument
  anchors, and scalar spill storage retain their existing lifetime contracts.
- Bounded immediate-consumer optional payload views and their guards still
  end after the complete immediate consumer. They are not promoted to the
  enclosing logical expression's full-expression lifetime.
- When the logical expression is an `if`, `elif`, or `while` condition,
  selected-path full-expression cleanup completes before either successor,
  before the body, and before the next loop condition attempt.
- A return secures its result before full-expression cleanup and the existing
  lexical cleanup sequence.

### Representation boundary

MIR continues to use ordinary branches and jumps and has no eager logical
scalar opcode. One explicit scalar result carrier is live before each logical
split, is assigned on both local paths, and is read only after those paths
join. The short path writes the operator's fixed canonical result; the right
path writes the evaluated right result.

Skald's current ownership and lifetime verifiers require compatible state at
ordinary joins. A simple Niflheim-style branch, write, and join is therefore
insufficient: the right path may have completed resources that the short path
never created. Cleaning those resources at the logical join would violate the
language's full-expression lifetime, while pretending they are live on the
short path would permit invalid cleanup.

SC0 and SC1 settle a compact, explicit path-condition and cleanup
representation equivalent to the following model:

- a deterministic activation identity records whether a logical right path
  executed;
- each activation is defined on every local path where its parent path is
  active and is observed only under that parent condition;
- a completed resource registration carries the path condition under which
  the resource exists;
- nested and mixed logical expressions may form nested or sibling activation
  conditions without cloning the general continuation;
- verification distinguishes unconditional, conditionally live, and dead
  state and rejects an unrepresented state mismatch; and
- the full-expression boundary expands the conditional cleanup plan into
  ordinary verified control flow, cleaning only active registrations in
  reverse completion order before reconverging after the boundary.

Exact Rust names, whether the internal carrier is described as an activation,
guard, predicate, or path condition, and the final block layout remain private
implementation details. The settled behavior must not require a runtime
symbol, exception mechanism, public ABI change, unrestricted continuation
cloning, or early operand cleanup.

The representation must support path relations such as `(a && b) || c`, where
both `b` and `c` can execute and complete temporaries, as well as
`a && (b || c)`, where the inner decision exists only when the outer right path
is active. A child activation may be read only after its parent has selected
the region in which the child was defined.

### Verification boundary

Verification must reject at least:

- a non-`bool` logical operand or result;
- a noncanonical fixed short-path result;
- a logical result carrier not live before the split;
- a result carrier missing an assignment on either selected path;
- a use of the result before all returning paths define it;
- an eagerly emitted, multiply evaluated, or short-path-reachable right
  operand;
- an activation missing a definition on a path where its parent is active;
- an activation read outside its parent condition or after its boundary;
- a conditionally live resource registered under the wrong activation;
- cleanup of a skipped-path resource;
- loss or early cleanup of a selected-path resource;
- cleanup outside reverse completion order on any executable path;
- incompatible ownership, storage, optional, checked-view, guard, or anchor
  state hidden behind an ordinary join;
- a bounded immediate-consumer guard extended to the full-expression
  boundary;
- a returning failure block or a failure reachable from a skipped edge; and
- a pass that changes any of these facts.

### Source activation policy

Lexer and parser work may land before downstream activation, but type checking
must keep logical operator selection behind one focused unsupported-feature
boundary until every current path-dependent resource family is represented
and verified. The gate is removed only in SC7, with arbitrary operand and
consumer matrices passing end to end. Until then, the language status remains
unimplemented.

## Non-goals

- Eager evaluation of `&&` or `||`, even as an optimization or temporary
  implementation.
- Truthiness, implicit boolean conversion, or expansion of the explicit cast
  matrix.
- Single-`&` or single-`|` operators, exponentiation, floating operators,
  object operators, user-defined operators, comparison chaining, or changes
  to contextual `is`.
- New optional-value semantics, ownership semantics, checked-cast semantics,
  array semantics, panic behavior, exception handling, closures, or async
  control flow.
- SSA, phi nodes, a general expression-result block-argument system, or a
  general cleanup IR beyond what the short-circuit contract requires.
- Constant folding, branch simplification, dead-code elimination, or a
  guarantee about target branch layout.
- New runtime functions, runtime metadata, ABI version changes, or
  architecture-independent promises about x86-64 instruction sequences.

## Progress

- [x] SC0 — Represent path conditions in verified MIR
- [ ] SC1 — Plan conditional full-expression cleanup
- [ ] SC2 — Lower structured logical HIR to selected boolean control flow
- [ ] SC3 — Preserve path-dependent object and optional-object lifetimes
- [ ] SC4 — Preserve path-dependent shared and array ownership
- [ ] SC5 — Compose bounded views, guards, failures, and enclosing control flow
- [ ] SC6 — Add source syntax and exact type selection behind the completion gate
- [ ] SC7 — Enable arbitrary valid operands through every expression consumer
- [ ] SC8 — Harden, document, and promote short-circuit expressions

## PR-sized implementation sequence

### SC0 — Represent path conditions in verified MIR

**Purpose:** Give MIR and its verifiers an explicit vocabulary for state that
exists on only some returning paths, without yet automating cleanup or changing
the accepted source language.

- [x] Add a target-independent, callable-owned path-condition identity with
      deterministic allocation, dump order, parent relation, and source
      provenance sufficient for diagnostics.
- [x] Define activation only along paths where its parent condition is active,
      while requiring both the local short and right paths to record their
      selected state before joining.
- [x] Add an explicit verified conditional-state form at pre-boundary joins;
      do not relax ordinary join equality or silently union live resources.
- [x] Represent unconditional, conditionally live, and dead resource state
      without treating conditional liveness as ordinary liveness on every
      predecessor.
- [x] Keep activation scalar storage target-independent, canonical,
      definition-before-use checked, and scoped to its enclosing
      full-expression epoch.
- [x] Extend block and storage-lifetime verification with focused
      path-condition checks and diagnostics. Provide reusable path-sensitive
      state for later cleanup and ownership verifiers without relaxing those
      verifiers before their dedicated tasks.
- [x] Teach MIR dumps and debug displays to expose conditions and conditional
      registrations deterministically without leaking pointer identity.
- [x] Exercise the representation with hand-built MIR containing
      unconditional, selected, skipped, nested, and sibling conditional
      states plus explicit cleanup branches before lowering constructs them.
- [x] Keep existing non-conditional joins strict and preserve all current
      ownership, loop/backedge, storage-epoch, and unreachable-block checks.

**Tests:** Direct MIR fixtures for one split, nested parent/child conditions,
sibling conditions that can both be active, unconditional resources mixed
with conditional resources, explicit active and inactive cleanup branches,
and post-boundary convergence; verifier mutations for undefined or
noncanonical activation state, wrong parent, child read outside its parent,
missing local definition, an ordinary mismatched join disguised as conditional
state, lost resources, cleanup under the wrong condition, and activation
leakage; deterministic MIR dumps; existing ownership and loop/backedge suites;
`cargo test --locked -p skald-compiler`, `make check`, and
`git diff --check`.

**Exit criteria:** Verified MIR can honestly express resources that exist only
on selected returning paths, including nested and simultaneously active
conditions, and rejects malformed condition relationships without weakening
ordinary join invariants. Cleanup construction and source syntax remain
unchanged.

### SC1 — Plan conditional full-expression cleanup

**Purpose:** Make the existing full-expression owner produce correct cleanup
for conditional registrations, independently of logical expression lowering.

- [ ] Extend the full-expression tracker from one traversal-global live list
      to ordered registrations that distinguish unconditional resources from
      resources conditional on a path condition.
- [ ] Preserve actual completion order independently of activation nesting so
      cleanup traverses registrations in reverse and ignores inactive ones.
- [ ] Build a deterministic cleanup decision graph at the full-expression
      boundary rather than cloning the general continuation.
- [ ] Test parent conditions before conditionally defined children and allow
      sibling conditions to be active together.
- [ ] Secure scalar results and later unconditional temporaries before entering
      the cleanup graph, then reconverge only after all path-dependent lifetime
      state is compatible.
- [ ] Keep result and activation storage live until the cleanup graph has
      consumed it, then emit the existing end-of-full-expression and reverse
      storage-death sequence.
- [ ] Extend cleanup-order, storage-lifetime, ownership, and control-flow
      verification for generated conditional cleanup and its final
      convergence.
- [ ] Keep x86-64 and the runtime unaware of conditional cleanup semantics:
      target lowering receives ordinary verified storage, branches, cleanup
      actions, and jumps.
- [ ] Add reusable internal test builders for path conditions and conditional
      cleanup without exposing roadmap-specific concepts in public facades.

**Tests:** Builder-driven MIR cases for no active registration, one selected
registration, nested conditions, siblings that are independently or jointly
active, unconditional registrations before and after conditional ones,
multiple resources on one condition, reverse completion order on every path,
result securing, storage death, and post-boundary convergence; destructor or
cleanup trace observations where current MIR fixtures support them; verifier
mutations for skipped cleanup, selected cleanup loss, early or duplicate
cleanup, wrong order, child testing before parent, storage death before
cleanup, and incompatible final joins; deterministic MIR/assembly dumps;
system assembler acceptance; `cargo test --locked -p skald-compiler`,
`make check`, and `git diff --check`.

**Exit criteria:** The full-expression owner can automatically turn explicit
conditional registrations into a verified, deterministic cleanup graph that
cleans exactly the active resources in reverse completion order and converges
with ordinary compatible state. No logical HIR or source syntax exists yet.

### SC2 — Lower structured logical HIR to selected boolean control flow

**Purpose:** Prove exact evaluation and result selection over the new path
model before ownership-heavy source operands depend on it.

- [ ] Add a dedicated typed HIR logical-expression node and exact `And`/`Or`
      semantic operator. Do not add the operators to eager binary operation
      families in HIR or MIR.
- [ ] Require `bool` operands and a `bool` result when constructing or
      validating the HIR node, while keeping source selection disabled.
- [ ] Mark logical HIR as expression-level control flow so enclosing eager
      operations spill block-local values before lowering it and reload only
      after its join.
- [ ] Lower the left operand once, create deterministic short, right, and join
      blocks, and establish a result carrier before the split.
- [ ] Write `false` on the `&&` short path and `true` on the `||` short path;
      evaluate the right operand once only on the selected right block and
      write its canonical result.
- [ ] Associate right-operand completion with the logical activation while
      leaving left-operand completion under the enclosing condition.
- [ ] Support recursively nested and mixed logical HIR on either side without
      treating an operand join as a full-expression boundary.
- [ ] Return the selected scalar to any larger expression while preserving the
      caller's current block, control-effect accounting, scalar spills, and
      one enclosing cleanup boundary.
- [ ] Extend HIR and MIR verification and dumps for structured logical shape,
      selected result, block reachability, and activation association.
- [ ] Keep backend lowering generic and preserve the runtime and ABI boundary.

**Tests:** Direct HIR fixtures for both truth tables; left-associated chains;
grouped right nesting; mixed `(a && b) || c` and `a && (b || c)`; use under
logical negation, equality, calls, and other eager consumers; call-based
evaluation count and order; scalar spill/reload around both operands; result
carrier liveness and canonicality; malformed HIR/MIR mutations for eager RHS,
wrong fixed result, wrong branch target, duplicate evaluation, missing store,
and use before join; deterministic HIR/MIR dumps; source syntax still rejected
at the completion gate; `make check` and `git diff --check`.

**Exit criteria:** Internal typed HIR can lower arbitrary nested logical
control flow to one verified canonical result with exact skip behavior and
correct enclosing-expression composition, while source programs still cannot
select the incomplete feature.

### SC3 — Preserve path-dependent object and optional-object lifetimes

**Purpose:** Apply selected-path completion to owned inline resources and
their storage before moving to shared and anchored ownership.

- [ ] Carry the current inline-object full-expression temporary through
      unconditional and conditional completion registrations.
- [ ] Carry class-optional storage, presence state, payload initialization,
      payload cleanup, and absent-state handling without initializing or
      cleaning a skipped right operand.
- [ ] Preserve construction, direct/static/instance call results, receiver
      temporaries, value arguments, explicit copy arguments, field access, and
      object-result destinations when they appear on either logical path.
- [ ] Keep left-completed objects alive through an evaluated right operand and
      through later consumers in the same full expression.
- [ ] Secure a boolean result derived from an object or optional-object
      receiver before selected-path object cleanup.
- [ ] Preserve reverse completion order when left, right, and later enclosing
      expression components all complete objects.
- [ ] Extend cleanup-order, initialization, storage-epoch, and object-result
      verification for conditional state instead of weakening existing
      invariants.
- [ ] Cover destructors with visible effects so skip behavior, lifetime
      extent, and cleanup ordering are native observations rather than only
      dump properties.

**Tests:** Internal HIR-to-native cases using constructors, object-returning
calls, boolean fields, instance/static methods returning `bool`, value and copy
arguments, receiver chains, present and absent class optionals, and
destructor-side-effect logs on left, selected right, skipped right, and later
consumer paths; nested logical expressions with multiple active object
registrations; mutations for uninitialized optional payload, skipped
destruction, selected destruction loss, early destruction, duplicate
destruction, result use after storage death, and incompatible join state;
deterministic MIR and assembly observations; `make check`,
`make msrv-check`, and `git diff --check`.

**Exit criteria:** Every existing inline-object and class-optional completion
path can participate in logical evaluation with correct selected-path lifetime
and observable reverse cleanup, while source activation remains gated.

### SC4 — Preserve path-dependent shared and array ownership

**Purpose:** Complete conditional ownership for retained resources, backing
anchors, and aggregates whose cleanup is not a single inline destruction.

- [ ] Carry shared-owner retain, release, call-result, receiver, argument, and
      full-expression temporary state under path conditions.
- [ ] Carry optional shared-owner unwrap and its secured resulting owner;
      skipping the unwrap must perform no presence check, retain, or release.
- [ ] Carry arrays, element initialization state, array cleanup, and hidden
      array anchors under conditional completion.
- [ ] Preserve checked object-place casts and their backing shared owners or
      anchors for the existing consuming full-expression lifetime.
- [ ] Preserve boolean field, method, type-test, presence-test, and array
      element results obtained through shared, optional-shared, array, or
      checked-place paths.
- [ ] Ensure view end precedes the matching anchor release and that nested
      selected paths cannot release an owner still needed by a later consumer.
- [ ] Extend shared-ownership, array, anchor, checked-view, storage, and cleanup
      verifiers to reason about explicit conditional state.
- [ ] Keep retain/release, array cleanup, and anchor operations absent from
      every skipped path and ordered deterministically on selected paths.

**Tests:** Internal HIR-to-native matrices for shared-returning calls,
shared-backed receivers, retained alias/copy/value arguments, optional shared
unwrap, checked casts, type tests, arrays of supported values, boolean array
elements, array-producing calls, and nested/mixed logical expressions;
visible lifetime logs and reference-count-sensitive cases; mutations for
skipped retain/release, lost selected owner, release before view end,
double-release, anchor leak, wrong condition, array cleanup mismatch, and
ordinary incompatible joins; deterministic ownership/anchor dumps and native
results; `make check`, `make msrv-check`, and `git diff --check`.

**Exit criteria:** Every existing shared-owner, optional-shared, array, anchor,
and checked-place ownership path has verified selected-path lifetime and
cleanup, with no operation invented for a skipped operand.

### SC5 — Compose bounded views, guards, failures, and enclosing control flow

**Purpose:** Close the subtle lifetime and termination boundaries that do not
follow ordinary full-expression temporary rules.

- [ ] Keep inline-class optional payload views and presence guards bounded to
      their complete immediate consumer inside a logical operand.
- [ ] Keep primitive optional unwrap bounded through its copied result, while
      preserving any ordinary temporary established by the larger consumer.
- [ ] Ensure optional shared unwrap follows the secured-owner behavior covered
      by SC4 rather than the bounded inline-view behavior.
- [ ] Compose logical lowering inside receiver evaluation, call arguments,
      field and array places, indices, assignments, unary/eager binary
      operands, comparisons, type tests, and presence tests without duplicating
      or moving effects.
- [ ] Compose optional-unwrap absence, checked-cast failure, array-bounds
      failure, allocation failure paths already represented by the compiler,
      explicit panic, and calls that do not return.
- [ ] Prove skipped failure paths are unreachable and contain no speculative
      guard, check, ownership, anchor, or cleanup operations.
- [ ] Finish a logical condition's selected-path cleanup before both `if` and
      `elif` successors and before a `while` body, exit, or next condition
      attempt.
- [ ] Preserve loop/backedge epoch equality after conditional cleanup and
      prevent path activations or result spill state from escaping their
      full-expression boundary.
- [ ] Preserve return-result securing, full-expression cleanup, and lexical
      cleanup order when a logical expression supplies a return value.
- [ ] Extend all affected verifier domains and pass-preservation checks with
      malformed nested, unreachable, and terminating CFG cases.

**Tests:** Internal HIR-to-native cases for present/absent optional boolean
unwrap, inline optional payload method/field consumers, shared optional
unwrap, checked casts, bounds checks, explicit and called panics, logical
expressions in every larger expression form, `if`/`elif`/`while`, loop
re-entry, and returns; skipped-side-effect and skipped-panic observations;
left failure, selected-right failure after earlier selected-path completion,
and nested terminating paths, with no unwinding cleanup expected; mutations
for guard leakage, view lifetime extension, guard ended before its consumer,
failure with an ordinary successor, skipped-edge reachability, cleanup after
a condition successor, and unequal backedge state; full validation gate.

**Exit criteria:** Logical HIR composes with every current bounded lifetime,
failure, condition, loop, and return boundary without changing their existing
contracts. The only missing layer is source construction and its diagnostic
surface.

### SC6 — Add source syntax and exact type selection behind the completion gate

**Purpose:** Establish the complete source and diagnostic shape without
prematurely advertising the feature.

- [ ] Add distinct longest-match `&&` and `||` tokens with precise spans and
      deterministic debug vocabulary.
- [ ] Insert left-associative logical-and and logical-or parser tiers at the
      frozen precedence, preserving comparison/contextual-`is`, grouping,
      prefix/postfix `!`, recovery, UTF-8 offsets, and nesting limits.
- [ ] Preserve operator identity and source grouping through AST and resolved
      IR without constructing eager binary semantics.
- [ ] Add focused recovery for missing right operands and malformed adjacent
      punctuation including lone `&`, lone `|`, `&&&`, and `|||`.
- [ ] Type-check both operands in source order and require exact `bool` on
      each; diagnostics name the operator and both actual types without
      suggesting a cast or truthiness conversion.
- [ ] Retain the checked operator and operand information needed for one
      focused downstream-completion diagnostic, but do not emit logical HIR
      during ordinary compilation until SC7 removes the gate.
- [ ] Add deterministic token, AST, resolved, and gated-diagnostic
      observations for precedence, associativity, grouping, nesting, and
      invalid types.
- [ ] Keep living status and implemented-feature documentation unchanged;
      grammar and frozen-design text already specify the eventual language.

**Tests:** Lexer matrices for `&`, `|`, `&&`, `||`, longer malformed runs,
adjacent punctuation, comments, whitespace, and Unicode surroundings; parser
and recovery cases for all precedence boundaries and missing operands;
left-associative and grouped AST/resolved dumps; exact-type matrices covering
all primitive, optional, class, array, shared, view, and `unit` types;
independent diagnostic accumulation and process determinism; downstream gate
diagnostic; existing eager `!`, equality, comparison, and contextual `is`
regressions; `make check`, `make msrv-check`, and `git diff --check`.

**Exit criteria:** Source syntax has one deterministic parse and exact type
selection with complete diagnostics, but no program can bypass the explicit
completion gate and claim partially supported logical operators.

### SC7 — Enable arbitrary valid operands through every expression consumer

**Purpose:** Remove the gate only when the feature works for the whole
currently valid expression language, not a privileged primitive subset.

- [ ] Remove the downstream-completion gate and connect exact type selection
      to the structured HIR and verified MIR paths established by SC0–SC5.
- [ ] Cover literals, bindings, grouping, prefix negation, boolean equality,
      contextual type and presence tests, primitive optional unwrap, and
      boolean fields and array elements.
- [ ] Cover direct, static, instance, interface, and external calls returning
      `bool`, including receivers and arguments that allocate, retain, copy,
      unwrap, cast, index, mutate, or complete temporaries.
- [ ] Cover nested and mixed logical expressions on both operands with
      observable left-to-right, exactly-once behavior and all combinations of
      selected and skipped paths.
- [ ] Cover local initialization, primitive reassignment, boolean field
      assignment, function/method/initializer/external arguments, receiver and
      index subexpressions, return, and `if`/`elif`/`while` conditions.
- [ ] Add side-effect probes for mutation, calls, output, allocation,
      construction, destruction, retain/release, optional checks, casts,
      bounds checks, and panic reachability on both selected and skipped
      paths.
- [ ] Add lifetime probes proving left temporaries survive the right operand,
      right temporaries survive later full-expression consumers, inactive
      resources never become live, and active resources clean in reverse
      completion order.
- [ ] Confirm canonical boolean results across storage, calls, returns,
      external output, branching, loops, and native ABI boundaries.
- [ ] Add source-to-native and compile-failure goldens rather than relying only
      on unit-level IR fixtures.
- [ ] Update the status matrix and implementation-facing language, phase, IR,
      backend, testing, and debugging documentation in the same change that
      removes the gate.

**Tests:** Complete source-to-native truth tables and precedence matrices;
arbitrary-operand and every-consumer matrices; visible skipped-side-effect,
evaluation-order, lifetime, and cleanup-order logs; selected and skipped
failure matrices; nested/mixed logical chains; repeated loop conditions;
canonical results through all ABI consumers; invalid exact-type combinations;
token/AST/resolved/HIR/MIR/assembly goldens; system assembler acceptance;
independent-process determinism; `make check`, `make msrv-check`,
`make robustness-long`, and `git diff --check`.

**Exit criteria:** `&&` and `||` are implemented source-to-native for every
currently valid `bool` operand and expression consumer, including selected-path
effects, failures, ownership, and cleanup. Only at this point may living
documentation call the feature implemented.

### SC8 — Harden, document, and promote short-circuit expressions

**Purpose:** Freeze the implementation boundary as a durable compiler feature
and remove roadmap-only scaffolding.

- [ ] Audit complete valid, invalid, nesting, consumer, failure, ownership,
      cleanup, loop, return, and determinism matrices against the frozen
      contracts.
- [ ] Add adversarial verifier mutations for every invariant listed in this
      roadmap and confirm transformations preserve conditional state.
- [ ] Confirm no pure-operand shortcut, eager logical rvalue, early cleanup,
      general-continuation cloning, runtime helper, ABI change, or
      target-specific logical semantics entered the implementation.
- [ ] Stress deeply nested and long mixed chains within the syntax budget and
      verify compiler work, CFG growth, cleanup decision growth, and diagnostic
      output remain bounded and deterministic.
- [ ] Audit touched lexer, syntax, AST, resolution, type checking, HIR, MIR,
      full-expression tracking, each verifier domain, passes, backend, dumps,
      facades, tests, and documentation by responsibility.
- [ ] Resolve small maintainability findings directly. Record material
      out-of-scope findings in
      `docs/roadmaps/SHORT_CIRCUIT_BOOLEAN_EXPRESSIONS_DISCOVERIES.md` and
      index that file under pending discoveries.
- [ ] Remove the temporary completion gate, rollout-only flags, task codes,
      and roadmap terminology from living code, diagnostics, fixtures, and
      general documentation.
- [ ] Confirm public runtime headers, archives, ABI version, panic catalog, and
      generated artifacts are unchanged unless a separately approved
      correction documents otherwise.
- [ ] Run the artifact-free final validation gate, repair documentation links,
      mark this roadmap complete, move it to `docs/archive/`, and update active
      and archive indexes.

**Tests:** Full logical source, IR, verifier-mutation, native, ownership,
optional, checked-cast, array, panic, loop, return, and determinism suites;
syntax-budget and robustness stress cases; documentation and link checks;
`cargo test --locked --workspace`, `make check`, `make msrv-check`,
`make robustness-long`, `git diff --check`, and an artifact-free final
repository-state check.

**Exit criteria:** Short-circuit boolean expressions are a deterministic,
fully documented source-to-native feature over the entire currently valid
operand language; all selected-path lifetime and cleanup invariants are
verified; no rollout scaffolding remains; and the completed roadmap is
archived.

## Cross-cutting test matrix

Each implementation task adds focused tests, but SC7 and SC8 must audit the
cross product below rather than treating the categories as isolated examples.

| Dimension | Required coverage |
| --- | --- |
| Operator/result | `&&`, `||`, all four truth-table rows, canonical stored and returned `bool` |
| Shape | single expression, left chain, grouped right nesting, mixed operators, logical expression on either operand |
| Operand producer | literal, binding, field, array element, optional unwrap, equality, `is`, direct/static/instance/interface/external call |
| Operand context | local initialization, assignment, field store, receiver, index, argument, eager operand, return, `if`, `elif`, `while` |
| Effect | mutation, output, allocation, construction, destruction, retain/release, copy, unwrap, cast, bounds check, explicit/called panic |
| Lifetime | unconditional left, selected right, skipped right, later enclosing temporary, object, optional object, shared owner, array, view, guard, anchor, scalar spill |
| Termination | returning paths, left failure, selected-right failure, skipped-right failure, loop re-entry, return cleanup |
| Observation | diagnostic, token/AST/resolved/HIR/MIR dump, verifier mutation, assembly legality, native side-effect and cleanup log |

Pairwise coverage is acceptable where a literal Cartesian product would add
no new compiler path, but every row must include both operators, selected and
skipped right paths, nesting, and at least one source-to-native observation.
Any family excluded because it cannot currently produce `bool` must be named
in the test audit rather than silently omitted.

## Niflheim comparison

The roadmap was checked against sibling Niflheim commit `3dcd543`. Niflheim
provides useful precedent for:

- distinct `&&` and `||` tokens;
- separate logical-or and logical-and parser tiers;
- a semantic boolean-logical operation distinct from eager arithmetic; and
- backend branch, short-result, right-result, and join blocks writing one
  destination.

Its boolean golden tests also demonstrate skipped invalid or effectful right
calls. Skald adopts those source and selected-result principles, but cannot
copy the simple join as its complete implementation. Skald's verified MIR has
deterministic full-expression object, shared-owner, optional, array,
checked-view, guard, anchor, and storage state. The additional SC0–SC5 work is
required so a selected right path may retain resources to the enclosing
boundary while the short path owns none.

## Ordering and dependencies

SC0 makes path-dependent state explicit instead of weakening join
verification. SC1 independently establishes conditional cleanup planning.
SC2 proves control flow and selected-result construction using internal HIR.
SC3 through SC5 migrate every current cleanup and failure family while source
activation remains gated. SC6 adds the source surface and exact diagnostics.
SC7 removes the gate only after the arbitrary-operand and every-consumer
source-to-native matrix passes. SC8 performs robustness, maintainability,
documentation, and archive closeout.

This roadmap depends on the completed eager boolean, conditional-control-flow,
while-loop, optional-value, object-lifecycle, shared-ownership, checked-cast,
array, panic, and primitive operation foundations; block-local MIR values;
scalar spills; deterministic phase dumps; and the frozen logical-expression
contract.

It does not depend on the deferred cast matrix, future operators, optimizer
work, exceptions, closures, async control flow, a new runtime service, or an
ABI revision.

## Roadmap maintenance and closeout

- Mark a checkbox only when its implementation and named tests pass.
- Keep the status line and progress list synchronized with task completion.
- Do not broaden an active task for a material discovery. Record it in the
  discoveries file named in SC8 and index it under pending discoveries.
- A task may refine private Rust names or block layout, but any change to the
  frozen language or compiler representation contract requires design review
  before implementation continues.
- The feature remains unimplemented in living status documentation until SC7
  meets its exit criteria.
- After SC8, move this file to `docs/archive/`, repair relative links, update
  both roadmap indexes, and ensure no active discovery entry remains
  unresolved.
