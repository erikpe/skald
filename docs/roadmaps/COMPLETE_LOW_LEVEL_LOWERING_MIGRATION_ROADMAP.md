# Complete Low-Level Lowering Migration Roadmap

Status: active; LM01–LM13 are complete and LM14 is next.
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
- [x] LM02 — Complete semantic layout and object-view planning facts
- [x] LM03 — Complete data, artifact and static-storage planning facts
- [x] LM04 — Checkpoint the complete fact model
- [x] LM05 — Lower complex places and aggregate call boundaries
- [x] LM06 — Lower dispatch, runtime type operations and object initialization
- [x] LM07 — Lower copy, cleanup and complete-class finalization
- [x] LM08 — Lower shared ownership and generated owner helpers
- [x] LM09 — Checkpoint the aggregate and lifecycle core
- [x] LM10 — Lower primitive and shared-owner optional behavior
- [x] LM11 — Lower aggregate, class and boxed optional behavior
- [x] LM12 — Lower array storage, construction, positions and anchors
- [x] LM13 — Lower array element lifecycle and generated helper families
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

- [x] Project checked exact/complete class layouts, base and field offsets,
  destruction plans, object-view component shapes and aggregate result layouts.
- [x] Freeze virtual-family, interface-requirement, conformance, method-slot and
  runtime-membership facts with typed identities and deterministic ordering.
- [x] Add recursive optional, optional-box, shared-header, array and element
  lifecycle layout facts needed by later families.
- [x] Reject foreign identities, recursive/overflowing layouts, missing dynamic
  metadata and shape/layout disagreement before lowering.

**Tests:** focused plan owner tests, malformed supplied-fact tests, representative
exact/base/interface/aggregate walkthroughs, and synthetic portability checks.

**Exit criteria:** every complex place, call component and runtime type query has
a checked plan source; downstream consumers need no MIR/type/layout lookup.

Completed on 2026-09-19. The checked plan now owns a typed semantic catalog for
class, field, container, shared-allocation, object-view, membership and dispatch
facts. The x86 projection computes layout and dispatch once, converts their
results into plan-owned records, and exposes narrow identity lookups for later
lowering. Independent checks reject malformed supplied facts; representative
inheritance/interface/aggregate/container and synthetic AArch64-profile tests
exercise the boundary. Executable use of these facts remains assigned to the
subsequent lowering tasks.

### LM03 — Complete data, artifact and static-storage planning facts

**Purpose:** settle finite program resources and the accepted complete-mode
inactive-static representation before executable consumers depend on them.

- [x] Add active and retained-inactive static storage dispositions, enforcing
  zero-only physical retention and excluding inactive storage from semantic
  initialization and shutdown.
- [x] Freeze literal backing, failure message, trace/TLS, dispatch/descriptor,
  runtime-service and generated-helper declarations with typed dependencies.
- [x] Define complete/reachable roots and reject retained-inactive references in
  reachable mode, missing storage, lifecycle promotion and forged substitutions.
- [x] Extend exact program reconciliation for every new data/declaration family
  without retaining executable drafts.

**Tests:** active/inactive static policy matrix, typed data/relocation failures,
artifact-policy roots, discarded-body/receipt closure, and deterministic plans.

**Exit criteria:** the complete plan can describe every LA04 callable/data family
and the inactive-static amendment has positive and adversarial evidence.

Completed on 2026-09-19. `plan::ResourceFacts` now freezes canonical static
dispositions and lifecycle work, data recipes and relocations, TLS, runtime
services, generated callable dependencies and complete/reachable roots. Complete
projection retains typed all-zero storage for inactive slots referenced by
physically retained bodies; reachable projection rejects the same leakage, and
neither activation nor shutdown can promote such a slot. Shared lowering consumes
the frozen data recipes directly, while program publication reconciles exact
initializers and generated dependencies without retaining callable drafts.
Focused owner tests cover malformed storage, lifecycle promotion, identities,
relocations, roots, deterministic complex catalogs, reachable pruning and the
complete-mode inactive-reference exception.

### LM04 — Checkpoint the complete fact model

**Purpose:** audit the first design checkpoint before executable feature work
amplifies any planning error.

- [x] Reconcile plan schemas against every operation/helper/data row in the
  migration coverage record and record remaining executable owners only.
- [x] Walk aggregate result, metadata receiver, recursive helper and retained
  inactive-data cases against the AArch64-informed synthetic target boundary.
- [x] Remove unused/provisional facts and expired allowances; amend the frozen
  design explicitly if an unresolved representation decision remains.
