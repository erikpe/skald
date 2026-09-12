# Compiler Facade Ownership Roadmap

Status: complete; archived after F04.

This roadmap implements
[cleanup finding A28](../roadmaps/CODEBASE_CLEANUP_AUDIT.md#a28--restore-concise-facades-in-selected-hotspots)
by restoring clear ownership around the type-checking program facade and the
resolver's program boundary. The result should make whole-program stage order,
public compiler paths, and responsibility-specific dependencies easy to scan
without changing any language or compiler behavior.

The work is deliberately selective. `typeck/program/mod.rs` still combines
orchestration, diagnostic vocabulary, type conversion, declaration validation,
and HIR construction. The resolver program facade is short, but its broad
parent imports supply an implicit dependency surface to responsibility modules.
Those are the demonstrated boundaries in scope; file length and wildcard-import
counts are evidence for review, not repository-wide cleanup targets.

## Scope and invariants

- Preserve the public `typeck::type_check`, `TypeCheckOutput`, and diagnostic
  constant paths and signatures. Preserve the private resolver entry points and
  every established downstream import path.
- Preserve type-check stage order, declaration-wide checking, diagnostic codes,
  messages, labels, notes, spans, severity, and deterministic emission order.
- Preserve resolved, HIR, preliminary MIR, planned MIR, and final MIR products
  and dumps exactly. Preserve stable identities, lookup behavior, specialization
  publication, entry selection, and executable-boundary checks.
- Keep `typeck/program/mod.rs` as the readable whole-program stage outline and
  HIR publication coordinator. Move substantial reusable conversion and
  declaration-validation concerns to cohesive private owners.
- Keep implementation modules private. Re-export only the existing public API
  through the established `typeck` facade, and use the narrowest practical
  visibility between type-checker siblings.
- Make imports explicit at the resolver program facade and at selected
  responsibility boundaries where a wildcard currently hides cross-module
  dependencies. Retain `use super::*` in tests and in tightly coupled internal
  fragments when spelling every shared implementation detail would reduce
  clarity.
- Do not impose one-type-per-file, zero-logic `mod.rs`, maximum-line-count, or
  repository-wide no-wildcard rules. Do not split cohesive algorithms merely
  to make files shorter.
- Do not redesign resolved or HIR models, diagnostic APIs, type capabilities,
  resolver stage products, or specialization publication. Record any such
  opportunity separately rather than expanding this behavior-preserving work.
- Keep A29's stale-comment and broad-allowance cleanup separate. A28 may remove
  imports or visibility made obsolete by its own moves, but it does not conduct
  a general lint-suppression or compatibility-alias sweep.

## Progress

- [x] F01 — Give type conversion and program diagnostics cohesive owners
- [x] F02 — Isolate declaration validation behind the type-check program facade
- [x] F03 — Make resolver program-boundary dependencies explicit
- [x] F04 — Audit the selected facades, validate behavior, and close A28

## PR-sized implementation sequence

### F01 — Give type conversion and program diagnostics cohesive owners

**Purpose:** remove widely reused type conversion and the stable program-level
diagnostic vocabulary from whole-program orchestration before moving validation
that depends on both.

- [x] Inventory the constants and conversion helpers currently owned by
  `typeck::program`, including all type-checker callers and public re-exports.
- [x] Move the program-level diagnostic constants to a cohesive private owner.
  Preserve every constant name, value, and public `typeck::CONSTANT` path.
- [x] Move resolved-to-HIR type and parameter-mode conversion, resolved-type
  equality, and closely related conversion helpers to a cohesive type-checker
  owner. Place optional alias eligibility with the narrowest owner supported by
  its actual dependencies rather than retaining it in program orchestration.
- [x] Replace imports that describe the old ownership with explicit imports
  from the new owners. Keep compatibility forwarding only when an established
  internal path has a demonstrated caller, and remove migration-only forwarding
  before completing the task.
- [x] Add focused unit tests for pure conversion or equality behavior only when
  current type-check tests do not exercise a meaningful boundary case.

**Tests:** Run `cargo fmt --all -- --check`, `cargo test --locked -p
skald-compiler typeck::tests`, `cargo test --locked -p skald-compiler --test
public_api`, `make check`, `make msrv-check`, and `git diff --check`.

**Exit criteria:** diagnostic constants and reusable conversion helpers have
clear private owners; all public constants retain their names and values; all
type-checker callers use intentional paths; conversion behavior is unchanged;
and `typeck/program/mod.rs` no longer owns those unrelated responsibilities.

Implemented: the private `diagnostic_codes` owner contains the 45 diagnostic
constants formerly mixed into program orchestration, while the `typeck` facade
retains every established public constant path. The private `conversion` owner
now maps closed resolved types and parameter modes to HIR and compares resolved
type identities. Its `lower_type` contract no longer accepts an unused program
context, and thin parameter, local, and declaration lowering helpers likewise
no longer forward that misleading dependency. The private `categories` owner
contains both phase adapters to the neutral capability vocabulary and the
optional-aware alias eligibility query.

All production callers import the responsible private owner directly; tests
use the supported `typeck` facade for public diagnostic constants. An exact
inventory comparison confirmed that all 45 moved names and values are
unchanged. Existing type-check coverage already exercises every resolved type
family, parameter mode, resolved identity comparison, optional alias category,
and diagnostic family, so no extraction-only test was added. The 526-test
focused type-check suite, public API integration tests, full repository gate
with 3,146 compiler tests and 629 golden cases, and Rust 1.82.0 workspace check
pass.

### F02 — Isolate declaration validation behind the type-check program facade

**Purpose:** leave the type-check program facade as a readable stage outline by
moving function declaration validation and lowering behind one cohesive private
boundary.

- [x] Extract internal-parameter, external-declaration, entry-point, and shared
  parameter validation into a private program declaration owner. Keep helpers
  used by class and interface checking available at the narrowest suitable
  visibility.
- [x] Move function declaration and parameter lowering with declaration
  validation when their shared inputs and invariants make that the clearer
  owner. Avoid a pass-through file whose only purpose is reducing line count.
- [x] Keep `type_check` responsible for ordering whole-program checks, invoking
  callable and class checking, deciding whether executable HIR may be
  published, and assembling the final `TypeCheckOutput`.
- [x] Preserve the early return for invalid optional types, continued
  declaration-wide diagnostics after other recoverable errors, entry-point
  requirements, external ABI restrictions, and the closed-interface boundary.
- [x] Review the resulting program facade and its existing `class`,
  `function_types`, `interfaces`, `overrides`, and `static_fields` children.
  Merge or adjust boundaries only where the extraction reveals a direct
  responsibility mismatch.
- [x] Strengthen source-to-diagnostic coverage only for a concrete ordering or
  validation gap found during the move.

**Tests:** Run focused declaration, interface, generic, array, alias-parameter,
and entry-point type-check tests; the complete `typeck::tests` suite; exact HIR
dump coverage; and phase-product determinism. Run `cargo fmt --all -- --check`,
`make check`, `make msrv-check`, and `git diff --check`.

**Exit criteria:** `typeck/program/mod.rs` presents a concise and complete
whole-program stage outline; declaration validation and lowering have one
cohesive private owner; sibling visibility remains narrow; and diagnostics,
HIR publication, exact dumps, and public paths are unchanged.

Implemented: the private `program::declarations` owner now contains internal
function parameter validation, shared class/function parameter rules, external
ABI validation, entry-point validation, and resolved function declaration and
parameter lowering. These operations share the same resolved declaration
inputs, type conversion rules, and diagnostic vocabulary. The class checker
uses the two shared helpers through this private owner; their visibility is
confined to the `program` module.

The 167-line program facade retains the complete semantic stage order, the
invalid-optional early return, callable and class body checking, the final
closed-interface assertion, executable HIR publication, and `TypeCheckOutput`
assembly. Reviewing its five existing responsibility modules found no direct
boundary mismatch: interface parameters use a distinct resolved model and
interface-specific rules, while function types, overrides, and static fields
remain cohesive owners. Existing tests already cover the moved diagnostic
families and ordering, exact declaration and parameter HIR, and the invalid-HIR
publication boundary, so no extraction-only test was added.

The 526-test focused type-check suite, all 53 cross-process phase-product
determinism tests, public API integration tests, full repository gate with
3,146 compiler tests and 629 golden cases, and Rust 1.82.0 workspace check pass.

### F03 — Make resolver program-boundary dependencies explicit

**Purpose:** make ownership visible where resolver program responsibilities
currently receive a large implicit namespace from their parent facade, without
turning every tightly coupled implementation file into an import inventory.

- [x] Inventory names supplied through broad parent imports in
  `resolve::resolver::program`, its direct responsibility modules, and the
  `generic_templates` and `specialization` subtree facades. Classify each use as
  a same-responsibility dependency, a dependency on an explicit stage product,
  or a cross-boundary dependency that should be named at its source.
- [x] Replace the broad import in the program facade with explicit imports and
  keep its module declarations, selective re-exports, and resolver entry points
  easy to scan.
- [x] Replace `use super::*` at direct production responsibility boundaries
  where it inherits the program facade's orchestration namespace. Import from
  the actual owning sibling, resolver facade, phase IR, identity, diagnostics,
  syntax, or module layer as appropriate.
- [x] Apply the same rule to the `generic_templates` and `specialization`
  subtree facades when their wildcard import exposes the whole program
  namespace to descendants. Do not mechanically rewrite leaf modules whose
  shared private context is cohesive and clearer through their local parent.
- [x] Remove import forwarding and visibility that become unnecessary. Do not
  widen a function, type, field, or module merely to avoid an explicit import.
- [x] Confirm the dependency cleanup preserves the stage-product and
  publication boundaries established by the completed resolver ownership work.

**Tests:** Run `cargo fmt --all -- --check`, the complete resolver tests,
generic specialization and publication tests, module graph tests, public API
and phase-boundary integration tests, and cross-process phase-product
determinism. Run `make check`, `make msrv-check`, and `git diff --check`.

**Exit criteria:** the resolver program facade names its dependencies
explicitly; selected responsibility and subtree facades no longer inherit an
unrelated orchestration namespace; remaining wildcard imports are local and
deliberate; visibility has not widened; and resolved products, diagnostics,
specialization publication, identities, and dumps are unchanged.

Implemented: `resolve::resolver::program` is now a 39-line facade whose imports
name only its two entry-point inputs, output, and coordinator. Its former broad
parent import and orchestration-wide forwarding imports are gone. The nine
direct production responsibility modules that previously inherited that
namespace now import diagnostics, identities, resolved IR, syntax, lookup,
body-resolution services, or sibling stage products from their actual owners.
No item or module visibility was widened.

The `generic_templates` and `specialization` facades now declare the private
context shared by their tightly coupled descendants explicitly. Their leaf
implementation modules retain local `super::*` imports where that context is
cohesive; test modules retain the same local convention. Relative paths that
reached through three module levels for resolver diagnostic codes were replaced
by explicit facade imports, and the range, iterable, and operator language-item
validators now name their resolver or resolved-IR dependencies at the source.
This leaves no wildcard import at the whole-program facade or any direct
production responsibility boundary.

Existing coverage already exercises every affected boundary, so no
import-structure-only test was added. The 351-test resolver suite, generic
specialization and publication cases, module graph cases, all 53 cross-process
phase-product determinism tests, public API and phase-boundary integration
tests, full repository gate with 3,146 compiler tests and 629 golden cases, and
Rust 1.82.0 workspace check pass.

### F04 — Audit the selected facades, validate behavior, and close A28

**Purpose:** review the type-check and resolver changes together, remove
migration scaffolding, and prove that improved navigation did not change phase
behavior or create artificial module boundaries.

- [x] Review each touched `mod.rs` as a front door. Keep module documentation,
  private module declarations, selective re-exports, small entry coordination,
  and central API types there; move remaining mixed implementation only when it
  has a demonstrated cohesive owner.
- [x] Review extracted files by responsibility rather than size. Merge trivial
  pass-through fragments, remove migration-only aliases and imports, and retain
  the narrowest practical visibility.
- [x] Audit public and crate-private paths, type-check stage order, resolver
  stage products, declaration-wide error collection, diagnostic ordering, and
  exact phase dumps against the pre-roadmap behavior.
- [x] Recount remaining production `use super::*` sites in the selected
  resolver boundaries and document why any retained site is local and clearer.
  Do not use the count itself as an acceptance threshold.
- [x] Update living architecture or contributor documentation only if it needs
  a durable module-ownership rule beyond the existing facade-oriented policy;
  do not document private filenames as stable architecture.
- [x] Mark A28 complete with delivered ownership and validation evidence, mark
  this roadmap complete, archive it, and repair active/archive indexes and all
  incoming links.
- [x] Record valuable work outside the selected boundaries in an indexed
  discoveries document rather than expanding closure scope. Do not create the
  document when no actionable follow-up remains.

**Tests:** Run all focused type-checker, resolver, public API, phase-boundary,
exact dump, and cross-process determinism tests. From an artifact-free snapshot
or clean checkout, run `make check`, `make msrv-check`, `cargo fmt --all --
--check`, the documentation checker, and `git diff --check`.

**Exit criteria:** the selected type-checker and resolver facades communicate
their APIs, stage order, and dependencies clearly; substantial implementation
has cohesive private owners; no accidental public path, visibility, diagnostic,
identity, phase product, dump, or executable behavior changed; no migration
scaffolding remains; A28 is complete; and the roadmap is archived and indexed.

Implemented: the closing review retained every touched facade and extracted
owner at its current boundary. The type-check program facade is a 167-line
whole-program stage outline, and its declaration, conversion, diagnostic, and
capability-category responsibilities each have cohesive private owners. The
resolver program facade is a 39-line front door with private responsibility
modules, one selective internal re-export, and two entry coordinators. The
larger generic-template and specialization facades define shared private
contexts for tightly coupled descendants; splitting those contexts into
pass-through modules would make their dependencies harder to follow. No
migration alias, forwarding import, or visibility widening remains.

The resolver recount found 22 production `use super::*` sites, all below the
generic-template or specialization subtree facades, plus five test-local sites.
These imports expose only the cohesive context declared by their immediate
parent and do not inherit the whole-program orchestration namespace. Direct
program responsibility modules and the program facade have none. The existing
facade-oriented module policy already states the durable ownership rule, so no
additional living architecture text or discoveries record was needed.

A pre-roadmap comparison confirmed that all 45 public diagnostic constant
names and values remain stable, the type-check stage sequence and resolver
products retain their established owners, and no public or crate-private
module was widened. The 526-test type-check suite, 351-test resolver suite,
public API and phase-boundary integration tests, all 53 cross-process
phase-product determinism tests, exact dump coverage, the full repository gate
with 3,146 compiler tests and 629 golden cases, and the Rust 1.82.0 workspace
check pass from the clean committed snapshot.

## Ordering and dependencies

Type conversion and diagnostic vocabulary move first because declaration
validation and most type-checker siblings already depend on them. Establishing
their durable paths before extracting validation avoids temporary forwarding
and repeated import churn. Declaration validation follows so the remaining
program facade exposes the actual whole-program stage sequence before resolver
imports are touched.

Resolver import cleanup comes after the type-check facade is stable because the
two phases share many semantic names but must retain separate ownership. The
completed resolver stage-product and specialization-publication work is the
contract this task preserves. The final audit can then judge both selected
facades consistently and close the finding without broadening it to unrelated
large modules.

A28 has no unresolved semantic dependency and does not require a separate
design proposal. A29 remains independent. Discovering that a clean extraction
requires a public API change, a phase-model redesign, different diagnostic
behavior, or new resolver publication authority is a reason to stop and record
a separate proposal rather than fold that decision into this roadmap.
