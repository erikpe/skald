# Complete Low-Level Lowering Migration Roadmap

Status: active; LM01 is complete and LM02 is next.
Implementation baseline: `9e6177fe`, the committed roadmap immediately before
implementation began. The accepted
[complete lowering migration design](COMPLETE_LOW_LEVEL_LOWERING_MIGRATION_DESIGN_PROPOSAL.md)
is committed as `a099d228`.
Parent program:
[Low-Level Compiler Architecture](LOW_LEVEL_COMPILER_ARCHITECTURE_DESIGN_PROPOSAL.md).
Exhaustive handoff:
[Low-Level Compiler Migration Coverage](LOW_LEVEL_COMPILER_MIGRATION_COVERAGE.md).

This roadmap expands the checked private native pilot into a complete
full-language x86-64 path. It preserves the production legacy backend as the
default and parity oracle until the separate architecture-adoption workstream.
Every admitted retained program uses one checked low-level pipeline without
per-callable or per-operation fallback.

## Scope and invariants

- Final MIR remains the only semantic backend input. No later phase queries
  AST, HIR, resolver, type-checker or pass-internal state.
- Planning freezes complete layouts, signatures, runtime services, generated
  inventories, data and artifact-policy facts before lowering consumers run.
- Lifecycle and aggregate behavior expands into ordinary checked LIR operations,
  calls and CFG. No opaque legacy-assembly or high-level target opcode bridge is
  permitted.
- Source and generated callables cross the same lowered, selected and physical
  verification/publication boundaries.
- Baseline placement remains allocation-independent and crosses the existing
  independent checker. Register allocation is outside this roadmap.
- Both trace policies, both artifact policies and supported MIR schedules retain
  semantic, ABI, diagnostic and deterministic parity.
- LA04 ends with complete private parity. Production default adoption, public
  driver observations, legacy deletion and cost acceptance remain LA05.
- Tests use an explicit private new-path entry. Failure after admission never
  retries through the legacy backend.

## Progress

- [x] LM01 — Establish durable migration ownership and the no-fallback gate
- [ ] LM02 — Complete semantic layout and object-view planning facts
- [ ] LM03 — Complete data, artifact and static-storage planning facts
- [ ] LM04 — Checkpoint the complete fact model
- [ ] LM05 — Lower complex places and aggregate call boundaries
- [ ] LM06 — Lower dispatch, runtime type operations and object initialization
- [ ] LM07 — Lower copy, cleanup and complete-class finalization
- [ ] LM08 — Lower shared ownership and generated owner helpers
- [ ] LM09 — Checkpoint the aggregate and lifecycle core
- [ ] LM10 — Lower primitive and shared-owner optional behavior
- [ ] LM11 — Lower aggregate, class and boxed optional behavior
- [ ] LM12 — Lower array storage, construction, positions and anchors
- [ ] LM13 — Lower array element lifecycle and generated helper families
- [ ] LM14 — Lower indexed construction, slices and array aliases
- [ ] LM15 — Lower strings, literal data and standard I/O
- [ ] LM16 — Lower static lifecycle and complete entry orchestration
- [ ] LM17 — Complete trace, data and artifact closure parity
- [ ] LM18 — Reconcile exhaustive private parity and measurements
- [ ] LM19 — Cumulative review, cleanup and LA05 handoff

## PR-sized implementation sequence

### LM01 — Establish durable migration ownership and the no-fallback gate

**Purpose:** replace scalar-pilot naming for reusable shared owners and make the
migration boundary explicit before expanding it.

- [x] Move reusable projection, fact, admission and lowering owners from
  `backend::pilot` into cohesive planning/lowering modules without widening
  visibility or changing behavior.
- [x] Retain a thin private whole-program native entry whose tests explicitly
  require admission and prove that post-admission errors cannot invoke legacy
  lowering.
- [x] Create the roadmap artifact ledger with every inherited allowance, private
  gate, adapter and differential fixture, including introducing commits and
  LM/LA05 removal owners.
- [x] Preserve the scalar pilot's exact dumps, native behavior and no-fallback
  negatives across the move.

