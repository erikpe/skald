---
name: manage-skald-roadmaps
description: Create, review, implement, update, and close Skald implementation roadmaps. Use for planning a substantial compiler or language change, dividing work into ordered PR-sized tasks, implementing a named roadmap task, recording discoveries without expanding active scope, updating roadmap progress, or archiving a completed roadmap.
---

# Manage Skald Roadmaps

Build roadmaps that make risky compiler work reviewable, keep them accurate
while implementing them, and leave only current behavior in living
documentation.

## Establish context

1. Inspect repository status and history. The user normally commits locally
   between PR-sized tasks; a clean working tree says nothing about whether
   earlier scaffolding still needs removal.
2. Record the roadmap baseline: the last commit before implementation began.
   Recover it from history for an existing roadmap, and distinguish unrelated
   intervening changes from roadmap work. Use a task-specific baseline as well
   when following behavior introduced or replaced by an earlier task.
3. Read the repository guidance, relevant living architecture, grammar,
   relevant specification sections, active roadmap index, and related tests.
4. Inspect the implementation before proposing boundaries. Base tasks on actual
   ownership, dependencies, invariants, and validation commands.
5. Inspect the sibling Niflheim repository when available. Use its roadmaps,
   specifications, diagnostics, tests, and architecture for inspiration where
   they are clearer, but treat Skald as authoritative and do not copy its
   implementation blindly.
6. Separate current behavior, desired outcome, exclusions, and open design
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
Implementation baseline: <commit hash; record before the first code change>.

<Why the outcome matters and what durable state it creates.>

## Scope and invariants

- <included outcome or invariant>
- <explicit non-goal>

## Progress

- [ ] <TASK0> — <semantic task name>
- [ ] <TASK1> — <semantic task name>
- [ ] <FINAL> — Cumulative review, cleanup, and closure

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
- Give every task a purpose, implementation checklist, test plan, and objective
  exit criteria. Include documentation with the behavior change. Make the final
  task the cumulative review described under "Close a roadmap"; include its
  cleanup and validation work before archival.
- When introducing temporary code, keep a compact artifact ledger in the
  roadmap: file/symbol, introducing task and commit when available, intended
  removal task, and final disposition. Cover bridges, aliases, gates, lint
  allowances, exploratory code, and instrumentation. Any retained artifact
  needs a continuing purpose and an explicit retention criterion.
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

1. Establish context as above, read the whole roadmap, and verify the selected
   task's prerequisites against their commits and current source.
2. Inspect affected owners, callers, tests, documentation, and the artifact
   ledger. Remove transitions due in this task even when already committed.
3. Implement in coherent increments, including small maintainability fixes that
   support the task. Update the ledger as artifacts are introduced or retired.
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

On an experimental no-go, identify the last accepted implementation commit and
selectively restore rejected code and scaffolding from the appropriate boundary.
Retain independently useful tests, measurements, guards, and decision records;
record their disposition in the ledger. Preserve unrelated work and local commit
history rather than resetting the whole tree.

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

Review the complete implementation before marking it complete or archiving it.

1. Review `git diff <baseline>..HEAD` (including its `--stat` and `--name-status`
   views), then include staged, unstaged, and untracked work in the review. Read
   the resulting code as one change, checking the accepted design, ownership,
   interfaces, invariants, and behavior across task boundaries.
2. Reconcile every ledger entry with current source and history; use symbol
   searches and `git log -S` when needed. Check beyond the ledger for duplicate
   implementations, leftover adapters, unnecessary exports, allowances, and
   stale names, comments, or diagnostics. Verify retained test or measurement
   machinery has a continuing purpose and appropriate visibility or test gates.
3. Make small cleanup and consistency fixes in the closing change. Resolve
   substantial design or correctness gaps in an explicit implementation task
   before closure; record independent improvements in indexed discoveries.
4. Verify tests exercise the final interfaces and documentation describes the
   final behavior. Reconcile the design, roadmap, audit entries, and indexes;
   remove task codes and rollout language from living code and documentation.
5. After fixups, run the full repository quality gate from an artifact-free
   snapshot or clean checkout, plus required supported-toolchain gates. Confirm
   all task checkboxes and exit criteria, including this review, are satisfied.
6. Record the reviewed baseline and endpoint, residual changes awaiting commit,
   artifact dispositions, and validation results concisely in the roadmap.
   Mark it complete, move it to `docs/archive/`, update both indexes, and repair
   links. Keep discoveries active while actionable work remains.
7. Check the final cumulative and closing diffs, formatting, links, and status.
   Prepare a reviewable closing change; leave committing to the user unless
   explicitly requested.
