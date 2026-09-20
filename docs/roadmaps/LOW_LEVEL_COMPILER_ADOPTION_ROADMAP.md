# Low-Level Compiler Consolidation and Adoption Roadmap

Status: planned; AD01 is next.
Accepted design:
[frozen consolidation and adoption design](../archive/LOW_LEVEL_COMPILER_ADOPTION_DESIGN_PROPOSAL.md).
Planning baseline: `b7c838fa`, the committed reviewed proposal.
Implementation baseline: record the committed roadmap revision in AD01 before
the first implementation change. Program baseline: `495debd3`. Complete private
pipeline endpoint: `0c56a9f9` after the placement-checking scalability closure.

Adopt the checked low-level pipeline as the only production native compiler,
make its verified phases observable through the existing request-local driver
services, accept its foundation costs, and retire the direct MIR-to-assembly
backend. The roadmap preserves a clean rollback boundary until the new default
has passed public and repository validation. It then removes the legacy path and
reviews LA01-LA05 cumulatively before handing a stable baseline-placement
architecture to LA06 register allocation.

## Scope and invariants

- `backend::emit_assembly` remains the target-dispatch facade. The final x86-64
  target implementation always runs checked planning, lowering, selection,
  baseline placement, frame planning, physical realization and verification,
  exact closure, and deterministic emission.
- Final MIR remains the only semantic backend input. Reporting, inspection,
  placement, physical realization, and emission cannot regain AST, HIR,
  resolver, type-checker, pass-internal, or mutable-certificate authority.
- Baseline stack placement remains the production placement strategy and always
  crosses the independent checker. Register allocation, scalar promotion, SSA,
  new low-level optimization, and a complete AArch64 backend are out of scope.
- No public mode, CLI flag, environment switch, persistent feature gate,
  per-callable fallback, or error-triggered retry chooses the direct backend.
- Temporary test or measurement selection is private, recorded in the artifact
  ledger, and removed with the direct path.
- Public observations use existing request-local reporting and inspection
  services. They read verified immutable products, stream requested dumps,
  produce no quiet-mode renderer work, and cannot affect compilation.
- Semantic behavior, diagnostic categories, ABI and runtime contracts, trace
  policy, artifact retention, sparse bodies, and configuration-local
  determinism remain stable. Assembly text may change.
- Two compatible full paired captures resolve every foundation workload before
  cutover. Incompatible, invalid, or inconclusive evidence blocks adoption;
  repeated review triggers require correction or an explicit accepted tradeoff.
- Cutover and legacy deletion are separate tasks. A failed cutover restores the
  still-present direct entry in that task; no deletion begins first.
- Shared phase products remain target-neutral. Synthetic second-target tests
  protect resource, overlap, preservation, ABI-role, placement, and link-role
  contracts without registering an AArch64 backend.
- Discoveries outside this accepted scope go in
  `LOW_LEVEL_COMPILER_ADOPTION_DISCOVERIES.md` if needed. Small fixes directly
  supporting a task may land with it.

## Progress

- [ ] AD01 — Freeze adoption inventory and comparison authority
- [ ] AD02 — Productionize native orchestration and errors
- [ ] AD03 — Integrate phase reporting and metrics
- [ ] AD04 — Integrate streaming low-level inspection
- [ ] AD05 — Prove public parity and target portability
- [ ] AD06 — Measure and decide foundation adoption cost
- [ ] AD07 — Adopt the checked production default
- [ ] AD08 — Retire direct lowering and transition scaffolding
- [ ] AD09 — Publish living contracts and reconcile migration records
- [ ] AD10 — Cumulative foundation review, cleanup, and closure

## PR-sized implementation sequence

Each task has one primary review boundary. Mark detail checkboxes only when the
result exists and the named tests pass; mark the progress checkbox after its
exit criteria pass. The user normally commits between tasks, so each task must
inspect both current source and committed history for artifacts introduced or
scheduled for removal by an earlier task.

