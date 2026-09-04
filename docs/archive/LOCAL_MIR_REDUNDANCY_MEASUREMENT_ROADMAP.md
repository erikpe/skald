# Local Final-MIR Redundancy Measurement Roadmap

Status: complete; archived after LMR0-LMR7.

This roadmap measures the local redundancy left by Skald's implemented
final-MIR pipeline and compares three concrete follow-ups: narrow scalar-spill
constant provenance (FMV-15), redundant primitive cast elimination (FMV-02),
and same-block primitive common-subexpression elimination (FMV-03). Its
purpose is to identify both the most useful next optimization and the compiler
boundary responsible for missed opportunities.

The durable result is a deterministic read-only opportunity census, a
representative repository corpus, and an evidence report that can be repeated
as the compiler changes. This roadmap does not implement any of the three
optimizations, assign them synthetic performance scores, or promise that a
counted MIR opportunity becomes a native speedup.

Implementation-specific findings outside this reviewed scope were recorded
separately while they remained actionable.
Candidate status, placement, and prioritization remain authoritative in the
[optimization candidate catalog](../roadmaps/OPTIMIZATION_CANDIDATE_CATALOG.md).
The frozen census, corpus, schema, and decision rules are authoritative in the
[measurement contract](LOCAL_MIR_REDUNDANCY_MEASUREMENT_CONTRACT.md).

## Dependencies

- The completed
  [selectable final-MIR pipeline roadmap](SELECTABLE_FINAL_MIR_OPTIMIZATION_PIPELINE_ROADMAP.md)
  provides deterministic verified checkpoints, typed pass measurements, and
  exact schedule identities.
- The completed
  [local final-MIR simplification roadmap](LOCAL_FINAL_MIR_SIMPLIFICATION_ROADMAP.md)
  provides primitive facts, exact constant evaluation, guarded value-use
  classification, CFG cleanup, and the repeated default schedule being
  measured.
- The completed
  [checked integer constant protocol simplification roadmap](CHECKED_INTEGER_CONSTANT_PROTOCOL_SIMPLIFICATION_ROADMAP.md)
  provides the first concrete scalar-spill provenance limitation and exact
  checked-protocol consumer.
- The completed
  [whole-world reachability roadmap](TARGET_INDEPENDENT_WHOLE_WORLD_REACHABILITY_ROADMAP.md)
  lets the census distinguish redundancy in definitions that survive semantic
  retention from redundancy later removed with dead definitions.
- Current driver inspection reaches static activation directly, while
  `MirPipelineInspector` is exposed at the final-MIR pipeline boundary. The
  measurement path must compose these services without making analysis part of
  compilation identity, pass selection, diagnostics, or ordinary reporting.
- Niflheim's assembly-measurement helpers demonstrate the value of typed,
  separately tested structural counters. Skald's census remains MIR-aware and
  follows Skald's verified identities, checked protocols, lifecycle metadata,
  and permanently whole-world compilation model.

## Scope and invariants

- Observe only verified final MIR through borrowed checkpoint capabilities. A
  census cannot mutate MIR, retain a verified borrow, reseal a product, or
  influence backend input.
- Measure the current `default` schedule at three semantic points: pipeline
  input; the verified checkpoint immediately before whole-world reachability;
  and the final verified reachable program. Record the exact resolved schedule
  with every result so later comparisons cannot silently cross schedule drift.
- Treat one whole-world compilation as the unit of observation. Imported
  definitions are counted once per compilation, not once per source file that
  mentions them.
- Keep per-workload results alongside totals. Deduplicate identical configured
  compilation roots in the corpus and never let a large standard-library graph
  conceal whether an opportunity is broad or isolated.
- Use deterministic integer structural evidence: candidate sites, affected
  instructions and values, callables containing opportunities, consumer
  families, rejection reasons, candidate overlaps, and checkpoint deltas.
  Timing is operational context only and never participates in determinism or
  correctness assertions.
- Report both raw and reachability-adjusted observations. Input and
  pre-reachability counts explain where redundancy arises; final counts are the
  primary estimate of work left in code that reaches backend lowering.