- [x] Update living low-level planning documentation and ledger dispositions.

**Tests:** complete plan/target-program owner suites, phase-boundary guards,
documentation checks, `make check`, and `make msrv-check`.

**Exit criteria:** the complete-fact checkpoint passes with no legacy planning
query, placeholder fact or unresolved prerequisite for LM05–LM17.

Completed on 2026-09-19. The exhaustive coverage record now maps every operation,
helper and data family to its checked plan inputs and names only the remaining
LM05–LM17 executable owner. A combined x86-64/AArch64 synthetic regression walks
aggregate result and receiver roles, mutually recursive helpers and complete-mode
retained inactive storage through one checked catalog. Review removed the unused
`DataKey::TraceRecord` identity: activation records are callable-local LIR
objects, while resource facts own trace bytes, contexts, locations and TLS. All
remaining staged allowances have named executable consumers in the ledger; no
design amendment, legacy planning query or unresolved fact prerequisite remains.

### LM05 — Lower complex places and aggregate call boundaries

**Purpose:** establish reusable address/origin and role-based call machinery for
all later object and lifecycle families.

- [x] Lower every place base/projection through checked layouts, preserving
  provenance, alias anchors, zero-size and metadata-only dispositions.
- [x] Materialize hidden result destinations, receiver/static/complete origins,
  aliases, views, owned places and shared-owner call components in source order.
- [x] Select and realize internal aggregate calls/results under integer/floating
  register and stack pressure, including later argument calls.
- [x] Cover aggregate return preservation across trace pop and cleanup without
  target-specific facts in shared lowering.

**Tests:** complex-place verifier negatives, object result/receiver pressure,
alias and origin corruption, assembler acceptance and native call parity.

**Exit criteria:** all complex places and aggregate call roles cross the checked
pipeline; no object call uses legacy marshalling.

Completed on 2026-09-19. Shared lowering now owns separate storage, place and
origin modules. Caller-provided aggregate results, aggregate value parameters,
aliases and receivers bind directly to checked signature inputs; local storage
retains symbolic LIR objects. Complex projections consume only frozen semantic
offsets and strides, and direct calls materialize their complete component list
from `ComponentRole` before target selection assigns ABI locations. Trivial
class cleanup is admitted as a semantic no-op, while nontrivial lifecycle and
dynamic dispatch remain rejected for LM07 and LM06 respectively. Existing LIR,
selected and physical adversarial suites cover provenance corruption, exact
role checking and mixed register/stack pressure; the new lowering regression
covers projected fields, aggregate parameters, aliases and direct receiver
origin forwarding. The backend suite, formatting and static checks pass.

### LM06 — Lower dispatch, runtime type operations and object initialization

**Purpose:** make object identity, dispatch and checked view construction usable
by lifecycle and ownership lowering.

- [x] Lower direct, virtual and interface method target selection from checked
  tables and exact receiver metadata.
- [x] Lower type tests, checked object casts and shared-origin membership,
  checked-view binding and success/failure carriers with exact source
  attribution. Owner-producing shared casts retain their explicit LM08 owner.
- [x] Lower initializer calls into final destinations and preserve complete
  object origins through base/interface views.
- [x] Publish required dispatch/metadata data with typed callable/data edges.

**Tests:** direct/virtual/interface execution, deep inheritance and interface
views, cast success/failure, malformed slots/metadata and aggregate ABI pressure.

**Exit criteria:** object construction, runtime membership and every dispatch
form execute without legacy selection or unchecked metadata access.

Completed on 2026-09-19. Shared lowering now loads virtual and interface targets
from checked signature-typed slots, evaluates membership from frozen object-view
sets, and binds checked views only on the successful cast edge. Initializers use
their final destination and exact class metadata, including inherited
initializer chains. Reachable planning retains those dispatch tables and only
the helper edges they actually reference; recursively empty class finalizers
publish ordinary verified no-op bodies until LM07 expands lifecycle lowering.
The private native path executes deep inheritance, direct/virtual/interface
calls, successful and failing casts, source-attributed failure reporting and an
eight-argument interface call under stack pressure. Shared-owner transfer,
retain and adopt behavior remains wholly owned by LM08. The complete repository
gate, 650-case golden suite and Rust 1.82 MSRV check pass.

### LM07 — Lower copy, cleanup and complete-class finalization

**Purpose:** establish the ordered lifecycle core on which ownership, optionals
and arrays depend.

- [x] Lower user/synthesized copy construction and assignment in certified
  base/field order with alias-safe self-assignment.
- [x] Lower full-expression cleanup and destruction while preserving completed
  results and reverse completion order.
