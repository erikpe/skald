# Dense MIR Identity Rewriting Discoveries

Status: active implementation follow-up record; no deferred findings are
currently recorded.

This document owns maintainability and architectural findings discovered while
implementing the
[dense callable-local MIR identity rewriting roadmap](DENSE_MIR_IDENTITY_REWRITING_ROADMAP.md)
that are valuable but too large or independent to add safely to the active
roadmap task.

Small cohesive improvements in code already touched by a task should be fixed
directly. A larger finding belongs here with:

- the problem and concrete implementation evidence;
- its likely semantic or module owner;
- impact and priority;
- why it is outside the active task; and
- a bounded follow-up that can become a separate roadmap task or proposal.

The record must not duplicate the broader
[optimization architecture discoveries](OPTIMIZATION_ARCHITECTURE_DISCOVERIES.md).
Proof-provenance normalization, SSA, alias/effect analysis, a general pass
registry, and backend virtual registers remain owned there unless
identity-rewriting implementation reveals a narrower actionable defect.

## Deferred findings

No findings are currently recorded.
