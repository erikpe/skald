# Complete Low-Level Lowering Migration Discoveries

Items here are intentionally outside the active PR-sized task that exposed
them. LM18 confirmed both on the complete private pipeline; cumulative review
should bound and assign them without weakening semantic expansion.

## D01 — Branch-heavy recursive optionals amplify baseline placement cost

LM11's focused two-layer nested optional copy executes correctly, and combined
recursive lowering closes the checked worklist. Combining several nested-copy,
inline-class, field-finalization and guard paths in one callable creates a much
larger LIR CFG and makes the allocation-independent baseline placement pass
noticeably slow. Keep lifecycle expansion structurally explicit; after LM14 has
completed recursive container CFGs, measure representative combined graphs and
consider a linear-time placement/worklist improvement or safe CFG compaction.
Do not flatten optional state or add an opaque lifecycle opcode to address this.
LM18's complete private pipeline suite confirms that recursive lifecycle cases
are among the dominant long-running tests. LM19 transfers this as a blocking
LA05 acceptance input: the adoption design must define a representative bound,
measure it on the production-selectable path, and either improve the placement
worklist/CFG cost or explicitly accept the measured limit before enabling the
new backend by default. Priority is high; placement owns the implementation.

## D02 — Live array element loads amplify baseline placement cost

LM12's checked LIR tests cover positive and negative element loads and stores,
while its native smoke test keeps the addressed stores and observes the planned
length. Returning or branching on a loaded element made the private physical
pipeline exceed 120 seconds for a three-element primitive array under the
allocation-independent baseline placer. This is a compile-time placement cost,
not a reason to weaken checked array addressing or add a target opcode. Measure
it together with D01; then improve the placement worklist or compact equivalent
CFG where the evidence points. LM18's full private suite again found the array
lifecycle cases to dominate runtime. LM19 transfers this as the same blocking
LA05 acceptance input as D01, with a separate loaded-element witness so a fix
cannot hide only recursive-optionals cost. Priority is high; placement owns the
implementation and LA05 owns the measured adoption decision.
