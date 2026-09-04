# Local Final-MIR Redundancy Measurement Roadmap

Status: planned; LMR0 is next.

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

Implementation-specific findings outside this reviewed scope belong in the
[local redundancy measurement discoveries](LOCAL_MIR_REDUNDANCY_MEASUREMENT_DISCOVERIES.md).
Candidate status, placement, and prioritization remain authoritative in the
[optimization candidate catalog](OPTIMIZATION_CANDIDATE_CATALOG.md).

## Dependencies

- The completed
  [selectable final-MIR pipeline roadmap](../archive/SELECTABLE_FINAL_MIR_OPTIMIZATION_PIPELINE_ROADMAP.md)
  provides deterministic verified checkpoints, typed pass measurements, and
  exact schedule identities.
- The completed
  [local final-MIR simplification roadmap](../archive/LOCAL_FINAL_MIR_SIMPLIFICATION_ROADMAP.md)
  provides primitive facts, exact constant evaluation, guarded value-use
  classification, CFG cleanup, and the repeated default schedule being
  measured.
- The completed
  [checked integer constant protocol simplification roadmap](../archive/CHECKED_INTEGER_CONSTANT_PROTOCOL_SIMPLIFICATION_ROADMAP.md)
  provides the first concrete scalar-spill provenance limitation and exact
  checked-protocol consumer.
- The completed
  [whole-world reachability roadmap](../archive/TARGET_INDEPENDENT_WHOLE_WORLD_REACHABILITY_ROADMAP.md)
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

- [ ] LMR0 — Freeze the census and corpus contract
- [ ] LMR1 — Compose verified MIR inspection through the driver
- [ ] LMR2 — Measure scalar-spill constant provenance
- [ ] LMR3 — Measure redundant primitive casts
- [ ] LMR4 — Measure local primitive common subexpressions
- [ ] LMR5 — Build deterministic corpus aggregation and reporting
- [ ] LMR6 — Run the study and select the next optimization project
- [ ] LMR7 — Harden the measurement boundary and close the roadmap

## PR-sized implementation sequence

### LMR0 — Freeze the census and corpus contract

**Purpose:** Make every later number interpretable before analysis code can
shape the question it is intended to answer.

- [ ] Specify the input, pre-reachability, and final checkpoint selection by
      semantic schedule position rather than a hard-coded numeric occurrence.
- [ ] Define the canonical schema for compilation identity, schedule identity,
      checkpoint identity, structural totals, per-candidate outcomes, blocker
      classifications, overlaps, and operational context.
- [ ] Define “interesting”, “proven candidate”, “blocked candidate”, and
      “estimated downstream unlock” so reports never present speculative sites
      as safe rewrites.
- [ ] Freeze disjoint primary attribution and overlap rules for FMV-15, FMV-02,
      and FMV-03.
- [ ] Select a versioned corpus covering focused candidate-positive and
      candidate-negative fixtures, optimization goldens, checked protocols,
      cast-heavy primitive code, control flow and lifecycle, standard-library-
      backed programs, generic-vector and range benchmarks, and the available
      larger solver workload.
- [ ] Record how corpus entries carry entry roots, provider roots, standard
      library selection, compiler arguments, stdin, and native arguments
      without duplicating source ownership from the golden suite.
- [ ] Define the final comparison matrix and the evidence threshold for a
      recommendation, including an allowed “none is justified yet” result.

**Tests:** Documentation link validation; schema examples covering zero,
positive, rejected, overlapping, and saturated counts; duplicate compilation
identity examples; schedule-position examples; `git diff --check`.

**Exit criteria:** The measurement units, snapshots, taxonomy, corpus, report
schema, and decision criteria are explicit enough that all three analyzers can
be implemented independently and still produce comparable results.

### LMR1 — Compose verified MIR inspection through the driver

**Purpose:** Give repository tools access to real whole-world final-MIR
checkpoints without adding an optimization pass or parsing dumps.

- [ ] Replace the driver's single-purpose inspected-call plumbing with one
      request-local inspection service that can independently carry optional
      static-activation and MIR-pipeline inspectors.
- [ ] Thread the optional `MirPipelineInspector` through module-graph and
      single-source compilation into the existing inspected pipeline runner.
- [ ] Keep inspectors outside `CompilationRequest`, request equality, report
      events, diagnostics, artifacts, and backend inputs.
- [ ] Preserve the ordinary no-inspector path without census construction,
      dump rendering, filesystem work, or additional MIR traversal.
- [ ] Expose only the minimal facade needed by workspace tools and integration
      tests; remove superseded overloads rather than accumulating parallel
      inspection APIs unless a concrete repository consumer requires a narrow
      compatibility delegate.
- [ ] Document the composable inspection boundary in driver, reporting,
      debugging, and public-API coverage.

