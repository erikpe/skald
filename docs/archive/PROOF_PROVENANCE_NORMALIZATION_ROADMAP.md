# Proof-Provenance Normalization Roadmap

Status: complete; PNR0 through PNR8 are complete.

This roadmap implements the frozen
[proof-provenance normalization design](PROOF_PROVENANCE_NORMALIZATION_DESIGN_PROPOSAL.md).
It turns final MIR from one product serving both lowering proof and backend
execution into two explicitly sealed stages over the same MIR model. A
mandatory one-way normalizer consumes verified path/logical provenance,
preserves its executable carrier behavior, and creates the stable boundary
needed by future target-independent CFG and protocol transformations.

The roadmap ends with one deliberately narrow production canary,
`post-proof-unreachable-block-elimination`. The canary proves that consumed
proof records no longer retain dead executable regions; broader forwarding,
merging, threading, logical simplification, checked-protocol normalization,
and storage cleanup remain separate optimization candidates.

Implementation-specific opportunities outside this reviewed scope belong in
the
[proof-provenance normalization discoveries](PROOF_PROVENANCE_NORMALIZATION_DISCOVERIES.md).
Candidate placement and status remain concise in the
[optimization candidate catalog](../roadmaps/OPTIMIZATION_CANDIDATE_CATALOG.md).

## Dependencies

- The completed
  [dense MIR identity rewriting roadmap](DENSE_MIR_IDENTITY_REWRITING_ROADMAP.md)
  provides exhaustive identity traversal, sparse transactions, and atomic
  dense commit.
- The completed
  [selectable final-MIR pipeline roadmap](SELECTABLE_FINAL_MIR_OPTIMIZATION_PIPELINE_ROADMAP.md)
  provides stable pass registration, exact schedules, private rewrite
  capabilities, verified execution, measurements, and borrowed inspection.
- The completed
  [static-lifecycle certificate roadmap](STATIC_LIFECYCLE_CERTIFICATE_ROADMAP.md)
  and
  [reachability-gated static lifecycle roadmap](REACHABILITY_GATED_STATIC_LIFECYCLE_ROADMAP.md)
  provide the permanent publication and lifecycle authority which must survive
  proof consumption.
- The completed
  [whole-world reachability roadmap](TARGET_INDEPENDENT_WHOLE_WORLD_REACHABILITY_ROADMAP.md)
  provides seal-bound facts which must be recomputed for normalized products
  and the final post-proof retention pass.
- The completed
  [local final-MIR simplification roadmap](LOCAL_FINAL_MIR_SIMPLIFICATION_ROADMAP.md)
  provides proof-aware CFG roots and the current conservative cleanup whose
  retained-block evidence motivates this boundary.
- Existing path-condition, logical-expression, optional, array, shared,
  cleanup, lifetime, checked-operation, and static-lifecycle verifiers are the
  authority for determining the last proof consumer. None may be bypassed or
  weakened to make normalization possible.
- Niflheim demonstrates the benefit of running broad executable CFG cleanup
  after semantic proof has been consumed. Skald retains its own final-MIR
  model, lifecycle authority, dense identity rules, and two-stage verifier
  contract.

## Scope and invariants

- Confirm and implement PPN1 through PPN14 as one trust boundary.
- Rename the current proof-bearing seal to `VerifiedProofMirProgram` and keep
  `VerifiedFinalMirProgram` as the normalized backend-ready public pipeline
  result.
- Keep one `MirProgram` data model. Empty proof tables and proof-only variants
  remain physically available to the proof-rich producer during this
  roadmap; the final seal proves their absence.
- Run complete proof-rich verification before any provenance is consumed.
- Normalize exactly once under every profile, including `none`; normalization
  is not registered, listed, selected, excluded, or repeated as a pass.
- Rewrite each path-condition rvalue into the same base-place activation load,
  preserving assignment identity, result type, and span.
- Reclassify condition activation storage from `PathCondition` to
  `ScalarSpill` while preserving storage identity, type, name, scope, stores,
  and lifetime operations.
- Remove all path-condition and logical-expression records atomically and
  reject every surviving consumed-proof reference before sealing.
- Factor verification into shared structural, proof-rich, and normalized
  owners; do not clone the full verifier or claim to re-prove erased path
  facts.
- Bind freshly computed target-independent reachability facts to every
  normalized seal and changed post-proof result.
- Give every selectable pass a closed `MirPassStage::{ProofRich, Final}` and
  reject wrong-stage ordering before execution.
- Give post-proof transformations a distinct narrow capability. Do not expose
  raw mutable MIR or allow them to recreate proof-rich records or lifecycle
  authority.