- For scalar-spill provenance, distinguish direct constant stores, one-hop
  store/load forwarding, transitive canonical chains, checked-protocol
  consumers, other scalar consumers, and rejection by multiple writes,
  dominance, type, place shape, alias exposure, lifecycle, or protected
  metadata.
- For redundant casts, distinguish exact identity casts, adjacent cast chains
  with a provably unchanged value domain, other repeated conversions, and
  patterns that require range, canonicalization, checked-failure, floating, or
  raw-bit reasoning. “Interesting” and “proven removable” counts must not be
  conflated.
- For common subexpressions, initially count exact repeated non-failing integer
  or boolean primitive rvalues in the same block with identical operation,
  type, and operand identities and an earlier dominating definition. Exclude
  loads, checked operations, calls, ownership, path/proof queries, floating
  operations, and casts already attributed to the cast census.
- Classify whether replacing a repeated value would be accepted by the current
  value-use boundary. Keep protected-use and source-observation concerns
  separate from expression equivalence.
- Define disjoint primary attribution and an explicit overlap matrix. In
  particular, report when scalar-spill provenance would expose a cast, checked
  fold, constant fold, branch fold, or common subexpression instead of counting
  the same prospective removal as independent wins.
- Do not infer runtime benefit directly from MIR counts. Record baseline MIR,
  assembly, executable-size, compile-time, and native-time context where the
  existing benchmark contracts support it, but describe transformation benefit
  as an upper bound until an implementation or bounded prototype measures it.
- Do not add a registered MIR pass, default-schedule occurrence, `skac`
  optimization option, or mandatory analysis cost. The repository measurement
  command is opt-in and outside ordinary compilation and golden correctness
  execution.
- Keep exact source semantics, diagnostics, evaluation order, failure timing,
  runtime traces, lifecycle behavior, dense identities, ABI, and emitted
  artifacts unchanged.
- Retain only analysis APIs and tooling that have a clear continuing owner.
  Remove investigative scaffolding at closure rather than preserving a shadow
  optimizer or a second MIR semantic table.
- Prefer a multidimensional recommendation over a fabricated weighted score.
  The final comparison must cover reachable frequency, breadth across
  workloads, direct structural ceiling, downstream leverage, blocker shape,
  implementation effort, semantic risk, and reusable architectural value.
- Keep `mod.rs` files concise facades and tests beside each analysis owner. The
  root Makefile remains the repository and external automation surface; add no
  repository CI or wall-clock correctness threshold.

## Progress

- [x] LMR0 — Freeze the census and corpus contract
- [x] LMR1 — Compose verified MIR inspection through the driver
- [x] LMR2 — Measure scalar-spill constant provenance
- [x] LMR3 — Measure redundant primitive casts
- [x] LMR4 — Measure local primitive common subexpressions
- [x] LMR5 — Build deterministic corpus aggregation and reporting
- [x] LMR6 — Run the study and select the next optimization project
- [x] LMR7 — Harden the measurement boundary and close the roadmap

## PR-sized implementation sequence

### LMR0 — Freeze the census and corpus contract

**Purpose:** Make every later number interpretable before analysis code can
shape the question it is intended to answer.

- [x] Specify the input, pre-reachability, and final checkpoint selection by
      semantic schedule position rather than a hard-coded numeric occurrence.
- [x] Define the canonical schema for compilation identity, schedule identity,
      checkpoint identity, structural totals, per-candidate outcomes, blocker
      classifications, overlaps, and operational context.
- [x] Define “interesting”, “proven candidate”, “blocked candidate”, and
      “estimated downstream unlock” so reports never present speculative sites
      as safe rewrites.
- [x] Freeze disjoint primary attribution and overlap rules for FMV-15, FMV-02,
      and FMV-03.
- [x] Select a versioned corpus covering focused candidate-positive and
      candidate-negative fixtures, optimization goldens, checked protocols,
      cast-heavy primitive code, control flow and lifecycle, standard-library-
      backed programs, generic-vector and range benchmarks, and the available
      larger solver workload.
