# Active and Planned Roadmaps

This directory contains implementation roadmaps that are planned or in
progress. Completed roadmaps and resolved discovery records move to
[`../archive/`](../archive/README.md).

## In progress

No implementation roadmaps are currently in progress.

## Planned

- [While Loops and Loop Exits Roadmap](WHILE_LOOPS_ROADMAP.md) — **planned;
  repeatable MIR storage lifetime epochs are next**. Delivers cycle-safe
  storage and verification foundations before activating source `while`, then
  adds `break` and `continue` as separate slices. It depends on the current
  deterministic-cleanup, ownership, optional, array, generic-CFG, pass, and
  x86-64 backend baselines; it requires no runtime ABI change.

## Design proposals

- [While Loops Design Proposal](WHILE_LOOPS_DESIGN_PROPOSAL.md) — **confirmed;
  contract promotion and implementation roadmapping complete, final validation
  and archival pending**. Defines the source semantics, control effects,
  cleanup boundaries, repeatable MIR storage lifetimes, generic CFG lowering,
  and optimization invariants for `while` and future `break` and `continue`.
  Decisions W1 through W13 adopt their recommended choices, and living language
  and compiler documents now own the frozen source, representation, runtime,
  and backend boundaries. The next action is final documentation validation and
  archival of the proposal.

## Pending discoveries

No maintainability discoveries are currently pending.

## Implementation baseline

The completed initial module system, polymorphism, object-cast, and constructor
profiles remain the implementation baseline. Module behavior is specified by
the implemented
[language contract](../language/MODULES_AND_INTEROP.md#initial-module-system)
and [compiler contract](../compiler/MODULE_SYSTEM.md). Constructor overload
and explicit-copy semantics are specified in
[Classes and Lifecycle](../language/CLASSES_AND_LIFECYCLE.md).
Shared-ownership language and implementation contracts are current in
[Shared Ownership and Heap Allocation](../language/SHARED_OWNERSHIP.md) and
the
[Shared-Ownership Compiler and Runtime Contract](../compiler/SHARED_OWNERSHIP.md),
including ordinary allocation and explicit exact-class copy allocation from a
target-directed checked source. Dynamic-type-preserving cloning remains
deferred.
Checked exceptions remain the later exploratory direction because they extend
cleanup to exceptional control flow. Other unscheduled language directions
and their maturity are owned by the
[status matrix](../language/STATUS.md#not-implemented).