- Keep body entry, static-publication endpoints, and other permanent semantic
  attachments as CFG roots after normalization.
- Register only the reviewed post-proof canary. It removes entry-unreachable
  blocks and their transient values; it does not remove storage or add
  forwarding, merging, threading, or protocol rewrites.
- Run whole-world reachability after the canary in the final stage so removed
  call sites can affect retained definitions.
- Make inspection, failure attribution, pass listing, and reporting explicit
  about the proof-rich/final stage boundary.
- Preserve evaluation order, short-circuit behavior, sequential alias-visible
  mutation, checked failure and spans, ownership, destruction, static
  activation/shutdown, runtime traces, diagnostics, ABI, and target behavior.
- Preserve deterministic identities, ordering, dumps, measurements, and
  assembly. Whole-world and single-threaded execution do not weaken any
  semantic obligation.
- Do not add SSA, a second optimization IR, alias/effect analysis, storage
  propagation, an optimization level, arbitrary user pass ordering, a dynamic
  plugin ABI, target-specific passes, or repository CI.
- Keep Rust module facades concise and place implementation-private tests
  beside their semantic owner.

## Progress

- [x] PNR0 — Partition proof-rich and normalized verification ownership
- [x] PNR1 — Implement atomic proof-provenance normalization
- [x] PNR2 — Establish the two sealed final-MIR products
- [x] PNR3 — Make pass policy and execution stage aware
- [x] PNR4 — Expose stage-aware inspection, failures, and reporting
- [x] PNR5 — Migrate reachability and backend consumption
- [x] PNR6 — Add post-proof unreachable-block elimination
- [x] PNR7 — Activate and validate the two-stage production schedule
- [x] PNR8 — Harden ownership, documentation, and roadmap closure

## PR-sized implementation sequence

### PNR0 — Partition proof-rich and normalized verification ownership

**Purpose:** Make the last proof consumers and the post-consumption checks
explicit before introducing a product that depends on their separation.

- [x] Add one exhaustive compiler-owned classification of MIR proof-bearing
  records, identities, storage kinds, rvalues, attachments, and continuing
  semantic protocols.
- [x] Refactor verifier orchestration into shared structural checks,
  proof-rich checks, and normalized-only checks without changing current
  accepted or rejected proof-rich MIR.
- [x] Keep optional initialization, arrays, shared ownership, cleanup, storage
  lifetime, logical-expression, and path-condition dataflow in the proof-rich
  owner.
- [x] Keep identity/reference validity, ordinary executable structure,
  surviving protocol checks, lifecycle realization, and reachability
  completeness in shared or explicitly reusable owners where their evidence
  permits.
- [x] Define a closed normalized-invariant error vocabulary, including leaked
  path/logical records, path rvalues, path storage kinds, and unknown
  proof-bearing sites.
- [x] Add maintenance tests which fail when a new relevant MIR variant or
  identity site lacks classification.
- [x] Document verifier ownership in the compiler phase contract without
  claiming normalization is implemented.

**Tests:** Existing MIR verifier suite unchanged; focused classification and
orchestration tests; malformed logical/path/ownership/lifetime fixtures;
`cargo test -p skald-compiler mir::verify`; formatter, linter, and docs links.

**Exit criteria:** Current proof-rich verification has byte-for-byte stable
observable results, every proof-bearing family has one explicit last-consumer
classification, and normalized checks can be invoked in focused tests without
duplicating the complete verifier.

### PNR1 — Implement atomic proof-provenance normalization

**Purpose:** Build and test the mechanical one-way representation conversion
before it is allowed to create backend input.

- [x] Add a crate-private normalization transaction over a consumed
  proof-verified program.
- [x] Inventory exact path-condition owners, activation storage, path reads,
  logical records, and proof-protected blocks before mutation.
- [x] Rewrite every `MirRvalueKind::PathCondition` to
  `MirRvalueKind::Load(MirPlace::base(activation))` while preserving the
  assignment, `ValueId`, type, and span.
- [x] Reclassify exactly the owned `MirStorageKind::PathCondition`
  declarations as `MirStorageKind::ScalarSpill` without deleting storage,
  stores, blocks, lifetime operations, or values.
- [x] Delete all logical-expression and path-condition records in the same
  atomic program operation.
- [x] Commit through the existing dense rewriting owner and run the exhaustive
  zero-consumed-proof check on the committed result.
- [x] Return structured normalization failures without exposing partially
  converted MIR.