- [x] Record how corpus entries carry entry roots, provider roots, standard
      library selection, compiler arguments, stdin, and native arguments
      without duplicating source ownership from the golden suite.
- [x] Define the final comparison matrix and the evidence threshold for a
      recommendation, including an allowed “none is justified yet” result.

**Tests:** Documentation link validation; schema examples covering zero,
positive, rejected, overlapping, and saturated counts; duplicate compilation
identity examples; schedule-position examples; `git diff --check`.

**Exit criteria:** The measurement units, snapshots, taxonomy, corpus, report
schema, and decision criteria are explicit enough that all three analyzers can
be implemented independently and still produce comparable results.

**Completion evidence:** The frozen
[measurement contract](LOCAL_MIR_REDUNDANCY_MEASUREMENT_CONTRACT.md) defines
whole-world compilation identity, semantic checkpoint selection, exact census
accounting, saturated counts, exclusive primary blockers, candidate-specific
eligibility, disjoint attribution, directed overlaps, a sixteen-workload
version-one corpus, golden-plan reuse, canonical JSON and human projections,
operational-field exclusion, manual audit sampling, and an explicit threshold
that permits a no-optimization conclusion. Documentation examples cover empty,
positive, rejected, overlapping, saturated, duplicate, and distinct
compilation identities. No compiler, pipeline, pass, or runtime behavior
changes in this contract-only milestone.

### LMR1 — Compose verified MIR inspection through the driver

**Purpose:** Give repository tools access to real whole-world final-MIR
checkpoints without adding an optimization pass or parsing dumps.

- [x] Replace the driver's single-purpose inspected-call plumbing with one
      request-local inspection service that can independently carry optional
      static-activation and MIR-pipeline inspectors.
- [x] Thread the optional `MirPipelineInspector` through module-graph and
      single-source compilation into the existing inspected pipeline runner.
- [x] Keep inspectors outside `CompilationRequest`, request equality, report
      events, diagnostics, artifacts, and backend inputs.
- [x] Preserve the ordinary no-inspector path without census construction,
      dump rendering, filesystem work, or additional MIR traversal.
- [x] Expose only the minimal facade needed by workspace tools and integration
      tests; remove superseded overloads rather than accumulating parallel
      inspection APIs unless a concrete repository consumer requires a narrow
      compatibility delegate.
- [x] Document the composable inspection boundary in driver, reporting,
      debugging, and public-API coverage.

**Tests:** No-inspector parity; activation-only, MIR-only, both-inspector, and
failing-compilation callback matrices; exact verified checkpoint order;
borrowed-only compile-fail coverage; request-equality and reporting parity;
public facade integration tests; full compiler tests.

**Exit criteria:** A workspace tool can inspect every verified final-MIR
checkpoint of an ordinary whole-world compilation while the uninspected
compiler path and all observable products remain unchanged.

**Completion evidence:** `CompilationInspectors` now composes independently
optional borrowed static-activation and MIR-pipeline callbacks for both
module-graph and single-source driver entry points, while ordinary adapters
continue through the allocation-free no-inspector runner. The phase-owned MIR
coordinator shares one instrumented execution path when inspection and trace
occurrence reporting are both enabled. Driver tests cover no inspection,
activation only, MIR only, both services, early compilation failure, exact
default-schedule checkpoint order, artifact/diagnostic parity, and normalized
report-event parity. Public API coverage exercises both callbacks together,
and compile-fail documentation proves that borrowed checkpoints cannot escape
their invocation. Driver, reporting, phase, and debugging documentation now
describe the composed boundary.

### LMR2 — Measure scalar-spill constant provenance

**Purpose:** Determine whether short private storage chains are now the main
barrier to constant-driven simplification and whether one narrow analysis would
serve several later passes.

- [x] Add one read-only seal-local scalar-spill provenance observer over exact
      storage declarations, writes, loads, types, CFG predecessors, and
      dominance.
- [x] Count direct, one-hop, and transitive canonical constant provenance
      separately; never publish a transitive fact after ambiguity or an
      unsupported boundary.
