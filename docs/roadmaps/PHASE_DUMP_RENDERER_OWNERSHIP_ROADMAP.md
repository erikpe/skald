# Phase Dump Renderer Ownership Roadmap

Status: in progress; D01 is complete and D02 is next.

This roadmap implements
[cleanup finding A26](CODEBASE_CLEANUP_AUDIT.md#a26--split-large-dump-renderers-by-responsibility)
by dividing the MIR, HIR, and resolved-program renderers into cohesive private
modules. Each phase keeps its existing dump entry point and exact textual
contract while giving program metadata, declarations, executable bodies,
expressions, aggregates, object views, and type names clear implementation
owners.

The work is source organization rather than a dump-format redesign. Stable
dumps are compiler observations used by exact tests, cross-process
determinism checks, debugging, measurements, and pipeline inspection, so every
move must preserve their bytes and ordering.

## Scope and invariants

- Preserve the public `dump_mir`, `dump_preliminary_mir`, `dump_hir`, and
  `dump_resolved` paths and signatures. Keep phase `mod.rs` facades and all
  downstream imports unchanged.
- Preserve every heading, indentation level, line and field order, separator,
  quoted string, identity spelling, semantic type name, span, optional
  section, and trailing newline byte-for-byte.
- Preserve deterministic iteration and the current behavior for incomplete or
  malformed internal data, including deliberate expectations used to expose
  compiler bugs. Do not hide invalid state with fallback formatting.
- Convert each large `dump.rs` owner into a recursive private `dump/` module.
  Keep its `mod.rs` focused on documentation, entry points, shared renderer
  context, module declarations, and narrowly scoped coordination.
- Split by phase-specific structural responsibility. Keep related nested
  helpers with their caller, and merge files that would otherwise become
  trivial pass-through layers.
- Use the narrowest practical visibility between fragments. Do not widen any
  dump API outside its phase merely to make an extraction compile.
- Continue using `dump_format` for genuinely phase-neutral indentation,
  quoting, and span primitives. Do not introduce a generic cross-phase dump
  model, visitor, renderer trait, or shared semantic vocabulary.
- Preserve the distinction between preliminary and final MIR headings and
  wrappers, resolved-program naming through its program context, and HIR type
  naming through its existing metadata tables.
- Keep exact expectations readable and owned by their existing phase tests.
  Add extraction-focused tests only when current exact or cross-process
  coverage does not exercise a moved responsibility.
- Treat file length as evidence for navigation work, not as a target. Do not
  change the textual format, IR models, lowering, resolution, type checking,
  verification, diagnostics, pipeline inspection, or compiler performance
  behavior in this roadmap.

## Progress

- [x] D01 — Establish the recursive layout with MIR dumping
- [ ] D02 — Divide typed HIR dumping by structural responsibility
- [ ] D03 — Divide resolved-program dumping by structural responsibility
- [ ] D04 — Audit ownership, validate all phase observations, and close A26

## PR-sized implementation sequence

### D01 — Establish the recursive layout with MIR dumping

**Purpose:** use the smallest of the three large renderers and its extensive
exact-output coverage to establish the private recursive layout before moving
the two stateful frontend dumpers.

- [x] Convert `mir/dump.rs` into a private recursive `mir/dump/` module while
  preserving `mir::dump_mir` and `mir::dump_preliminary_mir` through the
  existing MIR facade.
- [x] Keep program entry and phase-heading coordination in `dump/mod.rs` and
  extract cohesive owners for program/static-lifecycle metadata;
  classes/declarations/copy capabilities; executable bodies, blocks,
  instructions, and terminators; and operands, places, arrays, I/O, and object
  views. Adjust the exact grouping if review shows two adjacent families need
  the same context, but do not create one-function files.
- [x] Preserve free-function rendering where it remains clear. Introduce a
  private renderer context only if repeated state or dependencies demonstrate
  that it improves ownership.
- [x] Keep complete enum matching and explicit field formatting in the family
  that owns the MIR structure. Do not replace phase-specific formatting with
  debug output or a generic visitor.
- [x] Confirm exact preliminary/final headings, static lifecycle sections,
  declaration and definition order, block and instruction order, places,
  protocols, arrays, I/O, shared owners, object views, and spans.
- [x] Add a focused exact test only for a moved MIR family not represented by
  the current MIR dump corpus.

**Tests:** Run the exact MIR dump tests, representative MIR feature dump tests,
rewrite no-op/dump-preservation tests, and the cross-process phase-product
determinism filter. Run `cargo fmt --all -- --check`, `cargo test --locked -p
skald-compiler mir::tests`, `cargo test --locked -p skald-compiler
mir::rewrite`, `cargo test --locked -p skald-compiler --test
pipeline_determinism phase_products_are_deterministic_across_processes`,
`make check`, `make msrv-check`, and `git diff --check`.

**Exit criteria:** MIR dumping is a responsibility-oriented recursive private
module with concise entry coordination; all established imports compile; the
complete exact and deterministic output remains byte-identical; and the
resulting module pattern is suitable for the HIR and resolved renderers.

### D02 — Divide typed HIR dumping by structural responsibility

**Purpose:** separate the largest renderer without losing the shared type
metadata and indentation state that make HIR output coherent.

- [ ] Convert `hir/dump.rs` into a private recursive `hir/dump/` module while
  preserving the existing `hir::dump_hir` facade.
- [ ] Keep the `HirDumper` context and low-level line/indentation operations in
  one clear owner. Expose its fields and methods to child fragments only at
  the narrow visibility required by their calls.
- [ ] Extract cohesive implementations for program/type metadata and
  declarations; functions, locals, blocks, and statements; expressions,
  calls, and iteration; arrays and optionals; and shared ownership, object
  views, places, receivers, and origins. Keep small label/name helpers beside
  the family whose output they define.
- [ ] Preserve recursive rendering order, expression type suffixes, selected
  operation names, lifecycle and copy plans, object provenance, generic
  identities, spans, and every conditional section exactly.
- [ ] Avoid a generic node renderer or one method per file. Review cross-family
  calls explicitly so dependencies remain visible rather than being hidden by
  wildcard re-exports.
- [ ] Retain the local object-place exact-output test with its implementation
  owner, and add focused coverage only for a moved family absent from existing
  HIR dump and phase-product tests.

**Tests:** Run HIR/type-check dump tests, object and aggregate HIR tests, and
the cross-process phase-product determinism filter. Run `cargo fmt --all --
--check`, `cargo test --locked -p skald-compiler typeck::tests`, `cargo test
--locked -p skald-compiler hir::dump`, `cargo test --locked -p skald-compiler
--test pipeline_determinism phase_products_are_deterministic_across_processes`,
`make check`, `make msrv-check`, and `git diff --check`.

**Exit criteria:** the typed HIR renderer has clear private owners for its
metadata, declaration, statement, expression, aggregate, ownership, and
object-view responsibilities; shared renderer state remains coherent and
narrow; and every exact and cross-process HIR observation is unchanged.

### D03 — Divide resolved-program dumping by structural responsibility

**Purpose:** give the resolved renderer clear owners while preserving its
program-aware semantic naming and specialization evidence.

- [ ] Convert `resolve/dump.rs` into a private recursive `resolve/dump/` module
  while preserving the existing `resolve::dump_resolved` facade.
- [ ] Keep the `ResolvedDumper` program context, indentation primitives, and
  `ResolvedTypeNameContext` implementation in clear owners with narrow
  internal visibility.
- [ ] Extract cohesive implementations for program and language-item
  metadata; templates, constraints, specialization states, and semantic type
  names; class/interface/function declarations and definitions; blocks and
  statements; and expressions, operators, places, receivers, and
  dereferences.
- [ ] Keep specialization and template naming helpers with the generic/type
  responsibility. Preserve module qualification, selected identities,
  requirement reasons, declaration order, source-shaped resolved syntax, and
  all spans exactly.
- [ ] Avoid coupling dump fragments to resolver implementation modules or
  introducing an alternate semantic-name authority. The published
  `ResolvedProgram` remains the sole input.
- [ ] Add focused exact coverage only where the current resolved, generic,
  object, and cross-process suites do not represent a moved output family.

**Tests:** Run the complete resolver tests, focused resolved/object dump tests,
generic specialization publication determinism tests, and the cross-process
phase-product determinism filter. Run `cargo fmt --all -- --check`, `cargo test
--locked -p skald-compiler resolve::tests`, `cargo test --locked -p
skald-compiler --test pipeline_determinism
phase_products_are_deterministic_across_processes`, `make check`, `make
msrv-check`, and `git diff --check`.

**Exit criteria:** resolved-program metadata, generic/type naming,
declarations, statements, and expressions have cohesive private owners; the
renderer retains one program-aware naming context; all imports and exact dump
bytes remain stable; and independent processes produce identical phase
products.

### D04 — Audit ownership, validate all phase observations, and close A26

**Purpose:** review the three completed splits together, remove migration
scaffolding, and prove the repository still has one exact dump contract per
phase before closing the cleanup finding.

- [ ] Review every new facade and fragment by responsibility. Merge trivial or
  artificially separated files, remove migration-only imports and re-exports,
  and keep implementation-only visibility narrow.
- [ ] Confirm each public phase entry point still routes to one renderer and
  that no old `dump.rs`, duplicate formatting path, separately maintained
  output inventory, or generic cross-phase semantic layer remains.
- [ ] Compare representative pre-roadmap and final resolved, HIR, preliminary
  MIR, and final MIR bytes. Audit headings, ordering, empty sections, UTF-8 and
  quoted names, spans, generic/object/optional/array/shared forms, and trailing
  newlines.
- [ ] Run the exact phase suites and independent-process determinism coverage
  together. Add a regression only for a concrete coverage gap found during
  the audit.
- [ ] Update living compiler or testing documentation only if the stable dump
  contract changed in a way maintainers need to know; do not document private
  filenames as architecture.
- [ ] Mark A26 complete with the delivered ownership and validation evidence,
  mark this roadmap complete, archive it, and repair the active/archive
  indexes and every incoming link.
- [ ] Record actionable work beyond this behavior-preserving split in an
  indexed `PHASE_DUMP_RENDERER_OWNERSHIP_DISCOVERIES.md` rather than expanding
  closure scope. Do not create the file when no follow-up remains.

**Tests:** Run all focused resolved, HIR, MIR, dump-preservation, and
cross-process phase-product tests. From an artifact-free snapshot or clean
checkout, run `make check`, `make msrv-check`, `cargo fmt --all -- --check`,
the documentation checker, and `git diff --check`.

**Exit criteria:** the three phase renderers are navigable recursive private
modules with stable entry points and byte-identical deterministic output; no
substantial mixed-responsibility dump owner or migration scaffolding remains;
all repository gates pass; A26 is complete; and the roadmap is archived and
indexed.

## Ordering and dependencies

MIR comes first because its renderer is the smallest, primarily uses free
functions, and has broad exact-output coverage. It establishes the recursive
facade and review method without first solving shared state across child
modules. HIR follows with a metadata-backed dumper context and the broadest
expression/object surface. Resolved dumping comes last because its naming,
generic specialization, and source-shaped syntax all depend on one published
program context. The final audit can then compare the three phase-local
solutions and remove accidental inconsistencies without redesigning them.

A26 has no unresolved semantic dependency. The completed MIR identity
navigation work provides a useful private recursive-module precedent, while
the existing exact dump tests and `pipeline_determinism` suite own output
compatibility. The tasks should remain separate reviewable changes; discovering
that a split requires changing dump bytes, IR ownership, or a public entry
point is a reason to stop and record a separate proposal rather than fold that
change into this roadmap.
