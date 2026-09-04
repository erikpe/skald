# Local Final-MIR Redundancy Measurement Discoveries

Status: open companion record for the active
[local final-MIR redundancy measurement roadmap](LOCAL_MIR_REDUNDANCY_MEASUREMENT_ROADMAP.md).

Use this file for concrete maintainability findings, measurement limitations,
or optimization opportunities discovered while implementing the roadmap that
do not belong in its reviewed census scope. Each finding should record the
problem, evidence, likely owner, priority, and a bounded future direction.

Do not place the study's ordinary results here. The frozen
[measurement contract](LOCAL_MIR_REDUNDANCY_MEASUREMENT_CONTRACT.md) owns its
method, the future durable measurement report owns data and interpretation,
and the [optimization candidate catalog](OPTIMIZATION_CANDIDATE_CATALOG.md)
owns concise cross-domain status, placement, effort, value, and priority.
Recording a finding here does not authorize implementation of FMV-15, FMV-02,
FMV-03, or another optimization.

## Aggregate analyzer APIs do not retain site-level report examples

**Problem:** The three reusable census APIs deliberately return aggregate and
per-callable counts. The corpus report can therefore identify a callable that
contains proven sites, but it cannot publish the frozen schema's optional
block, instruction, value, classification, and reason example without running
a second analysis or weakening the borrowed-checkpoint boundary.

**Evidence:** `ScalarSpillProvenanceObservation`, `PrimitiveCastObservation`,
and `LocalCseObservation` expose totals and callable observations only. Their
implementation-private accumulators see exact sites while scanning, then
discard those identities when producing the stable observation. LMR5 retains
deterministic proven-callable examples and all aggregate blocker/consumer
detail, which is enough to locate MIR for manual audit but less precise than a
direct site example.

**Likely owner:** The shared `passes::redundancy` observation vocabulary and
the three analyzer accumulators, followed by the `skald-mir-measure` report
projection.

**Priority:** Medium before the evidence format is declared permanently
closed. It does not affect counts, candidate safety, checkpoint selection, or
the LMR6 recommendation threshold because manual review can use callable IDs
and deterministic MIR dumps.

**Bounded future direction:** Add one shared, bounded, deterministically sorted
read-only example record containing callable, block, instruction position,
optional value, classification, and ordered reasons. Populate it during the
existing scan, prove that it retains no MIR borrow, cap examples independently
of aggregate counts, and project it without a second semantic table or MIR
traversal.
