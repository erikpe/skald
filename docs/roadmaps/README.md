# Active and Planned Roadmaps

This directory contains implementation roadmaps that are planned or in
progress. Completed roadmaps and resolved discovery records move to
[`../archive/`](../archive/README.md).

- [Constructor Overloads and Explicit Copy Construction](CONSTRUCTOR_SEMANTICS_ROADMAP.md) —
  **in progress**; establishes the distinct `copy` lifecycle operation, adds target-directed
  `T(copy source)`, and enables compile-time most-specific ordinary `init`
  overloads including `super(...)`. The declaration and internal identity are
  distinct; direct and base overload selection and explicit target-directed
  copy construction are implemented, with the final hardening audit next. It
  depends on the completed lifecycle, polymorphism, and object-cast profiles.
- [Shared Ownership and Heap Allocation](SHARED_OWNERSHIP_ROADMAP.md) —
  **planned, blocked**; implements non-null strong owners, explicit ordinary
  and copy allocation, deterministic last-owner destruction, shared
  polymorphism, and hidden borrow anchors. Next after its prerequisite: parse
  and resolve shared types and allocation forms. It depends on completion of
  the constructor-semantics roadmap as well as the completed object-cast
  profile and frozen language/compiler contracts.

The completed polymorphism and object-cast profiles remain the implementation
baseline. Constructor overload and explicit-copy semantics are implemented as
specified in [Classes and Lifecycle](../language/CLASSES_AND_LIFECYCLE.md);
their final hardening task precedes shared work. Shared-ownership language and
implementation contracts are frozen in
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
