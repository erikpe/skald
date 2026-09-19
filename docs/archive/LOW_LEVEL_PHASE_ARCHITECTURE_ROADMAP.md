# Low-Level Phase Architecture and Backend Ownership Roadmap

Status: complete and archived, 2026-09-17; LP01–LP05 are complete. Implements LA01 of the
[low-level compiler architecture program](../roadmaps/LOW_LEVEL_COMPILER_ARCHITECTURE_DESIGN_PROPOSAL.md).
The [phase architecture design](LOW_LEVEL_PHASE_ARCHITECTURE_DESIGN_PROPOSAL.md)
is accepted and frozen as of 2026-09-17.

Planning baseline: `97fdaf49`. Implementation baseline: `495debd3`, the last
commit before LP01 began. The overarching program shares this baseline; no
earlier implementation exists. Preserve it across subsequent child roadmaps.
The initial tree was clean; `97fdaf49..495debd3` contains only design freeze,
roadmap creation and related documentation/index updates, with no compiler
scaffolding or unrelated implementation changes.

This workstream turns the accepted boundaries into an auditable migration
contract, regression protection, and reproducible before-change evidence.
Its completion makes the detailed LIR and target designs ready to proceed.
It does not deliver the new compiler pipeline; LA02–LA05 own that implementation
and adoption, followed by register allocation in LA06.

## Scope and invariants

- Preserve the public `BackendInput`/target/emission boundary and verified final
  MIR authority, accepted programs, diagnostics, x86 ABI/runtime layout,
  evaluation and destruction order, and enabled/omitted runtime traces.
- Carry the frozen design's layout-specialized shared lowering, distinct
  verified products, single-definition machine values with block parameters,
  explicit memory, and target-owned selection/ABI/frame contracts into the
  downstream handoff. Semantic MIR storage remains memory.
- Preserve sparse executable domains, certified active statics, stable dispatch
  slots, and complete/reachable artifact behavior. No second semantic
  reachability analysis or assembly-text dependency inference.
- Establish coverage and tests against the current implementation. Tests for
  nonexistent LIR seals, selected constraints, or placement checking belong
  to the downstream owners once those products exist.
- Establish the accepted foundation measurement policy before production
  migration. No speedup, reduced stack traffic, or allocator is required here.
- Do not introduce placeholder IR types, generic target traits without a real
  consumer, a second lowering pipeline, dormant switches, or an AArch64 stub.
  Detailed schemas, selection, baseline placement, and frame realization are
  LA02/LA03 work. Full migration is LA04; default adoption/removal is LA05.
- Standard-library cleanup, scalar promotion, MIR SSA, scheduling, alias
  optimization, and a complete AArch64 port remain outside this roadmap.

The current facade and private x86 orchestration already provide the entry
boundary needed for this preparatory work. Retain them initially. A small
extraction is justified only by a current test or consumer and must preserve
behavior; do not implement the illustrative future directory tree in advance.

## Progress

- [x] LP01 — Establish the migration contract and coverage inventory
- [x] LP02 — Protect existing backend boundaries and behavioral witnesses
- [x] LP03 — Make the foundation measurement protocol reproducible
- [x] LP04 — Capture and qualify the pre-migration baseline
- [x] LP05 — Cumulative review, downstream handoff, and closure

## Current owners and durable outputs

