# General Iteration Implementation Roadmap

Status: in progress. `IT1` is complete; `IT2` is next.

This roadmap implements the frozen
[general-iteration language contract](../language/ITERATION.md) and
[compiler contract](../compiler/ITERATION.md). The confirmed rationale and
alternatives remain in the
[design proposal](GENERAL_ITERATION_DESIGN_PROPOSAL.md). Those living contracts
own semantics; this document owns delivery order, progress, and discoveries.

## Goal

Deliver nominal state-based iteration through
`std::iter::Iterable<Item, State>` and the statement
`for (item [: ItemType] in expression) { ... }`. A conforming value is
evaluated and secured once, one hidden state is initialized once, each attempt
calls `iter_next` once, outer optional absence terminates, and every entered
iteration owns a fresh item. The structured source loop lowers to ordinary
verified cyclic MIR and native x86-64 code without a dedicated iterator MIR
operation, target primitive, allocation requirement, runtime symbol, or ABI
revision.

## Implemented baseline and dependencies

The roadmap starts from these implemented contracts:

- generic interfaces close applications and generic bounds to ordinary exact
  interface and requirement identities before HIR;
- ordinary, inherited, specialized generic, and bound-selected interface
  dispatch reaches verified MIR and native execution;
- optional values preserve nested outer presence and own produced call
  results, payload extraction, and cleanup;
- read-only exact, interface, produced, checked, shared, and array receivers
  already have owner, guard, and anchor vocabulary;
- stored-value capability and lifecycle plans cover primitive, class, array,
  optional, and shared-owner values;
- `while`, `break`, and `continue` provide callable-local `LoopId`, structured
  HIR control effects, repeatable MIR storage epochs, loop contexts, cleanup
  depths, verified cyclic CFG, and native backend realization; and
- `std::vec::Vec<T>` is ordinary generic standard-library source with indexed
  access and optional-backed storage.

Generic interfaces are therefore a completed prerequisite, not a task in this
roadmap. Operator overloading, primitive `ref` materialization, `Range<T>`,
`..`, generators, and compiler-provided primitive or array conformances are
not prerequisites. They must not expand this roadmap.

## Frozen invariants

Every task preserves these cross-cutting decisions:

1. The protocol is the exact public generic interface
   `std::iter::Iterable<Item, State>` with read-only `iter_state() -> State`
   and `iter_next(mut ref state: State) -> Item?`.
2. Selection is nominal. One exact closed application must remain after an
   optional exact item-type filter; same-named methods and implicit conversions
   do not participate.
3. Generic-bound selection is fixed at definition-site checking and survives
   specialization by identity substitution, not reselection.
4. The iterable expression is evaluated once. One read-only interface view and
   every owner, guard, or anchor needed by it remain live for the loop.
5. `iter_state` runs once. `iter_next` runs once per attempted iteration and
   never after the first absent outer result.
6. Outer absence terminates. Outer presence yields one exact `Item`, including
   a genuinely absent optional item when `Item` is itself optional.
7. State, result, item, body, and receiver cleanup is explicit and exact on
   normal, `continue`, `break`, and return paths. Panic remains non-unwinding.
8. `break` and `continue` reuse nearest-loop identity across `while` and
   `for-in`; definite return conservatively keeps a termination fallthrough.
9. Source, resolved IR, and HIR retain a dedicated `for-in` structure. MIR and
   every backend see only ordinary calls, optionals, lifetimes, cleanup, and
   cyclic CFG.
10. Initial `Vec<T>` adoption is ordinary source. No collection, range,
    operator, optimizer, target, or runtime special case defines iteration.

## Progress

