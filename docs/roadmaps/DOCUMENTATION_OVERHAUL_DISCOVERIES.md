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

### Alias and ownership maturity drift

The legacy draft mixed implemented inline alias parameters with unimplemented
shared sources, mandatory caller anchors, optional and array aliases, and
reference-count/runtime-layout algorithms. DOC7 retained only the verified
exact-class parameter contract and reduced every broader ownership form to its
actual exploratory or open maturity.

### Legacy lifecycle profile drift

The old local-destruction profile prohibited object copies, assignment, and
fresh-object temporaries in destructor bodies even after the implemented
object-value profile and its regression coverage allowed them. DOC6 replaced
the layered profiles with the verified current destructor-body and lifetime
contract instead of preserving the obsolete restriction.

### Legacy draft authority correction

The legacy draft now identifies itself as migration input and defers to each
established focused authority. DOC16 will remove it after the remaining
language areas migrate.

### Implemented grammar source cleanup

The focused implemented grammar replaced the stale rollout introduction,
removed the duplicated initializer alternative, and separated syntax from
semantic and backend narration. The old grammar path is now only a compatibility
link.
