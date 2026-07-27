# Active and Planned Roadmaps

This directory contains implementation roadmaps that are planned or in
progress. Completed roadmaps and resolved discovery records move to
[`../archive/`](../archive/README.md).

## In progress

No implementation roadmaps are currently in progress.

## Planned

No implementation roadmaps are currently planned.

## Pending discoveries

No maintainability discoveries are currently pending.

## Design inputs

- [Initial Skald Module-System Proposal](SKALD_INITIAL_MODULE_SYSTEM_PROPOSAL.md)
  — proposed first-version language and compiler design for anonymous
  composable module roots, `::` qualification, explicit selective imports,
  multiple local bindings for one canonical module or declaration,
  one-identifier module aliases, private-by-default declarations, acyclic
  whole-program loading, file or logical entry selection, singleton file
  entries, stable identities, diagnostics, and explicit exclusions. Import
  declarations always name canonical module paths. The CLI uses positional
  file entries or `--entry`, repeatable anonymous roots, a replaceable or
  disableable standard library, and final-module-component output defaults.
  Identical cross-module external ABI declarations coalesce while incompatible
  assertions are rejected. Filesystem normalization coalesces canonically
  equivalent roots, permits symlink targets outside roots, derives identities
  from lexical root-relative paths with exact case, rejects every exact
  logical-path collision between distinct providers, and permits one physical
  source to back distinct logical modules. The proposal is design-complete and
  ready for formal promotion before an implementation roadmap.
- [Niflheim Module-System Audit](MODULE_SYSTEM_NIFLHEIM_AUDIT.md) — complete
  audit of Niflheim's implemented imports, visibility, whole-program graph,
  identities, linkage, library model, and implications for Skald. The initial
  Skald proposal builds on this evidence; no syntax or semantics are frozen by
  the audit itself.

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