### AD01 — Freeze adoption inventory and comparison authority

**Purpose:** establish exact owners, rollback inputs, and compatible legacy
evidence before code changes make the direct backend harder to reproduce.

- [ ] Record the committed implementation baseline immediately before the first
  code change. Record the accepted design commit, program baseline, private
  pipeline endpoint, current toolchain, and clean/dirty state.
- [ ] Inventory every direct-backend module, public/private entry, test,
  planning observer, semantic projection, runtime-trace component, artifact
  closer, emitter, and lint allowance. Classify each as superseded, canonical
  shared input projection, durable behavioral witness, or retained contract.
- [ ] Reconcile the inherited migration-coverage allowance inventory against
  current symbols. Give each surviving allowance and facade re-export an AD02,
  AD03, AD04, AD08, or AD10 disposition rather than widening visibility.
- [ ] Freeze the private candidate injection used by public-path tests. It may
  select the checked pipeline only inside compiler tests and must not appear in
  `CompilationRequest`, CLI parsing, environment handling, or target lookup.
- [ ] Preserve a clean legacy `golden` compiler build from the implementation
  baseline in ignored storage, and record its revision, binary hash, exact
  rustc version, profile, build command, flags, runtime archive, and harness
  fingerprint. Prefer two preserved builds over a production selection switch.
- [ ] Verify that the current foundation collector can compare this preserved
  binary with a future candidate using one unchanged manifest and harness.
  Extend only provenance/build-record support needed for reproducible capture.
- [ ] Create the roadmap transition ledger below with current exact symbols,
  introducing commits recovered from history where relevant, intended removal
  tasks, and explicit retention criteria.

**Tests:** focused target-facade and private-injection tests, phase-boundary and
CLI option guards, `make measurement-support-test`, one nonqualifying foundation
smoke using equivalent legacy roles, `make docs-check`, and `git diff --check`.

**Exit criteria:** the direct and checked implementations have complete owner
inventories; the checked path can be selected only by a narrow private test
seam; a reproducible legacy build and compatible future measurement procedure
exist; every transition artifact has a removal or retention owner.

### AD02 — Productionize native orchestration and errors

**Purpose:** turn the complete private pilot into normal target-owned pipeline
code before any public default or observation behavior changes.

- [ ] Move `native::pilot` orchestration into the cohesive production native
  pipeline owner and remove experimental names from modules, functions, errors,
  fixtures, comments, and diagnostic text. Preserve private candidate access
  for AD03-AD07 and keep the public target facade on direct lowering.
- [ ] Replace `NativePilotError` with a typed production pipeline error covering
  planning, discovery, ABI construction, lowering, selection, selected closure,
  placement, frame, realization, physical verification, program closure, and
  observation. Keep phase payloads typed until facade mapping.
- [ ] Extend the backend error boundary as needed to preserve target, phase,
  callable/generated-artifact identity, and invariant/source context without
  matching display strings or exposing module layout.
- [ ] Separate compilation failure from observation-sink failure. Both are
  terminal for candidate publication, and neither may invoke the direct path.
- [ ] Remove allowances and facade re-exports now made live by production
  orchestration. Trim unused APIs instead of retaining private model surface
  solely because earlier tests referenced it.
- [ ] Keep profiling/test seams subordinate to the production orchestrator;
  there must be one phase order and one assembly publication implementation.

**Tests:** native pipeline success and phase-specific failure tests, malformed
plan/lowered/selected/placement/frame/physical products, observation writer
failure, error-context assertions, no-fallback tests, phase-boundary/privacy
tests, `make compiler-test`, `make static-check`, and `make msrv-check`.

**Exit criteria:** ordinary-named native orchestration and typed error mapping
are production-quality, candidate compilation has no pilot gate or fallback,
and the public target entry still deliberately calls the direct backend.

### AD03 — Integrate phase reporting and metrics

**Purpose:** expose honest operational phase events through the existing
request-local observer without coupling reporting to internal builders.

