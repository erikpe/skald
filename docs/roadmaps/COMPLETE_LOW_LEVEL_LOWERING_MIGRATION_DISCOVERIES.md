# Complete Low-Level Lowering Migration Discoveries

Items here are intentionally outside the active PR-sized task that exposed
them. They should be assessed after the roadmap unless a later task already
owns the required machinery.

## D01 — Branch-heavy recursive optionals amplify baseline placement cost

LM11's focused two-layer nested optional copy executes correctly, and combined
recursive lowering closes the checked worklist. Combining several nested-copy,
inline-class, field-finalization and guard paths in one callable creates a much
larger LIR CFG and makes the allocation-independent baseline placement pass
noticeably slow. Keep lifecycle expansion structurally explicit; after LM14 has
completed recursive container CFGs, measure representative combined graphs and
consider a linear-time placement/worklist improvement or safe CFG compaction.
Do not flatten optional state or add an opaque lifecycle opcode to address this.
Priority is medium; the placement pipeline owns the follow-up, with LM14 as the
earliest useful measurement boundary.

## D02 — Live array element loads amplify baseline placement cost

LM12's checked LIR tests cover positive and negative element loads and stores,
while its native smoke test keeps the addressed stores and observes the planned
length. Returning or branching on a loaded element made the private physical
pipeline exceed 120 seconds for a three-element primitive array under the
allocation-independent baseline placer. This is a compile-time placement cost,
not a reason to weaken checked array addressing or add a target opcode. Measure
it together with D01 after LM14 has completed array aliases and slices; then
improve the placement worklist or compact equivalent CFG where the evidence
points. Priority is medium and the placement pipeline owns the follow-up.

## D03 — Completed place admission leaves a no-op staging seam

LM14 removed the last place form rejected by the low-level admission pass:
array aliases. The shared `place` admission helper is therefore now a no-op,
although earlier instruction families still call it. Removing that seam touches
the full admission matrix and is better handled during the roadmap's final
cumulative cleanup, once LM15 and LM16 have removed the remaining staged
rejections. Priority is low; the admission boundary owns the follow-up.
