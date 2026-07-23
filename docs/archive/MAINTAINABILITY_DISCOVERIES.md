# Maintainability Roadmap Discoveries

Status: resolved and archived after the class-orchestration follow-ups were
implemented as PM1 and PM2 of the polymorphism roadmap.

This document records follow-up findings discovered while implementing the
compiler maintainability cleanup. They remain separate from that roadmap so
its reviewed scope and ordering stay stable.

## Resolved

The [polymorphism roadmap](POLYMORPHISM_ROADMAP.md) completed both
follow-ups before hierarchy implementation so new member and lifecycle
categories build on concise owners.

- [x] Decomposed class-body resolution behind a focused private owner. The
      previous initializer, copy-lifecycle, destructor, and method paths
      repeated body-environment construction and definition assembly. The new
      owner keeps body resolution separate from declaration collection.
- [x] Extracted class declaration and lifecycle-body orchestration from
      `typeck/program/mod.rs`. The focused class-program owner centralizes
      shared member-check context while preserving explicit lifecycle
      semantics, diagnostics, HIR ordering, and the top-level type-check
      facade.

## Completed

- [x] Narrowed the duplicated Makefile command inventory in
      the architecture migration guide; `make help` is the detailed command
      reference and `docs/development/README.md` owns validation interfaces.