- [ ] Add stable report phases for low-level planning; lowering/publication;
  selection/selected closure; placement/checking; frame planning; physical
  realization/verification; and program closure/emission. Keep
  `BackendEmission` as the enclosing total.
- [ ] Thread a borrowed request-local observer from driver orchestration through
  target dispatch into the candidate pipeline. Do not store it in compiler or
  target global state.
- [ ] Emit exactly one finish event for every started event. Close the owning
  nested and enclosing phases on failure without inventing events for phases
  that did not execute.
- [ ] Derive requested detail metrics from immutable products or receipts while
  resident: callable/data counts, lowered/selected structure, placement/check
  totals, frame sizes, physical instructions, and assembly bytes. Do not perform
  metric traversal in quiet or phase-only modes when details are not requested.
- [ ] Keep elapsed durations out of deterministic product dumps and keep report
  selection from changing assembly, diagnostics, retention, or phase behavior.
- [ ] Update CLI/report renderers and report tests for the nested phase order,
  outcome, metric ownership, and stable labels.

**Tests:** driver report phase/metric/failure tests at every candidate failure
boundary; Off/Phases/Details/Trace equivalence; repeated and parallel request
isolation; no fabricated events; assembly equality across detail levels; focused
CLI reporting expectations; `make compiler-test`, `make cli-test`, and
`make static-check`.

**Exit criteria:** candidate compilation reports accurate nested phases and
requested metrics through the normal driver observer, while quiet compilation
does no reporting-only traversal and the production default remains legacy.

### AD04 — Integrate streaming low-level inspection

**Purpose:** make verified low-level products inspectable through the existing
borrowed inspection service without retaining or exposing phase internals.

- [ ] Define borrowed checkpoint labels and inspector interfaces for lowered,
  selected, placement, frame, and physical products. Checkpoint products cannot
  escape their callback or provide edit/builder authority.
- [ ] Extend `CompilationInspectors` and driver/backend request plumbing with an
  optional low-level inspector. Empty services must construct no renderer and
  execute no dump traversal.
- [ ] Adapt phase-owned renderers to stream verified immutable products in
  stable program/callable/block order while each product is resident. Do not
  buffer the complete rendered program.
- [ ] Put shared program/catalog context in a presentation adapter so it prints
  once per requested stream rather than once per callable/checkpoint. Earlier
  checkpoints cannot read later-phase products.
- [ ] Make sink failure terminal before assembly publication and preserve its
  distinct error classification through AD02's boundary.
- [ ] Remove the private `NativePilotInspection` adapter and its allowances once
  all candidate tests use the maintained driver inspection service.

**Tests:** borrowed-lifetime compile-fail examples, each checkpoint alone and in
combination, stable ordering, cross-process output, bounded streaming behavior,
quiet no-renderer behavior, writer failure, repeated/parallel isolation, and
assembly/report equivalence with and without inspection. Run `make compiler-test`,
`make docs-test`, and `make static-check`.

**Exit criteria:** every accepted low-level checkpoint is available through one
request-local streaming inspection contract, the private pilot adapter is gone,
and inspection cannot affect or outlive compilation.

### AD05 — Prove public parity and target portability

**Purpose:** establish adapter readiness through the public compiler shape and
audit the target boundary while the direct backend remains the recoverable
default.

- [ ] Run the checked candidate through the same driver request, backend target
  facade, reporting, inspection, artifact publication, assembler, linker, and
  executable boundaries as production using only AD01's private injection.
- [ ] Cover the full configuration matrix: default/minimal MIR, enabled/omitted
  runtime tracing, and complete/reachable artifacts. Reconcile every E22 witness
  and relevant public golden/ABI/failure witness in the migration coverage
  record with maintained candidate-path evidence.
- [ ] Prove exact retained callable/data/link closure, sparse body behavior,
  static initialization/shutdown, generated helpers, object dispatch,
  ownership, optionals, arrays, strings, I/O, failures, and entry behavior.