- [x] Classify every rejected candidate by multiple or conditional writes,
      nondominance, mismatched type, noncanonical place projection, alias
      exposure, lifecycle participation, protected metadata, malformed
      identity, or unsupported producer.
- [x] Attribute consumers to checked integer protocols, total primitive
      operations, casts, branches, stores, returns, calls, and other families.
- [x] Estimate downstream opportunities only by invoking the existing exact
      evaluator or verified protocol observer with read-only virtual facts; do
      not rewrite MIR or duplicate arithmetic/protocol semantics.
- [x] Keep detailed facts internal and return deterministic aggregate and
      per-callable observations suitable for the common report schema.

**Tests:** Direct and nested checked constants; one-hop and transitive chains;
multiple writes and loop epochs; cross-block dominance; type mismatch;
projected and aliased places; lifecycle/proof-protected storage; exact consumer
classification; unchanged input MIR; deterministic order; malformed identity
handling.

**Exit criteria:** The census can say how often narrow provenance is provable,
why it is rejected, where it occurs in reachable code, and which existing or
future simplifications it would expose without claiming general load/store
propagation.

**Completion evidence:** The public read-only
`analyze_scalar_spill_provenance` entry point accepts only a
`VerifiedFinalMirProgram` and returns deterministic program and per-callable
counts while all instruction-position and constant facts remain internal. It
classifies direct, one-hop, and transitive chains; exact consumer families;
one exclusive primary blocker plus every applicable ordered barrier; distinct
supporting values and instructions; and conservative removal upper bounds.
One-step primitive and checked-integer unlocks reuse the implemented exact
evaluators. Focused tests cover verified checked protocols, direct and chained
provenance, primitive and checked unlocks, multiple writes, dominance and loop
epochs, type and place failures, alias and protected/lifecycle roles,
malformed declaration identity, empty programs, deterministic repetition, and
unchanged verified MIR. Public-facade and living phase/debugging documentation
cover the opt-in analysis without adding a pass, request option, report event,
or ordinary compilation cost.

### LMR3 — Measure redundant primitive casts

**Purpose:** Separate safely removable conversion churn from casts that encode
canonicalization, range, checked failure, or representation semantics.

- [x] Inventory primitive cast assignments by exact cast kind, source/result
      type, checkpoint, callable, and consumer family.
- [x] Recognize exact identity casts and adjacent block-local chains whose
      composed conversion is proven equal for the complete source value domain.
- [x] Use one explicit value-domain/cast-composition table owned by the census;
      reuse implemented primitive semantics and classify unsupported pairs
      rather than inferring from equal host widths.
- [x] Separate raw-bit reinterpretation, integer narrowing/widening, boolean
      canonicalization, integer/floating conversion, and checked
      floating-to-integer protocols.
- [x] Classify blockers including nonadjacent provenance, multiple uses,
      protected use roles, control-flow boundaries, lost-range knowledge,
      checked failure, floating payload concerns, and unsupported composition.
- [x] Record whether repeated casts appear to originate in lowering, source
      code, generic specialization, or earlier final-MIR rewriting when that
      distinction follows from deterministic checkpoint evidence.

**Tests:** Complete primitive type/kind matrix; identity and safe chain cases;
narrow/widen counterexamples; boolean canonicalization; raw-bit and NaN
barriers; checked conversion diamonds; cross-block and protected uses;
unchanged MIR; deterministic classification.

**Exit criteria:** The report can distinguish definite redundant casts from
interesting but proof-dependent conversions and identify whether the likely
owner is lowering canonicalization or a final-MIR pass.