**Tests:** backend phase-boundary/privacy tests, existing pilot planning/lowering/
native tests, exact dump comparisons, `make static-check`, and `make msrv-check`.

**Exit criteria:** reusable code no longer has pilot ownership; the remaining
pilot surface is the thin private orchestration/inspection entry recorded for
LA05, and no transition artifact lacks an owner.

Completed on 2026-09-19. `backend::planning` owns admission and immutable fact
projection, `backend::lowering` owns shared MIR-to-LIR construction, and the x86
fact adapter has a semantic filename. The private native pilot remains the sole
explicit no-fallback path. Owner, phase-boundary and post-admission terminal-error
tests pass together with `make static-check`, `make msrv-check` and `make check`.

### LM02 — Complete semantic layout and object-view planning facts

**Purpose:** give object, dispatch and aggregate lowering immutable checked facts
without importing legacy planners into later phases.

- [ ] Project checked exact/complete class layouts, base and field offsets,
  destruction plans, object-view component shapes and aggregate result layouts.
- [ ] Freeze virtual-family, interface-requirement, conformance, method-slot and
  runtime-membership facts with typed identities and deterministic ordering.
- [ ] Add recursive optional, optional-box, shared-header, array and element
  lifecycle layout facts needed by later families.
- [ ] Reject foreign identities, recursive/overflowing layouts, missing dynamic
  metadata and shape/layout disagreement before lowering.

**Tests:** focused plan owner tests, malformed supplied-fact tests, representative
exact/base/interface/aggregate walkthroughs, and synthetic portability checks.

**Exit criteria:** every complex place, call component and runtime type query has
a checked plan source; downstream consumers need no MIR/type/layout lookup.

### LM03 — Complete data, artifact and static-storage planning facts

**Purpose:** settle finite program resources and the accepted complete-mode
inactive-static representation before executable consumers depend on them.

- [ ] Add active and retained-inactive static storage dispositions, enforcing
  zero-only physical retention and excluding inactive storage from semantic
  initialization and shutdown.
- [ ] Freeze literal backing, failure message, trace/TLS, dispatch/descriptor,
  runtime-service and generated-helper declarations with typed dependencies.
- [ ] Define complete/reachable roots and reject retained-inactive references in
  reachable mode, missing storage, lifecycle promotion and forged substitutions.
- [ ] Extend exact program reconciliation for every new data/declaration family
  without retaining executable drafts.

**Tests:** active/inactive static policy matrix, typed data/relocation failures,
artifact-policy roots, discarded-body/receipt closure, and deterministic plans.

**Exit criteria:** the complete plan can describe every LA04 callable/data family
and the inactive-static amendment has positive and adversarial evidence.

### LM04 — Checkpoint the complete fact model

**Purpose:** audit the first design checkpoint before executable feature work
amplifies any planning error.

- [ ] Reconcile plan schemas against every operation/helper/data row in the
  migration coverage record and record remaining executable owners only.
- [ ] Walk aggregate result, metadata receiver, recursive helper and retained
  inactive-data cases against the AArch64-informed synthetic target boundary.
- [ ] Remove unused/provisional facts and expired allowances; amend the frozen
  design explicitly if an unresolved representation decision remains.
- [ ] Update living low-level planning documentation and ledger dispositions.

**Tests:** complete plan/target-program owner suites, phase-boundary guards,
documentation checks, `make check`, and `make msrv-check`.

**Exit criteria:** the complete-fact checkpoint passes with no legacy planning
query, placeholder fact or unresolved prerequisite for LM05–LM17.

### LM05 — Lower complex places and aggregate call boundaries

**Purpose:** establish reusable address/origin and role-based call machinery for
all later object and lifecycle families.

- [ ] Lower every place base/projection through checked layouts, preserving
  provenance, alias anchors, zero-size and metadata-only dispositions.
- [ ] Materialize hidden result destinations, receiver/static/complete origins,
  aliases, views, owned places and shared-owner call components in source order.
- [ ] Select and realize internal aggregate calls/results under integer/floating
  register and stack pressure, including later argument calls.
- [ ] Cover aggregate return preservation across trace pop and cleanup without
  target-specific facts in shared lowering.