- [ ] Prove assembler/linker acceptance, C and runtime ABI behavior, native
  outputs, diagnostic category/context, and independent-process determinism for
  assembly, requested checkpoints, and specified report metrics.
- [ ] Review shared planning, LIR, selection interfaces, placement checking,
  frame contracts, and typed closure for x86 register/ABI/layout/syntax leakage.
  Extend synthetic-target witnesses for scalar, branch, call, aggregate, and
  lifecycle shapes, overlaps, partial preservation, fixed/tied resources,
  stack components, and separate link roles.
- [ ] Resolve every adapter defect without weakening a phase verifier or adding
  fallback. If a target requirement is missing, amend the owning frozen design
  and add a second-target contract witness before continuing.
- [ ] Record the public-adapter go/no-go decision. On no-go, leave production
  legacy, keep independently sound pipeline/test work, and add explicit
  corrective tasks before AD06.

**Tests:** complete native candidate matrix, public driver/report/inspection
tests, target synthetic-contract suites, phase-boundary guards, C ABI probes,
all golden tests, `make check`, `make golden-determinism-test`, and
`make msrv-check`.

**Exit criteria:** the readiness checkpoint records a go decision with complete
public-shaped parity and portability evidence. Production still uses direct
lowering, and no unsupported configuration depends on fallback.

### AD06 — Measure and decide foundation adoption cost

**Purpose:** replace historical inconclusive timing classifications with
compatible candidate evidence before changing production behavior.

- [ ] Build and preserve the final candidate from committed source using the
  same optimized assertion-enabled profile, rustc, flags, runtime, standard
  library, manifest, harness, and host controls as AD01's legacy build. Record
  both build records and binary hashes.
- [ ] Run two independent full paired foundation captures with alternating
  starting order and qualifying warmup/repetition counts. Retain raw samples,
  untimed observations, repeated assemblies, native outputs, identities, and
  failure records under the maintained protocol.
- [ ] Compare the two reports with `compare-foundation`; verify compatible
  provenance, exact native observations, quiet/trace assembly equivalence,
  deterministic artifacts, supported frame/text metrics, and all 21 workload
  configurations.
- [ ] Resolve every `incompatible`, `invalid`, or `inconclusive` result by
  correcting inputs/behavior or recapturing with improved controls. Do not
  reinterpret historical evidence or average away a workload.
- [ ] Correct every repeated `review-required` regression or record an explicit
  accepted tradeoff naming workload, both captures, magnitude/variability,
  likely cause, architectural effect, and owner. Review frame growth and any
  new frame-limit failure independently.
- [ ] Retain the qualifying manifests, build records, compressed reports,
  comparison, hashes, and review decision durably; keep executables and large
  repeated assemblies out of Git.
- [ ] Record the cost go/no-go decision. On no-go, keep production legacy,
  retain valid evidence and the private checked path, remove any rejected
  measurement bridge, and amend the roadmap with corrective tasks before AD07.

**Tests:** `make measurement-support-test`, report/index replay and hash checks,
the two real qualifying captures, comparator result, `make docs-check`, and
focused deterministic artifact/native verification from the retained records.
Timing collection is intentionally outside `make check`.

**Exit criteria:** every configuration has compatible resolved evidence and the
foundation cost checkpoint records an explicit go decision or accepted reviewed
tradeoff; no inconclusive classification is silently cleared.

### AD07 — Adopt the checked production default

**Purpose:** make one reversible production-boundary change and validate it
before the direct implementation is removed.

- [ ] Change the x86-64 target facade so ordinary `backend::emit_assembly`
  always invokes the checked pipeline with request-local reporting/inspection.
  Add no runtime selector or fallback.
- [ ] Route all compiler, CLI, golden runner, measurement, and library callers
  through that ordinary facade. Remove AD01's private candidate injection when
  it no longer has a parity role, or ledger it for mandatory AD08 removal if
  legacy differential tests still require it.