**Completion evidence:** The public read-only
`analyze_redundant_primitive_casts` entry point accepts only a verified
final-MIR product and returns deterministic totals plus callable-ordered
observations. Every ordinary cast records its exact operation/source/target
shape, disposition, and semantic result consumers; checked floating-to-integer
rvalues and range checks are separate excluded-protocol counts. One explicit
complete-domain table proves identity, lossless integer composition, and the
canonical boolean round trip while retaining narrowing/widening, boolean,
floating, raw-bit/NaN, checked-failure, adjacency, use-count, protected-use,
control-flow, malformed-identity, and unsupported-composition barriers.
Supporting identities and conservative removal ceilings remain aggregate-only,
and repeated analysis leaves MIR unchanged. Single-snapshot results make no
unprovable source/lowering/generic origin claim; the frozen checkpoint
comparison in LMR5 is the deterministic evidence boundary for attributing an
input-created or earlier-pass-created pattern. Focused tests cover the full
25-cell primitive pair matrix, safe and unsafe chains, checked diamonds,
protected roles, cross-block replacement, exact shapes, deterministic results,
and unchanged MIR. Public-facade and living phase/debugging documentation are
updated without adding a pass or ordinary compilation cost.

### LMR4 — Measure local primitive common subexpressions

**Purpose:** Establish whether exact same-block scalar repetition is common
enough to justify CSE before investing in global value numbering or SSA.

- [x] Build a deterministic same-block expression key from exact operation,
      result type, and ordered operand identities for the frozen non-failing
      integer/boolean family.
- [x] Treat commutative reordering, reassociation, constants, casts, loads,
      floating operations, checked protocols, calls, ownership, and semantic
      queries as separate or excluded observations rather than silently
      broadening equivalence.
- [x] Require an earlier same-block definition and classify every later exact
      repeat by replacement-safe use roles, protected roles, dead result,
      source-observation concern, or malformed identity.
- [x] Count repeated definitions, replaceable uses, potentially removable
      values/instructions, affected callables, maximum repetitions per key, and
      operation-family distribution.
- [x] Record overlap where scalar-spill provenance would make operand identity
      or constant equivalence visible, while keeping the direct CSE count based
      only on current MIR identities.
- [x] Keep expression facts block-local and reset them at every CFG boundary;
      do not introduce global value numbering, memory numbering, or persistent
      optimization facts.

**Tests:** Exact repeats and near misses; operand order; result type and width;
dominance by instruction order; multiple consumers; protected uses; dead
results; block boundaries and loops; excluded families; deterministic maxima
and aggregate order; unchanged MIR.

**Exit criteria:** The census identifies the reachable direct ceiling for a
conservative same-block CSE pass and shows whether broader CFG or storage facts,
rather than expression matching, dominate the missed opportunities.

**Completion evidence:** The public read-only
`analyze_local_primitive_common_subexpressions` entry point measures exact
ordered unary, binary, and comparison keys over the frozen total integer and
boolean families. Facts reset for every basic block, so instruction order is
the only admitted dominance proof and loop or cross-block equivalence is never
silently promoted. Deterministic totals and callable-ordered observations
separate replaceable and dead results, count replaceable uses, operation and
consumer families, repetition maxima, supporting identities, blockers, and
conservative value/instruction removal ceilings. Constants, casts, loads,
floating operations, checked protocols, calls, ownership/lifecycle work,
input/output, and semantic queries remain explicit exclusions. A seal-local
query reuses scalar-spill provenance to count immediate constant-equivalence
overlap without changing direct CSE attribution or publishing persistent facts.
Focused tests cover exact repeats, multiple and dead consumers, operand order,
operation and result-type near misses, unary/comparison families, protected and
source-observation roles, malformed identities, block and loop boundaries,
excluded families, scalar-spill overlap, deterministic repetition, and
unchanged MIR. The three censuses now share one typed count wrapper while
retaining their established public aliases; no pass or ordinary compilation
cost was added.

### LMR5 — Build deterministic corpus aggregation and reporting

**Purpose:** Turn the three analyzers into a repeatable repository study rather
than a collection of synthetic unit-test counts.

- [x] Add an opt-in repository measurement tool with human and canonical JSON
      output; do not add a `skac` flag or registered MIR pass.
- [x] Parse and validate the frozen corpus configuration before compilation,
      canonicalize repository-relative identities, and reject duplicate or
      escaping paths deterministically.
- [x] Compile each configured root through the real driver and current default
      schedule, collecting the three semantic checkpoints through the composed
      inspection service.
- [x] Aggregate with saturating integer counts while preserving per-workload,
      per-checkpoint, per-candidate, blocker, consumer, and overlap detail.