- [x] Return deterministic counts for consumed records, rewritten reads,
  reclassified storage, changed callables, and blocks released from proof
  protection.

**Tests:** Empty, single, nested, and parented conditions; mixed logical and
non-logical path users; functions, methods, lifecycle members, and static
initializers; exact identity/type/span preservation; malformed ownership and
unknown-reference failures; atomic rollback; deterministic repeated output.

**Exit criteria:** A verified proof-rich test program can be converted into
raw MIR satisfying the normalized invariant with no runtime operation or
permanent root deleted, while every malformed or partial conversion fails
atomically.

### PNR2 — Establish the two sealed final-MIR products

**Purpose:** Turn proof consumption into a type-enforced trust boundary while
preserving a coherent public backend-ready final product.

- [x] Rename the current proof-bearing wrapper and its verifier boundary to
  `VerifiedProofMirProgram` and `verify_proof_mir` inside the pipeline.
- [x] Define backend-ready `VerifiedFinalMirProgram` over normalized MIR,
  normalized-verification authority, and fresh seal-bound reachability facts.
- [x] Give both wrappers private construction and invalidation paths scoped to
  their legitimate pipeline owners.
- [x] Connect the PNR1 transaction from `VerifiedProofMirProgram` to
  `VerifiedFinalMirProgram`; no raw-MIR shortcut may construct the latter.
- [x] Redefine `verify_final_mir` as an unambiguous verify-and-normalize public
  convenience returning the final product; keep `verify_proof_mir`
  crate-private.
- [x] Ensure normalized verification checks all surviving executable,
  lifecycle, sparse-definition, and reference invariants it can establish
  without reconstructing erased path proofs.
- [x] Add compile-fail coverage proving external callers cannot forge either
  seal, detach facts, invoke private invalidation, or skip normalization.
- [x] Migrate direct compiler callers and tests to explicit proof or final
  terminology.

**Tests:** Seal compile-fail tests; raw/proof/final transition tests; leaked
proof rejection; reachability-fact binding; unchanged/clone/debug behavior;
public compilation API checks; full `skald-compiler` unit and integration
tests.

**Exit criteria:** Rust types distinguish proof-verified intermediate MIR from
normalized backend-ready final MIR, every construction path is closed, and no
API named “final” returns the proof-rich intermediate.

### PNR3 — Make pass policy and execution stage aware

**Purpose:** Prevent passes from accidentally consuming the wrong seal and
make the mandatory boundary part of deterministic pipeline orchestration.

- [x] Add `MirPassStage::{ProofRich, Final}` to every registry descriptor and
  expose it through the public read-only descriptor query.
- [x] Classify current local and checked passes as proof-rich and
  whole-world reachability as final.
- [x] Resolve schedules into ordered proof-rich and final regions separated by
  exactly one implicit normalization occurrence.
- [x] Reject proof-rich occurrences after a final occurrence, unknown stage
  identities, and any attempt to select or repeat normalization.
- [x] Split proof-rich and final pass traits/capabilities so their input and
  output seals cannot be confused.
- [x] Run complete proof verification after every changed proof-rich pass,
  normalize once, and run normalized verification after every changed final
  pass.
- [x] Preserve unchanged seals within one stage and invalidate all local IDs,
  reachability facts, and snapshots after any changed occurrence.
- [x] Keep exclusions stable-name based across the complete schedule and
  preserve deterministic occurrence numbering.

**Tests:** Registry validation; descriptor listing order and stages; exact
default, `none`, all-disabled, selective, repeated, and internal schedules;
wrong-stage rejection; changed/unchanged verification counts; normalization
exactly once; deterministic failure attribution.

**Exit criteria:** Every registered pass has one enforced stage, all supported
schedules cross the mandatory boundary exactly once, and no callback or
capability accepts both seal types.

### PNR4 — Expose stage-aware inspection, failures, and reporting

**Purpose:** Preserve observability without hiding two different MIR
contracts behind the old single-checkpoint API.

- [x] Replace the homogeneous checkpoint value with a closed borrowed
  proof-rich/final view.
- [x] Emit proof-rich input and after-pass checkpoints, one named
  `after-proof-normalization` final checkpoint, post-proof after-pass
  checkpoints, and one final checkpoint in exact order.
- [x] Make checkpoint display names collision-free and stage explicit while
  retaining schedule-position and occurrence identity for selectable passes.
- [x] Add a normalization failure stage distinct from input verification,
  pass execution, structural rewrite, and output verification.
- [x] Add mandatory normalization counts to pipeline aggregates without
  inventing a selectable pass occurrence or duration contract.
