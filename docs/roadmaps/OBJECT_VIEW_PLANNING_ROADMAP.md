# Object-View Planning Roadmap

Status: planned; V01 is next.

This roadmap implements
[cleanup finding A14](CODEBASE_CLEANUP_AUDIT.md#a14--separate-object-view-planning-from-alias-argument-checking)
through the accepted
[object-view planning design](../archive/OBJECT_VIEW_PLANNING_DESIGN_PROPOSAL.md).
It gives reusable object-view source checking and planning one private owner,
then migrates consumers in stages that preserve their existing diagnostics and
typed HIR products.

The completed work leaves `typeck::expression::object_view` responsible for
checked source facts, closed-world target relations, requested access,
retention selection, projection planning, and conversion to one existing
`HirObjectView`. Alias dispatch, receiver places, iteration protocols, checked
casts, type tests, copy construction, and diagnostic wording remain with their
current semantic owners.

## Scope and invariants

- Create a private facade under `typeck::expression`; do not widen a Rust API
  or introduce a registration mechanism.
- Check and evaluate each source expression exactly once.
- Preserve source admission, read-only and mutable access, static target
  compatibility, ordered ancestor projections, complete-object provenance,
  optional guards, and source-specific anchor strategies.
- Preserve non-exclusive mutable aliases and the requirement for explicit
  shared dereference.
- Model immediate-consumer and loop-body retention explicitly. A loop-body
  request must retain even a stable shared binding because the loop may replace
  that binding.
- Direct view planning accepts only statically compatible relations. Runtime
  narrowing remains owned by explicit casts and type tests.
- Consume each phase-local plan into the existing `HirObjectView`; do not add
  planning state to resolved IR, HIR, MIR, or compiler reports.
- Keep primitive, array, shared-owner, optional-owner, and checked-cast alias
  paths outside the ordinary object-view plan.
- Keep ordinary object places and checked receiver-place carriers in their
  existing representations. Only receiver carriers already expressed as an
  `HirObjectView` join the shared planner.
- Preserve diagnostic codes, wording, labels, notes, order, spans, and recovery
  behavior exactly. Consumer adapters continue to render their own problems.
- Preserve typed HIR dumps, receiver-before-argument and left-to-right
  evaluation, MIR, ownership operations, cleanup order, assembly, native
  output, and failure behavior.
- Do not add first-class references, escaping aliases, borrow checking,
  exclusivity, new implicit conversions, anchor elision, or HIR/MIR variant
  consolidation.

## Progress

- [ ] V01 — Establish the facade and checked source product
- [ ] V02 — Introduce direct-view planning and migrate object aliases
- [ ] V03 — Migrate view receivers and iteration retention
- [ ] V04 — Reuse source facts in checked operations and close A14

## PR-sized implementation sequence

### V01 — Establish the facade and checked source product

**Purpose:** move the already-shared source and relation vocabulary behind one
private owner while every production consumer continues through its existing
entry point and produces identical HIR and diagnostics.

- [ ] Add the private recursive `expression::object_view` facade with cohesive
  `source`, `relation`, and later-planning modules. Keep implementation files
  private and expose only the narrow crate-private surface required by current
  type-checking siblings.
- [ ] Move `object_view_relation` and its focused tests under the facade without
  changing the closed-world `StaticSuccess`, `StaticFailure`, and `Runtime`
  results used by shared compatibility and type operations.
- [ ] Move and rename `CheckedObjectViewSource` as the facade-owned
  `ObjectViewSource`. Preserve every existing variant payload, source span,
  static target, exact dynamic class, access, origin, guard, anchor, and
  projection.
- [ ] Replace `ViewSourceUse` admission booleans with a closed
  `ObjectViewSourceAdmission` policy. Keep consumer-specific diagnostic
  wording at the existing adapter boundary during this extraction.
- [ ] Route current alias, iteration, cast, type-test, and copy-construction
  source checks through the facade without yet changing their relation or HIR
  assembly logic.
- [ ] Decide `shared_pointee` ownership based on cohesion: merge it into source
  implementation or keep it as a substantive sibling, but leave one owner and
  no pass-through module.
- [ ] Add focused source tests only for semantic admission and source facts not
  already pinned by end-to-end type-checking tests.

**Tests:** Preserve the existing object-view relation suite and add focused
coverage for existing-place versus produced-source admission, grouping spans,
explicit shared dereference, optional payload and optional-box sources, array
elements, produced inline objects, access, dynamic origin, projections, and
anchor facts. Run the affected compiler type-checking tests and the aliases,
objects, optionals, arrays, shared-ownership, and control-flow golden suites.
Run `cargo fmt --all -- --check`, Clippy with warnings denied, `make check`,
`make msrv-check`, and `git diff --check`.

**Exit criteria:** all object-view consumers obtain checked source facts and
relations through one private facade; no planning request exists yet; existing
consumer conversion and diagnostics remain authoritative; HIR and observable
behavior are unchanged; and the old source/relation ownership is removed.

### V02 — Introduce direct-view planning and migrate object aliases

**Purpose:** establish the typed planning contract on the ordinary object alias
branch, where access, compatibility, projections, and immediate retention are
currently most entangled with alias-specific dispatch and diagnostics.

- [ ] Add closed `ObjectViewRequest`, `ObjectViewRetention`, and
  `ObjectViewProblem` types. A request contains a target, required access, and
  immediate-consumer or loop-body retention; it contains no syntax, callable,
  parameter, or diagnostic data.
- [ ] Add a consumable, non-`Clone` `ObjectViewPlan` that proves source access,
  static compatibility, projection ordering, and source-specific guard or
  anchor selection before producing exactly one existing `HirObjectView`.
- [ ] Accept `StaticSuccess` for a direct plan. Return distinct structured
  problems for insufficient access, incompatible targets, and relations that
  require an explicit checked operation.
- [ ] Migrate the ordinary object alias-argument branch to source
  classification plus an immediate-consumer plan and wrap success in
  `HirCallArgument::View`.
- [ ] Keep alias parameter-mode dispatch and diagnostic rendering in
  `alias.rs`. Map planning problems to the exact existing parameter labels,
  mutable-access notes, spans, order, and recovery behavior.
- [ ] Leave primitive, array, shared-owner, optional-owner, and checked-cast
  alias paths explicit and outside the new plan.
- [ ] Remove superseded object-alias compatibility and view-construction
  helpers once no caller remains; do not retain parallel planning paths.

**Tests:** Add table-driven private planning tests for class/base/interface/
`Obj` targets, exact and dynamic relation sources, read-only and mutable access,
runtime-dependent rejection, projection order, and every source-to-view HIR
shape. Extend alias type-checking coverage where needed to pin exact invalid-
source, access, compatibility, temporary, and implicit-shared diagnostics.
Preserve overlapping mutable aliases, checked-cast arguments, optional guards,
anchors, HIR dumps, and native cleanup behavior. Run focused type-checking and
alias/MIR golden suites, then `make check`, `make msrv-check`, and
`git diff --check`.

**Exit criteria:** every ordinary object alias argument uses one typed direct-
view plan; unrelated alias families and checked casts retain their existing
owners; all planning problems are rendered by the alias adapter with exact
diagnostic equivalence; and no obsolete object-alias planning path remains.

### V03 — Migrate view receivers and iteration retention

**Purpose:** reuse direct-view planning for the remaining implicit-view
consumers and make iteration's longer shared-owner retention a request-time
decision rather than a post-construction mutation.

- [ ] Identify receiver carriers already represented by `HirObjectView` and
  route their source facts, requested target/access, and immediate retention
  through the planner. Add the existing static-object carrier to the source
  vocabulary if the migration requires it.
- [ ] Keep ordinary `HirObjectPlace`, checked receiver-place carriers, member
  selection, and receiver-specific diagnostics under `place`, `call`, and
  receiver checking.
- [ ] Replace iteration's generic view construction followed by
  `anchor_checked_iteration_source` mutation with a read-only `LoopBody`
  request that selects the correct shared anchor before HIR construction.
- [ ] Keep protocol selection, iterable type checking, guarded optional
  binding handling, and `HirForIn` assembly in the iteration checker.
- [ ] Remove receiver and iteration view-construction helpers made redundant by
  the planner, including post-hoc retention mutation.
- [ ] Document the type-checking boundary in living compiler documentation if
  its ownership or lifetime contract is not already described there.

**Tests:** Pin HIR for produced receivers, static object sources, shared
pointees, optional payloads, optional boxes, array elements, exact base views,
interfaces, and `Obj`. Cover immediate versus loop-body retention for stable,
replaceable, and produced shared owners; optional guards; nested projections;
receiver-before-argument and left-to-right effects; replacement inside a loop;
reverse cleanup; destructor timing; and iteration failures. Run focused
receiver and iteration type-checking tests, relevant object/control-flow/
shared/optional/array golden suites, then `make check`, `make msrv-check`, and
`git diff --check`.

**Exit criteria:** all implicit receiver views in scope and all protocol-
iteration views use the typed planner; loop-duration anchoring is selected by
retention policy before HIR construction; place-specific carriers and
protocol logic remain with their owners; and evaluation, lifetime, HIR, MIR,
diagnostic, and native observations are unchanged.

### V04 — Reuse source facts in checked operations and close A14

**Purpose:** remove the remaining duplicated source and relation decisions
without collapsing checked casts, type tests, or owning copy construction into
the direct-view result type, then leave one maintainable module boundary.

- [ ] Migrate object casts and type tests to facade-owned source facts and the
  shared relation classifier. Preserve their separate handling of static
  success, static failure, and runtime dependence.
- [ ] Migrate copy construction where shared source facts or direct/checked
  views remove real duplication. Keep allocation, owning-copy planning,
  destination safety, and copy diagnostics with copy/construction checking.
- [ ] Preserve `HirCheckedObjectView`, `HirTypeTestKind`, runtime-terminate
  behavior, checked-cast-to-alias wrappers, and operation-specific access and
  result metadata. Add a checked-view planning product only if the remaining
  duplication demonstrates one cohesive invariant.
- [ ] Audit all object-view source, relation, access, projection, anchor, and
  view-construction call sites. Remove superseded exports, helpers, booleans,
  duplicate matches, and temporary migration shims.
- [ ] Review the final recursive module by responsibility. Keep a concise
  facade and cohesive implementation files; avoid one-function wrappers and
  visibility widened only to support file movement.
- [ ] Update living compiler documentation for the final owner and invariants,
  mark A14 complete with its implementation and validation, then mark this
  roadmap complete and archive it with repaired links and indexes.
- [ ] Record any valuable work beyond this accepted scope in an indexed
  discoveries document instead of expanding the closure task.

**Tests:** Cover static-success, static-failure, and runtime cast/test matrices;
checked-cast alias arguments; explicit copies from every supported view source;
failure before destination allocation or mutation; exact diagnostic ordering;
HIR and final MIR dumps; ownership anchors; reverse cleanup; assembly and
native output; and panic traces. Run all affected compiler and golden suites,
the documentation checker, `make check`, `make msrv-check`, and
`git diff --check` from an artifact-free snapshot or clean checkout.

**Exit criteria:** one private facade owns reusable object-view source facts,
relations, and direct-view planning; every intended consumer uses it without a
universal checked-operation result; no superseded implementation remains;
public paths and observable behavior are unchanged; A14 is complete; and the
accepted design and completed roadmap are archived and indexed.

## Ordering and dependencies

V01 establishes source ownership while preserving current consumers, which
keeps diagnostic and HIR changes out of the module move. V02 then proves the
request and plan on the ordinary object alias branch before wider reuse. V03
depends on that stable direct-view contract and makes the exceptional loop
retention visible without involving checked operations. V04 can finally reuse
the settled source and relation facts in casts, tests, and copies while keeping
their distinct results.

Each task is one behavior-preserving review boundary. If migration exposes
different provenance, retention, or diagnostics between consumers, introduce
separate typed adapters or leave the consumer-specific path in place. Do not
add optional fields or booleans to force unrelated semantics through the plan.

A14 depends on the existing typed HIR provenance, shared-owner anchoring,
optional presence guards, relation classifier, and test matrices. It does not
depend on another active roadmap. Work on unrelated alias families, new
conversion semantics, borrow checking, HIR consolidation, or ownership
optimization may proceed only as separately designed follow-up work.