**Tests:** complex-place verifier negatives, object result/receiver pressure,
alias and origin corruption, assembler acceptance and native call parity.

**Exit criteria:** all complex places and aggregate call roles cross the checked
pipeline; no object call uses legacy marshalling.

### LM06 — Lower dispatch, runtime type operations and object initialization

**Purpose:** make object identity, dispatch and checked view construction usable
by lifecycle and ownership lowering.

- [ ] Lower direct, virtual and interface method target selection from checked
  tables and exact receiver metadata.
- [ ] Lower type tests, checked object/shared casts, checked-view binding and
  success/failure carriers with exact source attribution.
- [ ] Lower initializer calls into final destinations and preserve complete
  object origins through base/interface views.
- [ ] Publish required dispatch/metadata data with typed callable/data edges.

**Tests:** direct/virtual/interface execution, deep inheritance and interface
views, cast success/failure, malformed slots/metadata and aggregate ABI pressure.

**Exit criteria:** object construction, runtime membership and every dispatch
form execute without legacy selection or unchecked metadata access.

### LM07 — Lower copy, cleanup and complete-class finalization

**Purpose:** establish the ordered lifecycle core on which ownership, optionals
and arrays depend.

- [ ] Lower user/synthesized copy construction and assignment in certified
  base/field order with alias-safe self-assignment.
- [ ] Lower full-expression cleanup and destruction while preserving completed
  results and reverse completion order.
- [ ] Generate complete-class finalizers, including user bodies and every field/
  base cleanup shape, through the ordinary callable worklist.
- [ ] Preserve source trace frames for user destructors and inherited attribution
  for generated wrappers.

**Tests:** copy/assignment overlap, nested destruction order, early failures,
result preservation, recursive class graphs and generated-finalizer closure.

**Exit criteria:** class lifecycle and finalizers pass ordinary verification and
native parity with no opaque lifecycle operation.

### LM08 — Lower shared ownership and generated owner helpers

**Purpose:** complete allocation, retain/transfer/release and shared-field
semantics before recursive optional/array owners consume them.

- [ ] Lower allocation, unpublished initialization, publication, adopt/move,
  static/immortal handles, casts, copies and field initialization/replacement.
- [ ] Lower checked reference counts, null/zero hard defects, immortal no-ops,
  overflow reporting and retain-before-release replacement.
- [ ] Preserve the original allocation across last-owner finalization and free
  it exactly once after visible call clobbers.
- [ ] Generate retain/release helpers with exact signatures, attribution,
  recursive dependencies and ordinary verification.

**Tests:** count boundaries, self/reentrant replacement, allocation/finalizer
failure order, full caller-clobber pressure, original-header free and helper
recursion/closure.

**Exit criteria:** every shared instruction/terminator and helper row has native
evidence, including the last-owner acceptance witness.

### LM09 — Checkpoint the aggregate and lifecycle core

**Purpose:** stop before recursive wrapper/container families if object or owner
semantics are not independently sound.

- [ ] Reconcile LM05–LM08 against object, call, copy, cleanup, cast and shared
  rows in the coverage record.
- [ ] Run mixed object/shared/dispatch/trace pressure through both artifact and
  trace policies and default/minimal MIR schedules.
- [ ] Review effects, call roles, hard defects, reported failures, generated
  attribution and ledger transitions as one cumulative slice.
- [ ] Correct the owning contract or add an explicit corrective task for any gap.

**Tests:** focused cumulative native matrix, full repository gate, full golden
determinism, release goldens and MSRV.

**Exit criteria:** the lifecycle checkpoint passes and optionals/arrays can reuse
the core without exceptions or legacy fragments.

### LM10 — Lower primitive and shared-owner optional behavior

**Purpose:** implement optional forms whose payloads use scalar or existing
shared-owner lifecycle.

- [ ] Lower primitive presence, initialization, assignment, unwrap and absence
  failures with canonical payload/state representation.
- [ ] Lower optional shared initialization, copy/assignment, cleanup, return and
  unwrap using the zero niche and balanced retain/release.
- [ ] Lower optional and optional-box presence/view guard begin/end, overflow,
  underflow and pinned-mutation checks where applicable.
