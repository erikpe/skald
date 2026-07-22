# Documentation Overhaul Discoveries

Status: active; two pending documentation cleanups.

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

### Implemented grammar introduction and production cleanup

- **Problem:** `grammar/README.md` still says a “frozen next-slice extension”
  appears near its end even though the class-field, destruction, and
  object-value sections are implemented. Its `class-member` production also
  lists `initializer-declaration` twice.
- **Evidence:** the document introduction and the `Inline classes` production;
  current syntax, phase, and golden tests cover those extensions as implemented.
- **Owner and priority:** focused grammar rewrite; medium documentation
  correctness priority.
- **Boundary:** remove the stale rollout description and duplicate alternative
  while verifying and moving the grammar to its focused authority.

### Duplicate polymorphism roadmap test line

- **Problem:** the resolver class-orchestration task repeats its `Tests:` line,
  which makes the active plan look accidentally edited.
- **Evidence:** the first resolver-maintainability task in
  `POLYMORPHISM_ROADMAP.md` contains adjacent identical lines.
- **Owner and priority:** polymorphism roadmap maintenance; low priority.
- **Boundary:** remove only the duplicate line when that roadmap is next edited;
  do not change the task contract.

## Completed

Move resolved entries here with a concise link or description of the change
that established the authoritative result.
