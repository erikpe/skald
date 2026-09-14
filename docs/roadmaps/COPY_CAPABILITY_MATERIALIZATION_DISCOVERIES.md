# Copy-Capability Materialization Discoveries

Status: one deferred candidate remains after the measured A17 no-go decision.

The archived
[copy-capability materialization experiment](../archive/COPY_CAPABILITY_MATERIALIZATION_ROADMAP.md)
restored the original type-check solver because the one-pass replacement failed
its operational retention gate. This record keeps the only narrower follow-up
outside the completed A17 scope.

## Borrow provisional capability views

**Status:** Deferred; require fresh measurements before promotion.

**Problem:** Type-check convergence clones both complete class capability sets
before rebuilding each provisional HIR array table. The nine-class structural
fixture measured 72 cloned capability records across four provisional views.

**Evidence:** The
[measurement record](../development/COPY_CAPABILITY_MATERIALIZATION_MEASUREMENTS.md)
contains the exact structural counts. The rejected one-pass experiment does not
show that these clones alone cause a material compiler-time or memory cost.

**Likely owner:** `typeck::capabilities`, with the provisional input contract in
`typeck::arrays::capabilities`.

**Priority:** P3 until a representative type-check workload attributes material
cost to provisional capability-set cloning.

**Bounded implementation:** Replace owned provisional `CopyCapabilities`
snapshots with a borrowed view over constructor and assignment sets while
retaining the existing type-check availability solver, convergence order,
failure-path ownership, array reconstruction, and final HIR publication.

**Stop condition:** Do not proceed when borrowing widens visibility, complicates
lifetimes across type-check responsibilities, changes diagnostics or products,
or lacks a repeatable compile-time or peak-memory benefit. Do not reintroduce
neutral-authority wiring or the rejected one-pass materializer under this
candidate.