| Responsibility | Starting owner | Roadmap output |
| --- | --- | --- |
| Verified input, target registry, trace source access | [Backend facade](../../crates/skald-compiler/src/backend/mod.rs) | Boundary/privacy evidence and unchanged public contract |
| Planning, selection, trace activation and emission order | [x86 orchestration](../../crates/skald-compiler/src/backend/x86_64_sysv/mod.rs) and its private owners | Current-to-future responsibility map, including helper/artifact paths |
| Phase dependency rules | [Phase boundary tests](../../crates/skald-compiler/tests/phase_boundaries.rs) | Focused guards for existing boundaries; explicit future guard owners |
| ABI, arithmetic, lifecycle and sparse emission | [Backend owner tests](../../crates/skald-compiler/src/backend/x86_64_sysv/tests/mod.rs) and feature-owned goldens | Named behavioral witnesses usable during migration |
| Timing, identities, digests and workload selection | [Cleanup measurement script](../../scripts/measure_cleanup_baseline.py), [support tests](../../scripts/tests/test_cleanup_baseline.py) | Reproducible foundation manifest/procedure and any narrowly necessary harness support |
| Current compiler/test contracts | [Backend guide](../compiler/BACKEND.md), [reporting](../compiler/REPORTING.md), [testing](../development/TESTING.md) | Accurate current behavior and links to planned work, without claiming LIR exists |

The [migration coverage record](../roadmaps/LOW_LEVEL_COMPILER_MIGRATION_COVERAGE.md) is the
program-wide migration record created by LP01. Keep it active through LA05;
LA01 closure does not archive pending migration obligations. The
[foundation measurement procedure](../development/LOW_LEVEL_COMPILER_MEASUREMENTS.md)
defines reproducible collection/comparison, referring to the generic
[cleanup measurement contract](../development/CLEANUP_MEASUREMENTS.md).
The complete pre-migration baseline at `9e3cebb1` is durably retained in that
procedure. Its equivalent-build comparison preserves eleven inconclusive short
compile timings; cost clearance remains a future adoption obligation.

## PR-sized implementation sequence

### LP01 — Establish the migration contract and coverage inventory

**Purpose:** turn architectural categories into concrete obligations before
choosing LIR schemas or moving lowering code.

- [x] Record implementation/program baselines and inspect intervening commits.
  Read the frozen design against current source; document discrepancies rather
  than silently changing an accepted contract.
- [x] Enumerate every final-MIR instruction and terminator variant reaching the
  backend, plus generated helper families, entry/static lifecycle, runtime
  calls, trace actions, data and artifact roots. Distinguish executable bodies
  from retained declarations and layout-only visits.
- [x] For each entry record current owner, proposed phase owner, source/ABI/
  failure invariants, existing test identifiers, missing witness, and delivery
  owner. Mark all new-pipeline delivery pending. A shared family row must name
  its member variants; no catch-all row can imply coverage of an unreviewed case.
- [x] Map the five phase products and planning context to their producers,
  permitted inputs, publication checks, consumers, observation and error
  obligations. Assign executable seal/constraint/verifier work to LA02/LA03.
  Keep the accepted design authoritative rather than copying its prose.
- [x] Record the five design walkthroughs as downstream acceptance witnesses:
  live input with x86 ties, loop/join/division CFG, hidden result/receiver call,
  shared release across a finalizer, and target resource/width constraints.
  AArch64 mappings remain design witnesses, not implemented ABI support.
- [x] Record the concrete observation, error and artifact-retention handoffs
  without extending public APIs prematurely. Identify any required joint
  LA02/LA03 decisions, including helper inventories, frame scratch resources,
  and selected-stage edge transfers.
- [x] Index the coverage record and update this roadmap's progress/evidence.

**Tests:** `make docs-check`; manually reconcile the inventory against the MIR
enums, selector dispatch, helper constructors and existing tests. Include the
source revision and search scope so review can be repeated. No synthetic IR
implementation is needed to test a documentation inventory.

**Exit criteria:** every current backend operation/helper family has an explicit
disposition and future owner. The next steps have a bounded list of missing
current-behavior tests and measurement capabilities. No detailed schema decision
is disguised as an already-implemented phase contract.

**Completion evidence (2026-09-17):** the
[migration record](../roadmaps/LOW_LEVEL_COMPILER_MIGRATION_COVERAGE.md) reconciles 42
instructions, 20 terminators, 29 array operations, five I/O operations, all 12
termination reasons, and 19 rvalues (including the excluded proof-rich
`PathCondition`). It also covers place forms, generated bodies, runtime/data
families, phase publication, trace/report/error authority, and joint downstream
decisions. All new-pipeline delivery remains pending. Five bounded LP02 witness
handoffs and six LP03 measurement capability gaps are recorded there.

