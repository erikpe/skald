# Skald Documentation

Living documentation describes the current compiler or planned language
direction:

- [Draft language specification](SKALD_DRAFT_SPEC.md) — broader language design,
  implemented-profile annotations, and open questions.
- [Repository structure and compiler architecture](REPO_STRUCTURE.md) — current
  modules, phase contracts, backend/runtime boundaries, and tests.
- [Future development boundaries](NEXT_SLICE_BOUNDARIES.md) — constraints and
  likely sequencing for features that are not implemented yet.
- [Compiler debugging artifacts](DEBUGGING.md) — deterministic dumps,
  verification points, and assembly inspection.
- [Implemented grammar](../grammar/README.md) — exact source subset accepted by
  the current compiler.

Active implementation plans:

- [Polymorphism roadmap](POLYMORPHISM_ROADMAP.md) — single inheritance, base
  lifecycle composition, virtual dispatch, interfaces, and checked narrowing.

Completed implementation plans are historical records under
[`archive/`](archive/README.md). They should not be used to determine current
behavior.
