# Skald Documentation

Skald documentation is organized by the kind and maturity of the fact being
described. This index is the reader entry point for the focused language,
compiler, and development guides.

## Authority

Use the authority closest to the behavior:

- broad source-visible language meaning begins in the
  [language overview](language/README.md), while detailed rules belong in its
  focused language documents;
- current support and design maturity belong only in the
  [language status matrix](language/STATUS.md);
- exact accepted syntax belongs in the
  [implemented grammar](language/GRAMMAR.md);
- type, value, literal, and expression semantics, including the frozen complete
  primitive operator profile, implemented integer operator families, and
  implemented complete explicit primitive cast matrix, belong in
  [types and values](language/TYPES_AND_VALUES.md);
- the implemented raw-byte `std::str::Str` value, literals, library boundary,
  and frozen primitive textual conversions belong in
  [strings](language/STRINGS.md);
- the implemented nine-function `std::io` source contract belongs in
  [standard I/O](language/IO.md);
- the implemented source-visible invocation-vector, byte, snapshot, ownership, and
  Linux discovery contract belongs in
  [process arguments](language/PROCESS.md);
- the implemented `std::vec::Vec<T>` collection contract belongs in
  [vectors](language/VECTORS.md);
- the implemented source-visible structural bracket protocol, including array
  precedence and class/interface method selection, belongs in
  [structural indexing and slicing](language/INDEXING_AND_SLICING.md);
- the implemented source-visible contract for inline, shared, optional-shared,
  nested, indexed, and sliced arrays belongs in
  [arrays](language/ARRAYS.md);
- the implemented explicit `T?` and canonical `(shared T)?` contract,
  `shared? T` shorthand, compositional source syntax, nested optionals, and
  optional inline arrays belong in
  [optional values](language/OPTIONAL_VALUES.md);
- callable, binding, statement, return, and evaluation-order semantics belong
  in [functions and control flow](language/FUNCTIONS_AND_CONTROL_FLOW.md);
- the implemented nominal `Iterable<Item, State>` protocol, `for-in` selection,
  termination, and source-visible loop lifetimes belong in
  [general iteration](language/ITERATION.md);
- the frozen canonical `Successor<Output>` and `Range<T>` contract, half-open
  `..` expression, exact class opt-in, and tight primitive range-loop profile
  belong in [generic ranges](language/RANGES.md);
- exact classes, inline containment, receivers, ordinary initializer
  overloading, explicit copy construction, and object places belong in
  [classes and lifecycle](language/CLASSES_AND_LIFECYCLE.md);
- implemented explicit generic class applications, structural substitution,
  contextual requirements, nominal bounds, invariance, and complete-class
  validation belong in [generic classes](language/GENERIC_CLASSES.md);
- implemented call-scoped alias access and lifetime, including produced
  exact-class read-only sources, belong in
  [aliases and ownership](language/ALIASES_AND_OWNERSHIP.md);
- implemented non-null shared values, heap allocation, strong ownership,
  exact-class copy allocation, last-owner destruction, and borrow-anchor
  semantics belong in
  [shared ownership and heap allocation](language/SHARED_OWNERSHIP.md);
- implemented inheritance, polymorphic views, virtual/interface dispatch,
  type tests, and checked object casts belong in
  [polymorphism](language/POLYMORPHISM.md);
- C-style object casts, the complete inline/alias/shared conversion matrix,
  cast failure, and target-directed checked copy sources belong in
  [object casts](language/OBJECT_CASTS.md);
- the current compilation unit, top-level namespace, entry point, external
  declarations, and frozen initial module language belong in
  [modules and foreign interoperation](language/MODULES_AND_INTEROP.md);
- compile-time rejection, current runtime-failure boundaries, the frozen panic
  and static-message policy, and future exceptional cleanup belong in
  [errors and exceptional control flow](language/ERRORS.md);
- durable compiler structure, repository roles, crate API policy, and extension
  policy belong in the [compiler architecture](compiler/README.md);
- phase products, IR responsibilities, verification, dumps, trust boundaries,
  and the frozen primitive operator representation belong in
  [compiler phases and IR](compiler/PHASES_AND_IR.md);
- implemented canonical iteration identity, structured HIR, receiver ownership,
  MIR expansion, verification, target, and ABI boundaries belong in the
  [general-iteration compiler contract](compiler/ITERATION.md);