- [x] Keep post-proof canary and whole-world metrics ordinary pass-owned
  occurrence data.
- [x] Provide the correct seal-bound reachability dump from final checkpoints;
  proof-rich checkpoints remain unable to expose stale final facts.
- [x] Preserve quiet-path gating: no inspector, trace record, dump, label
  allocation, or optional metric scan occurs when not requested.
- [x] Update driver adapters and reporting renderers without mixing dumps into
  report events.

**Tests:** Borrow/lifetime compile-fail coverage; exact labels and callback
order; repeated names; failure cutoff at every stage; normalization metric
order; quiet/details/trace gating; writer failure; deterministic checkpoint
and report fingerprints.

**Exit criteria:** Tools and reports can distinguish every proof-rich and
final checkpoint, normalization failure is attributable, and ordinary quiet
compilation pays no optional inspection cost.

### PNR5 — Migrate reachability and backend consumption

**Purpose:** Complete the trust boundary by ensuring only normalized sealed
MIR reaches target-independent final retention and target lowering.

- [x] Make final-seal construction recompute dependency extraction and
  reachability from the exact normalized program.
- [x] Audit extraction exhaustiveness after path reads become ordinary loads;
  prove the conversion adds no callable, static, runtime-entity, or lifecycle
  dependency.
- [x] Adapt whole-world definition retention to consume and reseal only
  `VerifiedFinalMirProgram`.
- [x] Make `BackendInput` accept only `VerifiedFinalMirProgram` and remove any
  path-condition-specific lowering that the final invariant makes
  unreachable.
- [x] Keep static-lifecycle activation authority and permanent publication
  endpoints unchanged and visible to retained-domain/backend planning.
- [x] Verify sparse retained definitions and target-required entities after
  every changed final-stage pass.
- [x] Preserve target-private artifact retention as the final generated-symbol
  safety net.
- [x] Update backend and reachability contracts for the implemented type
  boundary while retaining target ABI and error categories.

**Tests:** Complete and sparse programs; direct/indirect/dispatch/lifecycle
dependencies; final-seal-only backend construction; rejected leaked proof;
static initializer/finalizer retention; runtime trace metadata; byte-identical
assembly for path read versus normalized load; backend artifact closure.

**Exit criteria:** Neither target-independent retention nor backend code can
observe proof-rich MIR, final reachability facts match the normalized program,
and existing complete/sparse backend behavior remains semantically identical.

### PNR6 — Add post-proof unreachable-block elimination

**Purpose:** Prove the architecture removes the original optimization barrier
with one conservative, independently selectable production transformation.

- [x] Add a final-stage CFG root query containing body entry,
  static-publication endpoints, and every remaining permanent semantic
  attachment, but no consumed proof root.
- [x] Add a final-stage rewrite capability limited to reviewed ordinary edge,
  block, and transient-value deletion operations.
- [x] Register `post-proof-unreachable-block-elimination` with one private
  numeric identity, exact description, `Final` stage, and deterministic
  measurements.
- [x] Compute executable reachability from all permanent entries and delete
  only unreachable blocks plus transient values defined in them.
- [x] Reuse the dense atomic transaction and reject dangling values,
  successors, attachments, lifecycle records, or foreign identities.
- [x] Report processed/changed callables, removed blocks/values, and retained
  permanent unreachable roots without logging or formatting in the pass.
- [x] Keep empty blocks, same-target/constant branch policy, storage
  declarations, stores, optional guards, and checked protocols unchanged.
- [x] Add listing, selection, exclusion, repetition, and unchanged-result
  behavior without activating the default occurrence yet.

**Tests:** Formerly proof-protected dead logical regions; nested logical CFG;
body/static publication/lifecycle roots; loops; checked failure blocks;
dangling-value rejection; methods and initializers; pass selection and
measurements; deterministic dense identity maps.

**Exit criteria:** The selectable canary removes a block retained solely by
consumed path/logical proof, cannot delete any permanent semantic root, and
always publishes a freshly normalized verified result.

### PNR7 — Activate and validate the two-stage production schedule

**Purpose:** Enable the mandatory boundary and canary only after full pipeline
composition has independent semantic evidence.

- [x] Freeze the exact default schedule as the existing proof-rich local
  occurrences, mandatory normalization,
  `post-proof-unreachable-block-elimination`, then
  `whole-world-reachability` last.
- [x] Make `none` contain zero selectable passes while still performing proof
  verification, mandatory normalization, and normalized verification exactly
  once each.
