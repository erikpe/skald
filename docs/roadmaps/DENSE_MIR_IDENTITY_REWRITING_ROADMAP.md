# Dense Callable-Local MIR Identity Rewriting Roadmap

Status: planned; DMR0 is next.

This roadmap implements the frozen
[dense callable-local MIR identity rewriting design](DENSE_MIR_IDENTITY_REWRITING_DESIGN_PROPOSAL.md)
and its promoted
[compiler phase contract](../compiler/PHASES_AND_IR.md#frozen-dense-callable-local-mir-identity-rewriting-direction).
It gives target-independent MIR passes a safe structural editing boundary while
retaining compact dense MIR for verification, analysis, dumps, and backends.

The primary result is enabling infrastructure for later optimization, not an
optimization pass or pass-selection policy. Because the change touches nearly
every callable-local MIR reference form, the roadmap also deliberately removes
adjacent duplication, ad-hoc remapping, broad mutation pressure, unclear module
ownership, and panic-prone internal failure handling where those cleanups are
small and cohesive with the active task. Larger findings belong in the
[identity-rewriting discoveries record](DENSE_MIR_IDENTITY_REWRITING_DISCOVERIES.md)
rather than expanding a reviewed task.

## Scope and invariants

- Keep committed `MirProgram` dense, deterministic, directly indexable, and
  valid under the existing ordinary and static-lifecycle verifiers.
- Keep program-level function, member, class, field, static-field, type,
  module, lifecycle, declaration, and existing source `BindingId` identities
  stable.
- Introduce a private callable edit transaction with stable sparse slots,
  tombstones, explicit live order, and no representation as `MirProgram`.
- Compact `StorageId`, `ValueId`, `BlockId`, and `PathConditionId` exactly once
  at commit and canonically remap `OptionalGuardId`.
- Cover function, member, and static-initializer executable packages, including
  receiver, parameters, return storage, body entry, path and logical metadata,
  optional guards, and static publication attachments.
- Centralize collection, validation, remapping, compaction, and rehoming over
  one exhaustive callable-local identity traversal.
- Reject deleted, unknown, duplicate, missing-order, or foreign references with
  deterministic structured internal errors; never guess a semantic repair.
- Keep final MIR verification authoritative for types, dominance,
  definition-before-use, liveness, ownership, cleanup, guard balance, path and
  logical semantics, and lifecycle realization.
- Keep static-lifecycle baseline authority immutable and unavailable through
  the editor.
- Invalidate analyses keyed by pre-commit callable-local IDs unless an owner
  explicitly proves and performs a complete update.
- Return already-known identity maps and structured change counts without
  logging or maintaining global analysis state.
- Keep editor allocation callable-local and deterministic so future compiler
  parallelism cannot affect final IDs. Generated Skald execution remains
  single threaded.
- Keep initial HIR-to-MIR lowering append-oriented; do not migrate it to the
  optimization editor.
- Add no persistent instruction ID, SSA form, public common callable-body
  restructuring, general optimization registry, optimization-level CLI,
  production optimization, proof-provenance normalization, alias analysis, or
  backend virtual-register layer.
- Keep `mod.rs` files as concise facades and place implementation-private tests
  beside the owner.
- Prefer small touched-area maintainability fixes that reduce future change
  cost. Record broader opportunities with problem, evidence, likely owner,
  priority, and a bounded follow-up in the discoveries record.
- Keep the root Makefile as the automation interface; add no repository CI.

## Progress

- [ ] DMR0 — Establish exhaustive local-identity traversal
- [ ] DMR1 — Introduce stable sparse edit storage
- [ ] DMR2 — Commit deterministic dense callable state
- [ ] DMR3 — Integrate every executable definition kind
- [ ] DMR4 — Publish the supported callable editing facade
- [ ] DMR5 — Add explicit cross-callable rehoming
- [ ] DMR6 — Integrate verified pipeline invalidation and resealing
- [ ] DMR7 — Harden the boundary and close maintainability debt

## PR-sized implementation sequence

### DMR0 — Establish exhaustive local-identity traversal

**Purpose:** Create the single compile-time maintenance point for every
callable-local MIR identity before any transformation depends on remapping.

- [ ] Add a private `mir::rewrite` facade with cohesive traversal and structured
      error owners; keep the facade concise and avoid exposing optimization
      policy from `mir`.
- [ ] Define one visitor/remapper contract covering storage, value, block, path
      condition, and optional-guard references through declarations,
      instructions, rvalues, arguments, places, projections, terminators,
      path conditions, logical expressions, callable attachments, and static
      publication.
- [ ] Use exhaustive enum matches and full destructuring for identity-bearing
      structures; do not permit wildcard variants or `..` to hide future
      fields from review.
- [ ] Add deterministic structural-site vocabulary that can identify a header,
      publication edge, declaration, block, instruction position, terminator,
      path condition, or logical record without using source diagnostics.
- [ ] Separate source-semantic `BindingId` and program-level semantic
      identities from the callable-local remapping inventory.
- [ ] Consolidate any immediately adjacent duplicate local-ID scanning needed
      by the new owner instead of leaving competing authoritative traversals.
- [ ] Document the maintenance rule that each new local identity or reference
      form updates traversal and census coverage in the same change.

**Tests:** Direct collector/remapper tests for every model family; ownership and
structural-site errors; representative ordinary, ownership-heavy, logical,
optional, array, I/O, checked-operation, member, and static-initializer MIR;
compile-time exhaustiveness through non-wildcard matches; deterministic visit
order.

**Gates:** `cargo test --locked -p skald-compiler mir::rewrite`;
`cargo test --locked -p skald-compiler mir::verify`;
`make fmt-check`; `make lint`; `make docs-check`; and `git diff --check`.

**Exit criteria:** One private traversal can collect and identity-map every
current callable-local reference with deterministic sites, new identity-bearing
MIR variants force review, and no editing or compaction behavior has changed
production MIR.

### DMR1 — Introduce stable sparse edit storage

**Purpose:** Add the private representation in which a pass can make several
structural changes without renumbering unrelated entities after each edit.

- [ ] Add a small reusable private slot-table owner for live entries,
      tombstones, monotonic allocation, ownership checks, and stable lookup;
      keep abstractions limited to the four repeated dense-table
      responsibilities.
- [ ] Move storage, values, blocks, and path conditions into sparse edit slots
      when opening an isolated callable transaction.
- [ ] Maintain explicit block order independently from block allocation and
      require append, before, or after placement for new blocks.
- [ ] Retain storage and value slot order and enforce parent-before-child path
      condition creation.
- [ ] Represent logical-expression records with ordered tombstones and seed a
      private optional-guard registry from existing references without adding a
      committed MIR table.
- [ ] Keep block instruction vectors block-owned and provide no persistent
      instruction identity.
- [ ] Prevent the transaction, its sparse tables, and its mutation methods from
      being passed to ordinary verification, dumping, lifecycle analysis, or a
      backend.
- [ ] Split storage, order, guard, and error responsibilities into cohesive
      modules rather than growing one editor implementation file.

**Tests:** Stable IDs across earlier and repeated deletions; monotonic new slot
allocation; invalid ownership; duplicate or missing block-order entries;
path-parent ordering; guard discovery and tombstoning; logical-record order;
deterministic results across equivalent edit sequences.

**Gates:** `cargo test --locked -p skald-compiler mir::rewrite`;
`make fmt-check`; `make lint`; `make docs-check`; and `git diff --check`.

**Exit criteria:** An isolated private callable transaction supports stable
sparse slots, deletion, allocation, and explicit ordering without exposing a
malformed `MirProgram` or renumbering live entities.

### DMR2 — Commit deterministic dense callable state

**Purpose:** Turn a completed sparse transaction into one atomic, compact,
directly indexable callable or reject it before malformed MIR is published.

- [ ] Define typed commit maps for storage, values, blocks, path conditions,
      and optional guards plus a structured pass-owned change summary.
- [ ] Build maps from the frozen canonical policies: live slot/allocation order
      for storage and values, explicit order for blocks, parent-valid creation
      order for path conditions, ascending live guard slots, and retained
      logical-record order.
- [ ] Validate live-order coverage and every retained attachment and reference
      before constructing the dense result.
- [ ] Rewrite declarations and all references through the exhaustive traversal
      and set each dense declaration ID to its exact table position.
- [ ] Reject references to tombstoned, unknown, or foreign slots with the
      deterministic structural site; do not redirect, substitute, delete, or
      reorder implicitly.
- [ ] Ensure commit consumes private state and never installs a partially
      compacted result.
- [ ] Return maps and already-known counts without logging, rendering,
      verifying, or updating external analyses.
- [ ] Remove any temporary duplicate remapping helpers introduced during the
      traversal migration.

**Tests:** No-op equality and dump parity for isolated callable packages;
artificial gaps in each identity kind compact back to the expected dense form;
new entries follow canonical order; all dangling and foreign reference kinds;
missing and duplicate order entries; deterministic first error and change
counts.

**Gates:** `cargo test --locked -p skald-compiler mir::rewrite`;
`cargo test --locked -p skald-compiler mir::verify`;
`make compiler-test`; `make fmt-check`; `make lint`; `make docs-check`; and
`git diff --check`.

**Exit criteria:** A transaction either yields one deterministic dense callable
with complete maps and no tombstones or one structured rewrite error, while
semantic validity remains the ordinary verifier's responsibility.

### DMR3 — Integrate every executable definition kind

**Purpose:** Apply the common transaction to the complete final-MIR executable
surface without exposing general mutable definition tables or restructuring
public MIR.

- [ ] Add a private owned callable-package adapter for function, member, and
      static-initializer definitions with stable semantic owner data, common
      editable state, and variant-specific attachments.
- [ ] Remap receiver, parameters, return storage, body entry, and both static
      publication block references atomically with common body state.
- [ ] Add narrow crate-private ownership transfer that extracts and rebuilds
      function, member, and lifecycle-initializer containers in deterministic
      container order only after requested edits commit.
- [ ] Preserve sparse program function slots, member map identities, static
      initializer activation order, lifecycle plan/coordinator data, and all
      program-level semantic IDs exactly.
- [ ] Share common lookup and attachment adaptation where it removes actual
      function/member/initializer duplication, but do not introduce the
      rejected public `MirCallableBody` restructuring.
- [ ] Keep production `iter_mut` access absent; retain narrowly named test-only
      corruptors only where verifier tests require malformed final MIR.
- [ ] Make a multi-callable program rewrite externally atomic: an error drops
      the attempted output rather than returning a partially rewritten
      `MirProgram`.
- [ ] Update current-phase documentation for the new private representation
      without describing the roadmap as implemented prematurely.

**Tests:** No-op equality and exact MIR dump parity across ordinary functions,
instance and static members, constructors/copy/finalizers, and explicit static
initializers; publication follows block reordering; all header/attachment
dangling errors; semantic and lifecycle IDs remain unchanged; public API
compile tests expose no mutation escape hatch.

**Gates:** `cargo test --locked -p skald-compiler mir::rewrite`;
`cargo test --locked -p skald-compiler static_lifecycle`;
`cargo test --locked -p skald-compiler --test public_api`;
`cargo test --locked -p skald-compiler --test pipeline_determinism`;
`make compiler-test`; `make docs-check`; `make msrv-check`; and
`git diff --check`.

**Exit criteria:** Every executable definition kind round-trips through one
common private commit path, all variant attachments are covered, program
containers retain their semantic order and identity, and no broad production
mutation API has been added.

### DMR4 — Publish the supported callable editing facade

**Purpose:** Give future passes small, explicit structural operations so they
do not manipulate sparse internals or reproduce coordinated rewrites.

- [ ] Expose crate-private typed iteration and lookup over live storage,
      values, blocks, path conditions, logical records, and guards.
- [ ] Expose typed allocation and explicit removal operations for each editable
      entity kind.
- [ ] Add functional per-block instruction-list rewriting whose positional
      handles cannot be mistaken for durable committed identities.
- [ ] Add same-type value-use substitution with callable ownership and MIR type
      checks while documenting that callers remain responsible for dominance
      and semantic equivalence.
- [ ] Add explicit storage/place substitution and executable-edge redirection
      primitives with similarly narrow preconditions.
- [ ] Require callers to rebuild or delete path conditions, logical records,
      guard pairs, storage-liveness operations, and other proof metadata;
      helpers must not infer semantic cascading deletion.
- [ ] Add coordinated helper operations only where at least two concrete test
      transformations demonstrate the repeated responsibility.
- [ ] Replace touched ad-hoc test remapping utilities with the supported editor
      when they describe valid transformations; keep direct corruption local
      to verifier tests.
- [ ] Keep the `mir::rewrite` facade concise and document supported versus
      internal operations.

**Tests:** Successful value substitution and definition deletion; storage
replacement and cleanup of its explicit liveness operations; block insertion,
redirection, removal, and ordering; explicit path/logical/guard cleanup;
rejection of type, owner, and dangling-reference mistakes; committed results
pass `verify_final_mir`; no-op and deterministic dump behavior remain stable.

**Gates:** Focused `mir::rewrite` and MIR verifier tests;
`cargo test --locked -p skald-compiler --test pipeline_determinism`;
`make compiler-test`; `make fmt-check`; `make lint`; `make docs-check`; and
`git diff --check`.

**Exit criteria:** Representative deletion, insertion, substitution, and CFG
edits use only the supported facade, commit densely, and either pass central
verification or fail at the correct structural/semantic boundary without
pass-local remapping.

### DMR5 — Add explicit cross-callable rehoming

**Purpose:** Make the identity foundation sufficient for future inlining and
specialization without implementing either optimization.

- [ ] Add a two-phase importer that allocates all destination slots before
      cloning selected source nodes through the exhaustive remapper.
- [ ] Require explicit substitutions for receiver, parameters, return
      destination, entry, exits, and every reference outside the selected clone
      set.
- [ ] Rehome storage, values, blocks, path conditions, optional guards, logical
      metadata, places, instructions, and terminators to the destination
      callable owner.
- [ ] Preserve program-level semantic callable, type, field, static,
      declaration, and lifecycle identities unchanged.
- [ ] Reject foreign `BindingId` provenance and require imported callee locals
      to use an explicit compiler-owned storage kind with no forged source
      binding.
- [ ] Keep import policy separate from call-site splitting, evaluation order,
      ownership transfer, cleanup, return merging, recursion limits, and
      profitability.
- [ ] Reuse commit maps and error vocabulary rather than adding importer-local
      identity tables or diagnostics.

**Tests:** Synthetic complete and partial clone sets; every local identity gains
the destination owner; guard pairs do not collide; nested path/logical metadata
rehomes; explicit boundary substitutions succeed; missing substitutions,
foreign bindings, and leaked source IDs fail deterministically; repeated
imports allocate deterministically and pass final MIR verification when the
fixture supplies semantically valid boundaries.

**Gates:** `cargo test --locked -p skald-compiler mir::rewrite`;
`cargo test --locked -p skald-compiler mir::verify`;
`make compiler-test`; `make fmt-check`; `make lint`; `make docs-check`; and
`git diff --check`.

**Exit criteria:** One supported importer can clone a closed selected MIR region
into another callable with total destination-local identity maps and explicit
boundary substitutions, without delivering an inliner or changing production
output.

### DMR6 — Integrate verified pipeline invalidation and resealing

**Purpose:** Connect the editor to the existing final-MIR trust boundary so a
future transforming pass cannot bypass input or output verification.

- [ ] Add the narrow pipeline-private operation that consumes a
      `VerifiedFinalMirProgram` into raw MIR when a transformation invalidates
      the seal; expose no equivalent public escape hatch.
- [ ] Add an owned program-rewrite coordinator that applies callable
      transactions and returns raw dense MIR, commit maps/change summaries, or
      one structured rewrite failure.
- [ ] Exercise the future transforming pipeline shape: verify raw input,
      invalidate internally, rewrite, and call `verify_final_mir` to construct
      the only backend-accepted result.
- [ ] Preserve the current empty pipeline's single verification execution and
      byte-for-byte result; do not create a production pass registry or enable
      a transformation.
- [ ] Support test/debug verification after a synthetic transformation so a
      semantic edit defect is localized without letting commit counterfeit the
      verifier seal.
- [ ] Keep static-lifecycle baseline authority inaccessible and prove
      lifecycle-effect-changing test rewrites return through realization
      verification.
- [ ] Integrate already-known rewrite counts with pass-owned measured results;
      the editor emits no report text and reporting records only executions
      that occurred.
- [ ] Update public compile tests and compiler documentation for the sealed
      ownership path without exposing private editor types.

**Tests:** Compile-fail/public API coverage for raw and sparse products at the
backend boundary; valid synthetic deletion and CFG rewrite reseal; structurally
committable but semantically invalid edit fails final verification; unauthorized
static-lifecycle realization fails; empty pipeline counts remain one
verification and zero transformations; transformed test path reports truthful
counts; backend sees only sealed final MIR.

**Gates:** Focused pass-pipeline, reporting, static-lifecycle realization, and
backend-input tests; `cargo test --locked -p skald-compiler --test public_api`;
`cargo test --locked -p skald-compiler --test pipeline_determinism`;
`make compiler-test`; `make docs-check`; `make msrv-check`; and
`git diff --check`.

**Exit criteria:** The pass owner can safely invalidate, rewrite, and centrally
reseal final MIR, sparse or raw intermediate products cannot reach a backend,
the empty production pipeline is unchanged, and accounting remains truthful.

### DMR7 — Harden the boundary and close maintainability debt

**Purpose:** Prove complete coverage, resolve touched-area architectural debt,
and leave a durable foundation rather than a minimally working editor.

- [ ] Audit every current callable-local identity occurrence and every model
      family against the traversal census; add missing direct or corpus tests.
- [ ] Run artificial-gap round trips across a representative source corpus and
      prove no-op equality and exact dump parity for every executable
      definition kind.
- [ ] Complete the malformed matrix for removed, unknown, duplicate, missing
      order, and foreign identities at headers, declarations, instructions,
      terminators, metadata, guards, and publication attachments.
- [ ] Add repeated and independent-process determinism coverage for committed
      IDs, MIR dumps, rewrite errors, maps, and change counts.
- [ ] Audit the rewrite implementation by responsibility: split oversized
      files/functions, remove transitional adapters and duplicate traversal,
      tighten visibility, and keep `mod.rs` files as facades.
- [ ] Audit touched MIR definition lookup and test mutation utilities; resolve
      small duplication or awkward naming directly and record broader public
      model changes in the discoveries record.
- [ ] Confirm there is no production direct dense-vector surgery for valid MIR
      transformation and document the rewrite facade as the required future
      pass boundary.
- [ ] Update the compiler phase contract from frozen direction to implemented
      behavior, update the optimization discoveries status, and remove stale
      rollout language from living documentation without roadmap codes.
- [ ] Review every recorded discovery: implement small in-scope findings,
      retain bounded actionable follow-ups, and archive or remove the record
      only if none remain.
- [ ] Run the full repository, determinism, MSRV, documentation, formatting,
      lint, and diff gates from a clean artifact state.

**Tests:** Full no-op and artificial-gap corpus; all successful edit and
rehoming fixtures; complete malformed matrix; public API compile boundaries;
static-lifecycle removal/narrowing/inlining-shaped realization; independent
process determinism; unchanged default source diagnostics, MIR dumps, assembly,
and native observations when no transformation is registered.

**Gates:** Focused rewrite and pipeline suites during development, then
`make check`; `make golden-determinism-test`; `make msrv-check`;
`make docs-check`; and `git diff --check` from a clean artifact state.

**Exit criteria:** The frozen identity-rewriting contract is fully implemented,
all reference families and executable definition kinds are covered, default
compiler behavior is unchanged, module and API ownership are maintainable,
the full repository gates pass, and every larger follow-up is bounded in the
indexed discoveries record.

## Ordering and dependencies

The exhaustive traversal lands first because sparse editing, compaction, and
rehoming must not each invent their own reference inventory. Sparse storage
then establishes stable edit slots before dense commit assigns final IDs.
Definition integration follows only after isolated commit is proven, avoiding
simultaneous debugging of remapping and program-container ownership.

The supported editing facade builds on a working all-definition transaction.
Rehoming follows the same maps and facade but remains separate because foreign
ownership and boundary substitutions are a distinct responsibility. Pipeline
invalidation comes after structural operations are trustworthy, so the seal is
not weakened merely to exercise incomplete infrastructure. Final hardening
then audits the complete surface, removes transitional code, resolves
touched-area maintainability issues, and runs the broad gates.

Tasks are sequential at their public boundaries. Within a task, independent
model-family tests may be developed in parallel, but no later task may publish
an alternative traversal, slot representation, or seal escape hatch while its
dependency remains unsettled.