- [ ] Preserve nested layer identity and success-only payload/owner availability.

**Tests:** every primitive kind, absent/present nesting, shared owner lifetime,
view counts, mutation rejection, ABI pressure and exact failure attribution.

**Exit criteria:** primitive and shared optional rows execute through checked LIR
with no guard or ownership special case below selection.

### LM11 — Lower aggregate, class and boxed optional behavior

**Purpose:** complete recursive optional storage, publication and finalization.

- [ ] Lower aggregate/class optional initialization, assignment, publication,
  payload access and conditional cleanup using checked aligned layouts.
- [ ] Lower nested optional arrays/classes/shared fields without flattening layer
  state or losing recursive lifecycle order.
- [ ] Generate exact optional-box finalizers through the ordinary worklist.
- [ ] Reject premature publication, payload access, cleanup and cross-layer guard
  consumption at verifier boundaries.

**Tests:** nested aggregate/class shapes, calls/results/fields, recursive cleanup,
box finalizers, pinned views, malformed state transitions and native failures.

**Exit criteria:** every optional instruction, terminator, layout and generated
family has checked native parity.

### LM12 — Lower array storage, construction, positions and anchors

**Purpose:** establish array backing representation and safety control flow before
element lifecycle and slices.

- [ ] Lower checked allocation/size, inline/shared headers, descriptors,
  publication, adopt/replace/release and immutable length.
- [ ] Lower position normalization, offsets, boundaries, bounds/operation checks
  and forward/reverse array loops before element address use.
- [ ] Lower anchor begin/end and alias binding for every inline/shared/optional
  owner kind across replacement.
- [ ] Preserve allocation-before-element failure order and initialized-prefix
  state without publishing partial storage.

**Tests:** empty/boundary/overflow allocation, positive/negative positions,
anchor replacement, inline/shared backing, failure edges and loop CFG negatives.

**Exit criteria:** safe array storage/addressing and all owner/anchor forms pass
verification and native execution.

### LM13 — Lower array element lifecycle and generated helper families

**Purpose:** implement every concrete element default/copy/assignment/destruction
shape and recursive helper closure.

- [ ] Lower primitive, optional, class, nested array, shared and optional-shared
  element initialization/copy/assignment/destruction plans.
- [ ] Lower ordered element-list construction and advance the prefix only after
  complete slot initialization.
- [ ] Generate the six array helper families plus raw-address class-copy wrappers
  with deterministic reservation/completion and typed dependencies.
- [ ] Cover recursive class/array graphs without recursive host calls or body
  duplication.

**Tests:** every element-plan cell, nested/shared arrays, reverse partial cleanup,
element-list failure order, helper recursion and exact receipt reconciliation.

**Exit criteria:** the coverage record splits and closes every concrete array
element shape; all helpers use the ordinary checked pipeline.

### LM14 — Lower indexed construction, slices and array aliases

**Purpose:** complete the remaining array protocols and checkpoint recursive
family closure.

- [ ] Lower indexed construction epochs, binding, one-time element evaluation,
  completion/backedges and publication.
- [ ] Lower slice bounds/length checks, copy snapshots, overlap-safe assignment
  and selected per-element lifecycle.
- [ ] Lower whole-array and exact-element aliases without transferring ownership
  or losing required backing anchors.
- [ ] Reconcile all 29 array variants and recursive optional/array helper receipts.

**Tests:** zero/one/many indexed construction, effectful producers, overlaps,
invalid/reversed bounds, aliases through replacement, recursive matrix and
malformed epoch/receipt negatives.

**Exit criteria:** the recursive-family checkpoint passes with every optional and
array row delivered and no recursion/emission special case.

### LM15 — Lower strings, literal data and standard I/O

**Purpose:** migrate remaining runtime-service and byte-buffer behavior using the
completed owner/array foundations.

- [ ] Lower immortal shared string initialization and canonical pooled literal/
  empty backing data without dynamic retain/free.
- [ ] Lower standard handle, open, read, write and close calls with ordered
  arguments, exact symbols and anchored byte buffers.
- [ ] Preserve partial transfers, capacity/offset checks, source panic slices and
  runtime failure attribution.
