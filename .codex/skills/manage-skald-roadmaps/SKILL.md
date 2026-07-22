---
name: manage-skald-roadmaps
description: Create, review, implement, update, and close Skald implementation roadmaps. Use for planning a substantial compiler or language change, dividing work into ordered PR-sized tasks, implementing a named roadmap task, recording discoveries without expanding active scope, updating roadmap progress, or archiving a completed roadmap.
---

# Manage Skald Roadmaps

Build roadmaps that make risky compiler work reviewable, keep them accurate
while implementing them, and leave only current behavior in living
documentation.

## Establish context

1. Read the repository guidance, living architecture, implemented grammar,
   relevant specification sections, active roadmap index, and related tests.
2. Inspect the implementation before proposing boundaries. Base tasks on actual
   ownership, dependencies, invariants, and validation commands.
3. Inspect the sibling Niflheim repository when available. Use its roadmaps,
   specifications, diagnostics, tests, and architecture for inspiration where
   they are clearer, but treat Skald as authoritative and do not copy its
   implementation blindly.
4. Separate current behavior, desired outcome, exclusions, and open design
   decisions. Resolve representation-level decisions before scheduling code
   that depends on them.

## Place roadmap documents

- Store planned and in-progress roadmaps in `docs/roadmaps/`.
- Maintain `docs/roadmaps/README.md` as the concise index of planned and active
  roadmaps. For each entry, state its status, purpose, next task, and material
  dependencies on other roadmaps.
- Store completed roadmaps in `docs/archive/` and list them in
  `docs/archive/README.md`.
- Keep detailed discoveries outside the roadmap being implemented. Put pending
  follow-up work in a clearly named document under `docs/roadmaps/` and index
  it while it remains actionable.
- Update every affected relative link when moving a document.

## Write a roadmap

Use this shape without referring to another roadmap as the template:

```markdown
# <Outcome> Roadmap

Status: planned; <TASK0> is next.

<Why the outcome matters and what durable state it creates.>

## Scope and invariants

- <included outcome or invariant>
- <explicit non-goal>

## Progress

- [ ] <TASK0> — <semantic task name>
- [ ] <TASK1> — <semantic task name>

## PR-sized implementation sequence

### <TASK0> — <semantic task name>

**Purpose:** <one clear responsibility and why it comes now>

- [ ] <implementation result>
- [ ] <documentation or migration result>

**Tests:** <focused tests and repository gates that prove the result>

**Exit criteria:** <observable state required before the next task starts>

## Ordering and dependencies

<Why the order avoids churn and which work may proceed independently.>
```

Apply these rules:

- Use short task codes only inside roadmap and archive documents. Everywhere
  else use semantic feature or behavior names.
- Order tasks by dependency and stable boundary, not by file order. Settle
  contracts before representations, representations before consumers, and
  focused behavior before broad hardening.
- Make every named task fit one reviewable PR with one primary purpose. Split a
  large finding across multiple tasks; combine small findings only when they
  share ownership and validation.
- Give every task an explicit purpose, implementation checklist, focused test
  plan, and objective exit criteria. Include documentation work in the task
  that changes the documented behavior.
- Preserve important behavior and exclusions explicitly: diagnostics, dumps,
  evaluation order, ownership, IDs, ABI, public paths, and deterministic output
  where relevant.
- Prefer maintainability improvements that reduce future change cost: clear
  ownership, concise facades, cohesive modules, explicit invariants, reusable
  test utilities, and narrow dependencies. Avoid abstractions without a
  demonstrated repeated responsibility.
- State the repository quality gates and do not create repository CI when the
  Makefile is the local and external automation interface.

## Implement a roadmap task

1. Read the whole roadmap and the selected task before editing. Confirm its
   dependencies are complete and keep the task's purpose as the scope boundary.
2. Inspect all affected owners, callers, tests, and living documentation.
3. Implement in coherent increments. Refactoring that materially improves
   long-term clarity and maintainability is encouraged when it supports the
   task and preserves behavior.
4. Mark detail checkboxes as their results are actually completed. Mark the
   progress-summary checkbox only after tests and exit criteria pass.
5. Put additional candidates in the roadmap's discoveries document instead of
   expanding the reviewed task. Record the problem, evidence, likely owner,
   priority, and a useful boundary for later work.
6. Keep tests with their owner: implementation-private phase tests colocated,
   public/cross-phase Rust tests in the crate integration-test directory,
   reusable non-Rust corpora under the top-level test tree, and complete
   source-to-observation behavior in golden tests.
7. Run proportionate focused checks during implementation, then the documented
   full repository gate. Run the MSRV target when Rust targets, manifests, or
   supported syntax may be affected.

## Keep documentation current

- Write living documentation as a crisp description of current behavior and
  planned direction, never as a chronological implementation diary.
- Remove roadmap task codes from active test names, comments, grammar notes,
  architecture text, and general documentation. Replace them with semantic
  wording before closing the responsible task or roadmap.
- Preserve milestone vocabulary inside roadmap files and archived roadmaps;
  those documents are historical records.
- Update architecture, grammar, specification, debugging, API, toolchain,
  runtime ABI, and test guidance in the same task that changes their contract.
- Prefer one authoritative location for each fact and link to it rather than
  maintaining duplicated inventories that can drift.

## Close a roadmap

1. Confirm every task checkbox, test plan, and exit criterion is complete.
2. Audit remaining large files and functions by responsibility rather than
   size alone. Resolve high-priority hotspots; place lower-priority findings in
   the indexed discoveries document.
3. Remove roadmap codes and stale rollout language from living code and docs.
   Do not rewrite historical vocabulary in existing archived roadmaps.
4. Run the full repository quality gate from an artifact-free snapshot or clean
   checkout, plus any separate supported-toolchain gates.
5. Set the roadmap status to complete and mark its progress summary complete.
6. Move it from `docs/roadmaps/` to `docs/archive/`, remove it from the active
   index, add it to the archive index, and repair incoming links.
7. Leave pending discoveries under `docs/roadmaps/`; archive or remove a
   discoveries document only when no actionable item remains.
8. Verify formatting, links, repository status, and diff hygiene before handoff.
