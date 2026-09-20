# Complete Low-Level Lowering Migration Discoveries

Status: resolved and archived. Compact placement state closed both findings on
2026-09-20 without weakening semantic expansion.

Both findings were implemented through the frozen
[Placement Checking Scalability Design Proposal](PLACEMENT_CHECKING_SCALABILITY_DESIGN_PROPOSAL.md)
and its [completed roadmap](PLACEMENT_CHECKING_SCALABILITY_ROADMAP.md). The
[retained measurements](../development/PLACEMENT_CHECKING_PERFORMANCE.md) keep
them as separate acceptance witnesses for future regressions.

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
The promoted design keeps this finding as the recursive-optional performance
witness.

**Resolution:** the compact finite-bitset state reduced the witness median from
46.59 seconds to 2.416 seconds (19.3x) and peak RSS from 117.9 MiB to 48.1 MiB.
The ordinary native witness remains enabled, and LA05 can use the retained
measurement command without carrying a blocking placement-cost qualification.

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
implementation and LA05 owns the broader measured adoption decision. The
promoted design keeps this finding as the live array-element performance
witness.

**Resolution:** all five retained D02 witnesses improved by 45.5x–119.9x. The
former worst witness now takes 1.630 seconds and 51.3 MiB rather than 194.85
seconds and 891.8 MiB. All remain enabled ordinary native tests; LA05 receives
the scalable baseline checker and the reproducible regression protocol.
