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