- [x] Generate complete-class finalizers, including user bodies and every field/
  base cleanup shape, through the ordinary callable worklist.
- [x] Preserve source trace frames for user destructors and inherited attribution
  for generated wrappers.

**Tests:** copy/assignment overlap, nested destruction order, early failures,
result preservation, recursive class graphs and generated-finalizer closure.

**Exit criteria:** class lifecycle and finalizers pass ordinary verification and
native parity with no opaque lifecycle operation.

Completed on 2026-09-19. Shared lowering now expands user and synthesized class
copy construction/assignment through certified base and scalar/class field
plans. Cleanup and full-expression cleanup call ordinary generated finalizers
after scalar results are secured. Complete-class finalizers execute user bodies,
nested class fields and bases in frozen order; their exact reserved dependencies
close through the normal worklist, including inherited attribution back to the
generated boundary. Native tests cover default/minimal MIR, enabled/omitted
tracing, self assignment, nested copy, inheritance, result preservation and a
source-attributed destructor failure. Optional, shared and array lifecycle
shapes remain explicitly rejected for their later roadmap owners. `make check`
and `make msrv-check` pass for the completed slice.

### LM08 — Lower shared ownership and generated owner helpers

**Purpose:** complete allocation, retain/transfer/release and shared-field
semantics before recursive optional/array owners consume them.

- [x] Lower allocation, unpublished initialization, publication, adopt/move,
  static/immortal handles, casts, copies and field initialization/replacement.
- [x] Lower checked reference counts, null/zero hard defects, immortal no-ops,
  overflow reporting and retain-before-release replacement.
- [x] Preserve the original allocation across last-owner finalization and free
  it exactly once after visible call clobbers.
- [x] Generate retain/release helpers with exact signatures, attribution,
  recursive dependencies and ordinary verification.

**Tests:** count boundaries, self/reentrant replacement, allocation/finalizer
failure order, full caller-clobber pressure, original-header free and helper
recursion/closure.

**Exit criteria:** every shared instruction/terminator and helper row has native
evidence, including the last-owner acceptance witness.

Completed on 2026-09-19. Shared owners now use the checked one-word handle
representation for local, parameter and result storage. Allocation,
initialization, publication, transfers, static backing, field edges and checked
owner casts lower to ordinary verified memory, calls and CFG. One generated
retain/release pair owns the shared-header layout: null and zero states trap,
immortal counts return unchanged, retain exhaustion reports the canonical
failure, and last release stores zero before calling the dynamic finalizer and
freeing the original header. Class finalizers now release shared fields through
that same helper. Focused graph tests cover exact helper dependencies, count
boundaries, inherited attribution, retain-before-release self assignment and
original-header preservation. Native witnesses cover both MIR schedules and
trace policies, field replacement and casts, one-time last-owner finalization,
real runtime free and source-attributed finalizer failure.
The complete repository gate, 650-case golden suite and Rust 1.82 MSRV check
pass for the completed slice.

### LM09 — Checkpoint the aggregate and lifecycle core

**Purpose:** stop before recursive wrapper/container families if object or owner
semantics are not independently sound.

- [x] Reconcile LM05–LM08 against object, call, copy, cleanup, cast and shared
  rows in the coverage record.
- [x] Run mixed object/shared/dispatch/trace pressure through both artifact and
  trace policies and default/minimal MIR schedules.
- [x] Review effects, call roles, hard defects, reported failures, generated
  attribution and ledger transitions as one cumulative slice.
- [x] Correct the owning contract or add an explicit corrective task for any gap.

**Tests:** focused cumulative native matrix, full repository gate, full golden
determinism, release goldens and MSRV.

**Exit criteria:** the lifecycle checkpoint passes and optionals/arrays can reuse
the core without exceptions or legacy fragments.

Completed on 2026-09-19. The checkpoint reviewed the committed LM05–LM08 slice
from `f4159542` through `e481f791` against the coverage inventory and current
source. A mixed aggregate/object/shared program now crosses default and minimal
MIR, enabled and omitted tracing, and complete and reachable artifact policies.
Its requested checkpoints jointly witness aggregate result roles, receiver
metadata, runtime calls, allocation/free effects, reported and hard failures,
and inherited generated attribution before native execution.

The review found one staged-contract defect: complete emission rejected every
class program because planning eagerly declared raw-address class-copy wrappers
that belong to the later array helper family. Complete mode now admits the
implemented class metadata, copy, finalizer and shared-owner families; raw copy
wrappers acquire roots with array lifecycle, while optional and array families
remain explicitly rejected. Focused planning and private native suites pass;
the complete repository, golden and MSRV gates pass for the checkpoint.