An independent one-off enum/table comparison confirmed each instruction,
terminator, array operation and rvalue appears exactly once in its inventory;
I/O/reason members are present. A source check resolved all 74 named witness
functions to their linked owners. Manual selector/helper/retention review found
no frozen-contract conflict or demonstrated semantic defect. `make docs-check`
and whitespace checks passed. This documentation-only task introduces no Rust
code, test fixtures or transitional implementation artifacts; current behavior
suites were inspected, not rerun. Changes remain uncommitted for the user.

### LP02 — Protect existing backend boundaries and behavioral witnesses

**Purpose:** make architectural migration failures observable through tests
that exercise today's compiler and can survive ownership moves.

- [x] Reuse existing compile-fail facade tests and phase dependency policies.
  Add only missing checks for unverified/proof-rich input, private mutation
  authority, forbidden frontend access, and omitted-trace source isolation.
  Test any scanner/policy changes with permitted and forbidden references.
- [x] From LP01's gaps, add focused cases for source-visible arithmetic after
  a live-input use, division boundaries and joins, hidden-result/receiver calls
  under argument pressure, and finalizer ordering with the original allocation
  live across the call. Reuse existing coverage when it already proves the
  obligation; do not duplicate the full backend suite.
- [x] Verify the selected witnesses cover enabled/omitted tracing, relevant
  complete/reachable emission behavior, and both MIR profiles through existing
  test facilities. Record which owner test establishes each dimension rather
  than multiplying every fixture across an unnecessary Cartesian product.
- [x] Keep semantic/native expectations independent of temporary scratch
  choices and offsets. Retain precise assembly assertions only where they
  prove a current ABI/encoding contract, with their migration owner recorded.
- [x] Record future lowered/selected/placement/physical negative-test obligations
  in the coverage record. Do not claim the current source scanner proves
  internal phase isolation or create a mock LIR solely to make those tests pass.
- [x] Apply any small current-boundary extraction needed by these tests under
  the existing facade organization; otherwise retain current orchestration.
  Update living test/backend guidance only for actual contract or test changes.

**Tests:** focused owner tests and goldens, `cargo test --locked -p
skald-compiler --test phase_boundaries`, `cargo test --locked -p skald-compiler
--doc`, then `make check`. Run `make msrv-check` if Rust targets, manifests or
supported syntax change. Any source-level wrong-code discovery must be fixed
with a reproducer before its behavior becomes baseline evidence; a substantial
semantic change needs an explicit task/design amendment.

**Exit criteria:** current boundaries and missing behavioral witnesses are
protected, tests use maintained owners, and future validation is clearly
distinguished from what this task actually proves.

**Completion evidence (2026-09-17):** task baseline `d7163d9d`, the user's
committed LP01 documentation. History and current-source review found no
previous implementation bridge or temporary artifact to remove. Existing
raw/proof-rich-input and private-authority doctests were retained; new doctests
reject attaching omitted sources and taking mutable access through the final
seal's program accessor. A focused synthetic dependency test accepts
source/MIR/seal services and rejects all six frontend roots. Scanner/policy
implementation, public contracts and orchestration remain unchanged.

Two feature-owned source goldens close the live-input, division/join and
aggregate-result/receiver pressure gaps, with 21 native executions across the
default, minimum and omitted-trace variants. A private release-selector native
probe closes the original-header/finalizer gap by replacing the owner slot and
clobbering every caller-saved integer/SIMD register. Expectations check semantic
results, callback order and allocation identity. The migration record closes
G01–G05, names exact mode coverage from reused trace/sparse/reporting tests,
and keeps future seal/constraint/placement negatives with LA02–LA05. Test
guidance and feature READMEs are updated. No substantial independent discovery
or necessary production extraction was identified.