- [x] Record compiler revision or dirty-state context, resolved schedule,
      target, runtime-trace policy, corpus version, assembly/executable size
      where produced, and explicitly nondeterministic elapsed-time context.
- [x] Keep generated outputs under ignored `build/measurements/`; make the
      canonical report reproducible without checking binaries or dumps into
      source control.
- [x] Add a focused Makefile target and usage documentation without adding the
      measurement to ordinary correctness gates.

**Tests:** Manifest validation and path containment; duplicate identities;
empty and partial corpora; compiler failure attribution; canonical JSON key and
array order; human/JSON semantic parity; saturating aggregation; independent-
process deterministic output after removing operational fields; focused
end-to-end repository corpus smoke test.

**Exit criteria:** One documented command reproducibly measures all three
candidate families across the reviewed corpus and produces stable machine-
readable evidence without changing ordinary compiler behavior.

**Completion evidence:** The `skald-mir-measure` repository tool validates the
versioned sixteen-workload manifest against the complete golden plan before
compilation, canonicalizes contained repository paths, rejects duplicate IDs
and compilation identities, and accepts explicit partial selections. It uses
the real in-process whole-world driver with omitted runtime traces and the
current default pass schedule, resolves the pre-reachability snapshot by the
sole final `whole-world-reachability` occurrence, and runs all three read-only
censuses over verified borrowed checkpoints. One typed report owns canonical
JSON and human projections, workload/checkpoint/candidate/callable breakdowns,
directed overlap counts, category breadth, assembly size, native input digests,
revision and dirty context, optional nondeterministic compile duration, and
sticky saturating totals. Tests cover manifests, path containment, duplicate
identity rejection, empty and partial corpora, compilation failure ownership,
SHA-256 vectors, saturation, projection parity, real-driver checkpoint
selection, and independent-process structural determinism. The focused
`make mir-redundancy-measure` command writes only below ignored
`build/measurements/` and remains outside ordinary correctness gates.

### LMR6 — Run the study and select the next optimization project

**Purpose:** Convert measurements into an explicit architectural decision
rather than choosing the largest unqualified raw count.

- [x] Run the complete corpus from an artifact-free state and retain the
      canonical result together with compiler revision, schedule, target, and
      corpus identity.
- [x] Review outliers manually against MIR dumps and source so every material
      counter represents the classified shape rather than an analyzer bug or
      duplicated compilation root.
- [x] Compare raw and reachable opportunities, workload breadth, removable
      structural ceiling, downstream unlocks, overlaps, blocker distributions,
      likely creation phase, implementation effort, semantic risk, and reusable
      analysis value.
- [x] For existing benchmark workloads, record baseline compile time, assembly
      and executable size, and native time as context without claiming an
      unimplemented delta.
- [x] Publish a durable measurement report with per-workload data, totals,
      limitations, rejected interpretations, and the evidence supporting its
      recommendation.
- [x] Select exactly one next action: a candidate-specific design/roadmap, a
      bounded lowering cleanup, a broader prerequisite investigation, more
      representative workload collection, or no optimization yet.
- [x] Update FMV-15, FMV-02, FMV-03 and the suggested evaluation order in the
      optimization candidate catalog; update related discoveries without
      duplicating the report.

**Tests:** Canonical report regeneration; independent-process structural
determinism; hand-audited sample agreement; baseline native equivalence across
measurement on/off paths; documentation links; no wall-clock assertions.

**Exit criteria:** The repository contains reproducible evidence explaining
what local redundancy remains, which boundary creates or blocks it, and why one
specific next action is preferred—or why the current evidence supports none.

**Completion evidence:** The durable
[measurement report](LOCAL_MIR_REDUNDANCY_MEASUREMENT_REPORT.md) records the
clean compiler revision, complete nine-occurrence schedule, fixed target and
trace policy, corpus identity, canonical JSON byte count and SHA-256,
byte-identical independent regeneration, checkpoint totals, all sixteen
per-workload results, blocker and overlap interpretation, and native benchmark
context. All 26 proven final sites were checked against final-MIR dumps and
source: 25 direct scalar carriers belong only to the focused checked-protocol
fixture, and one `f64` identity cast belongs to the primitive cast matrix. No
exact local CSE site or classification error was found. None of the three
candidates crosses the frozen breadth and material-consumer threshold, so the
report selects exactly one next action: start no candidate-specific
optimization and proceed to measurement-boundary hardening and roadmap
closure. The candidate catalog and nested-checked discovery link to this
result without copying its evidence tables.

