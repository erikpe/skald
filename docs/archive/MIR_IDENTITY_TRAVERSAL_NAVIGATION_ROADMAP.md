# MIR Identity Traversal Navigation Roadmap

Status: complete; all tasks finished on 2026-09-12.

This roadmap implements
[cleanup finding A21](../roadmaps/CODEBASE_CLEANUP_AUDIT.md#a21--make-the-shared-mir-traversal-easier-to-navigate)
by dividing the exhaustive callable-local MIR identity traversal into cohesive
private modules. The resulting layout must remain one structural inventory
that generates both mutable mapping and read-only observation, while making
callable attachments, instructions, terminators, places, and identity leaves
possible to find and review independently.

The change is an internal source-organization refactoring. It does not alter
MIR, identity semantics, traversal behavior, rewrite authority, or the
crate-private rewrite facade.

## Scope and invariants

- Preserve one macro-generated structural inventory for mutable mapping and
  immutable observation. Do not introduce separately maintained visitors.
- Preserve the exact deterministic visitation order, early error propagation,
  identity sites, storage and value use roles, write authorizations, definition
  classification, and callable-owner validation.
- Keep full struct destructuring and exhaustive enum matches. New MIR fields or
  variants must continue to cause a compile-time maintenance obligation; do
  not add permissive default branches.
- Keep `StorageId`, `ValueId`, `BlockId`, `PathConditionId`, and
  `OptionalGuardId` in scope. Program-semantic identities and source
  `BindingId` values remain excluded.
- Preserve the current `mir::rewrite` facade, internal function signatures,
  visibility, mapper and observer traits, malformed-reference behavior, and
  downstream import paths.
- Use a recursive private module under `mir::rewrite::map`, with a concise
  `mod.rs` that owns composition and selective re-exports. Split substantial
  structural families by responsibility and avoid one-function pass-through
  files.
- Keep both generated traversal modules assembled by one top-level macro. Each
  extracted fragment must be expanded into both modules with the selected
  mutability and leaf behavior rather than defining concrete mutable or
  immutable traversal functions itself.
- Preserve all MIR, pass, verification, dump, assembly, diagnostic, ownership,
  cleanup-order, and native behavior. This roadmap introduces no optimization,
  new analysis fact, or MIR representation change.
- Keep structural CFG and dominance ownership in `mir::analysis`; A18 is
  complete and A21 must not move those facts into the rewrite traversal.

## Progress

- [x] T01 — Establish traversal composition and extract definition structure
- [x] T02 — Extract core instruction and operation traversal
- [x] T03 — Extract aggregate and I/O instruction traversal
- [x] T04 — Extract terminators and places, then close A21

## PR-sized implementation sequence

### T01 — Establish traversal composition and extract definition structure

**Purpose:** prove that the large traversal macro can be divided across private
modules without creating a second inventory or changing the generated mapping
and observation surfaces.

- [x] Convert `mir/rewrite/map.rs` into a recursive `map/mod.rs` facade while
  preserving every existing path and visibility used by rewrite internals.
- [x] Introduce the private macro-fragment composition pattern. Keep
  `map_identity`, `observe_identity`, and the single top-level traversal
  instantiation visibly owned by the facade.
- [x] Extract function, member, and static-initializer entry traversal;
  attachments; common declaration tables; body sequencing; path-condition
  metadata; and logical-expression metadata into cohesive definition/body
  fragments.
- [x] Keep shared leaf adapters available to later fragments without widening
  them outside `mir::rewrite::map` or coupling callers to the internal layout.
- [x] Retain exact mapper/observer event parity for functions, members, and
  static initializers, including attachment order and sites.
- [x] Document the internal composition rule where future maintainers add a
  new identity-bearing MIR field.

**Tests:** Run the rewrite tests that cover all identity families,
mapper/observer parity, deterministic visit order, member and static
attachments, identity-preserving mapping, reindexing, and exact owner-error
sites. Add a focused composition test only if the current suite does not prove
the extracted definition paths. Run `cargo fmt --all -- --check`,
`cargo test --locked -p skald-compiler mir::rewrite`, `make check`,
`make msrv-check`, and `git diff --check`.

**Exit criteria:** `map/mod.rs` owns the established facade and fragment
composition; the definition, attachment, body, and proof-metadata inventory
lives in cohesive private fragments; both generated traversals still emit the
same event sequence and errors; the remaining inventory stays single-sourced
while awaiting T02 through T04; and no duplicate implementation remains.

Implemented: `map/mod.rs` now owns leaf behavior, fragment composition, both
generated traversal instantiations, established re-exports, and owner-
validation adapters. Private `definition` and `body` fragments contain the
definition, attachment, declaration-table, body-order, path-condition, and
logical-expression inventory. Each fragment expands into both mapping and
observation, so their structure and order remain single-sourced. Existing
parity, deterministic-order, remapping, attachment, identity-preservation, and
exact owner-error tests cover the moved paths. The focused rewrite suite, full
repository gate with 3,146 compiler tests and 629 golden cases, and Rust 1.82.0
workspace check pass.

### T02 — Extract core instruction and operation traversal

**Purpose:** give ordinary instructions and their shared operation structures a
clear owner before separating the larger aggregate families.

- [x] Extract the exhaustive `MirInstruction` dispatcher and ordinary storage,
  value, assignment, rvalue, call, argument, cleanup, initialization, store,
  copy, shared-owner, cast, and string-operation traversal into a private
  instruction facade with cohesive implementation files.
- [x] Keep each nested helper beside the operation family whose fields it
  classifies. Avoid splitting short helpers solely to reduce file length.
- [x] Preserve source-before-destination and receiver-before-argument visit
  order, value definition/use distinctions, storage use roles, and write
  authorization decisions exactly.
- [x] Keep instruction entry points and imports used by sparse editing and
  cross-callable import execution unchanged through the `map` facade.
- [x] Extend focused parity and role tests only for operation families whose
  structural contract is not already represented.

**Tests:** Run focused rewrite mapping, value-use, storage-use, import, edit,
and owner-validation tests. Confirm mutable and immutable traversal produce
the same ordered identity events for the extracted instruction families. Run
`cargo fmt --all -- --check`, `cargo test --locked -p skald-compiler
mir::rewrite`, `make check`, `make msrv-check`, and `git diff --check`.

**Exit criteria:** ordinary instruction traversal is navigable through one
private instruction owner; every match remains exhaustive; downstream callers
retain their existing paths; and parity, roles, authorization, order, and
error behavior are unchanged.

Implemented: the private `instruction` facade now owns the exhaustive
`MirInstruction` dispatcher, while its cohesive `operation` fragment owns
ordinary assignments and rvalues, calls and arguments, cleanup and
initialization, stores and copies, checked views, shared-owner operations,
casts, and string initialization. Both fragments still expand into the same
mutable and immutable traversal modules, and the established
`map_instruction` path remains unchanged. Optional, array, and I/O helpers
remain in the composition module for T03. A source reconstruction check
confirmed that composing the extracted fragments reproduces the pre-T02
inventory exactly, including order and classifications. Existing parity,
role, authorization, edit, import, remapping, and exact owner-error tests cover
these paths, so no duplicate extraction-only test was added. The 96-test
focused rewrite suite, full repository gate with 3,146 compiler tests and 629
golden cases, and Rust 1.82.0 workspace check pass.

### T03 — Extract aggregate and I/O instruction traversal

**Purpose:** separate the largest nested instruction families along their MIR
model boundaries without hiding their identity and authorization decisions.

- [x] Extract optional, optional-box, optional-shared, array, and I/O traversal
  into cohesive private instruction submodules. Keep the instruction facade as
  their composition point.
- [x] Preserve nested optional-source recursion, object and array place order,
  guards, anchors, normalized indexes, range endpoints, buffer identities, and
  every storage write authorization.
- [x] Retain exhaustive matching for all aggregate and I/O enums and complete
  destructuring for their payload structs.
- [x] Avoid generic field walkers that erase semantic roles or make adding a
  protocol-bearing identity compile without an explicit classification.
- [x] Add representative mapper/observer parity and semantic-role cases where
  existing tests do not cover an extracted family.

**Tests:** Run focused rewrite, optional, array, I/O, value-use, storage-use,
edit, import, and malformed-owner tests. Run `cargo fmt --all -- --check`,
`cargo test --locked -p skald-compiler mir::rewrite`, `make check`,
`make msrv-check`, and `git diff --check`.

**Exit criteria:** aggregate and I/O identities have clear private owners under
the instruction facade; their generated mutable and immutable walks remain one
inventory; all classifications and ordering are preserved; and no broad
fallback or duplicate traversal has been introduced.

Implemented: private `optional`, `array`, and `io` fragments now own their
respective instruction families beneath the instruction facade. They retain
explicit exhaustive matches and complete destructuring, including optional
source recursion, guards and owners, array construction and slicing state,
aliases and anchors, normalized indexes, I/O buffers, and every existing role
and authorization decision. Each fragment expands into both mutable mapping
and immutable observation, leaving one structural inventory and no generic
field walker. A source reconstruction check confirmed that the composed
fragments reproduce the pre-T03 inventory exactly. Existing representative
optional, array, and I/O coverage already participates in mapper/observer
event equality, deterministic order, remapping, role, import, edit, and owner
validation tests, so no extraction-only test was added. The 96-test focused
rewrite suite, full repository gate with 3,146 compiler tests and 629 golden
cases, and Rust 1.82.0 workspace check pass.

### T04 — Extract terminators and places, then close A21

**Purpose:** complete the navigable structural inventory, audit its boundaries,
and leave future MIR additions with one obvious exhaustive maintenance path.

- [x] Extract terminator traversal, block-target helpers, places, projections,
  object views, receivers, origins, and leaf identity dispatch into cohesive
  private modules.
- [x] Preserve terminator operand and successor order, conditional edge
  identity, call result definitions, panic inputs, checked-operation metadata,
  place base/projection order, and provenance-carrier roles exactly.
- [x] Review the complete recursive module by responsibility. Merge fragments
  that became trivial, remove migration-only imports or re-exports, and keep
  `map/mod.rs` focused on composition and its established internal facade.
- [x] Audit every callable-local identity-bearing MIR field against the final
  inventory and confirm mapper/observer parity, ownership validation, sparse
  editing, dense commit, import, census, CFG-root, storage-use, and value-use
  consumers still share it.
- [x] Update living compiler documentation only if navigation or the
  maintenance contract is described there; avoid documenting private file
  names as architecture.
- [x] Mark A21 complete with delivered structure and validation, mark this
  roadmap complete, archive it, and repair the active and archive indexes and
  all incoming links.
- [x] Record valuable work beyond this behavior-preserving scope in an indexed
  discoveries document rather than expanding the closure task.

**Tests:** Exercise identity remapping, identity preservation, exact malformed
owner sites, all use-role censuses, rewrite edit/import/commit behavior,
mapper/observer event equality, and deterministic visitation. Run the full
compiler and golden behavior gates through `make check`, the Rust 1.82.0 gate
through `make msrv-check`, `cargo fmt --all -- --check`, and
`git diff --check` from an artifact-free snapshot or clean checkout.

**Exit criteria:** the exhaustive traversal is divided by structural
responsibility and easy to navigate; one macro-composed inventory still
generates mapping and observation; all established paths and behavior are
unchanged; no substantial mixed-responsibility traversal file remains; A21 is
complete; and the completed roadmap is archived and indexed.

Implemented: private `terminator`, `place`, and `leaf` fragments now own
control-flow exits and ordered targets; place projections, object views,
receivers, and origins; and typed identity-role dispatch. The 202-line facade
owns fragment composition, mutable and immutable leaf behavior, established
re-exports, borrowed-definition observation, and owner validation. The final
responsibility audit found no trivial fragments, widened paths, duplicated
visitor inventory, or substantial mixed-responsibility owner. Exact source
reconstruction proves that terminator operands and successors, checked
protocols, place bases and projections, provenance carriers, and every leaf
classification retain their previous order and behavior. Existing remapping,
identity-preservation, owner-error, use-role, edit, commit, import, census,
CFG, mapper/observer parity, and deterministic-order tests exercise the shared
inventory, so no extraction-only test was added. The full repository gate and
Rust 1.82.0 workspace check pass from an artifact-free exported snapshot,
including 3,146 compiler tests and 629 golden cases. No follow-up discovery
remains to record.

## Ordering and dependencies

T01 selects and proves the only new implementation mechanism before moving the
large instruction inventory. T02 establishes the instruction facade and moves
the common operation families that the aggregate traversal calls. T03 can then
place optional, array, and I/O traversal beneath that settled owner. T04 moves
the remaining terminator and place families and performs the complete
maintenance-boundary audit.

A18 is complete, so structural CFG and dominance queries already have their
neutral owner. A19 and A20 are also complete and consume observation through
the existing rewrite facade; their cache and pipeline-reporting contracts must
remain unchanged. No other active roadmap blocks T01.

Each task is one reviewable source-organization change. If an extraction
requires changing MIR semantics, visitor capabilities, identity roles,
rewrite authority, traversal order, or a downstream public path, stop and
record that work as a separate design candidate rather than folding it into
A21.
