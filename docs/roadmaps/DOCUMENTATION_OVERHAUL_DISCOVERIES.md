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

### Legacy draft retains duplicated target ABI details

**Problem:** The legacy draft's inline-object layout and receiver-ABI section
still repeats primitive sizes, class layout, target errors, register classes,
and hidden-receiver placement after those claims gained one authoritative home.
Keeping both copies increases drift risk and leaves the migration document
looking normative.

**Evidence:** `docs/SKALD_DRAFT_SPEC.md` section 13.4 contains the detailed
target contract, while `docs/compiler/BACKEND.md` already owns and verifies the
same layout and calling-convention facts.

**Owner and priority:** Documentation overhaul cleanup; low priority because
the legacy monolith is already scheduled for removal and the focused backend
document is explicitly authoritative.

**Boundary:** During removal of superseded monoliths, ensure all incoming links
target `docs/compiler/BACKEND.md`, then remove the duplicated legacy section
with the rest of the draft rather than migrating any of its prose again.

## Completed

### Duplicate polymorphism roadmap test line

The reported adjacent duplicate was no longer present when DOC8 re-audited the
active roadmap against its new focused design authority. The stale backlog
entry was closed without changing the affected task contract.

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