### LMR7 — Harden the measurement boundary and close the roadmap

**Purpose:** Leave a maintainable analysis surface, authoritative evidence,
and no permanent investigative debris.

- [x] Audit checkpoint plumbing, shared census vocabulary, each candidate
      analyzer, corpus ownership, aggregation, rendering, and tests by
      responsibility; split only genuine mixed owners and keep facades concise.
- [x] Remove temporary probes, duplicate semantic tables, debugging output,
      hard-coded schedule positions, absolute paths, roadmap codes, and unused
      compatibility helpers from production code and living documentation.
- [x] Confirm the tool remains read-only, opt-in, deterministic, target-
      independent in its MIR classifications, and absent from ordinary
      compilation cost and pass registration.
- [x] Retain reusable candidate facts only where a selected future pass has a
      concrete owner; otherwise keep the stable aggregate census or remove the
      unused internal detail.
- [x] Reconcile the report, discoveries, catalog statuses, candidate links,
      and selected next action.
- [x] Run the complete repository validation from an artifact-free snapshot,
      supported MSRV, independent-process determinism, the full measurement
      corpus, and documentation/diff hygiene checks.
- [x] Mark every task complete, archive the roadmap and durable measurement
      report, update active/archive indexes, and repair all incoming links.
- [x] Archive or remove the discoveries record only if no actionable finding
      remains; otherwise keep it indexed under `docs/roadmaps/`.

**Tests:** `make check`; `make check-long`; `make msrv-check`; focused
measurement determinism and corpus regeneration; `make docs-check`; `git diff
--check`; repository-status review; and manual comparison of the documented
schema and recommendation against generated evidence.

**Exit criteria:** The study is repeatable, its recommendation is evidence-
backed, ordinary compilation is unchanged, no temporary measurement mechanism
remains, completed records are archived, and every surviving discovery has a
clear future owner.

**Completion evidence:** The measurement tool was audited by responsibility.
Verified-checkpoint collection and semantic selection remain in the real-driver
owner, while stable report projection and callable labeling moved from that
mixed 678-line implementation into a private `projection` module. Corpus
resolution, aggregation, rendering, revision inspection, digesting, and each
compiler census retain separate cohesive owners behind concise facades. The
surviving APIs exposed stable aggregate evidence only at roadmap closure; the
subsequently completed bounded site-example extension replaced the temporary
audit probe without changing census semantics. No registered pass, ordinary
compilation hook, hard-coded numeric
schedule position, duplicate semantic table, temporary probe, absolute path,
or optimization mutation was retained. The complete corpus regenerated twice
with byte-identical structural JSON after operational fields were excluded.
`make check`, `make check-long`, `make msrv-check`, `make docs-check`, and
`git diff --check` passed from the closure snapshot. The report, contract,
catalog, living workflow, indexes, and actionable discoveries were reconciled;
the roadmap, contract, and version-one report are archived.

## Ordering and dependencies

The contract and corpus land first so the three opportunity analyzers cannot
choose incompatible units or favorable workloads. Driver inspection follows
before candidate logic because real whole-world checkpoints are the shared
input and dump parsing is not an acceptable analysis boundary.

Scalar-spill provenance lands before cast and CSE census because it is the only
candidate expected to expose opportunities in the other two; this permits
explicit overlap reporting without inflating their direct counts. Casts land
before CSE so conversions have one primary attribution and the CSE key can
exclude them deliberately. Each analyzer remains independently testable and
read-only.

Corpus aggregation waits until all three schemas are stable. Interpretation
waits until the complete corpus is available, and closure waits until the
catalog and discoveries can reflect the actual recommendation. Implementation
of any winning optimization is a separate reviewed project.