**Tests:** No-inspector parity; activation-only, MIR-only, both-inspector, and
failing-compilation callback matrices; exact verified checkpoint order;
borrowed-only compile-fail coverage; request-equality and reporting parity;
public facade integration tests; full compiler tests.

**Exit criteria:** A workspace tool can inspect every verified final-MIR
checkpoint of an ordinary whole-world compilation while the uninspected
compiler path and all observable products remain unchanged.

### LMR2 — Measure scalar-spill constant provenance

**Purpose:** Determine whether short private storage chains are now the main
barrier to constant-driven simplification and whether one narrow analysis would
serve several later passes.

- [ ] Add one read-only seal-local scalar-spill provenance observer over exact
      storage declarations, writes, loads, types, CFG predecessors, and
      dominance.
- [ ] Count direct, one-hop, and transitive canonical constant provenance
      separately; never publish a transitive fact after ambiguity or an
      unsupported boundary.
- [ ] Classify every rejected candidate by multiple or conditional writes,
      nondominance, mismatched type, noncanonical place projection, alias
      exposure, lifecycle participation, protected metadata, malformed
      identity, or unsupported producer.
- [ ] Attribute consumers to checked integer protocols, total primitive
      operations, casts, branches, stores, returns, calls, and other families.
- [ ] Estimate downstream opportunities only by invoking the existing exact
      evaluator or verified protocol observer with read-only virtual facts; do
      not rewrite MIR or duplicate arithmetic/protocol semantics.
- [ ] Keep detailed facts internal and return deterministic aggregate and
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

### LMR3 — Measure redundant primitive casts

**Purpose:** Separate safely removable conversion churn from casts that encode
canonicalization, range, checked failure, or representation semantics.

- [ ] Inventory primitive cast assignments by exact cast kind, source/result
      type, checkpoint, callable, and consumer family.
- [ ] Recognize exact identity casts and adjacent block-local chains whose
      composed conversion is proven equal for the complete source value domain.
- [ ] Use one explicit value-domain/cast-composition table owned by the census;
      reuse implemented primitive semantics and classify unsupported pairs
      rather than inferring from equal host widths.
- [ ] Separate raw-bit reinterpretation, integer narrowing/widening, boolean
      canonicalization, integer/floating conversion, and checked
      floating-to-integer protocols.
- [ ] Classify blockers including nonadjacent provenance, multiple uses,
      protected use roles, control-flow boundaries, lost-range knowledge,
      checked failure, floating payload concerns, and unsupported composition.
- [ ] Record whether repeated casts appear to originate in lowering, source
      code, generic specialization, or earlier final-MIR rewriting when that
      distinction follows from deterministic checkpoint evidence.

**Tests:** Complete primitive type/kind matrix; identity and safe chain cases;
narrow/widen counterexamples; boolean canonicalization; raw-bit and NaN
barriers; checked conversion diamonds; cross-block and protected uses;
unchanged MIR; deterministic classification.

**Exit criteria:** The report can distinguish definite redundant casts from
interesting but proof-dependent conversions and identify whether the likely
owner is lowering canonicalization or a final-MIR pass.

### LMR4 — Measure local primitive common subexpressions

**Purpose:** Establish whether exact same-block scalar repetition is common
enough to justify CSE before investing in global value numbering or SSA.

- [ ] Build a deterministic same-block expression key from exact operation,
      result type, and ordered operand identities for the frozen non-failing
      integer/boolean family.
- [ ] Treat commutative reordering, reassociation, constants, casts, loads,
      floating operations, checked protocols, calls, ownership, and semantic
      queries as separate or excluded observations rather than silently
      broadening equivalence.
- [ ] Require an earlier same-block definition and classify every later exact
      repeat by replacement-safe use roles, protected roles, dead result,
      source-observation concern, or malformed identity.
- [ ] Count repeated definitions, replaceable uses, potentially removable
      values/instructions, affected callables, maximum repetitions per key, and
      operation-family distribution.
- [ ] Record overlap where scalar-spill provenance would make operand identity
      or constant equivalence visible, while keeping the direct CSE count based
      only on current MIR identities.
- [ ] Keep expression facts block-local and reset them at every CFG boundary;
      do not introduce global value numbering, memory numbering, or persistent
      optimization facts.

**Tests:** Exact repeats and near misses; operand order; result type and width;
dominance by instruction order; multiple consumers; protected uses; dead
results; block boundaries and loops; excluded families; deterministic maxima
and aggregate order; unchanged MIR.

**Exit criteria:** The census identifies the reachable direct ceiling for a
conservative same-block CSE pass and shows whether broader CFG or storage facts,
rather than expression matching, dominate the missed opportunities.

### LMR5 — Build deterministic corpus aggregation and reporting

**Purpose:** Turn the three analyzers into a repeatable repository study rather
than a collection of synthetic unit-test counts.

- [ ] Add an opt-in repository measurement tool with human and canonical JSON
      output; do not add a `skac` flag or registered MIR pass.