The focused goldens, release probe, 11 phase-boundary tests and compiler
doctests passed. Final `make check` passed formatting/build/lint/docs checks,
workspace and runtime suites, all 23 compiler doctests and 650 golden leaves.
`make msrv-check` passed on Rust 1.82.0. Final documentation/whitespace checks
passed after progress updates. No production behavior, new LIR implementation,
measurement evidence or temporary migration infrastructure is introduced.
Changes remain uncommitted for the user; LP03 is next.

### LP03 — Make the foundation measurement protocol reproducible

**Purpose:** make the frozen foundation cost policy executable without building
a parallel benchmark framework.

- [x] Freeze a versioned manifest of the four compile workload families and
  range-loop, vector-growth and runtime-trace native workloads from the existing
  harness. Add a nonconstant scalar/call kernel only if LP01 found a material
  coverage gap. Record inputs, expected observations and mode/toolchain settings.
- [x] Check existing support for explicit compiler/runtime identities, warmups,
  repetition counts, per-variant order, deterministic output, watchdogs and
  semantic digests. Extend the existing harness/support only for demonstrated
  gaps in comparing two compiler builds. Identify binaries by revision/hash;
  do not label a baseline binary with the current checkout's revision.
- [x] Specify and implement reproducible collection of compiler time/RSS,
  native time, assembly/native text size, per-callable frame sizes and static
  frame-access counts. Keep target-specific measurement separate from semantic
  artifact closure. Unknown code shapes produce an explicit unsupported metric,
  never a guessed zero; validate extraction against known small assemblies.
- [x] Test new manifest/collection/comparison behavior, including mismatched
  inputs, missing metrics, noisy/inconclusive timings and semantic divergence.
  Test the accepted thresholds if classification is automated; do not encode
  operational timing thresholds into ordinary correctness tests.
- [x] Document exact commands and provenance fields, storage/retention of raw
  samples, and the paired comparison procedure. Use at least five compile and
  nine native samples per variant, warmups and alternating order. Report median,
  MAD and range. Reproduce timing regressions in a second paired run and apply
  the frozen twice-larger-MAD rule.
- [x] Carry forward the accepted review gates: repeatable increases above 10%
  in compile/native median time or 15% in peak RSS/native text size require
  correction or an explicit recorded tradeoff before adoption. Keep frame
  growth visible and correctness/ABI/trace parity unconditional.

**Tests:** `make measurement-support-test`, focused deterministic collection
and comparison tests, a small real-compiler smoke run, `make docs-check`, and
`make check`. Build timing is excluded; no full performance comparison is
claimed by the smoke run. Changes to Rust also follow the supported-toolchain
rule above. Measurement runs remain outside the ordinary gate.

**Exit criteria:** another developer can run the protocol against explicitly
identified compiler builds and distinguish valid evidence, incompatible inputs,
and inconclusive runs. No future LIR events or allocator infrastructure is
needed to collect the baseline; new-phase observations are a later extension.

**Completion evidence (2026-09-17):** task baseline `e446ecdb`, the user's
committed LP02 implementation. History/source review confirmed the prior native
probes are enduring tests, with no bridge due for removal. Foundation mode now
extends the existing cleanup entry point and Make target. Shared corpus/MIR
observation functions moved into cohesive script owners, retaining ordinary
cleanup behavior. Manifest format/workload version 1 freezes four compile and
17 native configurations, including enabled/omitted runtime-input scalar/call/
division kernels (`17 1000000`, independent stdout `-993156\n`).

