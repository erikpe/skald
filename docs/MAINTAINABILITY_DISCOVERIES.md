# Maintainability Roadmap Discoveries

This document records maintainability findings discovered while implementing
the active cleanup roadmap. They remain separate from the roadmap currently in
progress so its reviewed scope and ordering stay stable.

## Pending

- [ ] Decompose `ProgramResolver::resolve_class_bodies` after the active
      roadmap. Initializers, copy lifecycle members, destructors, and methods
      repeat syntax-member lookup, declaration lookup, body-environment
      construction, and definition assembly. Keep this separate from CQ6 so
      declaration collection and body resolution remain independently
      reviewable.
- [ ] Remove or narrow the duplicated Makefile command inventory in
      `REPO_STRUCTURE.md`. It has already drifted by omitting `make msrv-check`;
      CQ18 should make `make help` the detailed command reference and keep only
      architecture-relevant validation guidance in living documentation.
