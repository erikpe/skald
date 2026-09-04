# Local Final-MIR Redundancy Measurement Discoveries

Status: open; one bounded evidence-format follow-up remains from the completed
[local final-MIR redundancy measurement roadmap](../archive/LOCAL_MIR_REDUNDANCY_MEASUREMENT_ROADMAP.md).

The frozen
[measurement contract](../archive/LOCAL_MIR_REDUNDANCY_MEASUREMENT_CONTRACT.md)
owns the method, the
[durable report](../archive/LOCAL_MIR_REDUNDANCY_MEASUREMENT_REPORT.md) owns the
version-one evidence and recommendation, and the
[optimization candidate catalog](OPTIMIZATION_CANDIDATE_CATALOG.md) owns
optimization status and priority. This record contains only the remaining
maintainability limitation in the reusable census boundary.

## Aggregate analyzer APIs do not retain site-level report examples

**Problem:** The three reusable census APIs deliberately return aggregate and
per-callable counts. The corpus report can therefore identify a callable that
contains proven sites, but it cannot publish the frozen schema's optional
block, instruction, value, classification, and reason example without running
a second analysis or weakening the borrowed-checkpoint boundary.

**Evidence:** `ScalarSpillProvenanceObservation`, `PrimitiveCastObservation`,
and `LocalCseObservation` expose totals and callable observations only. Their
implementation-private accumulators see exact sites while scanning, then
discard those identities when producing the stable observation. The reporting
tool retains deterministic proven-callable examples and all aggregate blocker
and consumer detail, which is enough to locate MIR for manual audit but less
precise than a direct site example. The
[version-one study](../archive/LOCAL_MIR_REDUNDANCY_MEASUREMENT_REPORT.md#manual-audit)
therefore required a bounded temporary probe to audit all 26 proven final
sites; the probe was removed after comparison.

**Likely owner:** The shared `passes::redundancy` observation vocabulary and
the three analyzer accumulators, followed by the dedicated
`skald-mir-measure` report-projection module.

**Priority:** Medium before another corpus study or report-schema revision,
but not a prerequisite for an optimization project. It does not affect counts,
candidate safety, checkpoint selection, or the version-one recommendation
because manual review can use callable IDs and deterministic MIR dumps.

**Bounded future direction:** Add one shared, bounded, deterministically sorted
read-only example record containing callable, block, instruction position,
optional value, classification, and ordered reasons. Populate it during each
existing scan, prove that it retains no MIR borrow, cap examples independently
of aggregate counts, and project it without a second semantic table or MIR
traversal. Version or explicitly extend the report schema if its serialized
shape changes.