- [ ] Publish literal/runtime data and relocations through typed program closure.

**Tests:** binary/partial I/O, closed descriptors, invalid progress, allocation
failure, literal identity/empty backing, string panic bytes and trace variants.

**Exit criteria:** all string/I/O operations and literal/runtime data execute and
close through the new pipeline.

### LM16 — Lower static lifecycle and complete entry orchestration

**Purpose:** integrate certified program startup/shutdown with the complete
language surface and accepted static storage dispositions.

- [ ] Lower active static access, zero/default versus explicit initialization,
  all cleanup shapes and exact reverse shutdown.
- [ ] Generate program initializer/finalizer coordinators through the ordinary
  worklist without rediscovering activation.
- [ ] Materialize retained-inactive zero storage only for complete-mode typed
  references and prove it has no lifecycle participation.
- [ ] Complete the exported entry sequence: ABI marker, initialization, language
  entry, result preservation, normal finalization and return.

**Tests:** dependency order/cycles, each cleanup shape, startup/entry/shutdown
failures, complete/reachable inactive statics, result preservation and trace
frame exclusions.

**Exit criteria:** entry and static lifecycle parity hold for both artifact
policies with no legacy coordinator or fallback slot planner.

### LM17 — Complete trace, data and artifact closure parity

**Purpose:** close cross-cutting observation and publication obligations after
all executable families exist.

- [ ] Cover every call-attribution category, source-frame eligibility, initial/
  call/failure locations, inherited helpers and push/pop balance.
- [ ] Prove omitted mode performs no source lookup and publishes no trace action,
  object, metadata, TLS artifact or relocation.
- [ ] Reconcile every callable/data/helper/static/descriptor root and typed edge
  in complete and reachable modes after body release.
- [ ] Preserve structured phase/callable/origin errors and deterministic private
  requested checkpoints without exposing public driver adapters.

**Tests:** enabled/omitted trace matrix, sparse/complete closure, missing/extra
receipts and dependencies, writer failures, independent-process dump/assembly
determinism and no-lookup probes.

**Exit criteria:** trace and artifact policy can no longer distinguish a missing
feature family; all private checkpoints are complete and deterministic.

### LM18 — Reconcile exhaustive private parity and measurements

**Purpose:** prove full-language completeness and prepare a trustworthy LA05
adoption decision without claiming it prematurely.

- [ ] Remove the last feature allowlist/unsupported admission path; retained
  supported programs plan or return ordinary checked backend errors.
- [ ] Reconcile every MIR instruction, rvalue, terminator, array operation,
  helper, runtime entity, data, trace and publication row in the coverage record
  with named durable evidence.
- [ ] Run the full parity matrix across MIR schedule, trace policy, artifact
  policy, callable/ABI family and success/failure/pressure behavior.
- [ ] Capture structural and cost observations with the maintained protocol,
  retaining the eleven inconclusive timing classifications for LA05.

**Tests:** complete focused parity suites, `make check`, full determinism and
release goldens, relevant long robustness/runtime gates, MSRV and measurement
evidence verification.

**Exit criteria:** full private parity checkpoint passes, no supported retained
program is rejected as unsupported, and LA05 receives complete coverage and
measurement evidence without cost clearance claims.

### LM19 — Cumulative review, cleanup and LA05 handoff

**Purpose:** review the complete roadmap diff as one architecture change and
leave no hidden migration residue before production adoption.

- [ ] Review `git diff 9e6177fe..HEAD`, committed task history, and all staged,
  unstaged and untracked changes against the frozen design and coverage record.
- [ ] Reconcile every artifact-ledger entry; remove expired aliases, gates,
  adapters, allowances, duplicate implementations, stale pilot names/comments
  and exploratory instrumentation.
- [ ] Retain only the thin private native entry, legacy production backend,
  focused parity evidence and explicit LA05-owned observation/adoption artifacts.
- [ ] Review AArch64 portability, phase dependencies, public visibility, errors,
  deterministic inspection and complete/reachable closure holistically.
- [ ] Run artifact-free repository, long golden/robustness/runtime, MSRV and
  documentation gates; record baseline, endpoint and validation evidence.