### LM10 — Lower primitive and shared-owner optional behavior

**Purpose:** implement optional forms whose payloads use scalar or existing
shared-owner lifecycle.

- [x] Lower primitive presence, initialization, assignment, unwrap and absence
  failures with canonical payload/state representation.
- [x] Lower optional shared initialization, copy/assignment, cleanup, return and
  unwrap using the zero niche and balanced retain/release.
- [x] Confirm that scalar tagged and nullable shared-owner optionals require no
  view guard, overflow, underflow or pinned-mutation operations; retain those
  operations with the aggregate/class/boxed representations owned by LM11.
- [x] Preserve optional identity without flattening unsupported nested layers,
  and make payloads/owners available only on successful access edges.

**Tests:** every primitive kind, absent/present nesting, shared owner lifetime,
view counts, mutation rejection, ABI pressure and exact failure attribution.

**Exit criteria:** primitive and shared optional rows execute through checked LIR
with no guard or ownership special case below selection.

Completed on 2026-09-19. Shared lowering now expands primitive tagged optionals
and nullable shared-owner optionals into ordinary checked loads, stores,
comparisons, calls and control flow. Primitive payloads are read only on present
edges. Shared copies retain conditionally, assignments retain before releasing
the prior owner (including self-assignment), cleanup releases conditionally, and
returns preserve the null niche. Complete planning admits only these two
representations; aggregate, class and boxed optionals, together with their view
guards and pin checks, remain explicitly assigned to LM11. Native tests cover
all primitive kinds, composed absent/present paths, shared lifetime and
synthesized field finalization, register and stack argument pressure, both trace
and artifact policies, and exact absent-access reporting. `make check`, full
golden determinism, release golden execution and the Rust 1.82 MSRV check pass.

### LM11 — Lower aggregate, class and boxed optional behavior

**Purpose:** complete recursive optional storage, publication and finalization.

- [x] Lower aggregate/class optional initialization, assignment, publication,
  payload access and conditional cleanup using checked aligned layouts.
- [x] Lower nested optional classes/shared fields without flattening layer state
  or losing recursive lifecycle order. Inline-array payload execution remains
  behind the explicit LM12–LM13 array-helper gate.
- [x] Generate exact optional-box finalizers through the ordinary worklist.
- [x] Reject premature publication, payload access, cleanup and cross-layer guard
  consumption at verifier boundaries.

**Tests:** nested aggregate/class shapes, calls/results/fields, recursive cleanup,
box finalizers, pinned views, malformed state transitions and native failures.

**Exit criteria:** every non-array optional instruction, terminator, layout and
generated family has checked native parity; array-backed payload execution stays
with the array tasks that own its storage and helpers.

Completed on 2026-09-19. Shared lowering now expands tagged aggregate and
inline-class optional initialization, assignment, publication, conditional
cleanup and checked payload guards into ordinary LIR. Recursive nested layers
retain independent state, optional fields participate in synthesized copy and
class finalization, and exact optional-box allocations publish typed descriptors
whose generated finalizers close through the ordinary worklist. Exact and
polymorphic box views use the shared-header-relative planned layer offsets, so
static view metadata does not pretend to own an exact allocation layout. Native
tests cover nested copies, class fields, exact/polymorphic boxes, both artifact
policies and pinned mutation failures. Inline-array optional payload operations
remain rejected until LM12–LM13 provide their storage and helper bodies.

### LM12 — Lower array storage, construction, positions and anchors

**Purpose:** establish array backing representation and safety control flow before
element lifecycle and slices.

- [x] Lower checked allocation/size, inline/shared headers, descriptors,
  publication, adopt/replace/release and immutable length.
- [x] Lower position normalization, offsets, boundaries, bounds/operation checks
  and forward/reverse array loops before element address use.
- [x] Lower anchor begin/end and alias binding for every inline/shared/optional
  owner kind across replacement.
- [x] Preserve allocation-before-element failure order and initialized-prefix
  state without publishing partial storage.

**Tests:** empty/boundary/overflow allocation, positive/negative positions,
anchor replacement, inline/shared backing, failure edges and loop CFG negatives.

**Exit criteria:** safe array storage/addressing and all owner/anchor forms pass
verification and native execution.