The collector attests binary revisions, dirty states, build profiles/toolchains/
flags and hashes separately from collecting-checkout identity. It forces shared
runtime/stdlib/C-driver inputs; records source/runtime/harness/tool/host identity;
alternates both compile and native compiler roles; retains warmup/measured raw
events, medians/MADs/ranges, deterministic assembly and untimed reporting
equivalence/observations. Target metrics cover ELF text sections, per-callable
fixed frames, peak explicit stack reservations and direct static frame accesses.
Unknown recipes remain unsupported. A bounded physical stack-depth extractor
handles conditional/nested helper reservations without influencing semantic
artifact closure. Comparison requires two independent paired captures, validates
inputs/provenance/semantics/determinism and applies the frozen repeated cost/MAD
rules per workload. Smoke/subset captures remain nonqualifying.

Frame-recipe review exposed an unaligned exhaustion-reporter call in the
frameless generated retain helper. A private native ABI probe failed with a
hard trap before correction and passed after an eight-byte nonreturning-edge
reservation. This small correction must be included in LP04's selected compiler
revision. An external-output-root reporting bug was also fixed in ordinary
cleanup mode. No substantial independent discovery remains deferred.

`make measurement-support-test` passed all 33 deterministic support tests.
An untimed real-compiler audit extracted supported metrics for all 21 workloads
and 1,099 callable observations. Two independent paired smoke captures exercised
small-source compilation and both scalar trace modes with exact manifest
semantics; CLI comparison correctly reported **inconclusive** for their reduced
counts/nonqualifying scope. Ordinary cleanup and Make foundation smoke commands
also passed with external output roots. These checks used the dirty current
compiler build based on `e446ecdb` and Rust 1.97.1; they are functional harness
validation, not baseline/performance evidence.

Final `make check` passed formatting/build/lint/docs, workspace/runtime suites
and all 650 golden leaves. `make msrv-check` passed on Rust 1.82.0. Final
documentation/whitespace checks passed after progress updates. The measurement
procedure documents exact commands, provenance attestations, raw retention and
outcome actions. Qualified full-corpus capture and durable reviewed records
remain LP04 obligations. Changes are uncommitted for the user; no new compiler
phase, production instrumentation or transitional pipeline is introduced.

### LP04 — Capture and qualify the pre-migration baseline

**Purpose:** preserve trustworthy evidence before any production lowering move.

- [x] Select and record the pre-migration compiler revision after LP02/LP03's
  current-boundary and ABI fixes.
  Distinguish this measurement revision from the program and child implementation
  baselines. If later fixes change semantics, explicitly requalify affected
  evidence instead of silently replacing the comparison baseline.
- [x] Build the compiler/runtime outside timing, record toolchains, build
  profiles, source/runtime/binary hashes, host conditions and dirty-state
  provenance, and execute the full frozen manifest using LP03's commands.
- [x] Confirm repeated assembly determinism within each configuration and exact
  native status/stdout/stderr digests. Capture reporting-on/off equivalence and
  keep instrumentation observations in a separate untimed run.
- [x] Exercise comparison reproducibility with equivalent baseline builds or
  repeated baseline runs. This qualifies the procedure; it does not measure
  an old/new architecture improvement. Resolve absent metrics and insufficient
  workload duration, and retain noisy classifications as inconclusive.
- [x] Store compact reviewed results and artifact hashes in the measurement
  document. Preserve raw samples and manifests durably through foundation
  acceptance: ignored build paths alone are not evidence retention. Use small
  checked-in machine-readable evidence or an explicitly recorded durable store;
  keep large binaries/build products out of Git and give rebuild commands.
- [x] Record the readiness outcome and any blocking environment/corpus issue.
  A timing fluctuation does not justify weakening the frozen adoption policy.

**Tests:** `make cleanup-baseline` with the documented protocol overrides as
needed, `make measurement-support-test`, `make golden-determinism-test`, and
`make golden-release-test`. Use LP03's explicit invocation where the Make target's
defaults are insufficient. Validate all retained evidence and documentation links.

**Exit criteria:** the baseline is reproducible, complete for the manifest and
qualified for later comparison. Missing evidence leaves this task open; it
cannot be marked complete because the architecture implementation has not begun.

