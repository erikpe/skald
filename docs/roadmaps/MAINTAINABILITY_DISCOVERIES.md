# Maintainability Roadmap Discoveries

Status: active follow-up backlog; resolver class-body orchestration is next.

This document records follow-up findings discovered while implementing the
compiler maintainability cleanup. They remain separate from that roadmap so
its reviewed scope and ordering stay stable.

## Pending

These are medium-priority follow-ups. Revisit them before adding more class
lifecycle categories, but they do not block the current compiler contracts.

- [ ] Decompose `ProgramResolver::resolve_class_bodies` in a focused follow-up.
      Initializers, copy lifecycle members, destructors, and methods
      repeat syntax-member lookup, declaration lookup, body-environment
      construction, and definition assembly. Keep this separate from class
      declaration collection so declaration and body resolution remain
      independently reviewable.
- [ ] Extract class declaration and lifecycle-body orchestration from
      `typeck/program.rs` before polymorphism expands member categories. The
      current `check_class_definitions` repeats `MemberCheckContext` assembly
      for initializers, copying, destruction, and methods; a focused class
      program owner should preserve diagnostics and HIR ordering while keeping
      the top-level type-check entry point concise.

## Completed

- [x] Narrowed the duplicated Makefile command inventory in
      `REPO_STRUCTURE.md`; `make help` is the detailed command reference and
      the architecture document lists only validation interfaces.