- [ ] Update production-facing error, report, metric, and checkpoint tests to
  assert the checked path without naming rollout tasks or pilot concepts.
- [ ] Run the complete supported configuration matrix through the actual default,
  including assembler/linker/native/ABI/trace/artifact/determinism behavior.
- [ ] Record the exact adoption commit and validation results. If any exit gate
  fails, restore only the facade selection to the still-present direct entry in
  this task, retain useful diagnostics/tests, and schedule correction before
  retrying. Do not add automatic fallback.

**Tests:** focused target-facade/default-selection tests, `make check`,
`make check-long`, `make msrv-check`, public report/inspection determinism, C ABI
and runtime probes, and a nonqualifying current-default foundation smoke.

**Exit criteria:** the checked LIR pipeline is the unconditional production
default and all adoption gates pass. The direct source remains temporarily
available only as AD08 deletion material and is unreachable from production.

### AD08 — Retire direct lowering and transition scaffolding

**Purpose:** remove duplicate backend ownership once the checked default has a
validated rollback commit.

- [ ] Delete the superseded direct legality/layout/planning, lowering, machine,
  frame, runtime-trace expansion, artifact closure, literal/static, symbol, and
  emission implementations identified by AD01. Preserve only canonical facts
  or projections consumed by the checked path under accurate owners and names.
- [ ] Migrate durable semantic, ABI, runtime, and failure tests to public or
  phase-owned suites. Remove incidental assertions about legacy scratch
  registers, offsets, labels, instruction spellings, or assembly organization.
- [ ] Delete differential-only fixtures, implementation selectors, comparison
  adapters, fallback/eligibility gates, old planning observers, and old/new
  aliases. Search committed history as well as the current diff for scaffolding
  introduced in AD01-AD07.
- [ ] Remove remaining `pilot`, `legacy`, rollout, and task-code vocabulary from
  production code and current tests where it no longer describes a real concept.
- [ ] Remove residual `cfg_attr(not(test), allow(dead_code))` and private facade
  `allow(unused_imports)` annotations carried for deferred native consumption.
  Retained annotations need a narrow current reason and owner in the ledger.
- [ ] Prove there is one implementation of selection, ABI assignment, placement
  policy, frame planning, physical realization, artifact closure, trace
  expansion, and assembly emission.
- [ ] Record the legacy-retirement go/no-go decision. Any hidden caller,
  unowned invariant, or missing replacement witness blocks deletion completion
  and LA05 closure; it does not justify a production fallback.

**Tests:** symbol/module searches, phase dependency guards, all migrated owner
tests, `make static-check`, `make compiler-test`, `make golden-test`,
`make golden-determinism-test`, `make msrv-check`, and `git diff --check`.

**Exit criteria:** direct MIR-to-assembly lowering and all temporary adoption
scaffolding are absent; every retained test and allowance has a continuing
owner; production contains one checked native pipeline.

### AD09 — Publish living contracts and reconcile migration records

**Purpose:** make current documentation and maintained evidence describe the
adopted architecture before the cumulative closing audit.

- [ ] Rewrite backend and phase documentation around checked planning, LIR,
  selection, baseline placement, frame planning, physical verification, typed
  closure, and deterministic target emission. Remove direct-backend and private
  pilot descriptions.
- [ ] Update low-level IR, physical realization, placement checking, reporting,
  debugging, testing, runtime ABI, trace, profile, and measurement guidance for
  production ownership, public observation labels, streaming checkpoints,
  failure behavior, and accepted cost evidence.
- [ ] Reconcile every operation/helper/configuration row and retained-artifact
  obligation in `LOW_LEVEL_COMPILER_MIGRATION_COVERAGE.md` with final source and
  durable tests. Move independent actionable findings to an indexed discoveries
  record rather than broadening this roadmap.
