# Active and Planned Roadmaps

This directory contains implementation roadmaps that are planned or in
progress. Completed roadmaps and resolved discovery records move to
[`../archive/`](../archive/README.md).

## In progress

No implementation roadmaps are currently in progress.

## Planned

No implementation roadmaps are currently planned.

## Design proposals

- [While Loops Design Proposal](WHILE_LOOPS_DESIGN_PROPOSAL.md) — **confirmed;
  contract promotion complete, implementation roadmap pending**. Defines the
  source semantics, control effects,
  cleanup boundaries, repeatable MIR storage lifetimes, generic CFG lowering,
  and optimization invariants for `while` and future `break` and `continue`.
  Decisions W1 through W13 adopt their recommended choices, and living language
  and compiler documents now own the frozen source, representation, runtime,
  and backend boundaries. The next action is creating the PR-sized
  implementation roadmap.

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
