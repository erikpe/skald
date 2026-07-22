# Skald Documentation

Skald documentation is organized by the kind and maturity of the fact being
described. This index is the reader entry point while the focused language,
compiler, and development guides are being created.

## Authority

Use the authority closest to the behavior:

- source-visible language meaning belongs in focused language documents;
- exact accepted syntax belongs in the implemented grammar;
- compiler phases, IR, targets, runtime ABI, driver behavior, and contributor
  workflows belong in their respective implementation or development guides;
- active roadmaps own implementation order and unresolved feature decisions;
- archived roadmaps and Git history explain how the project reached its current
  state, but never define current behavior.

During the documentation overhaul, the existing broad documents remain the
working authorities below. Their content is migrated only after it has been
checked against implementation and tests:

- [Draft language specification](SKALD_DRAFT_SPEC.md) — language design,
  implemented-profile annotations, and open questions;
- [Implemented grammar](../grammar/README.md) — exact source subset accepted by
  the current compiler;
- [Repository structure and compiler architecture](REPO_STRUCTURE.md) — phase,
  backend, runtime, driver, and test boundaries;
- [Future development boundaries](NEXT_SLICE_BOUNDARIES.md) — stable extension
  constraints and planned sequencing;
- [Compiler debugging artifacts](DEBUGGING.md) — dumps, verification points,
  and assembly inspection.

The [migration inventory](roadmaps/DOCUMENTATION_OVERHAUL_INVENTORY.md) maps
every current heading and incoming reference to its intended focused owner.
The [documentation overhaul roadmap](roadmaps/DOCUMENTATION_OVERHAUL_ROADMAP.md)
defines the ordered migration.

## Maturity

Claims about features must use one of these states:

- **implemented contract** — current compiler behavior protected by current
  implementation and tests;
- **frozen design** — settled behavior owned by an active implementation plan;
- **exploratory direction** — non-normative constraints or examples;
- **open question** — a choice that must be resolved before implementation;
- **implementation detail** — compiler, target, runtime, driver, or test
  behavior rather than a language rule;
- **history** — implementation narrative retained only in archives or Git.

If prose disagrees with implementation evidence, do not silently choose a new
language behavior. Correct plainly stale prose, strengthen tests for intended
current guarantees, or record the discrepancy in the
[documentation discoveries backlog](roadmaps/DOCUMENTATION_OVERHAUL_DISCOVERIES.md).

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