- [ ] Update the overarching architecture, cleanup audit A22, optimization
  catalog/discoveries, and active index to mark LA05 implemented and LA06 as the
  next separate workstream. Do not claim register allocation or A22 complete.
- [ ] Prepare the migration coverage record for archival after AD10 validation;
  preserve it as historical evidence rather than living architecture.
- [ ] Reconcile the transition ledger and identify any substantial gap that
  requires an explicit corrective task before cumulative closure.

**Tests:** `make docs-check`, `make docs-test`, current-source symbol/link
searches, coverage-to-test spot checks across every feature family, and
`git diff --check`.

**Exit criteria:** living documents contain one coherent adopted architecture,
all historical rollout claims are confined to roadmap/archive records, and the
coverage/ledger audit has no unexplained artifact or missing owner.

### AD10 — Cumulative foundation review, cleanup, and closure

**Purpose:** review LA01-LA05 as one architecture, remove cross-task residue,
and leave an artifact-free, documented foundation ready for LA06 design.

- [ ] Review `git diff 495debd3..HEAD` and the LA05 implementation-baseline diff
  using `--stat`, `--name-status`, and full patches. Include staged, unstaged,
  and untracked work; distinguish unrelated intervening commits.
- [ ] Read phase and target boundaries as one design. Verify final-MIR authority,
  target neutrality, consuming edits, independent checks, exact snapshot/parent
  receipts, sparse bodies, trace omission, ABI ownership, baseline placement,
  typed closure, deterministic output, and request-local observation.
- [ ] Reconcile every ledger entry with current source and `git log -S` where
  needed. Search beyond it for duplicate implementations, selectors, adapters,
  compatibility aliases, gates, allowances, dead exports, test-only production
  seams, stale comments, and experimental names.
- [ ] Make small cleanup/consistency fixes directly. Add an explicit corrective
  task for any substantial correctness, architecture, portability, or evidence
  gap; do not close around it.
- [ ] Verify retained measurements, synthetic-target tests, public parity tests,
  living documentation, and final interfaces. Run the full gates from an
  artifact-free snapshot or clean checkout.
- [ ] Record the reviewed baseline and endpoint, residual uncommitted changes,
  final artifact dispositions, cost decision, adoption/deletion commits, and
  validation results.
- [ ] Mark the roadmap complete, archive it and the reconciled migration coverage
  record, update both indexes and all links, and leave the closing change for the
  user to commit. Record LA06 register-allocation design as the next workstream.

**Tests:** focused backend/driver/phase/portability tests; `make check-long`;
`make msrv-check`; `make measurement-support-test`; retained measurement replay;
documentation/link checks; clean-tree or artifact-free snapshot confirmation;
and final cumulative diff review.

**Exit criteria:** one production checked low-level compiler remains; all LA05
and inherited transition artifacts are reconciled; foundation behavior, cost,
portability, observations, and documentation have accepted evidence; the
roadmap and coverage record are archived; LA06 has a clean handoff.

## Go/no-go control flow

AD05, AD06, AD07, and AD08 are mandatory checkpoints, not informational tasks.

| Checkpoint | Required go evidence | No-go disposition |
| --- | --- | --- |
| AD05 public-adapter readiness | Complete public-shaped parity, observation correctness, determinism, and portability review | Keep legacy default; retain sound adapters/tests; insert corrective tasks before measurement |
| AD06 foundation cost | Two compatible full pairs; every workload resolved; repeated review triggers corrected or explicitly accepted | Keep legacy default; retain valid evidence; remove rejected bridges; amend and retry before cutover |
| AD07 production adoption | Unconditional checked default passes focused, complete, long, MSRV, native, ABI, trace, artifact, and observation gates | Restore facade selection to direct entry while it still exists; diagnose and retry without fallback |
| AD08 legacy retirement | No production caller or unique invariant remains; replacement witnesses and owner searches complete | Leave source temporarily unreachable but present; add corrective task; do not call LA05 complete |