- frozen canonical range identities, primitive successor realization,
  ordinary-construction HIR provenance, immediate integer loop fusion, and
  performance acceptance belong in the
  [generic-range compiler contract](compiler/RANGES.md);
- frozen multiple-file providers, filesystem resolution, entry selection,
  identities, loading, and linkage belong in the
  [module-system compiler contract](compiler/MODULE_SYSTEM.md);
- implemented template identities, closed specialization, requirement evaluation,
  recursion, lower-IR exclusion, and target realization belong in the
  [generic-class compiler contract](compiler/GENERIC_CLASSES.md);
- the implemented array phase, lifecycle, backing, anchor, verification, and
  runtime responsibility design belongs in
  [the array compiler and runtime contract](compiler/ARRAYS.md);
- the frozen syntax, resolution, call-normalization, and lower-phase boundary
  for class/interface brackets belongs in the
  [structural indexing and slicing compiler contract](compiler/INDEXING_AND_SLICING.md);
- the implemented optional HIR, checked views, verification, x86-64 layout,
  internal ABI, and explicit exclusions belong in
  [the optional-values compiler contract](compiler/OPTIONAL_VALUES.md);
- implemented language-item, literal-data, immortal-backing, verification, and
  target responsibilities belong in
  [the strings compiler contract](compiler/STRINGS.md);
- the frozen private byte-array intrinsic, phase, target, and version-9 runtime
  boundary belongs in
  [the standard I/O compiler and runtime contract](compiler/IO.md);
- implemented shared-owner lowering, allocation layout, generated reference
  counting, finalizers, and the future minimal allocation ABI belong in the
  [shared-ownership compiler and runtime contract](compiler/SHARED_OWNERSHIP.md);
- target legality, layout, ABI realization, and code generation belong in the
  [backend and target contract](compiler/BACKEND.md);
- the public C runtime surface and compiler/runtime compatibility mechanism
  belong in the [runtime ABI](compiler/RUNTIME_ABI.md);
- compiler orchestration, CLI behavior, tool invocation, and artifact
  publication belong in [driver and artifacts](compiler/DRIVER_AND_ARTIFACTS.md);
- the frozen request-scoped event, timing, metric, renderer, and report-selection
  design belongs in [structured compiler reporting](compiler/REPORTING.md);
- contributor prerequisites and validation belong in the
  [development workflow](development/README.md);
- test ownership, placement, fixtures, determinism, and robustness belong in
  [testing](development/TESTING.md);
- phase inspection, dump use, verifier boundaries, and assembly debugging
  belong in [debugging the compiler](development/DEBUGGING.md);
- active roadmaps own implementation order and unresolved feature decisions;
- archived roadmaps and Git history explain how the project reached its current
  state, but never define current behavior.

The archived [migration inventory](archive/DOCUMENTATION_OVERHAUL_INVENTORY.md) maps
the removed legacy headings and references to their focused owners.
The [documentation overhaul roadmap](archive/DOCUMENTATION_OVERHAUL_ROADMAP.md)
records the completed migration.

## Maturity

The [language status matrix](language/STATUS.md) defines the maturity labels
and is the sole feature-support inventory. Other documents state their own
authority and maturity, then link to the matrix rather than repeating it.

If prose disagrees with implementation evidence, do not silently choose a new
language behavior. Correct plainly stale prose, strengthen tests for intended
current guarantees, or record the discrepancy in the relevant active roadmap
or a clearly named, indexed discovery backlog under `docs/roadmaps/`.

## Linking and maintenance

- Keep one authoritative statement for each fact. Summaries stay short and
  link to that statement.
- Use repository-relative Markdown links and valid local heading anchors.
- Link to semantic document names, not private implementation files, when the
  semantic contract is the subject.
- Update general documentation in the same change as the behavior or workflow
  it describes. Keep it crisp; do not preserve rollout diaries in living docs.
- Repair links in archived roadmaps when authorities move, but do not rewrite
  their historical task descriptions or milestone vocabulary.

Planned and active work, including dependencies, is listed in the
[roadmap index](roadmaps/README.md). Completed plans are listed in the
[archive index](archive/README.md).

## Verification

Run `make docs-check` for repository-local Markdown files, local anchors, and
required documentation, roadmap, and archive index entries. It is included in
`make check`.

Existing external infrastructure regularly runs `make check` from clean
checkouts, so it picks up documentation validation through the same local
Makefile interface. The repository does not duplicate that infrastructure with
a CI configuration.