- [ ] Mark this roadmap complete, freeze its final handoff, archive it and its
  design, update all indexes/links, and make LA05 design the next program step.

**Tests:** all LM18 gates from a clean snapshot plus documentation/link checks,
whitespace/diff checks and targeted searches for ledgered/stale artifacts.

**Exit criteria:** the accepted LA04 design is fully implemented, the cumulative
change is reviewable and clean, all roadmap/coverage checkboxes are reconciled,
and only explicitly transferred LA05 work remains.

## Ordering and dependencies

LM01 establishes durable ownership before expansion. LM02–LM04 settle the fact
model, including the only inherited representation amendment, before complex
lowering consumes it. LM05–LM09 establish calls, objects and lifecycle in the
order required by wrappers and containers. LM10–LM14 build optional and array
recursion on that checked core. LM15–LM17 add services, whole-program lifecycle
and cross-cutting closure only after their dependencies are executable. LM18
proves completeness and gathers the evidence LA05 needs; LM19 performs the
required cumulative review and archival.

Tasks inside one family are sequential. Focused test-fixture preparation may be
shared, but no task may publish authority for a later task's incomplete family.
If a checkpoint reveals a new representation, ABI, runtime or phase-authority
decision, stop dependent work and amend the frozen design or add a focused
proposal. Record independent improvements in a roadmap-specific discoveries
file, creating and indexing it when first needed, rather than expanding the
active task.

## Transition artifact ledger

Update this table as tasks and manual commits land. A retained artifact needs a
continuing purpose and explicit removal owner.

| Artifact | Introduced | Removal owner | Current disposition |
| --- | --- | --- | --- |
| Shared `backend::pilot` projection/admission/lowering names | LA03 commits through `8f8c1825` | LM01 | Removed in LM01: admission/fact projection are owned by `backend::planning`, lowering by `backend::lowering`, and target fact adapters use semantic names |
| `backend::planning::{mod,facts}` non-test unused/dead-code allowances | NP03, `879bfb47`; renamed LM01 | Consuming LM02–LM17 tasks; residual facade allowance LA05 | Retained only for the private admitted product, structured errors and trace facts; narrow as feature consumers land |
| `backend::lowering::{mod,worklist}` non-test unused/dead-code allowances | NP04 `a4c3f189`, streaming entry NP17 `2d252cc3`; renamed LM01 | Consuming LM05–LM17 tasks; residual facade allowance LA05 | Retained only for private checked lowering entries while production still uses legacy emission |
| `x86_64_sysv::fact_projection` non-test function allowances | NP03, `879bfb47`; renamed LM01 | LM02–LM03 fact expansion; residual root allowance LA05 | Retained target-owned layout/trace projection; no shared `pilot` naming remains |
| Thin `x86_64_sysv::native::pilot` entry/error/inspection adapter and native-facade allowances | NP17 `2d252cc3`, observation NP18; reviewed through `8f8c1825` | LA05 adoption | Retain through LM19 as the explicit private parity entry; ordinary emission cannot reach it |
| `native::pilot::pipeline::compile_admitted` post-admission seam | LM01 working tree; commit pending | LA05 adoption | Retain as the single private continuation used by compilation and the terminal post-admission failure regression; it has no legacy callback |
| Feature admission allowlist and structured unsupported reasons | NP03–NP06, reviewed through `8f8c1825`; renamed LM01 | LM18 | Narrow during family delivery, then remove after complete coverage |
| Legacy/new differential and forced-new-path fixtures | NP17–NP19 through `8f8c1825`; extended LM01 | LM19/LA05 | Retain focused admission, post-admission failure, dump, native and policy regressions; remove broad duplicates during cumulative review/adoption |
| Legacy x86 lowering/frame/machine/emitter | Pre-program production backend | LA05 adoption | Preserve unchanged as default and parity oracle during LA04 |

## Required documentation updates

Update the living low-level IR, backend, placement, frame, physical realization,
runtime ABI, driver/artifact, reporting, debugging and testing documents in the
task that changes their contracts. Update the migration coverage record with
actual delivery evidence after every family. Roadmap task codes stay in roadmap
history and must not leak into production symbols, comments, diagnostics or
living architecture text.