**Completion evidence (2026-09-17):** task/measurement baseline
`9e3cebb172db1c5b0f7813ce7c34a87b019c9e9d`, the user's committed LP03 result.
History/source review confirmed LP03's collector, scalar kernel, metric fixtures
and ABI probe have continuing owners; no transition is due for removal here.
The measurement revision includes the retain-helper correction and is distinct
from program/child implementation baseline `495debd3`.

`make golden-tools runtime` completed before timing. One preserved clean golden
compiler serves both roles with Rust 1.97.1, no extra flags and the same runtime.
Two independent full captures (`pair-azv57129`, `pair-uk5oz741`) use CPU 14,
opposite starting orders, warmups, 15 compile/31 native samples per role/workload.
Both collecting identities were clean at `9e3cebb1`. WSL2 governor/turbo controls
are unavailable and Windows background load is uncontrolled; exact host/tool
conditions, commands and hashes are retained.

All 21 configurations pass semantic digests, assembly determinism and untimed
reporting equivalence, with all frame/text metrics supported. Static metrics,
assembly and linked executable hashes agree across all four role/capture
combinations. Native durations satisfy the noise rule even for the shortest
workloads. Checked-in records preserve 1,260 measured compile and 2,108 measured
native events plus 152 warmups, all per-callable metrics, copied manifest,
build attestation, comparison, and raw untimed stderr/native output. The six
hashed evidence records total about 355 KB; no large assembly or executable is
checked in. All 56 source/harness hashes were independently checked against the
selected commit. The
[measurement document](../development/LOW_LEVEL_COMPILER_MEASUREMENTS.md#reviewed-pre-migration-baseline)
owns compact results, artifact hashes, rebuild/retention and requalification
instructions.

Baseline **input readiness passes**, with no absent or unsupported metric.
Equivalent-build **cost classification remains inconclusive**: all RSS/text and
17 native timing gates are within limits, ten compile timing gates are within
limits, and eleven short compile timings are noisy. They are retained unchanged;
no threshold is weakened and no cost exception is granted. This task qualifies
complete baseline inputs/procedure, not architecture adoption. Future adoption
must resolve those cost uncertainties with compatible paired evidence and
improved host control/measurement support; more repeats alone do not eliminate
within-sample MAD.

The new untimed evidence verifier checks hashes, equivalent build/manifest
identities, complete metrics/counts/alternation, row/event agreement, raw
observations/native output, deterministic artifacts and comparison replay.
Corruption, missing warmups/samples/metrics, changed attestations and invented
results have regressions; noisy complete evidence remains distinguishable.
`make measurement-support-test` passed all 43 tests. Extended
`make golden-determinism-test` and `make golden-release-test` each passed all
650 leaves, as did final `make check` with workspace/runtime/static/doc checks.
No Rust/toolchain surface changed. No substantial independent discovery is
deferred. Changes remain uncommitted for the user; LP05 is next.

### LP05 — Cumulative review, downstream handoff, and closure

**Purpose:** close LA01 preparation with explicit obligations for the
remaining program and no unowned transitional code.

- [x] Review `git diff <implementation-baseline>..HEAD`, its stat/name-status
  views, staged/unstaged changes and untracked files as one change. Inspect
  earlier task commits even when the working tree is clean; separate unrelated
  intervening changes from this roadmap's work.
- [x] Reconcile the transition ledger against source/history, using symbol
  searches and `git log -S` where needed. Remove expired helpers, flags, scanner
  exceptions, duplicate fixtures and measurement shortcuts. Record a continuing
  purpose and owner for every retained adapter.
- [x] Check the frozen decisions against all outputs. Confirm no phantom phases,
  allocator requirements, x86 types in future shared contracts, or unsupported
  AArch64 implementation claims have entered the handoff. Resolve substantial
  gaps before closure; record independent opportunities separately.
- [x] Publish a concise handoff covering accepted contracts, coverage/test gaps
  owned by LA02–LA05, qualified baseline references, and joint LA02/LA03 design
  questions. LA02 owns concrete schemas and seals; LA03 owns target constraints,
  transfers and frames. Neither starts dependent implementation until its
  focused design and roadmap settle those details.
- [x] Run final gates after fixups from an artifact-free snapshot or clean
  checkout. Record baseline, reviewed endpoint, residual uncommitted changes,
  evidence and ledger dispositions. Leave committing to the user.
- [x] Mark LA01 complete only after all task exits pass. Archive this roadmap
  and its focused frozen design, update archive/active indexes and every link,
  and keep the overarching proposal and program coverage record active. A22
  remains in progress; LIR, foundation adoption and allocation are not delivered.

**Tests:** `make check`, `make measurement-support-test`, and `make msrv-check`
when Rust/toolchain surface changed. LP04 supplies extended determinism/release
evidence; rerun affected extended gates if subsequent edits invalidate it.
Run `make docs-check` again after archival and `git diff --check` on closing
changes. Do not rerun lengthy timing experiments solely because docs moved.

**Exit criteria:** cumulative changes meet LA01's accepted scope, every
transitional artifact has a disposition, evidence remains accessible, and
the next work is the LA02 proposal with coordinated LA03 interface review.

**Cumulative review (2026-09-17):** reviewed implementation baseline `495debd3`
through endpoint `472bce66`, all four intervening task commits, their stat/name
views, and the uncommitted closing changes including both archived files.
History contains no unrelated intervening implementation. Source, symbol and
`git log -S` review reconciled every ledger entry: no expired bridge, gate,
scanner exception, lint allowance, placeholder phase or duplicate lowering
remains. Private native ABI/count probes, semantic goldens, public privacy
tests, measurement owners and raw records have continuing purposes. Original
cleanup workload definitions are AST-identical to the pre-roadmap definitions.
The frozen contracts require no amendment; LIR/target interfaces and all new
phase delivery remain pending.

One small closing defect was reproduced: second-operand stack/base writes in
`xchg`/`xadd` and unsupported `loop` control flow could be reported as supported
zero-stack recipes. Conservative rejection and negative fixtures now protect
the extractor; ordinary frame-memory swaps remain supported. Untimed
re-extraction agrees with all 4,396 retained callable observations. Original raw
records, source attestations and comparison remain unchanged. The measurement
guide records the changed harness fingerprint and future baseline/candidate
recapture requirement. No new timing experiment or cost clearance is claimed.

The active [handoff](../roadmaps/LOW_LEVEL_COMPILER_MIGRATION_COVERAGE.md#preparation-handoff-and-next-designs)
records accepted constraints, pending delivery/test ownership, baseline limits
and joint LA02/LA03 decisions. Next is LA02's focused design with coordinated
target review before dependent implementation. A22 and the overarching program
remain in progress; the child design and roadmap are archived without archiving
the program coverage record. No independent substantial discovery was found.

**Closing validation:** an artifact-free source snapshot at
`/tmp/skald-low-level-phase-closure-b2w94lyh` copied all 2,272 tracked/untracked
source files, including the archived documents and closing fixes, with no
`target`, `build` or `.git`. The source manifest was retained outside Git at
`/tmp/skald-lp05-source-manifest.json`. Offline `RUSTUP_TOOLCHAIN=stable make check`
passed fresh formatting/build/lint/docs, workspace/privacy/native/runtime suites
and all 650 golden leaves. In the same snapshot,
`make measurement-support-test` passed all 44 tests, and the untimed retained
baseline verifier passed with the expected **inconclusive** cost result.
LP04's extended determinism/release evidence remains valid: this task changes
only Python extraction/tests and documentation. No Rust/toolchain surface
changed, so no additional MSRV run was required; earlier Rust changes already
passed Rust 1.82.0. Post-archival documentation and whitespace checks passed.
Residual uncommitted changes are the conservative metric guard/regressions,
handoff/status/index/link updates, and child document archival/closing evidence.
Committing remains with the user; the reviewed endpoint is `472bce66` plus this
closing change, not an invented closing commit.

## Ordering and checkpoints

LP01 → LP02 → LP03 → LP04 → LP05 is the default sequence. Protocol work can be
prepared after LP01 alongside regression work, but baseline capture follows
both; capture must reflect any correctness fixes. Independent downstream design
discussion may proceed from the frozen contracts without implying permission
to implement undecided schemas.

| Checkpoint | Proceed when | Otherwise |
| --- | --- | --- |
| Contract readiness after LP01 | All current families have owners and reviewable test dispositions | Complete the inventory or explicitly amend a conflicting design; do not invent a legacy fallback |
| Baseline readiness after LP04 | Identity, semantics, determinism and all required metrics are complete/qualified; noisy cost classifications retained explicitly | Repair missing/invalid collection/corpus evidence and leave capture open; preserve uncertainty without claiming adoption cost clearance |
| LA01 closure after LP05 | Contracts, guards, evidence and cumulative review pass | Resolve scope gaps in this roadmap; record independent work separately |
| Future foundation adoption in LA05 | Full new-pipeline parity and frozen cost gates pass | Fix regressions or record a reviewed cost exception; keep production adoption pending. Do not require LA06 to rescue the baseline |

These are readiness gates, not an experiment to revert an accepted architecture
on the first noisy measurement. A future rejected prototype is removed using
its task baseline; independently useful tests/evidence remain. Any change to
frozen policy is an explicit design amendment with rationale and affected owners.

## Transition ledger and discoveries

LP01 introduced no transitional implementation artifact. Update this ledger
as work proceeds; the user normally commits between tasks. The maintained
migration record carries pending program obligations through LA05.

| Artifact/file or symbol | Introducing task/commit | Removal or transfer owner | Final disposition and evidence |
| --- | --- | --- | --- |
| No bridge, gate, exception or exploratory code introduced | LP01; `d7163d9d` | — | Documentation inventory only; source/history review recorded above |
| Live-input/aggregate-pressure goldens and private count probe | LP02; `e446ecdb` (task baseline `d7163d9d`) | Retain through downstream migrations | Enduring regression fixtures/test doubles; no production bridge, gate, placeholder IR or extraction introduced |
| Foundation collector/comparator, versioned manifest, scalar kernel and metric fixtures | LP03; `9e3cebb1` (task baseline `e446ecdb`) | Retain through foundation adoption | Enduring opt-in machinery under the existing cleanup entry point; no production instrumentation, rollout gate or new pipeline |
| Generated retain exhaustion stack alignment/probe | LP03; `9e3cebb1` (task baseline `e446ecdb`) | Retain | ABI correction with native failure-before/fix-after proof; not migration scaffolding |
| Retained baseline records and untimed evidence verifier/tests | LP04; `472bce66` (task/measurement baseline `9e3cebb1`) | Retain through foundation adoption | Complete checked-in raw evidence with supported metrics; eleven noisy compile timing gates remain inconclusive. No production bridge or new phase |
| Conservative stack recipe guard/regressions | LP05; commit pending (task baseline `472bce66`) | Retain | Reject unsupported second-operand stack/base writes and `loop` edges; all 4,396 retained callable metrics unchanged |

Carry continuing program obligations into the migration record with explicit
owners; do not reset their history at a child-roadmap boundary. Shared baseline
placement is a planned implementation, not a temporary bridge to delete during
LA01 closure. The legacy selector remains production code until LA05 retires it.

Record substantial independent maintainability findings in
`docs/archive/LOW_LEVEL_COMPILER_ARCHITECTURE_DISCOVERIES.md` when the first
actionable finding exists, and index it then. Include evidence, impact, owner,
and a bounded follow-up. Required correctness/contract gaps stay in the active
roadmap rather than being deferred merely to mark it complete.