- [x] Prove all-disabled equals `none`; disabling the canary retains normalized
  proof-only dead CFG, and disabling reachability retains complete normalized
  definition bodies.
- [x] Add focused golden fixtures for nested logical conditions, path-sensitive
  optional/array/shared/lifetime behavior, static initialization/shutdown,
  ownership/destruction, panic spans, and runtime traces.
- [x] Pin proof-rich, after-normalization, and final MIR dumps plus exact
  structural/normalization/pass measurements.
- [x] Prove representation-only normalization emits the same machine
  operations under `none` as the former backend path.
- [x] Run default, `none`, selective-disabled, all-disabled, debug, release,
  repeated-process, and native-equivalence matrices.
- [x] Update candidate status and all living selection, phase, reporting,
  backend, debugging, and testing contracts as behavior becomes current.

**Tests:** Focused pipeline/driver/backend tests; dedicated golden group; full
golden and native matrices; cross-process fingerprints; assembly comparison;
`make check`; supported MSRV gate.

**Exit criteria:** The two-stage schedule is the production default, `none`
has the frozen mandatory-normalization meaning, native and diagnostic behavior
is unchanged, and deterministic output is proven across all supported
selection modes.

### PNR8 — Harden ownership, documentation, and roadmap closure

**Purpose:** Audit the delivered boundary as long-term infrastructure and
close the roadmap without leaving stale one-seal assumptions or hidden
maintenance hotspots.

- [x] Audit verifier, normalizer, pipeline, rewriting, reachability, backend,
  inspector, and reporting modules by responsibility; split oversized owners
  behind concise facades where doing so materially improves maintenance.
- [x] Search code, tests, and living docs for stale claims that
  `VerifiedFinalMirProgram` contains proof records, `none` performs only one
  verification, or every pass uses the same stage.
- [x] Ensure exhaustive maintenance tests identify every future
  proof-bearing MIR addition that must update classification and normalization.
- [x] Confirm no roadmap task code appears in living source, test names,
  diagnostics, dumps, metrics, or public documentation.
- [x] Resolve small maintainability findings directly and record larger
  follow-ups with evidence, impact, likely owner, priority, and bounded
  direction in the discoveries file.
- [x] Promote implemented status in the candidate catalog and living compiler
  contracts; remove obsolete discovery wording now owned by implemented
  documentation.
- [x] Run the complete repository and supported-toolchain quality gates from
  an artifact-free snapshot.
- [x] Mark every task complete, archive this roadmap and frozen design, update
  both indexes and all incoming links, and leave only actionable discoveries
  under `docs/roadmaps/`.

**Tests:** Focused suites from every prior task; `make check`; full golden and
native tests; independent-process determinism; docs links/indexes; formatter;
linter; supported MSRV; clean-tree rerun or equivalent artifact-free snapshot.

**Exit criteria:** The implementation and living documentation are
authoritative, every reviewed invariant is covered, actionable follow-ups are
indexed separately, the roadmap/design are archived, and repository status is
clean apart from intentional delivered changes.

**Closure audit:** Verification remains partitioned behind the MIR verifier
facade; normalization keeps planning, errors, and tests separate; pipeline
policy, execution, seals, optimizations, and observation have distinct owners;
rewriting retains one cohesive exhaustive traversal kernel behind its facade;
reachability separates extraction, roots, solving, verification, lifecycle,
and static access; backend target work remains below the sealed input facade;
and driver inspection and reporting stay independent. Splitting the traversal
kernel merely by line count would weaken its single compile-time inventory, so
no ownership split was warranted. The one remaining storage-provenance issue
is bounded in the indexed discoveries record.

## Ordering and dependencies

PNR0 must establish which checks can survive proof consumption before PNR1
erases anything. PNR1 keeps conversion mechanical and independently testable;
PNR2 then gives that conversion type-level authority. PNR3 makes the existing
pass framework understand those products before any post-proof pass exists.
PNR4 updates observation against the settled execution model instead of
creating a temporary checkpoint API.

PNR5 closes the backend trust path before PNR6 demonstrates executable CFG
deletion. PNR7 proves composition and activates the exact schedule. PNR8 is a
genuine ownership, stale-contract, and
artifact-free validation audit rather than a documentation-only tail.

Verifier factoring and normalization fixtures may be prepared in parallel
inside PNR0/PNR1, but seal naming depends on their exact contracts. Backend,
reporting, and canary work must not bypass the stage-aware runner. Broader CFG
and protocol candidates begin only after this roadmap is complete and their
own designs define additional post-proof capability.

The root Makefile remains the repository and external automation interface.
This roadmap adds no repository CI.
