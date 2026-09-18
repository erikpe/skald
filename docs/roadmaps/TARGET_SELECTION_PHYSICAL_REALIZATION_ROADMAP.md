# Target Selection, Checked Placement, and Physical Realization Roadmap

Status: in progress, 2026-09-18; NP01–NP11 complete; NP12 is next.
Accepted design: [frozen native target design](TARGET_SELECTION_PHYSICAL_REALIZATION_DESIGN_PROPOSAL.md).
Planning baseline: `8834bcd6`, the reviewed draft commit.
Implementation baseline: `f150d028`, immediately before NP01 code changes.
The working tree was clean; planning and model work were manually committed.
Program baseline: `495debd3`; completed model endpoint: `f59d3fff`.

Deliver a private whole-program scalar/control-flow/call native pilot through the
new phases. This establishes a target structure and checked placement interface
that a future allocator can use. It does not adopt the pilot as the production
backend or claim full-language migration.

## Scope and invariants

- Preserve the [phase contract](../archive/LOW_LEVEL_PHASE_ARCHITECTURE_DESIGN_PROPOSAL.md),
  [model contract and accepted streaming amendment](../archive/LOW_LEVEL_IR_MODEL_DESIGN_PROPOSAL.md#accepted-streaming-publication-amendment),
  [completed-model evidence](LOW_LEVEL_COMPILER_MIGRATION_COVERAGE.md#common-model-readiness)
  and [native obligations](LOW_LEVEL_COMPILER_MIGRATION_COVERAGE.md#target-implementation-obligations).
- Linux x86-64 SysV remains the only native pilot target. Synthetic second-target
  tests exercise shared contracts without registering an AArch64 backend.
- Admission matches the frozen pilot whitelist across the whole retained program.
  All admitted scalar operations, casts, checks, calls, trace policies and entry
  protocol must execute through the new phases. Unsupported input rejects;
  failures after admission never invoke legacy lowering.
- Planning alone accesses certified final MIR and checked semantic services.
  Selection, placement and physical owners consume narrow immutable phase facts.
  Keep facade-oriented modules cohesive; split implementation by responsibility,
  without empty framework modules or a universal cross-target opcode enum.
- Single definitions, memory/value/location separation, ordered effects, guard
  evidence, signature components and exact snapshot derivation remain mandatory.
  No semantic corrections, scratch, helper discovery or allocation can hide in
  emission. Every published stage has its independent required checks.
- Discovery receipts are discarded. Final selected and physical products reconcile
  exact executable-pass parent receipts before final artifact publication.
- Baseline placement uses unique private homes. Shared contracts/checking must also
  accept manually supplied register placements; no producer-only proof.
- Public CLI, target registry, diagnostics, runtime ABI and default legacy emission
  remain unchanged. No allocator, semantic SSA, scalar promotion, scheduling,
  persisted cache/importer, full lifecycle/helper migration or public LIR API.
- LA04 owns remaining language/helper/static migration, including complete-mode
  inactive-static reconciliation. LA05 owns default adoption, public observations,
  legacy removal and full foundation cost qualification. LA06 owns allocation.

## Progress

- [x] NP01 — Streaming publication authority
- [x] NP02 — x86 resources and component ABI
- [x] NP03 — Whole-program admission and fact projection
- [x] NP04 — Scalar memory and control-flow lowering
- [x] NP05 — Guarded arithmetic and conversion lowering
- [x] NP06 — Calls, traces and pilot entry lowering
- [x] NP07 — Concrete scalar selected payload and verifier
- [x] NP08 — Constrained numeric selection recipes
- [x] NP09 — Native call and trace selection
- [x] NP10 — Placement representation and checker contract
- [x] NP11 — Independent placement checking
- [ ] NP12 — Baseline placement and parallel transfers
- [ ] NP13 — Symbolic frame planning and limits
- [ ] NP14 — Typed physical realization
- [ ] NP15 — Independent physical verification
- [ ] NP16 — Typed program closure and fragment storage
- [ ] NP17 — Whole-program native pilot
- [ ] NP18 — Native hardening, observations and handoff
- [ ] NP19 — Cumulative review, cleanup and closure

## PR-sized implementation sequence

Each task updates relevant current API/architecture/test guidance and records
focused validation. Mark detail boxes as results are delivered; mark the summary
box only after its tests and exit criteria pass. A failed prerequisite blocks its
consumers. If a task cannot fit a reviewable PR, split it explicitly while keeping
its acceptance boundary; do not silently defer part of its contract.

### NP01 — Streaming publication authority

**Purpose:** Make callable streaming possible without weakening complete-program proof.

- [x] Record the actual child implementation baseline before the first code change; inspect committed model history and import exact retained-artifact owners from the handoff.
- [x] Implement plan-bound frozen target declarations and source selection from actual verified lower callables. Separate catalog authority from finalized program authority; discovery receipts cannot certify executable bodies.
- [x] Consume worklists at selected-program closure, reconcile exact chosen lower witnesses, and preserve parent-bound authority, thunk/data closure and replacement invalidation. Update model API documentation to describe implemented behavior.

**Tests:** Adapt existing inventory/selection tests without losing invariant coverage: wrong plan/profile/trace policy, wrong finalized parent, same-ID replacement, stale receipts, missing/late thunk, conflicting definitions, and premature publication. Demonstrate per-callable body release before lower-program closure.

**Exit criteria:** Catalog freeze and callable selection precede lower-program finalization; complete selected publication still proves exact parent derivation. No unchecked or permissive compatibility constructor remains.

### NP02 — x86 resources and component ABI

**Purpose:** Establish target facts shared by selection, checking and later realization.

- [x] Define concrete register banks/views, overlap and width preservation, reservations, caller/preserved footprints and encoding restrictions; keep flags atomic rather than virtual values.
- [x] Classify checked logical signatures into entry/call/return roles, independent integer/SIMD exhaustion and stack slots. Keep hidden result, receiver and alias-origin components explicit; distinguish internal component ABI from scalar C interop.
- [x] Freeze division, shifts, indirect-call target protection and trace event requirements with opcode/event walkthroughs. Reject unsupported external signatures explicitly; add no MIR queries to the target owner.

**Tests:** Table tests for six integer/eight SIMD arguments, seven/nine pressure, mixed components, hidden result and receiver, return roles, unsupported C aggregates/variadics, high-byte restrictions and partial preservation. Cross-check existing ABI witnesses, not only the new classifier.

**Exit criteria:** Immutable resource and ABI contracts explain all required operand events and clobbers before selected consumers are implemented.

### NP03 — Whole-program admission and fact projection

**Purpose:** Connect verified final MIR to checked planning facts through a narrow shared owner.

- [x] Implement the frozen pilot whitelist against all physically retained callable bodies, places, signatures, helpers and policies. Return explicit unsupported reasons before executable construction; never drop retained bodies to gain admission.
- [x] Project complete immutable signature/layout/declaration pools and canonical source/generated/data keys from existing checked services and certified domains. Keep frontend queries out of downstream phases.
- [x] Expose private admitted-plan construction for owner tests; preserve public backend input/errors and isolate enabled-only trace source lookup.

**Tests:** Admission positives across primitive/function-pointer forms and negatives for every excluded family, including statics, receiver-bearing source forms and unsupported intrinsics. Test complete/reachable retained domains, sparse IDs and omitted trace source lookup.

**Exit criteria:** An admitted whole-program plan has all required facts and immutable domains; unsupported programs cannot enter the private pilot or silently fall back.

### NP04 — Scalar memory and control-flow lowering

**Purpose:** Produce executable shared lowered bodies for ordinary scalar storage and graph structure.

- [x] Lower constants, scalar locals/loads/stores, primitive unary/binary operations, comparisons, jumps, branches and returns into the implemented lowered inventory with origins and explicit effects.
- [x] Keep semantic memory objects distinct from computed values and later placement storage. Preserve single definitions, edge occurrences, block parameters, evaluation order and certified retention.
- [x] Use ordinary builders, graph checks and lower worklists; remove inherited allowances on the APIs now consumed.

**Tests:** Verified lowered fixtures for loops/joins, duplicate-successor edges, local memory, signed/unsigned/float predicates, unreachable retained structure and sparse identities. Negative tests for malformed construction use final publication APIs.

**Exit criteria:** Admitted ordinary scalar/CFG bodies publish genuine lower receipts without target registers, offsets or late semantic queries.

### NP05 — Guarded arithmetic and conversion lowering

**Purpose:** Make failure and numeric semantics explicit before target recipes.

- [x] Lower the full admitted cast matrix, checked shifts, division/remainder and explicit checks using the closed operation inventory and required guard evidence.
- [x] Preserve failure kind/message/attribution and integer floor/minimum-overflow behavior, float range/NaN rules and full-width shift-count validation.
- [x] Record recipe associations and correction obligations that native selection must discharge; do not introduce new runtime helpers to conceal missing semantics.

**Tests:** Lowered guard/effect verification and independent semantic boundary cases for zero divisors, signed minimum/-1, negative floor/remainder, NaN/infinity/range boundaries and shift counts before narrowing. Test that forged or bypassed guards fail.

**Exit criteria:** All admitted numeric semantics and failures are explicit verified lower operations; target selection has no implicit semantic repair obligation.

### NP06 — Calls, traces and pilot entry lowering

**Purpose:** Complete the admitted shared lower inventory, including process protocol.

- [x] Lower scalar direct/indirect calls, admitted scalar C externs, reporter/hard-trap intrinsics and returns with checked signatures and ordered effects.
- [x] Implement shared enabled/omitted trace actions and attribution. Preserve result storage before trace cleanup, failure-only location updates and no source/TLS data requests when omitted.
- [x] Construct the minimal exported entry/runtime-marker/startup/shutdown protocol without statics; keep coordinators consistently empty or absent. Complete required lower worklist bodies/data.

**Tests:** Verified call/trace/entry fixtures, indirect-signature rejection, failure-only attribution, result preservation, no-return reporter then trap, omitted metadata isolation and runtime ABI marker/protocol checks.

**Exit criteria:** The entire admitted pilot can reach verified lower-program closure; lifecycle and aggregate source migration are still explicitly unsupported.

### NP07 — Concrete scalar selected payload and verifier

**Purpose:** Turn native opcode contracts into a target-owned selected representation.

- [x] Resolve [signature-boundary ABI slot validation](LOW_LEVEL_COMPILER_ARCHITECTURE_DISCOVERIES.md#abi-slot-shapes-must-be-local-to-the-signature-boundary) before native selected consumers; preserve context authority and strict per-boundary shape checks. Split an explicit prerequisite if necessary.
- [x] Implement immutable inspect/edit payload contracts and exhaustive opcode-derived descriptors for constants, bit operations, addresses, loads/stores, integer/float arithmetic, comparisons and graph flow.
- [x] Implement the mandatory independent x86 TargetVerifier against actual fields, resources, effects and encoding rules. Malformed drafts must be describable without panic; descriptor self-comparison is insufficient.
- [x] Preserve recipe/origin provenance and normalize parameter-transfer edges before selected publication; consuming edits create fresh identities and witnesses.

**Tests:** Positive payload/graph publication and malformed-field/descriptor/effect/resource tests; flag-bundle atomicity, NaN comparison semantics, pointer-width rules, duplicate edge occurrences and required edge forwarding.

**Exit criteria:** Ordinary native selected graphs receive joint shared/target verification; no opcode binds selected values to baseline stack homes.

### NP08 — Constrained numeric selection recipes

**Purpose:** Discharge guarded numeric semantics through explicit native instructions and correction CFG.

- [x] Implement concrete signed/unsigned dividend setup, divide-pair, fixed RAX/RDX requirements, divisor nonaliasing, destructive ties and CL shifts.
- [x] Implement exact integer/float conversion cells and checked correction recipes, including unsigned float ranges, floor division and minimum/-1 behavior; all semantic CFG precedes publication.
- [x] Extend the independent target verifier to reject substituted/bypassed recipe associations and undeclared scratch, calls or traps.

**Tests:** Selected recipe walkthroughs for every admitted cast cell and numeric boundary, fixed-register conflicts, live destructive inputs and full-width count guards. Independent reference cases establish intended semantics before native execution later in the roadmap.

**Exit criteria:** Numeric selection exposes all constraints and correction flow; realization will only expand finite declared straight-line recipes.

### NP09 — Native call and trace selection

**Purpose:** Expose ABI and trace requirements as selected events rather than emitter conventions.

- [x] Implement entry/call/return payloads, actual call clobbers, incoming/outgoing/result roles and secured indirect targets using the frozen component ABI.
- [x] Select TLS/trace memory operations explicitly for enabled traces; select no TLS or source metadata in omitted mode. Marshal simultaneously and preserve results before cleanup.
- [x] Use canonical pure target request rules for constants/failure bytes and supported thunks; unsupported thunk forms reject explicitly, supported bodies pass normal target verification.

**Handoff prerequisite (resolved):** Shared indirect-target checking now accepts
portable target-chosen early or late timing while requiring a typed use. Native
checking requires the frozen late R11 target contract; synthetic counterexamples
exercise both timings and reject an incorrectly typed target. No early-use
workaround remains.

The ABI-compatible pilot requires no native thunk. All thunk forms reject
explicitly; no stubbed body is introduced. Any future supported family must use
ordinary selected publication and target verification.

**Tests:** Selected scalar/C/indirect-call fixtures, pressure signatures, trace events/clobbers, secured target during marshaling, no-return behavior and missing/late request rejection. Walk both trace policies through complete selected descriptors.

**Exit criteria:** The full pilot has a concrete verified selected representation and reproducible discovery requests; no late runtime or ABI discovery is needed.

### NP10 — Placement representation and checker contract

**Purpose:** Freeze the algorithm contract before implementing acceptance or producer heuristics.

- [x] Define drafts bound to the exact immutable selected callable, assignments at every operand event and explicit entry/instruction/edge transfer points. Separate value storage, semantic objects and ABI slots.
- [x] Document finite abstract location contents, overlap/width kills, definition epochs, simultaneous edge parameter rebinding, conservative joins, loop convergence and unreachable structural checks. Define a finite bound and deterministic failure reporting.
- [x] Write independent counterexample fixtures for live ties, call-clobbered values, protected indirect targets, mixed-bank copies, duplicate edges and loop rebinding. Check a second synthetic target with non-x86 roles and partial preservation.
- [x] Checkpoint: establish that these cases have unambiguous expected acceptance/rejection under the frozen design. Stop and amend or split a focused proposal on a mismatch; do not invent a provisional checked-placement seal.

**Tests:** Representation validation and fixture/oracle tests; demonstrate same-ID stale selected inputs cannot confer authority. Review the checker state against cyclic CFG and overlapping-width counterexamples independently of the future producer.

**Exit criteria:** A concrete documented state/join/iteration contract and executable counterexamples are ready before checker implementation; drafts cannot authorize realization.

### NP11 — Independent placement checking

**Purpose:** Create the sole checked-placement authority from selected facts and transfer semantics.

- [x] Implement requirement reconstruction from the exact verified selected input, complete legal assignments, overlap/reservation/ABI/tie/event checks and independent transfer/clobber simulation.
- [x] Implement the specified finite CFG analysis through loops and joins; prove required tokens available at uses, preserve live tied inputs and account for simultaneous scratch lifetimes and width preservation.
- [x] Make CheckedPlacement privately constructible only after full checking; realization APIs require it. Keep checker logic independent of producer availability maps and success flags.

**Tests:** Run all preceding counterexamples plus manually register-resident positive placements. Corrupt otherwise valid placements one obligation at a time; test loop definition epochs, divergent joins, calls, scratch aliasing and unreachable structural defects.

**Exit criteria:** Every acceptance invariant is enforced without trusting the baseline producer; genuine register placements and deliberately invalid placements distinguish the interface from stack-only checking.

### NP12 — Baseline placement and parallel transfers

**Purpose:** Build the first producer against the completed strategy-independent checker.

- [ ] Assign deterministic unique private homes without reuse/rematerialization; load legal operand resources, honor ties/fixed events and store results, including entry and ABI marshaling.
- [ ] Resolve parallel copies deterministically with declared typed cycle-breaking scratch and legal memory-to-memory resources; no undeclared emitter temporary or push/pop scratch.
- [ ] Feed baseline output through independent checking and compare resolved transfer sequences against simultaneous-copy semantics. Remove producer scaffolding and allowances now consumed.

**Tests:** Baseline acceptance across the full selected pilot; two-/three-cycles, mixed-bank, memory copies, duplicate-successor edges, live ties, secured targets, caller clobbers and pressure. Permute construction order and require identical canonical results.

**Exit criteria:** The baseline produces checked placements for admitted selected bodies; register-holding manual fixtures still pass the same checker.

### NP13 — Symbolic frame planning and limits

**Purpose:** Freeze concrete frame and physical-state contracts before realization.

- [ ] Resolve [signature-local ABI area layout authority](LOW_LEVEL_COMPILER_ARCHITECTURE_DISCOVERIES.md#symbolic-abi-area-extent-needs-target-slot-layout) before publishing native ABI-area objects; representation byte widths do not establish SysV slot stride or area padding.
- [ ] Combine semantic objects with placement homes, spills, saves, outgoing areas and transfer scratch as typed symbolic requirements; compute deterministic checked offsets using conservative object lifetimes.
- [ ] Implement frame-pointer/fixed-outgoing-area policy, independent incoming slots, ABI alignment and width-aware preserved-resource save areas. Freeze supported x86 size/displacement limits and explicit rejection beyond bounds.
- [ ] Checkpoint: document entry/return stack states, CFG join rules, prologue/epilogue and forwarding provenance, legal addressing recipes and declared scratch bounds. Test narrower synthetic target displacements and non-x86 result/link roles before consumers.

**Tests:** Layout extent/alignment/nonoverlap and overflow boundaries, outgoing pressure, manual callee-save placements, partial preserved widths and permitted address materialization versus unsupported size errors.

**Exit criteria:** Frame plans and physical-checker rules are concrete and compatible with both baseline and nonbaseline placements; realization cannot discover storage or scratch.

### NP14 — Typed physical realization

**Purpose:** Expand checked selected operations into concrete target code without late semantic work.

- [ ] Implement physical drafts with concrete registers, offsets, immediates, blocks, relocations, calls and typed dependencies; retain exact selected/placement/frame provenance.
- [ ] Expand transfers and bounded declared recipes, prologue/epilogue, saves and ABI movements. Require CheckedPlacement and checked frame plans; no virtual operands, symbolic offsets or assembly-string proof.
- [ ] Keep target-owned instruction formatting reusable only as a typed leaf. Do not expose a final assembly entry until physical verification and program closure exist.

**Tests:** Physical draft fixtures for constrained arithmetic, conversion cells, frame/call pressure, trace memory, parallel copies and saves; inspect exact declared recipe footprints and reject unavailable sizes before expansion.

**Exit criteria:** Realization produces fully concrete typed drafts from checked inputs and introduces no new semantic edge, call, trap, dependency family or temporary.

### NP15 — Independent physical verification

**Purpose:** Authorize physical callable publication only after legality and state checks.

- [ ] Implement exhaustive opcode/register/width/immediate/displacement checks, closure of symbolic operands and relocations, declared recipe/provenance and dependency checks.
- [ ] Check CFG stack state, call alignment, saves/restores, return state, loops/joins and scratch footprints; linearly balanced stack adjustments are not sufficient.
- [ ] Publish VerifiedPhysicalCallable and exact derivation receipts only from this checker. Add deterministic immutable physical inspection and requested-only frame/placement observations.

**Tests:** Positive baseline/manual-register physical bodies and negative illegal encodings, unresolved references, undeclared scratch, wrong save widths, stack join mismatch, misaligned calls, bypassed epilogue and stale input provenance.

**Exit criteria:** No renderer/publication path accepts a physical draft; physical legality and selected derivation are enforced independently of realization.

### NP16 — Typed program closure and fragment storage

**Purpose:** Bound resident bodies while retaining exact whole-program artifact authority.

- [ ] Implement discovery/freeze and executable-pass orchestration with pure matching request rules; reconcile lower/selected/physical receipts against exact finalized parents and all required data/thunks.
- [ ] Render only verified physical callables into a private temporary fragment store keyed canonically. Retain typed dependencies/derivations; never parse fragments to discover symbols or prove correctness.
- [ ] Publish deterministic complete/reachable assembly only after full closure, preserving the final String interface. Account for store/read/write failures and cleanup of failed passes.

**Tests:** Wrong-parent and replaced-input closure, missing bodies/data/thunks, late requests, duplicate canonical keys, attempted premature emission, complete/reachable retention, randomized request order and fragment I/O failure cleanup. Verify predecessor bodies can be released.

**Exit criteria:** Only a completely reconciled physical program produces a checked final artifact; fragment text is storage, never authority.

### NP17 — Whole-program native pilot

**Purpose:** Exercise the entire new path from real source through execution.

- [ ] Add the private explicit pilot entry and owner integration harness requiring whole-program admission; ordinary public/default emission remains the legacy path with no pilot fallback.
- [ ] Run admitted source programs through projection, discovery, both construction passes, selection, checking, frame/physical publication and final artifact closure. Preserve entry protocol and runtime ABI.
- [ ] Add independent native phase/C probes for hidden result, receiver and alias-origin component pressure, without falsely claiming excluded aggregate source migration.

**Tests:** Source-to-native primitive/memory/loop/call/cast/division/failure witnesses; bidirectional scalar C calls and indirect targets; seven-integer/nine-float native component probes. Verify both trace policies, default/minimal MIR modes and complete/reachable artifacts using an explicit dimension matrix.

**Exit criteria:** The complete admitted pilot executes through every new phase with observable parity and mandatory native checks; excluded source forms and public production behavior remain accurately documented.

### NP18 — Native hardening, observations and handoff

**Purpose:** Close recipe, portability and policy gaps using final native interfaces.

- [ ] Complete adversarial numerical/ABI/trace/frame tests through the verified path; cover NaN predicates, unsigned float casts, minimum overflow/floor semantics, full-count shifts, live ties and caller-clobber result preservation.
- [ ] Validate requested-only immutable dumps/checkpoints and quiet mode without fabricating public phase events. Keep target-independent contracts free of x86 frame/register assumptions with the synthetic target.
- [ ] Update coverage with exact native witnesses and dimensions, remaining full-language/helper/static gaps, consumed allowance disposition and remaining removal owners. Record exploratory pilot cost evidence only; retain all foundation timing qualifications.

**Tests:** Native trace attribution, omission isolation, failure messages/termination, runtime marker, deterministic assembly across construction order and resource fixtures. Run applicable release and determinism gates; use independent native probes where legacy output alone cannot establish semantics.

**Exit criteria:** The pilot validation matrix has no unqualified coverage claims; LA04 receives full migration and inactive-static contract obligations, LA05 public observation/adoption/cost obligations, and LA06 allocation obligations.

### NP19 — Cumulative review, cleanup and closure

**Purpose:** Review committed and uncommitted work as one architecture change before closing.

- [ ] Review child baseline..HEAD with stat/name-status/full diff plus staged, unstaged and untracked changes. Inspect relevant foundation history and current owners; distinguish unrelated intervening changes. Manual task commits do not remove cleanup obligations.
- [ ] Reconcile every artifact ledger entry against source/history, including inherited allowances, bridging APIs, draft consumers, extracted adapters, fragment experiments and private pilot gates. Remove expired artifacts; transfer necessary continuing items with exact file/symbol and owner.
- [ ] Fix small ownership/interface/diagnostic inconsistencies. Resolve substantial design/correctness gaps in explicit tasks before closure; record independent opportunities in the indexed architecture discoveries.
- [ ] After fixups, run artifact-free full validation and supported-toolchain/native gates. Record reviewed baseline/endpoint, residual closing changes awaiting user commit, artifact disposition and results.
- [ ] Mark completion only after all exit criteria pass; archive this roadmap and the frozen target design, repair links/indexes and update parent, coverage and audit readiness. Keep full migration/adoption/allocation pending; leave committing to the user.

**Tests:** make check and serial make msrv-check from an artifact-free snapshot; make check-long for extended native/release/determinism coverage. Check final documentation links, whitespace, cumulative and closing diffs. If a gate fails, fix or explicitly reopen the responsible implementation task rather than marking closure.

**Exit criteria:** One coherent checked private native pilot remains, every temporary artifact has a verified disposition, full gates pass, and archived decisions plus the active handoff accurately bound the next workstream.

## Ordering and dependencies

The default sequence is serial and follows authority: amended publication,
target facts, admitted planning, complete shared lowering, native selection,
placement contract/checker/producer, frame contract, realization/checker,
complete artifact closure, native integration and hardening, then closure.
NP02 and NP03 have distinct owners but both depend on the publication boundary;
do not assume concurrent implementation or bypass the listed checkpoints.

NP10 freezes the concrete checker algorithm before NP11. NP13 freezes physical
state/encoding bounds before NP14–NP15. Numeric/call schema walkthroughs in NP02
precede NP07–NP09. The recorded contracts and executable counterexamples are the
review artifacts for these checkpoints. A pass permits dependent implementation;
a failure pauses those tasks, records the exact mismatch and requires an explicit
owning-design amendment or focused child proposal. Never substitute permissive
verification or an unchecked bridge. There is no measured allocator go/no-go in
this roadmap: full foundation cost acceptance remains with adoption.

## Validation and evidence

Use owner-local Rust tests for private construction/checker/recipe behavior;
crate integration tests for cross-owner boundaries, existing reusable corpora for
source cases, and golden/public tests for production behavior. Native pilot tests
must explicitly require the new entry: an unsupported skip or legacy result is
not new-path evidence. Hand-built aggregate ABI probes complement scalar source
coverage; they do not qualify full object/lifecycle migration.

Every implementation task runs proportionate focused tests plus `make check`;
run `make msrv-check` when Rust targets/manifests/syntax change. Run native probes
when their complete verified path first exists, then strengthen coverage using
final interfaces. Record deferred execution witnesses until run; schema tests
alone never certify numeric or ABI native parity. Track a compact matrix of
trace enabled/omitted, ordinary/minimal MIR, complete/reachable artifacts,
source/native phase probes, supported operations and deliberate rejection cases.
Choose representative combinations plus targeted interactions; justify missing
cells rather than blindly multiplying every case.

Run applicable release/determinism checks for native changes; closing validation
uses `make check-long` plus the ordinary artifact-free gate. Run golden/MSRV
commands serially: the [architecture discoveries](LOW_LEVEL_COMPILER_ARCHITECTURE_DISCOVERIES.md)
track the independent concurrent artifact race. Do not fix it by broadening this
roadmap or accept a contaminated run as evidence.

The [foundation measurement protocol](../development/LOW_LEVEL_COMPILER_MEASUREMENTS.md)
and preserved compiler `9e3cebb1` remain authoritative. Pilot timing/code/frame
counts are exploratory, not full-corpus adoption evidence. Do not rewrite historic
baseline records or clear the eleven inconclusive timings. If the measurement
harness changes, adoption must recapture compatible paired baseline/candidate
runs. No stack placer result proves allocator benefits.

## Temporary artifact ledger

Inherited owner groups and introducing commits were imported in NP01. Update
introduction commits from history when the user commits; a clean working tree is not a disposition.
Expand grouped entries as artifacts actually arise. No speculative artifact is
permission to introduce it unnecessarily.

| File/symbol or artifact | Introduction | Removal/transfer owner | Required final disposition |
| --- | --- | --- | --- |
| `backend/effects.rs`: `MemoryRegion`, `Effect`, `Effects` and their checked set/remapping methods | `68dec8ed` | First native effect/verification consumers; conservative barriers remain durable; reconcile NP19 | Scoped allowances remain until native consumption; durable APIs remain. Audit all annotations in these files, not only principal symbols |
| `backend/plan/{facts,identities,check,view,services}.rs`, `plan/mod.rs`: `PlanFacts`, `CheckedPlan`, `PlanView`, typed IDs/declarations, checked services and explicit re-export groups | `ff12421d`, services `68dec8ed` | First native planning/lowering consumer; remaining full-surface facts retire their allowances with complete migration; reconcile NP19 | Scoped allowances remain until native consumption; durable APIs remain. Audit all annotations in these files, not only principal symbols |
| `backend/graph/{arena,edit}.rs`, `graph/verify/{model,check,analysis}.rs`, `graph/mod.rs`: `OwnedArena`, `IdMap`, `GraphView`, `check_graph`, `GraphSession` and explicit re-export groups | `ff12421d`, verification `eecdb4cc`, edits `03e4ae42` | First native graph/analysis/edit consumers; shared algorithm stays durable; reconcile NP19 | `check_graph` allowance retired NP04; remaining analysis/edit allowances stay until their consumers. Durable APIs remain. Audit all annotations in these files, not only principal symbols |
| `backend/lir/{model,scalar,builder,schema,read,call,trace,observable}.rs`, `lir/graph/{storage,operands}.rs`: draft records, `DraftBuilder`, `DraftChecks` and graph adapter impls | `43df9ce6`, effects `68dec8ed`, graph adapters `eecdb4cc` | First native lowered construction/checking consumers; remaining vocabulary with complete migration; reconcile NP19 | `DraftBuilder` allowances retired NP04; division/shift/conversion descriptor allowances retired NP05. `Conversion::PointerBits` stays narrowly scoped until LA04 full migration; other vocabulary allowances stay until their consumers. Call construction/checker and trace construction/checker allowances retired NP06. Durable APIs remain. Audit all annotations in these files, not only principal symbols |
| `backend/lir/verify/{check,domains,memory,trace,lift,failure,publication}.rs`: full checking helpers, structured failures, `VerifiedCallable` and `CompletionReceipt` | `e87fa392` | First native lowered verification/publication consumer; no replacement with builder trust; reconcile NP19 | Callable checking and publication/receipt allowances retired NP04; numeric guard-checker allowances retired NP05; trace checker allowances retired NP06. Callable analysis remains scoped until its consumer. Durable APIs remain. Audit all annotations in these files, not only principal symbols |
| `backend/lir/program/{inventory,data,target}.rs`: `ProgramBuilder`, `VerifiedProgram`, data validation, `TargetDeclarations`, `TargetCatalog` | `ddc5a97d`; catalog migrated NP01 `2cbd7ffd` | First native inventory/discovery consumer; preserve streaming receipts, plan freeze and exact parent reconciliation; reconcile NP19 | Worklist begin/completion/state allowances retired NP04; constructor, data construction/validation and final closure allowances retired NP06. Other catalog/program allowances remain pending their consumers. Audit all annotations in these files, not only principal symbols |
| `backend/selected/{storage,context,builder,graph,abi,resources,description}.rs`: `SelectedDraft`, `SelectionContext`, `SelectedBuilder`, `Payload`, ABI/resource records and descriptions | `63291d6d` | Real target schema/selection consumer; retain immutable opcode-derived descriptions; reconcile NP19 | Scoped allowances remain until native consumption; durable APIs remain. Audit all annotations in these files, not only principal symbols |
| `backend/selected/verify/{check,descriptors,failure,publication,program}.rs`: shared checking helpers, `TargetVerifier`, `VerifiedSelectedCallable`, `SelectedReceipt`, `SelectedProgramBuilder` | `73b1fafe`; closure migrated NP01 `2cbd7ffd` | First native selected verification consumer; both shared and target checks remain mandatory; reconcile NP19 | Scoped allowances remain until native consumption; durable APIs remain. Audit all annotations in these files, not only principal symbols |
| `backend/{lir,selected}/edit/{mod,editor,rebuild}.rs`, `lir/edit/split.rs`: `LoweredEditor`, `SelectedEditor`, `LoweredRemap`, `SelectedRemap`, remapping/rebuilding and split helpers | `03e4ae42` | Native edits/analysis consumers; retain consuming authority and full reverification; reconcile NP19 | Scoped allowances remain until native consumption; durable APIs remain. Audit all annotations in these files, not only principal symbols |
| `backend/{inspection,lir/inspect,selected/inspect}.rs`, `lir/mod.rs`, `selected/mod.rs`: visitors/renderers, immutable enumeration and explicit facade re-export groups | `495df6b9` | First native inspection/checkpoint consumers; no fabricated observations; reconcile NP19 | Scoped allowances remain until native consumption; durable APIs remain. Audit all annotations in these files, not only principal symbols |
| `backend/x86_64_sysv/native/{mod,resources,abi}.rs`: `Gpr`, `NativeResources`, `ComponentAbi`, classifier, explicit facade imports and scoped non-test allowances | NP02, baseline `2cbd7ffd`; introducing commit `9a9e3bc1` | NP07/NP09 actual native selection consumers; NP12/NP13 placement/frame consumers; reconcile NP19 | NP07 retired resource/classifier/type/entry allowances. NP08 retired call/noreturn and caller-clobber allowances through the numeric reporter consumer. Only outgoing extent, preservation and REX queries retain item-scoped allowances until NP12–NP14. Tests retain lint checks; no legacy adapter or production switch |
| `backend/pilot/{mod,facts,projection}.rs`, `x86_64_sysv/pilot_facts.rs`: private admission facade, admitted getters, projection entry points and item-scoped non-test allowances | NP03, baseline `9a9e3bc1`; introducing commit `879bfb47` | NP04/NP06 lower consumers; NP17 private pipeline; reconcile NP19 and LA05 public adoption | Durable checked planning boundary; program/plan/layout/signature getter allowances retired NP04; trace fact allowances retired NP06; admission entry remains scoped until private orchestration. No temporary lowerer, fallback, fake intrinsic input or production switch |
| `backend/pilot/lower/`: shared adapter and private facade allowance | NP04, baseline `879bfb47`; introducing commit `a4c3f189` | NP05 numeric lowering; NP06 calls/trace/entry and program construction; NP17 private pipeline; reconcile NP19 | NP05 retired numeric pending gates. NP06 retired all remaining pending-feature variants, duplicated preflight and call/trace/entry rejection branches, and replaced the intrinsic test bypass with complete worklist closure. Retain shared adapter and streaming `lower_program`; its private entry allowance remains until NP17 orchestration. No fake body, trap, receipt or fallback |
| `backend/pilot/lower/tests/oracle.rs`: private numeric fixture execution oracle | NP05, baseline `a4c3f189`; introducing commit `430a30bc` | Permanent owner-local tests; reconcile purpose NP19 | Retain independent lowering association/boundary evidence, with unsupported operations rejected. No production interpreter, target parity claim or executable authority |
| `backend/x86_64_sysv/native/selected/{context,select}.rs`: private entry non-test allowances and explicit unsupported recipe preflight | NP07, baseline `5369ea69`; committed with NP08 as `e35be34f` | NP08 numeric/failure recipes, NP09 calls/trace; NP17 private orchestration; reconcile NP19 | Retain real immutable scalar selector/verifier, no fake body or fallback. NP08 retired numeric/check/failure preflight and rejection cases; NP09 retired the call/trace and nonreturning/trap gates; out-of-pilot pointer/scaled-index recipes still reject. Retire entry allowances when orchestration consumes them |
| `backend/x86_64_sysv/native/selected/{numeric,recipes,verify/numeric}`: concrete numeric cells, domains and zero-code operation/result associations | NP08 atop uncommitted NP07, HEAD `5369ea69`; committed together as `e35be34f` | Durable target checking metadata; NP14 finite expansion, reconcile NP19 | Retain markers only while independently checking actual CFG/definitions; no machine instruction, operand, hidden correction or emission authority |
| `backend/x86_64_sysv/native/selected/tests/oracle.rs`: concrete selected-cell test interpreter | NP08 atop uncommitted NP07; committed as `e35be34f` | Permanent owner-local recipe evidence, reconcile NP19 | Test-only, independent of selector/verifier mappings; rejects unsupported cells; never production execution or native parity authority |
| Native `Opcode::Failure` and shared call trace-barrier clarification | NP08 numeric prerequisite; commit `e35be34f` | NP09 general call/trace consumers; NP14 declared reporter/trap expansion; reconcile NP19 | Durable narrow canonical reporter terminal; retire no implementation bridge. General calls/traces are selected by NP09 |
| `SelectionContext::abi_areas`, `ObjectRole::Abi`, selected slot checks | Global shapes introduced LA02; replaced NP07, baseline `5369ea69` | NP07, checked again NP19 | Global shape storage and unqualified ABI objects removed; one signature registry preserves shared program authority; no compatibility adapter |
| Finalized-parent construction compatibility adapters, if needed | NP01; record exact symbols/commit | NP01, checked again NP19 | NP01 directly replaced the old API; no compatibility adapter or alias introduced |
| Frozen lower trace facts in `lir::CompletionReceipt`; native call fields, concrete trace cells and pure requests | NP09, task baseline `e35be34f` | NP10–NP16 placement/realization/closure; reconcile NP19 | Durable narrow immutable facts survive body release; no retained executable lower body, opaque trace payload, thunk bridge or instrumentation |
| `backend/placement/{model,structure}.rs`: unchecked drafts and non-test dead-code allowances | NP10, task baseline `3413f792`; committed `1d0b84e6` | NP11 checker and NP12 producer; reconcile NP19 | NP11 retired module-wide model/structure allowances and consumed draft/type allowances; retain only draft builder methods and storage-purpose construction allowances until NP12/NP13. Structural validation grants no authority; no producer success flag or realization bypass |
| `backend/placement/check.rs`: private immutable checked queries; native `check_native_placement` entry and facade allowance | NP11, task baseline `1d0b84e6` | NP12 producer, NP13/NP14 frame/realization consumers and NP17 private orchestration; reconcile NP19 | Sole checked constructor runs legality, finite convergence and strict replay. Retain query/entry allowances only until actual consumers; no unchecked constructor, mutable draft accessor or producer flag |
| `backend/placement/tests/oracle.rs`: independent finite-state specification and native resource probe | NP10, task baseline `3413f792` | Durable independent checker regression reference, reconcile NP19 | Remains test-only and independent of producer/checker availability maps. No production path sampling, placement authority or temporary execution bridge |
| Discovery-only receipts/request instrumentation | NP16 orchestration; record exact symbols/commit | NP16/NP19 | Discovery receipts never become executable authority; remove exploratory instrumentation, retain pure request rules |
| Draft physical test consumers/renderer shortcuts, if introduced | NP14; record exact symbols/commit | NP15–NP16 | Final emission requires verified physical callable and complete program closure; no unchecked production route |
| Fragment storage experiments/adapters, if introduced | NP16; record exact symbols/commit | NP16/NP19 | Retain only failure-safe typed-key storage with a demonstrated bounded-body purpose; no serializer/importer/cache |
| Private explicit pilot entry/admission gate | NP17; record exact symbols/commit | LA05 adoption, transfer NP19 | Required private test consumer until adoption; retire or become the sole production entry, never a permanent fallback route |
| Synthetic second-target fixtures and manual register placements | NP02/NP10 onward; record owners/commits | Permanent owner-local tests, audit NP19 | Retain independent portability/checker witnesses; no production registration, exports or blanket test-only phase gating |

## Discoveries and closure record

Record independent candidates in the existing indexed
[architecture discoveries](LOW_LEVEL_COMPILER_ARCHITECTURE_DISCOVERIES.md), with
evidence, owner, priority and a bounded follow-up. Implement small maintainability
fixes that support the current task directly; substantial design/correctness gaps
must be resolved before dependent tasks or closure rather than hidden there.

Closing record (fill in NP19): implementation baseline; reviewed committed endpoint;
uncommitted closing changes; cumulative ownership/contract review outcome; artifact
dispositions and transferred owners; artifact-free ordinary/MSRV/extended native
validation results; final handoff and archival links. The user performs commits.

### Streaming publication implementation record

NP01 baseline: `f150d028`; all earlier model/planning changes are committed.
`TargetCatalog` replaces `TargetExtension` directly, with one plan lifetime and
no alias or temporary adapter. `SelectedBuilder` requires a genuine verified
lower body; source snapshots reconcile only at consuming selected-program closure.
The resulting program borrows its finalized parent, and selected edits preserve
that exact publication binding. Existing invariant/inspection/edit tests use the
final APIs. New owner tests exercise callable release before closure and hostile
receipt/parent/catalog substitutions. Native orchestration remains NP16.

Inherited non-test allowances remain because the new APIs still have only private
model/test consumers. No native consumer has landed; removal is due as each later
consumer arrives, with remaining full-surface obligations transferred to LA04.
Validation: 602 focused backend tests passed, including seven new streaming
regressions. `make check` passed (3,300 compiler unit tests, workspace/integration/
documentation/support checks and 650 golden cases). Serial `make msrv-check`
passed with Rust 1.82.0. Documentation links and whitespace checks passed. No
compatibility scaffold or independent discovery was introduced; committing stays
with the user.

### Native resource and component ABI implementation record

NP02 baseline: `2cbd7ffd`, the manually committed publication task. The native
owner supplies complete conservative register units/views and caller/preserved
footprints, reserved stack/frame aliases, low-byte encoding facts and clobber-only
flags. Classification derives immutable entry/call/result bindings from checked
signatures, canonical logical roles and independently exhausted argument banks.
Symbolic spill shapes remain local to each signature; no native selection or
production emission path is introduced. The [native contracts](../compiler/X86_NATIVE_CONTRACTS.md)
freeze constrained numeric/call/trace walkthroughs for later consumers.

The ABI boundary-shape discovery is an explicit NP07 prerequisite, with wider
work split before consumers if needed. Native facts are implemented and tested;
shared selected integration is not claimed. Scalar representation mapping is
shared between component classification and common ABI verification to avoid
duplicate semantic tables.

Validation: eight native-owner tests and all 32 shared selected tests passed.
`make check` passed (3,308 compiler unit tests, workspace/integration/support
checks and 650 golden cases). Serial `make msrv-check` passed with Rust 1.82.0.
Documentation link and whitespace checks passed. No compatibility adapter,
production switch or exploratory emitter was introduced. Native fact allowances
remain scoped and ledgered until their actual consumers arrive. Committed by the user as `9a9e3bc1`.


### NP03 implementation evidence

Task baseline: `9a9e3bc1`, the user-committed native-resource/component-ABI task.
The working tree was clean. The shared `backend::pilot` owner now admits the
whole physically retained program before freezing a checked plan. Every retained
body is inspected under both artifact policies; unsupported bodies are never
trimmed or routed to legacy emission. Complete emission also rejects unsupported
generated metadata/lifecycle families; reachable emission checks certified runtime
obligations. Receiverless scalar static methods remain eligible when no unsupported
family is retained.

The admitted product borrows the inspected MIR snapshot and owns checked pools,
semantic layout maps, exact canonical higher-order signature maps, present/absent
source declarations, entry, C/runtime declarations and pilot failure data.
Source signature equality, rather than physical-cell equality, selects canonical
code signature IDs. Unused aggregate/alias declarations remain inspectable without
acquiring executable authority. No lifecycle coordinator body is declared when
static activation is empty. Narrow x86 service adapters reuse checked layout and
canonical trace planning and discard target symbols; enabled metadata is owned
and typed, while omitted tracing performs no source lookup or trace declaration.

Validation: all 11 pilot-owner regressions passed, including excluded-family
fixtures, sparse callable holes, complete/reachable policy behavior, canonical
receiverless method addresses, physically equal but semantically distinct absent
alias signatures, C scalar cells, real normalized bit/I/O intrinsics, string panic,
and enabled/omitted source access. `make check` passed (3,319 compiler unit tests,
workspace/integration/support checks and all 650 golden observations). Serial
`make msrv-check` passed with Rust 1.82.0; documentation and tracked/untracked
whitespace checks passed. New planning APIs remain private and item-scoped lint
allowances are ledgered for their actual consumers. No executable lowerer,
compatibility bridge, fallback, fake intrinsic input or production switch remains.
The existing ABI slot-shape prerequisite is unchanged; no additional independent
follow-up was identified. Implementation committed by the user as `879bfb47`.

### NP04 implementation evidence

Task baseline: `879bfb47`, the user-committed admission/projection task; the
working tree was clean. Shared lowering now reserves the actual retained block
and value domains, declares semantic storage objects, initializes logical inputs,
and lowers ordinary constants, addresses, local memory, lifetimes, unary/binary
operations, comparisons and control flow. Each completed body passes ordinary
full verification before its exact receipt is registered in the lower worklist.
Unreachable blocks and duplicate edge occurrences are preserved. MIR values are
block-local, so block parameter and edge argument lists are empty; semantic locals
remain memory rather than being implicitly promoted. Source value/storage origins,
raw floating bits, canonical callable signatures and explicit derived effects
survive this boundary. No target register, home or frame offset is selected.

Consumed allowances were removed from `DraftBuilder`, graph/callable checking,
callable/receipt publication, worklist construction state and begin/completion,
and admitted program/layout/signature getters. Worklist `new`, its declared-state
variant, data/final closure, callable analysis and admitted trace facts retain
narrow non-test allowances pending their actual orchestration/analysis consumers.
The private lowering entry retains one scoped non-test allowance until the pilot
pipeline calls it. Pending-feature preflight rejects numeric checks/casts, calls,
tracing and entry before work begins; NP05/NP06 remove those gates as their real
construction becomes available. Per-body success cannot fake program closure.

Eight owner regressions cover loops/joins, parameter and byte/bool memory,
primitive predicates and arithmetic, raw NaN payloads, canonical callable
addresses, sparse retained identities, duplicate edges, retained unreachable
structure, foreign contexts, pending features and malformed final publication.
Focused tests and `make check` passed (3,327 compiler unit tests, workspace,
integration/support/runtime checks and all 650 golden observations). Serial
`make msrv-check` passed with Rust 1.82.0. Final documentation/index and
tracked/untracked whitespace checks passed.
The existing ABI slot-shape prerequisite remains assigned to NP07; no additional
independent discovery was identified. Committed by the user as `a4c3f189`.

### NP05 implementation evidence

Task baseline: `a4c3f189`, the user-committed ordinary scalar/CFG lowering task;
the working tree was clean. Shared numeric lowering now consumes actual verified
MIR check diamonds. It moves the exact secured success load into the check block,
retaining the source value identity/origin and using it for both the check relation
and guarded operation. No reload is treated as evidence for an earlier value.
All retained graph structure, result storage and remaining evaluation order survive;
ordinary locals remain semantic memory. Full callable publication independently
checks the resulting definitions and success-edge protection.

The complete primitive cast matrix, float bit cells, quotient/remainder and all
integer shift directions lower into the existing closed operations. Typed numeric
failure reporters preserve exact failure data, source attribution and effects
through the existing runtime panic declaration. No extra helper, fake trap/body,
unchecked receipt or production route was introduced. Enabled tracing remains
explicitly pending until NP06, including failure-only location updates. The
[shared numeric handoff](../compiler/LOW_LEVEL_IR.md#guarded-numeric-lowering)
records the operation/guard associations and explicit overflow, floor, rounding,
unsigned conversion and count-narrowing obligations for NP08 selection.

The numeric pending variant and both its preflight and dispatch rejection branches
were removed from committed NP04 scaffolding. Consumed division/shift/conversion
vocabulary and guard-checker allowances were retired; only the out-of-pilot pointer
conversion variant retains a narrow non-test allowance until full migration.
Tests are grouped by ordinary and numeric responsibility. Their private numeric
oracle remains a small fixture-only independent execution witness; unsupported
operations fail, and it grants no native equivalence or executable authority.
Retain it while it distinguishes shared lowering relationships from native recipes,
and reconcile that purpose in NP19 rather than promoting it into a language VM.

Seven numeric regressions extend the eight ordinary regressions: the 25 cast cells,
constant division/shift/cast boundary execution, NaNs/infinities/range endpoints,
real normalized bit intrinsics and payloads, exact reporting data/effects/origins,
forged/bypassed guards and numeric definitions in loops. Focused numeric/ordinary
checks passed. `make check` passed (3,334 compiler unit tests, workspace,
integration/support/runtime checks and all 650 golden observations). Serial
`make msrv-check` passed with Rust 1.82.0. Final documentation/index and
tracked/untracked whitespace checks passed. Native numeric
recipe execution remains due in NP08 and the whole-program pilot; these tests do
not claim native parity. The existing NP07 ABI prerequisite is unchanged, and no
additional independent discovery was identified. Changes await the user's commit.


### NP06 implementation evidence

Task baseline: `430a30bc` (the user's committed NP05); roadmap baseline remains
`f150d028`. History review identified the remaining pending-feature gates and
normalized-intrinsic direct-construction test bypass; both are removed rather
than hidden by the clean starting tree.

The shared adapter now lowers scalar direct/indirect calls, C externs and sparse
receiverless static calls with checked logical signatures and ordered effects.
Numeric reporter terminals retain exact failure bytes, source attribution and
mandatory report/hard-trap effects; normalized binary64 bit intrinsics remain
conversions. The admitted source surface has no standalone hard-trap intrinsic;
user string panic remains unsupported. Concrete reporter-then-defensive-trap
instructions belong to selection and are not claimed by this lower-only task.

Enabled source frames own the existing two-word runtime record, with its
size/alignment projected from the legacy target's canonical constants. A bounded model
clarification adds `TracePlan.initial_location`, required for eligible frames and
checked against their declared locations, so selection cannot recover it from MIR
or a source database. Push precedes parameter stores, location replacement is
immediately adjacent to each attributed call/reporter, failure replacement occurs
only in the failure block, and result computation/source storage precede return
pop. Generated entry has process-boundary calls and no source frame. It calls the
ABI marker then main, returning the exact result; admission excludes statics, so
coordinators are absent. Omitted mode requests no source/TLS metadata.

Streaming `lower_program` passes verified bodies to a fallible consumer and keeps
only completion receipts. Frozen failure/trace bytes, context and location data
are constructed with checked relocations before final lower-inventory publication.
Trace TLS zero storage remains a physical target artifact obligation. All admitted
body/data inventory can now close; consumer failure cannot publish authority.
Public emission stays legacy; lifecycle/aggregate migration and native equivalence
remain unimplemented.

Validation: `make check` passed (3,340 compiler tests, all workspace and runtime
suites, and 650 golden cases), followed by serial `make msrv-check` on Rust
1.82.0. The final trace-layout projection/allowance cleanup passed
`make static-check` and all 32 pilot owner tests. `git diff --check` passed.
Owner fixtures cover
calls, entry order/result identity, enabled/omitted tracing, failure attribution,
result stores before pop, indirect signature rejection, sparse static methods,
streaming-consumer failure and bit-intrinsic callers. Independent publication
rejects missing or undeclared initial trace locations. The next task is NP07,
including its signature-boundary ABI slot prerequisite.

### NP07 implementation evidence

Task baseline: `5369ea69` (committed NP06). Earlier bodies and gates were reviewed
from history; no reset or assumption of an uncommitted predecessor was used.
Implementation remains uncommitted for the user.

The native selected owner has cohesive opcode, description, editing, selection,
context and independent payload/callable-verification modules. Ordinary integer/
floating operations, constants, code/object addresses and memory/CFG publish
with virtual register constraints, declared ties/scratch/flags and lower recipe
provenance. Conditional parameter edges use separate forwarding blocks per
occurrence; stack homes remain exclusively a later placement responsibility.

The ABI prerequisite is resolved with one signature-keyed area registry; incoming
native and outgoing shared publication regressions cover heterogeneous slot zero
without changing live context authority. Consumed selected builder/context/ABI
and checking allowances and native fact/classifier allowances were retired;
remaining unused future queries have narrow item allowances. No unused move
opcode, compatibility bridge or exploratory execution path remains.

Owner tests cover unordered IEEE predicates independently of native mappings,
malformed concrete fields/caches/events/resources, total malformed descriptors,
pointer-layout rejection, object address substitution, edge forwarding,
consuming splits/rebuilds and stale receipts. Numeric/failure and call/trace
recipes reject explicitly until NP08/NP09. Production and execution coverage
remain unchanged; native execution evidence is due with the complete pilot.

The ABI-area object checker still sums representation widths, while SysV stack
slots have eight-byte stride. No native ABI-area object is introduced here. The
indexed layout-authority discovery is an explicit NP13 prerequisite; it must be
resolved before physical areas are published, without putting x86 layout into
shared shape checking.

Validation: `make check` passed (3,351 compiler owner tests, all workspace,
integration, documentation and runtime checks; all 650 golden observations).
Final native owner rerun passed all 10 tests after test hardening; shared selected
owner regressions passed all 33 tests. `make msrv-check` passed on Rust 1.82.0. Final `make static-check` passed with the completed documentation and hardened
tests; links/indexes and the working diff are valid.


### NP08 implementation evidence

Task baseline: HEAD `5369ea69` plus NP07's uncommitted selected implementation.
The task-start selected owner and roadmap were captured under
`/tmp/skald-np08-start` for local comparison; that private snapshot is not a git
commit or durable artifact. Roadmap baseline remains `f150d028`. NP07 work is
preserved, and both tasks await the user's manual commit.

Numeric selection publishes concrete dividend setup/divide pairs, fixed RAX/RDX
and CL requirements, nonaliasing divisor choices and declared destructive ties.
Explicit selected diamonds implement floor correction, MIN/-1, unsigned
integer-to-float rounding and unsigned float-to-integer correction; ordinary
casts and bit reinterpretations expose closed concrete cells. Raw cells use no
implicit scratch, helper, branch or emitter semantic repair. Zero-code semantic
association markers are retained as independently checked target metadata.

Independent target checking reconstructs actual guards, constants, source and
result definitions and protected arms. It rejects guard bypass/substitution,
wrong correction results, wrong full-count narrowing and overflow paths. Genuine
consuming edits remap all metadata and relocate concrete overflow branches while
retaining immutable lower origins. Selected visitors now permit session-local
borrows without cloning payloads; builder handle recovery is ownership checked.
Recipe construction propagates structural/arena failures instead of panicking.

NP08 also supplies the narrow numeric failure prerequisite: canonical panic ABI,
message/length, attribution, complete caller clobbers and atomic call/UD2 contract.
The accepted effect clarification preserves the service's unknown memory read
and trace-state call barrier without requesting caller TLS in omitted mode.
Non-call trace effects still require enabled tracing and TLS. General calls,
tracing, entry call selection and physical realization remain with their scheduled
tasks. Numeric/check/failure preflight gates and consumed native call/noreturn/
caller-clobber allowances are retired; no compatibility selector or fake body
remains.

Owner regressions independently execute concrete selected cells for all 25 cast
cells, normalized bit intrinsics with NaN payloads/signed zero, division/remainder
floor and overflow boundaries, full-width shifts and live inputs. They cover
nested correction joins, loops, malformed same-type substitutions, swapped guard
edges, reporter arguments/clobbers/effects and consuming rebuilds/splits. Shared
publication independently distinguishes omitted-mode call barriers from forbidden
explicit trace actions. These are selected recipe witnesses, not actual native
execution or placement-conflict simulation; those remain required later.

Validation: `make check` passed (3,364 compiler tests, workspace/integration/
documentation/runtime suites and all 650 golden observations). Serial
`make msrv-check` passed without warnings on Rust 1.82.0 after final test
cleanup. The task-start comparison retains all predecessor regressions and retires
only the numeric pending rejection; tracked/untracked whitespace and final
documentation checks passed.
No additional independent follow-up was identified; existing frame-layout and
golden-artifact discoveries retain their owners.


### NP09 implementation evidence

Task baseline: `e35be34f`, the user's combined NP07/NP08 commit; the working tree
was clean. The roadmap implementation baseline remains `f150d028`. History and
current-source review identified and removed the committed general call/trace
and nonreturning/trap gates and their obsolete rejection test. Scalar/numeric
selection and predecessor regressions remain intact; committing stays with the
user.

Calls expose canonical ordered input/result components, full late caller clobbers,
fixed secured R11 indirect targets, logical signatures and attribution. Generated
entry uses the same selector and preserves the marker/main protocol. Nonreturning
calls declare atomic call/UD2 terminals, while standalone hard traps are explicit.
The shared indirect check permits portable target-defined event timing; native
checking independently reclassifies ABI roles and enforces its concrete contract.

Enabled trace actions become TLS addresses and explicit record/head loads/stores,
with mandatory memory/trace effects. The call-free local-exec address recipe has
two steps, no scratch/flags and an explicit FS:0 memory read. Independent checking
reconstructs the actual memory sequence, source record/initial location,
site-to-location attribution, adjacency and CFG frame balance. Narrow immutable
trace facts remain in completion receipts after lower executable body release;
omitted mode creates neither these retained facts nor TLS/trace-memory operations.
Consuming rebuilds remap record origins and concrete operands. This retention
clarification is recorded in the frozen target design.

Pure canonical requests use verified lower dependencies; immediate constants need
no target data artifact. Catalog authority and reference-set checks reject missing
or late requests and every unsupported thunk form. The pilot needs no ABI thunk
or fake generated body. Source-to-selected whole-program fixtures reconcile exact
receipts after releasing both callable bodies. Placement owns simultaneous
transfer construction, result capture, live target protection and scratch conflict
checking; physical encoding/native execution remain mandatory subsequent work.

Validation: `make check` passed on final implementation (3,372 compiler tests,
workspace/integration/documentation/runtime checks and all 650 golden
observations). Serial `make msrv-check` passed without warnings on Rust 1.82.0.
All 29 native selected-owner tests pass; call/trace regressions cover direct/C/
indirect/unit/entry calls, mixed bank pressure, both trace policies, numeric
failure terminals, nonreturning calls, malformed clobbers/effects/ABI/attribution,
trace publication substitution, consuming rebuilds, loops/correction joins,
request rejection and released-body selected closure. The shared early/late
indirect-use regression also passes. Final `make static-check` and tracked/untracked
whitespace checks passed. No independent follow-up was added; existing
frame-layout and golden-artifact discoveries retain their owners. The next task
is NP10's placement representation and checker contract.

## NP10 implementation evidence

Completed 2026-09-18. Task baseline: `3413f792`, the user's committed NP09;
roadmap baseline remains `f150d028`. The working tree was clean. Reviewed the
committed call/trace selection handoff and artifact ledger; no earlier transition
is due for removal in this representation task. Committing remains with the user.

`backend/placement` now owns unchecked exact-selected-borrow drafts, complete
entry/parameter/operand/scratch/edge coordinates, ordered explicit transfer
points, typed value storage and signature-qualified ABI locations. Transfers
distinguish selected values from original preserved resource contents, allowing
save/restore obligations without inventing a virtual definition. Structural
validation checks completeness, coordinate/type/storage validity and explicit
move kinds; it deliberately cannot certify content flow or grant realization.
Same-ID republication fails exact snapshot binding. No producer, checked seal,
frame offset, semantic-object home, unchecked renderer or temporary bridge was
introduced. Non-test allowances have their NP11/NP12 removal owners in the ledger.

The authoritative [placement checking contract](../compiler/PLACEMENT_CHECKING.md)
and frozen-design clarification settle finite identity sets, width/overlap kills,
definition-epoch invalidation, simultaneous edge capture/rebinding, conservative
joins, descending iteration, checked finite bounds and stable failures. ABI
signature shapes do not create disjoint physical slots; placement reconstructs
relative target footprints and later frames verify their concrete realization.
The existing ABI-area extent/stride discovery remains with NP13.

**Checkpoint outcome: go for NP11.** The independent test-only oracle has
unambiguous rejected/corrected live-tie, caller-clobber, secured-target,
mixed-bank cycle, duplicate-edge, loop epoch and partial-preservation fixtures.
Additional cases cover equal parameter identities, cyclic loop swaps and
original callee-save contents. A native-owner probe supplies canonical x86
RDI/R11, byte/full RAX, XMM and caller footprints; the synthetic profile supplies
different resource roles and low-lane floating preservation. Genuine selected
publication tests check stale same-ID input, entry/edge completeness,
unreachable scratch, storage, ABI slots and bitwise transfer types. The oracle
remains independent reference evidence; NP11 must check actual resolved
transfers against captured parallel relations and create the sole acceptance
authority. A contract mismatch still requires an explicit amendment before a seal.

Validation on final Rust source: `make check` passed (3,391 compiler unit tests,
including 19 new placement/resource tests, all workspace/integration/runtime/doc
tests, and 650 goldens). Serial `make msrv-check` passed Rust 1.82 workspace/all
targets compilation without warnings. Living architecture, migration coverage,
roadmap status/index and frozen decision links are updated. No independent
larger maintainability discovery was added. Final `make static-check` and
tracked/untracked whitespace checks passed. NP11 is next.

## NP11 implementation evidence

Task baseline: `1d0b84e6`, the user-committed NP10 implementation. Earlier placement
scaffolding and the roadmap baseline `f150d028` were inspected; no reset or commit
was performed. Changes here remain for the user to commit.

The shared checker reconstructs requirements from exact verified selected facts
and immutable target semantics. Static checks cover all coordinates, including
unreachable blocks, resource reservations/overlaps, fixed/ABI constraints, ties,
simultaneous definitions/scratch, storage lifetimes and explicit move recipes.
An independent must-contents interpreter implements width/unit kills, original
preservation tokens, definition epochs, ordered bit moves and simultaneous edge
rebinding. Synchronous CFG rounds intersect every predecessor occurrence, include
the entry seed and use the checked finite lattice bound. Strict replay follows
convergence; neither intermediate top facts nor producer claims confer authority.

Only this checker constructs immutable `CheckedPlacement`, consuming its draft
while borrowing the exact selected publication. Frame/realization consumers are
scheduled in NP13/NP14 and must take this product; no provisional realizer or draft
bypass was added. Native checking reconstructs canonical x86 target facts, full
64-bit callee promises, eight-byte relative ABI footprints and exact move scratch.
Concrete ABI-area extent/padding remains the existing NP13 prerequisite. Native
classification and checking share the stack-slot width constant; structural and
flow checking share the representation compatibility rule in the placement model.

Hand-written register placements and corrected/corrupted pairs cover live ties,
secured targets, caller saves/reloads, real native call results, mixed-bank cycles,
duplicate edges, divergent joins, cyclic parameter swaps and stale homes/epochs,
narrow writes, partial floating preservation and link restoration on a second
synthetic profile, simultaneous definitions/scratch, ABI aliases across signatures,
transfer lifetimes, unreachable defects and selected diagnostic provenance.
The earlier oracle remains test-only and independent. Tests are divided by
responsibility, and consumed model/structure/event lint allowances are retired;
remaining consumer allowances have removal owners in the ledger. Native profile
validation is shared by selection and placement, and diagnostics retain selected
site/origin metadata without querying earlier phases. Native memory moves also
exercise missing/reserved working scratch and destruction of its held value.

Validation: `make check` passed on the final implementation: formatting, locked
workspace/all-target builds, Clippy without warnings, documentation links/indexes,
all workspace Rust tests (3,414 compiler unit tests), runtime contracts and all
650 golden cases. The 23 new regressions include genuine native register/call
placements and explicit memory-move scratch. `make msrv-check` passed serially on
Rust 1.82.0 for all workspace targets without warnings. Final `make static-check`
and tracked/untracked whitespace checks passed. Logs:
`/tmp/skald-np11-final-check.log`, `/tmp/skald-np11-msrv.log` and
`/tmp/skald-np11-final-static.log`.

Final review corrected inherited structural validation of original preservation
tokens: promised width stays exact, while raw original bits may pass through
another bank using explicit bitwise transfers. Its positive/corrupted regression
checks restoration after the original floating register is overwritten.
NP11 is complete; NP12 is next. No additional independent discovery was needed.
