# Active and Planned Roadmaps

This directory contains implementation roadmaps that are planned or in
progress. Completed roadmaps and resolved discovery records move to
[`../archive/`](../archive/README.md).

## In progress

- [Explicit Optional Values](OPTIONAL_VALUES_ROADMAP.md) — **in progress**. Adds
  inline `T?`, optional shared owners spelled `shared? T`, explicit `none`,
  non-failing presence tests, checked postfix unwrap, conditional lifecycle,
  and dynamically guarded inline payload views while preserving every
  non-optional validity guarantee. OP0 froze the focused language and compiler
  contracts in living documentation, OP1 added syntax and resolved type
  identities, and OP2 executes primitive optional locals and checked
  inspection. OP3 carries primitive optionals through stored and callable
  boundaries. OP4 added inline-class optional lifecycle, and OP5 added bounded
  checked payload views with dynamic presence guards, OP6 added optional shared
  owners with a verified one-word zero niche, and OP7 completed alias,
  overload, conversion, and polymorphism integration; OP8 is next. The roadmap
  depends on the
  completed inline-object, alias, shared-ownership, polymorphism, object-cast,
  and constructor profiles.

## Implementation baseline

The completed polymorphism, object-cast, and constructor profiles remain the
implementation baseline. Constructor overload and explicit-copy semantics are
specified in [Classes and Lifecycle](../language/CLASSES_AND_LIFECYCLE.md).
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
