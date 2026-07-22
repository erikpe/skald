# Future Development Boundaries

This document records the architectural constraints and unresolved design work
that future Skald slices should preserve. It describes the current extension
surface; completed implementation history lives in [`archive/`](archive/README.md).

## Compiler architecture and extension policy

Durable phase responsibilities and the rules for extending them have moved to
the [compiler architecture](compiler/README.md) and
[compiler phases and IR](compiler/PHASES_AND_IR.md). This document temporarily
retains only feature-sequencing material until the documentation overhaul
distributes it to focused status and roadmap owners.

## Object-model sequence

The implemented object core includes direct and nested inline objects,
restricted call-scoped aliases, deterministic destruction, copy construction
and assignment, internal exact-class value parameters/results, explicit return
storage, bounded temporaries, and permitted constructor elision. These
contracts continue through verified MIR and native x86-64 execution. The
remaining progression is:

1. **Polymorphism.** Add inheritance, base projections, lifecycle composition,
   virtual dispatch, interfaces, type tests, and checked narrowing. The
   exploratory source-visible direction is in
   [Polymorphism](language/POLYMORPHISM.md), and the focused implementation
   plan is the [Polymorphism Roadmap](roadmaps/POLYMORPHISM_ROADMAP.md).
2. **Shared ownership.** Add allocation, reference counting, complete dynamic
   destruction, and syntax-directed borrow anchors.
3. **Checked exceptions.** Integrate partial construction and cleanup with
   exceptional control flow rather than retrofitting it afterward.

The completed implementation history and acceptance criteria for class-typed
fields are preserved in the
[archived inline-field roadmap](archive/INLINE_OBJECT_FIELDS_ROADMAP.md).
The local cleanup contract and its implementation record are preserved in the
[archived deterministic-destruction roadmap](archive/DETERMINISTIC_DESTRUCTION_ROADMAP.md).
The exact-class copy/value contract is preserved in the
[archived object-value roadmap](archive/OBJECT_VALUE_SEMANTICS_ROADMAP.md).

Each step needs a dedicated roadmap. Polymorphism must extend the established
copy, destruction, ABI, return-storage, temporary, and cleanup contracts rather
than introducing a second object-value model.

## Other planned language work

- loops and iterator protocols;
- arrays and their storage/initialization rules;
- optional values and scoped access to conditional payloads;
- integer division, remainder, comparisons, bitwise operations, shifts, and
  explicit casts;
- richer floating-point operations and user-facing formatting;
- strings and a Skald-written standard library;
- access control, `final`, static members, and broader module organization.

Arrays, optionals, loops, local/shared alias sources, destruction, shared
ownership, and checked exceptions all interact with lifetime or control-flow
rules. They should not be added as isolated parser features.

## Compiler evolution

Compiler extension constraints now live in the
[compiler architecture](compiler/README.md). Target availability and language
maturity live in the [status matrix](language/STATUS.md); module and broader
interoperation choices remain in
[modules and foreign interoperation](language/MODULES_AND_INTEROP.md).