Completed on 2026-09-19. Shared lowering now constructs inline and shared array
backings from checked layout facts, initializes and publishes their headers only
after successful allocation, and keeps the initialized prefix explicit. Array
length, signed position normalization, boundary selection, operation checks,
ordinary loops, aliases and every anchor ownership form lower to ordinary LIR
CFG and address operations. Primitive/trivial arrays provide executable helper
bodies so reachable programs close the exact callable worklist; LM13 retains
ownership of the remaining element lifecycle shapes and complete-artifact
admission. Focused lowering tests cover primitive and shared/optional-shared
owners, positive and negative positions, loops and allocation rejection. The
private native path exercises allocation, addressed stores, length observation
and cleanup under both MIR schedules. Corrupt zero reference counts hard-trap
instead of underflowing. The focused plan and lowering suites pass; the full
repository gates are recorded with this implementation.

### LM13 — Lower array element lifecycle and generated helper families

**Purpose:** implement every concrete element default/copy/assignment/destruction
shape and recursive helper closure.

- [x] Lower primitive, optional, class, nested array, shared and optional-shared
  element initialization/copy/assignment/destruction plans.
- [x] Lower ordered element-list construction and advance the prefix only after
  complete slot initialization.
- [x] Generate the six array helper families plus raw-address class-copy wrappers
  with deterministic reservation/completion and typed dependencies.
- [x] Cover recursive class/array graphs without recursive host calls or body
  duplication.

**Tests:** every element-plan cell, nested/shared arrays, reverse partial cleanup,
element-list failure order, helper recursion and exact receipt reconciliation.

**Exit criteria:** the coverage record splits and closes every concrete array
element shape; all helpers use the ordinary checked pipeline.

Completed on 2026-09-19. Array element initialization, copying, assignment and
destruction now consume every selected primitive, optional, class, nested-array,
shared and optional-shared lifecycle fact. Ordered construction advances its
initialized prefix only after the selected helper returns. The six array helper
families and only the raw-address class-copy wrappers required by their retained
dependency graph are reserved, lowered and reconciled through the ordinary
worklist. Nested optional/array/class graphs close through calls between reserved
helpers rather than recursive Rust lowering. Shared-array cleanup walks the
initialized prefix in reverse. Focused planning, lowering and native tests cover
complete and reachable policies, nontrivial element graphs, recursive closure,
exact receipts and executable copy/assignment/destruction; the repository gates
are recorded with this implementation.

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
| `x86_64_sysv::fact_projection` non-test function allowances | NP03, `879bfb47`; renamed LM01; semantic catalog expanded LM02 and resource consumers added LM03 | Residual root allowance LA05 | Retained target-owned semantic layout/dispatch and trace projection; legacy planner types do not escape into the checked plan |
| `plan::ResourceFacts` and `planning::resources` complete resource catalog | LM03, `dbada8c6`; audited LM04 | Executable consumers LM05–LM17 | Retain as the checked owner of static dispositions/lifecycle, typed data, runtime/generated declarations and artifact roots; complete-mode inactive storage is physical-only and all-zero |
| Eager complete-mode `RawClassCopy` declarations for every class | LM03, `dbada8c6` | LM09 | Removed in LM09: raw-address wrappers are array-element machinery and will acquire roots from their actual array-helper consumers |
| Thin `x86_64_sysv::native::pilot` entry/error/inspection adapter and native-facade allowances | NP17 `2d252cc3`, observation NP18; reviewed through `8f8c1825` | LA05 adoption | Retain through LM19 as the explicit private parity entry; ordinary emission cannot reach it |
| `native::pilot::pipeline::compile_admitted` post-admission seam | LM01, `51cb4f75` | LA05 adoption | Retain as the single private continuation used by compilation and the terminal post-admission failure regression; it has no legacy callback |
| Feature admission allowlist and structured unsupported reasons | NP03–NP06, reviewed through `8f8c1825`; renamed LM01 | LM18 | Narrowed through LM11: both artifact policies admit the complete class/shared and non-array optional core; array, string, I/O and static families retain explicit later owners |
| Legacy/new differential and forced-new-path fixtures | NP17–NP19 through `8f8c1825`; extended LM01 | LM19/LA05 | Retain focused admission, post-admission failure, dump, native and policy regressions; remove broad duplicates during cumulative review/adoption |
| Legacy x86 lowering/frame/machine/emitter | Pre-program production backend | LA05 adoption | Preserve unchanged as default and parity oracle during LA04 |

## Required documentation updates

Update the living low-level IR, backend, placement, frame, physical realization,
runtime ABI, driver/artifact, reporting, debugging and testing documents in the
task that changes their contracts. Update the migration coverage record with
actual delivery evidence after every family. Roadmap task codes stay in roadmap
history and must not leak into production symbols, comments, diagnostics or
living architecture text.
