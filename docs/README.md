# Skald Documentation

Skald documentation is organized by the kind and maturity of the fact being
described. This index is the reader entry point for the focused language,
compiler, and development guides.

## Authority

Use the authority closest to the behavior:

- broad source-visible language meaning begins in the
  [language overview](language/README.md), while detailed rules belong in its
  focused language documents;
- current support and design maturity belong only in the
  [language status matrix](language/STATUS.md);
- exact accepted syntax belongs in the
  [implemented grammar](language/GRAMMAR.md);
- type, value, literal, and expression semantics belong in
  [types and values](language/TYPES_AND_VALUES.md);
- callable, binding, statement, return, and evaluation-order semantics belong
  in [functions and control flow](language/FUNCTIONS_AND_CONTROL_FLOW.md);
- exact classes, inline containment, receivers, initialization, and object
  places belong in [classes and lifecycle](language/CLASSES_AND_LIFECYCLE.md);
- call-scoped alias access, lifetime, and future ownership boundaries belong in
  [aliases and ownership](language/ALIASES_AND_OWNERSHIP.md);
- implemented static inheritance and frozen dynamic polymorphism, interfaces,
  type tests, and narrowing belong in
  [polymorphism](language/POLYMORPHISM.md);
- the current compilation unit, top-level namespace, entry point, external
  declarations, and future module boundary belong in
  [modules and foreign interoperation](language/MODULES_AND_INTEROP.md);
- compile-time rejection, current runtime-failure boundaries, and future
  exceptional cleanup belong in
  [errors and exceptional control flow](language/ERRORS.md);
- durable compiler structure, repository roles, crate API policy, and extension
  policy belong in the [compiler architecture](compiler/README.md);
- phase products, IR responsibilities, verification, dumps, and trust
  boundaries belong in [compiler phases and IR](compiler/PHASES_AND_IR.md);
- target legality, layout, ABI realization, and code generation belong in the
  [backend and target contract](compiler/BACKEND.md);
- the public C runtime surface and compiler/runtime compatibility mechanism
  belong in the [runtime ABI](compiler/RUNTIME_ABI.md);
- compiler orchestration, CLI behavior, tool invocation, and artifact
  publication belong in [driver and artifacts](compiler/DRIVER_AND_ARTIFACTS.md);
- contributor prerequisites and validation belong in the
  [development workflow](development/README.md);
- test ownership, placement, fixtures, determinism, and robustness belong in
  [testing](development/TESTING.md);
- phase inspection, dump use, verifier boundaries, and assembly debugging
  belong in [debugging the compiler](development/DEBUGGING.md);
- active roadmaps own implementation order and unresolved feature decisions;
- archived roadmaps and Git history explain how the project reached its current
  state, but never define current behavior.

The archived [migration inventory](archive/DOCUMENTATION_OVERHAUL_INVENTORY.md) maps
the removed legacy headings and references to their focused owners.
The [documentation overhaul roadmap](archive/DOCUMENTATION_OVERHAUL_ROADMAP.md)
records the completed migration.

## Maturity

The [language status matrix](language/STATUS.md) defines the maturity labels
and is the sole feature-support inventory. Other documents state their own
authority and maturity, then link to the matrix rather than repeating it.

If prose disagrees with implementation evidence, do not silently choose a new
language behavior. Correct plainly stale prose, strengthen tests for intended
current guarantees, or record the discrepancy in the relevant active roadmap
or a clearly named, indexed discovery backlog under `docs/roadmaps/`.

## Linking and maintenance

- Keep one authoritative statement for each fact. Summaries stay short and
  link to that statement.
- Use repository-relative Markdown links and valid local heading anchors.
- Link to semantic document names, not private implementation files, when the
  semantic contract is the subject.
- Update general documentation in the same change as the behavior or workflow
  it describes. Keep it crisp; do not preserve rollout diaries in living docs.
- Repair links in archived roadmaps when authorities move, but do not rewrite
  their historical task descriptions or milestone vocabulary.

Planned and active work, including dependencies, is listed in the
[roadmap index](roadmaps/README.md). Completed plans are listed in the
[archive index](archive/README.md).

## Verification

Run `make docs-check` for repository-local Markdown files, local anchors, and
required documentation, roadmap, and archive index entries. It is included in
`make check`.

Existing external infrastructure regularly runs `make check` from clean
checkouts, so it picks up documentation validation through the same local
Makefile interface. The repository does not duplicate that infrastructure with
a CI configuration.