- [ ] Parse and validate the frozen corpus configuration before compilation,
      canonicalize repository-relative identities, and reject duplicate or
      escaping paths deterministically.
- [ ] Compile each configured root through the real driver and current default
      schedule, collecting the three semantic checkpoints through the composed
      inspection service.
- [ ] Aggregate with saturating integer counts while preserving per-workload,
      per-checkpoint, per-candidate, blocker, consumer, and overlap detail.
- [ ] Record compiler revision or dirty-state context, resolved schedule,
      target, runtime-trace policy, corpus version, assembly/executable size
      where produced, and explicitly nondeterministic elapsed-time context.
- [ ] Keep generated outputs under ignored `build/measurements/`; make the
      canonical report reproducible without checking binaries or dumps into
      source control.
- [ ] Add a focused Makefile target and usage documentation without adding the
      measurement to ordinary correctness gates.

**Tests:** Manifest validation and path containment; duplicate identities;
empty and partial corpora; compiler failure attribution; canonical JSON key and
array order; human/JSON semantic parity; saturating aggregation; independent-
process deterministic output after removing operational fields; focused
end-to-end repository corpus smoke test.

**Exit criteria:** One documented command reproducibly measures all three
candidate families across the reviewed corpus and produces stable machine-
readable evidence without changing ordinary compiler behavior.

### LMR6 — Run the study and select the next optimization project

**Purpose:** Convert measurements into an explicit architectural decision
rather than choosing the largest unqualified raw count.

- [ ] Run the complete corpus from an artifact-free state and retain the
      canonical result together with compiler revision, schedule, target, and
      corpus identity.
- [ ] Review outliers manually against MIR dumps and source so every material
      counter represents the classified shape rather than an analyzer bug or
      duplicated compilation root.
- [ ] Compare raw and reachable opportunities, workload breadth, removable
      structural ceiling, downstream unlocks, overlaps, blocker distributions,
      likely creation phase, implementation effort, semantic risk, and reusable
      analysis value.
- [ ] For existing benchmark workloads, record baseline compile time, assembly
      and executable size, and native time as context without claiming an
      unimplemented delta.
- [ ] Publish a durable measurement report with per-workload data, totals,
      limitations, rejected interpretations, and the evidence supporting its
      recommendation.
- [ ] Select exactly one next action: a candidate-specific design/roadmap, a
      bounded lowering cleanup, a broader prerequisite investigation, more
      representative workload collection, or no optimization yet.
- [ ] Update FMV-15, FMV-02, FMV-03 and the suggested evaluation order in the
      optimization candidate catalog; update related discoveries without
      duplicating the report.

**Tests:** Canonical report regeneration; independent-process structural
determinism; hand-audited sample agreement; baseline native equivalence across
measurement on/off paths; documentation links; no wall-clock assertions.

**Exit criteria:** The repository contains reproducible evidence explaining
what local redundancy remains, which boundary creates or blocks it, and why one
specific next action is preferred—or why the current evidence supports none.

### LMR7 — Harden the measurement boundary and close the roadmap

**Purpose:** Leave a maintainable analysis surface, authoritative evidence,
and no permanent investigative debris.

- [ ] Audit checkpoint plumbing, shared census vocabulary, each candidate
      analyzer, corpus ownership, aggregation, rendering, and tests by
      responsibility; split only genuine mixed owners and keep facades concise.
- [ ] Remove temporary probes, duplicate semantic tables, debugging output,
      hard-coded schedule positions, absolute paths, roadmap codes, and unused
      compatibility helpers from production code and living documentation.
- [ ] Confirm the tool remains read-only, opt-in, deterministic, target-
      independent in its MIR classifications, and absent from ordinary
      compilation cost and pass registration.
- [ ] Retain reusable candidate facts only where a selected future pass has a
      concrete owner; otherwise keep the stable aggregate census or remove the
      unused internal detail.
- [ ] Reconcile the report, discoveries, catalog statuses, candidate links,
      and selected next action.
- [ ] Run the complete repository validation from an artifact-free snapshot,
      supported MSRV, independent-process determinism, the full measurement
      corpus, and documentation/diff hygiene checks.
- [ ] Mark every task complete, archive the roadmap and durable measurement
      report, update active/archive indexes, and repair all incoming links.
- [ ] Archive or remove the discoveries record only if no actionable finding
      remains; otherwise keep it indexed under `docs/roadmaps/`.

**Tests:** `make check`; `make check-long`; `make msrv-check`; focused
measurement determinism and corpus regeneration; `make docs-check`; `git diff
--check`; repository-status review; and manual comparison of the documented
schema and recommendation against generated evidence.

**Exit criteria:** The study is repeatable, its recommendation is evidence-
backed, ordinary compilation is unchanged, no temporary measurement mechanism
remains, completed records are archived, and every surviving discovery has a
clear future owner.

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
