# Active and Planned Roadmaps

This directory contains implementation roadmaps that are planned or in
progress. Completed roadmaps and resolved discovery records move to
[`../archive/`](../archive/README.md).

## In progress

No implementation roadmaps are currently in progress.

## Planned

- [Private Members and Static Methods Roadmap](PRIVATE_AND_STATIC_MEMBERS_ROADMAP.md)
  — **planned; receiver-model groundwork is next**. Implements exact
  declaring-class privacy, public and private receiverless static methods,
  verified receiver presence, complete x86-64 execution, and the documentation
  confirmation that clears the string-design freeze gate. It builds on the
  completed module, polymorphism, ownership, optional-value, and array
  contracts. Source-visible static fields remain separate future work.

## Design proposals

- [String Types Design Proposal](STRINGS_DESIGN_PROPOSAL.md) — **proposed
  design complete**. Defines raw-byte `std::str::Str` values, compiler-emitted
  immortal shared-array literal backing, the compiler/standard-library
  boundary, and the freeze criteria. Next action: complete the
  [private-members and static-methods roadmap](PRIVATE_AND_STATIC_MEMBERS_ROADMAP.md),
  whose final task confirms and promotes this proposal to frozen language and
  compiler contracts. No other feature is a freeze dependency.

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