| Task | Status | Outcome |
|---|---|---|
| [IT0](#it0--canonical-iterable-language-item) | [x] Complete | Canonical dependency-free `std::iter::Iterable<Item, State>` is loadable, validated, and represented by exact identities. |
| [IT1](#it1--for-in-source-syntax) | [x] Complete | Lexer, parser, AST, recovery, template scanning, and dumps retain the frozen statement shape. |
| [IT2](#it2--nominal-protocol-selection-and-loop-scopes) | [ ] Planned | Exact claims and generic bounds select deterministic protocol evidence before the typed item scope and loop body resolve. |
| [IT3](#it3--structured-hir-and-core-lifecycle-plans) | [ ] Planned | Typed HIR owns `Item`, `State`, receiver, state, result, item, effects, and cleanup plans. |
| [IT4](#it4--ordinary-mir-cfg-and-loop-exits) | [ ] Planned | Core iteration lowers to verified ordinary MIR with exact call order, storage epochs, and exit cleanup. |
| [IT5](#it5--loop-duration-receiver-composition) | [ ] Planned | Every frozen exact, produced, polymorphic, checked, shared, optional, and array receiver family remains valid for the loop duration. |
| [IT6](#it6--complete-item-state-and-nested-optional-matrix) | [ ] Planned | All admitted stored-value families and genuine optional items execute with exact lifecycle. |
| [IT7](#it7--ordinary-vec-adoption) | [ ] Planned | `Vec<T>` implements `Iterable<T, u64>` in ordinary source and composes with generic consumers. |
| [IT8](#it8--verification-diagnostics-and-determinism-hardening) | [ ] Planned | Negative verifier evidence, stable diagnostics, dumps, and reordered-declaration determinism close trust gaps. |
| [IT9](#it9--native-boundary-and-release-closure) | [ ] Planned | Native evidence, unchanged runtime/ABI checks, full gates, living status, and archival complete delivery. |

Only one task should be marked in progress at a time. Check an item only after
its tests and exit condition pass. Discoveries belong in the final section and
do not silently expand an active task.

## IT0 — Canonical Iterable language item

Purpose: establish the dependency-free source declaration and validated exact
identity boundary before syntax can request protocol selection.

Primary implementation areas:

- `std/std/iter.ska`;
- standard-library module acquisition in `module`/`driver` request loading;
- `crates/skald-compiler/src/resolve/resolver/program/` language-item
  validation and program assembly;
- resolved program metadata and deterministic dumps; and
- focused resolution/module tests.

Checklist:

- [x] Add exactly the frozen public generic interface declaration in
  `std::iter`, with no imports or dependency cycle.
- [x] Add typed `GeneralIteration` compiler-dependency evidence to module-graph
  edges; `IT1` attaches parsed `for-in` spans and acquires `std::iter`
  without creating a source binding; preserve ordinary provider ambiguity and
  missing-module diagnostics.
- [x] Validate module path, public visibility, interface kind, generic arity,
  direct requirement set, names, receiver access, parameter count/mode,
  substituted `State`/`Item?` signatures, and absence of incompatible extras.
- [x] Store exact template and requirement identities in a focused resolved
  language-item record; request ordinary closed applications through the
  existing specialization coordinator.
- [x] Keep all registries request-local and source identities deterministic.
- [x] Diagnose missing and ambiguous canonical modules through ordinary module
  lookup, and diagnose private, wrong-kind, wrong-arity, malformed, shadow, and
  incompatible declarations at canonical dependency evidence. Explicit
  imports, direct `std::iter` entry, and `IT1` source syntax provide evidence
  through the typed dependency hook.
- [x] Expose stable resolved dump evidence without making source spelling a
  lower-phase discriminator.

Tests:

- standard-library source parse and dependency-cycle coverage;
- module-graph tests for typed compiler-dependency evidence and canonical path
  mapping, explicitly imported reuse, missing roots, ambiguous providers, and
  same-named noncanonical interfaces; `IT1` covers source-to-evidence parser
  activation;
- resolver table/dump tests for exact generic and requirement identities;
- one mutation case per structural validation rule; and
- existing generic-interface specialization and standard-library tests.

Exit condition: a compiler request can deterministically obtain and validate
the canonical declaration and exact identities, while malformed alternatives
fail before HIR; no `for-in` statement is accepted yet.

Completion evidence: focused module-graph tests cover dependency kinds,
missing and ambiguous providers; resolver tests cover the canonical source,
exact metadata and dumps, ordinary closed specialization, noncanonical
lookalikes, and one mutation per structural rule. The standard-library fixture
includes the dependency-free module. Source activation remained correctly in
`IT1` and is now attached to parsed `for-in` keyword spans.

## IT1 — For-in source syntax

Purpose: retain the complete frozen source shape without guessing the item
type, protocol application, or body semantics.

Primary implementation areas:

- `crates/skald-compiler/src/lexer/` token and keyword tables;
- `crates/skald-compiler/src/syntax/ast.rs`, `parser/statement.rs`,
  `parser/recovery.rs`, generic-template nesting/scanning, and `syntax/dump.rs`;
- the resolver's explicit not-yet-supported statement gate; and
- syntax and recovery tests.

Checklist:

- [x] Reserve `for` and recognize `in` contextually only after the optional
  item annotation in a `for` header.
- [x] Parse `for (name in expression) block` and
  `for (name: storage-type in expression) block` with mandatory delimiters and
  body.
- [x] Retain binding, annotation, iterable, body, delimiter, and complete spans
  in a dedicated AST node.
- [x] Add recovery for missing binding, colon type, contextual `in`, iterable,
  parentheses, and block without swallowing later statements.
- [x] Update generic body discovery, logical-depth checks, statement-start
  inventories, exhaustive matches, and deterministic AST dumps.
- [x] Add an explicit resolver gate that diagnoses the parsed feature until
  canonical selection can determine an exact item type before body resolution.
  Do not add an untyped `ResolvedLocal` placeholder or resolve item member uses
  against guessed types.

Tests:

- positive syntax/dump cases for inferred and annotated bindings, nesting, and
  contextual `in` as an identifier outside the delimiter position;
- diagnostic/recovery cases for every delimiter and component;
- generic class/callable template-body discovery and specialization parsing;
- deterministic span and dump assertions.

Exit condition: syntax preserves the statement exactly and deterministically,
and resolution stops it with one intentional diagnostic rather than an
internal assertion or lossy rewrite.

Completion evidence: lexer and syntax tests cover the reserved/contextual word
policy, both header forms, nested and generic-template bodies, exact spans and
dumps, logical-depth enforcement, and recovery for every mandatory component.
Module-graph tests prove implicit canonical acquisition, explicit-edge reuse,
typed keyword-span evidence, and ordinary missing/ambiguous providers.
Resolution tests prove one selection-pending diagnostic for ordinary and
generic-template bodies without guessed bindings or secondary name errors.

## IT2 — Nominal protocol selection and loop scopes

Purpose: select one exact closed application and requirement pair for every
accepted loop, then resolve the body with its exact item type and loop identity.

Primary implementation areas:

- resolver generic-interface claim, inheritance, and bound inventories;
- resolved `for-in` evidence in `resolve/ir/body.rs`;
- specialization substitutions and generic-bound member mappings;
- type-checking interface lookup helpers; and
- focused selection/diagnostic tests.

Checklist:

- [ ] Enumerate only exact canonical `Iterable<Item, State>` applications
  reachable through the iterable static type's nominal claims, inherited
  claims, specialized generic claims, or exact generic bound.
- [ ] Canonicalize and sort candidates by exact identities before selection,
  diagnostics, or dumps.
- [ ] Infer `Item` and `State` when exactly one candidate exists.
- [ ] Apply an explicit item annotation as exact equality over `Item`; reject
  implicit conversion, unwrap, dereference, covariance, and best-match rules.
- [ ] Diagnose zero candidates, multiple candidates, annotation mismatch, and
  ambiguity remaining after filtering with declaration and use spans.
- [ ] Retain selected interface, `iter_state`, `iter_next`, `Item`, `State`,
  and bound-requirement identities in resolved evidence.
- [ ] Resolve the iterable expression in the enclosing scope and select the
  protocol from its statically determined ordinary type or generic template
  type term before resolving the body. Allocate one source-ordered
  callable-local `LoopId`; ordinary bodies receive an exact-typed item local,
  while generic templates retain a structurally typed binding that
  specialization materializes as an exact ordinary local. Do not add an
  unresolved inferred type variant to the ordinary local table.
- [ ] Reuse the active loop stack so nearest `break`/`continue` targets mixed
  `while`/`for-in` nesting correctly; keep the item out of the iterable and
  post-loop scopes and diagnose duplicate body-scope declarations.
- [ ] Freeze generic-bound selection at template definition-site checking and
  substitute its already selected identities during specialization.
- [ ] Prove that a concrete type's additional Iterable claims cannot change a
  generic body's selection.
- [ ] Reject structural same-named methods and lookalike interface identities.

Tests:

- direct, inherited, specialized generic, and interface-view claims;
- one claim, no claim, multiple distinct `Item`, same `Item` with distinct
  `State`, annotation resolving ambiguity, and annotation resolving nothing;
- generic bounds in generic-class methods, multiple bounds, nested
  specialization, and additional concrete claims;
- scope, shadowing, duplicate declaration, mixed-loop exit identity, and
  outside-loop rejection;
- declaration-order and module-order perturbation with identical selection and
  diagnostic ordering;
- same-named methods and same-spelled noncanonical interface negatives.

Exit condition: every semantically viable loop has one deterministic exact
selection record and an ordinary exact-typed body binding, all ambiguity is
rejected before HIR, and specialization cannot reselect the protocol.

## IT3 — Structured HIR and core lifecycle plans

Purpose: give type checking one explicit structured representation that owns
the protocol types, calls, receiver, state, result, item, and control effects.

Primary implementation areas:

- `crates/skald-compiler/src/hir/ir/body.rs`, HIR facades, and dump rendering;
- `crates/skald-compiler/src/typeck/function/statement.rs` plus a focused
  iteration module;
- stored-value, optional, interface receiver, and control-effect planning; and
- HIR/type-check tests.

Initial core matrix for this task is deliberately narrow: named stable exact
class and interface-view receivers with primitive `State` and primitive or
trivially copied exact-class `Item`. Later tasks complete every frozen family.

Checklist:

- [ ] Add `HirForIn` with exact loop/binding/interface/requirement identities,
  `Item`, `State`, `Item?`, typed iterable, receiver plan, call plans, state
  initialization, result/payload/item plans, body, spans, and effects.
- [ ] Type-check exact read-only receiver access for `iter_state` and
  `iter_next`, including exact mutable aliasing of the hidden state.
- [ ] Reuse stored-value capability analysis for `State` and `Item`; issue
  source diagnostics rather than lowering assertions for unsupported values.
- [ ] Reuse the canonical optional type table and one-layer payload extraction
  rather than constructing an ad hoc sentinel or flattened optional.
- [ ] Represent receiver acquisition once and distinguish its loop-duration
  lifetime from ordinary call-duration alias carriers.
- [ ] Type-check the body with an immutable owning item binding and ordinary
  fresh body scope.
- [ ] Summarize normal fallthrough, nearest-loop break/continue, outer exits,
  return, and divergence; always retain possible termination fallthrough for
  definite return.
- [ ] Extend every exhaustive HIR statement consumer, including cell-write and
  static-effect walkers, without hiding calls or lifecycle effects.
- [ ] Dump exact selected identities, types, receiver plan, and effects in
  stable order.

Tests:

- manually constructed HIR dump and identity invariants;
- inferred and annotated primitive item loops;
- direct and bound-selected interface dispatch metadata;
- state/item stored-value capability failures;
- immutable item assignment/mutable-alias rejection and item scope;
- conservative definite-return and mixed nested-loop effects;
- HIR exhaustive-consumer unit tests.

Exit condition: the core matrix reaches complete typed structured HIR with no
unresolved candidate or generic term and with explicit ownership/effect plans;
MIR lowering remains gated.

## IT4 — Ordinary MIR CFG and loop exits

Purpose: lower the core HIR matrix to the existing verified MIR vocabulary
with exact execution and cleanup order.

Primary implementation areas:

- `crates/skald-compiler/src/mir/lower/loop_flow.rs`, `statement.rs`,
  `loop_context.rs`, cleanup, full-expression, call, and optional lowering;
- MIR dump and verification coverage; and
- MIR/native loop fixtures for the core matrix.

Checklist:

- [ ] Allocate deterministic preheader, header, present/body, optional latch,
  outer-cleanup, and exit regions before emitting edges.
- [ ] Evaluate/acquire the receiver once, call `iter_state` once, and begin one
  state storage epoch before the header.
- [ ] In the header call `iter_next` once through a mutable alias to the exact
  state, finish the call result, and test only its outer presence.
- [ ] On presence, initialize a fresh item from the payload, consume/clean the
  result wrapper, then enter the body.
- [ ] Route normal completion and `continue` through iteration-scope cleanup
  and the latch; route `break` through outer loop cleanup to the exit.
- [ ] Compose return cleanup with body/item/result/state/receiver ownership and
  keep panic non-unwinding.
- [ ] Omit unreachable latches with the same effect criterion as `while` while
  retaining the conservative source fallthrough rule.
- [ ] Reuse or extract a private loop-CFG helper only where the `while` and
  iteration invariants truly coincide; keep protocol evaluation explicit.
- [ ] Introduce no dedicated MIR instruction, terminator, or model identity for
  iteration.
- [ ] Pass ordinary MIR verification for calls, aliases, optionals, path state,
  cleanup, and repeatable storage epochs.

Tests:

- zero, one, repeated, break, continue, return, nested, and mixed-loop CFG;
- call counters/effects proving one receiver evaluation, one state call, one
  next call per attempt, and no call after termination;
- MIR dumps proving ordinary operations and deterministic block ordering;
- cleanup traces for item/body before latch and state/receiver on outer exit;
- core verifier and native x86-64 cases;
- regression coverage for existing `while`, `break`, and `continue` lowering.

Exit condition: the core matrix executes through verified ordinary MIR and
native code with exact call/cleanup observations and no new backend/runtime
iteration concept.

## IT5 — Loop-duration receiver composition

Purpose: complete the frozen receiver matrix and prove that one retained
read-only view remains valid across arbitrary loop bodies.

Primary implementation areas:

- type-check receiver/view-source planning and HIR receiver carriers;
- MIR expression stabilization, interface views, shared owners, checked casts,
  optional guards, produced receivers, and array anchors;
- receiver-carrier and ownership tests.

Checklist:

- [ ] Cover named exact-class places and existing interface views.
- [ ] Cover inherited/base/interface projections while retaining complete-object
  provenance and exact dispatch.
- [ ] Cover compatible produced exact-class expressions with one hidden owning
  temporary spanning the whole loop.
- [ ] Cover named, produced, and replaceable shared-owner sources with a secured
  strong anchor that survives body mutation of the original owner place.
- [ ] Cover checked casts and optional-derived views with guards/anchors whose
  validity spans every iteration and outer cleanup edge.
- [ ] Cover inline/shared/optional array-backed receiver paths admitted by
  ordinary object-view rules, including detached backing anchors.
- [ ] Reject mutable/consuming receiver requirements and sources that cannot
  supply a safe loop-duration read-only interface view.
- [ ] Permit ordinary independent mutation by the body without adding snapshot
  or concurrent-modification rules.
- [ ] Prove receiver release occurs after state cleanup and exactly once on
  termination, break, and return.

Tests:

- one positive and one lifetime-negative case per receiver source family;
- produced-expression side-effect counter proving single evaluation;
- replaceable shared/optional roots mutated in the body while the anchored
  receiver remains valid;
- checked-view guard success/failure and nested loop cleanup;
- array backing replacement/detachment cases;
- virtual and interface dispatch across repeated attempts;
- ownership trace and MIR storage-lifetime assertions.

Exit condition: every receiver family admitted by the frozen contract has
explicit HIR/MIR ownership evidence and native lifetime tests, and unsafe or
unsupported sources fail before lowering.

## IT6 — Complete item, state, and nested-optional matrix

Purpose: compose iteration with every admitted ordinary stored-value family
and prove outer termination never collapses an optional item.

Primary implementation areas:

- stored-value and optional-result HIR planning;
- MIR optional, class, array, shared-owner, optional-box, copy/transfer, and
  cleanup lowering;
- lifecycle and native matrix fixtures.

Checklist:

- [ ] Cover primitive, exact-class, inline array, optional, shared-owner, and
  supported optional shared-owner states.
- [ ] Cover primitive, copy-capable exact-class, inline array, optional,
  shared-owner, and supported optional shared-owner items.
- [ ] Preserve ordinary `iter_next` result construction, caller-owned returned
  optional storage, payload transfer/copy, and result cleanup.
- [ ] Prove `Item = T?` selects result `T??`: outer `none` terminates,
  `some(none)` enters the body with an absent item, and `some(some(value))`
  enters with a present item.
- [ ] Begin a fresh owning item epoch on each entered iteration and end it on
  normal, continue, break, and return paths.
- [ ] Preserve copy capability and failure diagnostics from collection/protocol
  implementations rather than adding iteration-specific implicit copying.
- [ ] Exercise destructors and shared counts for state, result payload, item,
  and nested body locals in observable order.
- [ ] Reject unsupported recursive/storage categories before MIR.

Tests:

- a compact cross-product covering each state and item family without
  duplicating lower-level lifecycle suites;
- class destructor and copy counters for yielded and terminating attempts;
- arrays, shared owners, optional owners, and optional box results;
- explicit nested-optional sentinel distinction and multi-layer optionals;
- break/continue/return at multiple nesting depths;
- verifier/native trace coverage for balanced ownership and storage epochs.

Exit condition: every frozen state and item category composes through verified
MIR/native execution, nested optional termination is exact, and all cleanup
counts and order match ordinary value semantics.

## IT7 — Ordinary Vec adoption

Purpose: prove the protocol is practical and allocation-free for the primary
standard-library collection without compiler knowledge of vectors.

Primary implementation areas:

- `std/std/vec.ska` and imports from `std::iter`;
- standard-library/generic conformance tests;
- golden programs using concrete and generic iteration.

Checklist:

- [ ] Make `Vec<T>` claim `Iterable<T, u64>` in ordinary source.
- [ ] Implement `iter_state` as index-zero state initialization.
- [ ] Implement `iter_next` using ordinary length, checked storage access,
  exact item production, state increment, and outer optional termination.
- [ ] State and test the required `T` capabilities through existing generic
  contextual requirements; do not add compiler-only Vec privileges.
- [ ] Iterate empty, singleton, multi-element, nested, optional-element,
  class-element, array-element, and shared-owner-element vectors.
- [ ] Use a generic consumer bounded by `Iterable<Item, State>` and prove
  definition-site selection plus ordinary interface dispatch.
- [ ] Observe no iterator/shared allocation attributable to the loop mechanism
  for a primitive Vec loop; ordinary Vec/item behavior may still allocate.
- [ ] Preserve existing Vec indexing, slicing, copy, mutation, and lifecycle
  tests.

Tests:

- standard-library parse/type-check and no dependency cycle;
- focused Vec unit/golden cases for the complete admitted element matrix;
- generic consumer, inherited/specialized Vec use, break/continue/return;
- runtime trace/assembly statistics sufficient to show no mandatory iterator
  allocation or new symbol;
- all existing standard-library and Vec goldens.

Exit condition: `Vec<T>` is an ordinary conforming implementation, concrete
and generic loops execute across admitted element types, and no compiler path
branches on Vec identity.

## IT8 — Verification, diagnostics, and determinism hardening

Purpose: close malformed-IR and unstable-observation gaps after the complete
semantic matrix executes.

Primary implementation areas:

- resolver/type-check diagnostic catalogs and dump renderers;
- HIR invariants and exhaustive consumers;
- `crates/skald-compiler/src/mir/verify/` mutation tests;
- determinism and robustness suites.

Checklist:

- [ ] Stabilize diagnostics for canonical declaration failure, no candidate,
  ambiguity, annotation mismatch, invalid state/item capability, unsafe
  receiver, invalid scope, and malformed header.
- [ ] Ensure primary labels point at the actionable loop/interface component
  and secondary labels identify conflicting claims/declarations.
- [ ] Stabilize syntax, resolved, HIR, and MIR dumps with exact identities and
  source order; avoid unordered candidate/debug rendering.
- [ ] Audit every exhaustive statement/control-flow/static-effect/cell-write
  match introduced or affected by `HirForIn`.
- [ ] Mutate interface and requirement identities, Item/State/result types,
  state alias targets, optional presence layers, loop destinations, storage
  epochs, and cleanup edges; require verifier rejection or demonstrate the
  malformed state cannot be represented below HIR.
- [ ] Mutate receiver owner/guard/anchor lifetimes and duplicate/missing cleanup.
- [ ] Prove calls after termination and item use outside its epoch cannot pass
  final MIR verification.
- [ ] Run repeated and declaration/module-order-perturbed compilations and
  compare diagnostics, dumps, and assembly.
- [ ] Add depth/stress cases without introducing recursive parser/resolver or
  verifier behavior beyond repository limits.

Tests:

- diagnostic snapshots and recovery continuation;
- direct HIR/MIR construction and mutation negatives;
- dump and assembly determinism repetitions;
- reordered imports, claims, declarations, and generic specialization demand;
- deep mixed loops and ownership-heavy stress;
- full compiler unit suite.

Exit condition: every trust-boundary invariant has positive and negative
evidence, all user failures are deterministic source diagnostics, and repeated
or reordered equivalent inputs produce stable observable output.

## IT9 — Native boundary and release closure

Purpose: prove the frozen target/ABI boundary, update living maturity, run all
repository gates, and archive the completed delivery record.

Primary implementation areas:

- native/golden suites under `tests/golden/` and backend tests;
- runtime header/symbol/version assertions;
- living language/compiler docs and roadmap archive/indexes.

Checklist:

- [ ] Add end-to-end native programs for direct, inherited, generic-specialized,
  bound-selected, receiver-heavy, nested-optional, Vec, mixed-loop, and cleanup
  cases.
- [ ] Inspect representative MIR/assembly to confirm only ordinary calls,
  optional operations, branches, jumps, and cleanup cross the backend boundary.
- [ ] Assert the public runtime header, exported symbols, and ABI marker remain
  unchanged; add no iteration runtime harness.
- [ ] Run golden expectation and determinism checks for all new programs.
- [ ] Change the status matrix and focused living docs from frozen/not
  implemented to implemented only after every preceding exit condition passes.
- [ ] Remove the temporary resolution-gate wording from grammar and status
  documentation after selection and lowering are implemented.
- [ ] Remove stale future-language in loop, generic-interface, Vec, backend,
  and runtime docs while preserving explicit exclusions.
- [ ] Mark every roadmap task complete, record final evidence, move the design
  proposal and roadmap to `docs/archive/`, and update both indexes.

Tests and quality gates:

```text
make compiler-test
make cli-test
make docs-test
make golden-test
make golden-determinism-test
make runtime-test
make test
```

Run narrower crate/test filters during development, but the final task requires
the repository-owned aggregate gates above plus `git diff --check` and the
documentation checker. Record any environment-only unavailable gate and its
replacement evidence rather than claiming it passed.

Exit condition: the complete frozen contract is implemented, verified,
native, deterministic, documented as current behavior, and archived with no
operator/range scope added and no runtime ABI change.

## Ordering and pull-request boundaries

The intended dependency chain is:

```text
IT0 -> IT1 -> IT2 -> IT3 -> IT4 -> IT5 -> IT6 -> IT7 -> IT8 -> IT9
```

- `IT0` precedes selection so every later identity is canonical and ordinary.
- `IT1` precedes semantics so syntax and spans are complete before resolution;
  `IT2` then selects an exact item type before declaring the ordinary body
  local or resolving its uses.
- `IT2` precedes HIR so candidate sets and unresolved generic terms stop at the
  resolution/type-check boundary.
- `IT3` precedes MIR so lifetime and cleanup intent is explicit and reviewable.
- `IT4` establishes one small executable vertical slice before expanding the
  ownership matrix in `IT5` and `IT6`.
- `IT7` follows full composition so Vec remains a consumer rather than an
  accidental definition of the protocol.
- `IT8` hardens the complete shape, and `IT9` changes maturity only after all
  evidence exists.

Each task is intended as one reviewable pull request. If implementation reveals
that one task cannot remain PR-sized, split it in this roadmap before coding
the additional scope; do not combine adjacent purposes merely because their
files overlap.

## Deferred consumers

The following require their own design proposals and roadmaps:

- interface-based operator overloading and compiler-provided primitive
  operator conformances;
- primitive scalar materialization for read-only `ref` arguments if required
  by those operator interfaces;
- `Range<T>`, its successor/step and overflow policy, and its exact bounds;
- lexical and precedence rules for `start .. end` and any inclusive range;
- compiler-provided primitive or array Iterable applications;
- generators, resumable frames, shared iterator objects, adapters, borrowed
  items, mutable/consuming iteration, labels, and loop values; and
- devirtualization, inlining, vectorization, exact-size metadata, or range fast
  paths.

A future range roadmap may depend on the completed general-iteration contract.
It must not change the protocol, optional termination, loop cleanup, or MIR
boundary established here.

## Discoveries

No implementation discoveries are recorded yet.

When a task uncovers a fact that affects later work, append a dated entry with:

- the observed implementation fact and evidence;
- which later task or frozen invariant it affects;
- whether it is an implementation choice within the frozen contracts or a
  genuine design conflict; and
- the smallest proposed roadmap/doc update.

If a discovery conflicts with a frozen invariant, stop the active task and
promote that conflict for an explicit design decision. Do not silently revise
the language through code.