A no-go does not discard independently accepted phase contracts, private
pipeline correctness, tests, measurement support, or evidence. It also does not
authorize skipping the checkpoint or carrying two selectable production paths.

## Transition artifact ledger

AD01 fills exact introducing commits and expands grouped rows before code work.
Every later task updates disposition as artifacts are added, removed, or
retained. A clean working tree never implies that a committed bridge is gone.

| Artifact | Present owner / purpose | Introduced | Removal or retention owner | Status |
| --- | --- | --- | --- | --- |
| `x86_64_sysv::native::pilot`, `compile_native_pilot*`, `NativePilotError` | Complete private no-fallback pipeline, inspection and profiling | Recover in AD01 | AD02 production naming/errors; AD04 inspection adapter | Pending |
| `x86_64_sysv::emit_assembly_observed` and legacy `planning::PlanningObserver` | Direct-backend default and planning tests | Recover in AD01 | AD07 cuts over; AD08 deletes or migrates witnesses | Pending |
| Direct `abi`, `array_legality`, `artifacts`, `dispatch`, `emit`, `frame`, `layout`, `legality`, `literal_data`, `lower`, `machine`, `planning`, `runtime_trace`, `static_fields`, and `symbol` owners | Current production MIR-to-assembly implementation | Historical | AD08, except exact canonical inputs proven consumed by checked owners | Pending inventory |
| `x86_64_sysv::fact_projection` exports and facade allowance | Canonical projection bridge into checked planning | LA03/LA04 history | AD02/AD08: retain under accurate owner only when checked pipeline consumes it | Pending review |
| Shared LIR/plan/selected/inspection item allowances and facade re-export allowances listed in migration coverage | Private delivered APIs awaiting production consumption | LA01-LA04 history | AD02-AD04 consume/trim; AD08/AD10 remove residuals | Pending |
| `NativePilotInspection` and private checkpoint writer path | Requested-only verified checkpoint adapter | LA03/LA04 history | AD04 replaces through driver inspection | Pending |
| Native pilot profiling seam and phase names | Private phase-cost/test support | LA03/LA04 history | AD02/AD03 retain only as production-owned measurement support | Pending review |
| AD01 private candidate injection | Public-shaped candidate parity while legacy remains default | AD01 / commit pending | AD07 after cutover, mandatory AD08 latest | Planned |
| AD01 preserved legacy binary and build record | Compatible AD06 baseline | AD01 / external ignored binary | AD06 retains small records/hashes; executable stays out of Git and may be discarded after accepted evidence | Planned |
| AD06 paired raw reports, comparison and decision | Foundation cost acceptance | AD06 / commit pending | Durable evidence retained through AD10 and future architecture review | Planned |
| Legacy differential tests and comparison fixtures | Temporary behavioral/cost oracle before deletion | AD01-AD06 as needed | AD08 after replacement witness reconciliation | Planned |
| Synthetic target fixtures and malformed verifier inputs | Durable portability and independent-checking evidence | LA01-LA04 history | Retain while shared target contracts exist | Retained |
| Semantic goldens, native execution, C/runtime ABI probes and deterministic reporting tests | Durable observable-behavior evidence | Existing/history | Retain under public/compiler/golden owners | Retained |

## Ordering and dependencies

AD01 freezes evidence and ownership before source moves. AD02 establishes the
normal pipeline/error boundary that AD03 reporting and AD04 inspection adapt;
those two tasks may be split further but both must finish before AD05. AD05
proves correctness and portability while rollback is trivial. AD06 then decides
cost using the final candidate. Only a go decision permits AD07's one-line
architectural cutover plus its surrounding integration. AD08 removes the direct
path after the cutover commit is validated. AD09 publishes the final contracts,
and AD10 reviews the complete committed history and archives the records.

Do not begin LA06 allocation design until AD10 closes this roadmap. Allocation
must consume the adopted checked placement boundary rather than influencing the
foundation cost decision or preserving an obsolete direct backend.
