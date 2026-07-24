# Active and Planned Roadmaps

This directory contains implementation roadmaps that are planned or in
progress. Completed roadmaps and resolved discovery records move to
[`../archive/`](../archive/README.md).

- [Shared Ownership and Heap Allocation](SHARED_OWNERSHIP_ROADMAP.md) —
  **in progress, SO6 next**; implements non-null strong owners,
  explicit ordinary and copy allocation, deterministic last-owner destruction,
  shared polymorphism, and hidden borrow anchors. Its constructor-semantics and
  object-cast prerequisites are complete; source syntax, resolved identities,
  typed owner vocabulary, exact-class local initialization and assignment in
  verified MIR, native allocation/copy/release execution, and the minimal
  allocation runtime ABI are implemented. The next task carries shared owners
  across calls and results.

The completed polymorphism, object-cast, and constructor profiles remain the
implementation baseline. Constructor overload and explicit-copy semantics are
specified in [Classes and Lifecycle](../language/CLASSES_AND_LIFECYCLE.md).
Shared-ownership language and implementation contracts are frozen in
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
