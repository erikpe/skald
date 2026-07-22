# Documentation Overhaul Discoveries

Status: active; one pending documentation cleanup.

This backlog owns contradictions, behavior defects, unresolved choices, and
unrelated cleanup found while the documentation overhaul is being implemented.
It keeps reviewed migration tasks focused without losing evidence.

## Recording a discovery

Each entry must state:

- the problem and why it matters;
- concrete documentation, implementation, or test evidence;
- the likely semantic or implementation owner;
- priority and the smallest useful follow-up boundary.

Documentation-only migration must not silently change compiler behavior. A
discovery that belongs to another active roadmap should link to that roadmap;
one that requires a new behavior decision remains here until it has an owner.

## Pending

### Duplicate polymorphism roadmap test line

- **Problem:** the resolver class-orchestration task repeats its `Tests:` line,
  which makes the active plan look accidentally edited.
- **Evidence:** the first resolver-maintainability task in
  `POLYMORPHISM_ROADMAP.md` contains adjacent identical lines.
- **Owner and priority:** polymorphism roadmap maintenance; low priority.
- **Boundary:** remove only the duplicate line when that roadmap is next edited;
  do not change the task contract.

## Completed

### Implemented grammar source cleanup

The focused implemented grammar replaced the stale rollout introduction,
removed the duplicated initializer alternative, and separated syntax from
semantic and backend narration. The old grammar path is now only a compatibility
link.
