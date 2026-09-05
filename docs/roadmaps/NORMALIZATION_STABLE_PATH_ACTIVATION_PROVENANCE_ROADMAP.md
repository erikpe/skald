# Normalization-Stable Path-Activation Provenance Roadmap

Status: in progress; NSR0 through NSR2 are complete and NSR3 is the next task.

This roadmap implements the frozen
[normalization-stable path-activation provenance design](../archive/NORMALIZATION_STABLE_PATH_ACTIVATION_PROVENANCE_DESIGN_PROPOSAL.md).
It gives the executable boolean storage left by proof normalization a stable,
final-only classification and narrows the normalized definite-initialization
exception to that exact class.

The result is an optimization foundation, not a storage optimization. This
roadmap preserves every executable operation and identity. In particular,
FMM-13 dead normalized condition-carrier cleanup remains a separate selectable
future pass which may consume the new classification after proving its own
load, store, lifetime, attachment, and deletion conditions.

## Dependencies

- The completed
  [proof-provenance normalization roadmap](../archive/PROOF_PROVENANCE_NORMALIZATION_ROADMAP.md)
  provides the proof-rich and final seals, exhaustive proof classification,
  mandatory atomic normalizer, and private consumed-proof authority.
- The completed
  [dense MIR identity rewriting roadmap](../archive/DENSE_MIR_IDENTITY_REWRITING_ROADMAP.md)
  provides exhaustive identity traversal and deterministic dense commit.
- The completed
  [post-proof CFG canonicalization roadmap](../archive/POST_PROOF_CFG_CANONICALIZATION_ROADMAP.md)
  provides narrow final-CFG edit capabilities and normalized reverification.
- The completed
  [convergent local constant propagation roadmap](../archive/CONVERGENT_LOCAL_CONSTANT_PROPAGATION_ROADMAP.md)
  provides the proof-transition transaction which may compose logical
  selection with mandatory normalization.
- Whole-world compilation makes the storage-role inventory closed. It does not
  permit a malformed or phase-illegal declaration to cross either seal.
- Single-threaded generated execution removes concurrent observation, but it
  does not weaken definite initialization, sequential effects, lifetimes,
  aliasing, failure timing, cleanup, or ownership requirements.

## Scope and invariants

- Implement frozen decisions NSP1 through NSP14 without widening them.
- Add exactly one unit storage kind,
  `MirStorageKind::NormalizedPathActivation`, meaning an executable boolean
  path-activation carrier whose proof was validated and consumed.
- Keep `PathCondition` legal only in proof-rich MIR and
  `NormalizedPathActivation` legal only in normalized MIR.
- Make the validated mandatory normalizer the sole production constructor of
  the final-only kind; source lowering and ordinary rewrites must not create it.
- Retain no path-condition identity, logical-expression identity, predecessor,
  merge, parent, or generalized provenance payload after normalization.
- Preserve `StorageId`, storage order, declarations, values, blocks,
  instructions, terminators, spans, stores, loads, lifetimes, evaluation order,
  and executable behavior through the representation conversion.
- Restore ordinary definite-initialization checking for genuine `ScalarSpill`
  declarations in both verifier stages. Exempt only the structurally valid
  normalized activation class under consumed-proof authority.
- Validate every normalized activation as a compiler-generated boolean,
  reject it in proof-rich MIR, reject source-backed or malformed instances,
  and continue all applicable place, lifetime, identity, and structural checks.
- Provide one semantic query over the declared storage kind. Do not infer the
  role from names, spans, CFG topology, erased records, or an external identity
  set.
- Make enum and phase classification exhaustive in the model, verifier,
  normalizer, rewriter, dumps, analyses, and backend-facing storage handling.
- Preserve the kind by default through current final block/value/CFG edits.
  Any future capability that can edit storage must reject or handle it
  explicitly.
- Keep backend layout and lowering identical to the current boolean
  `ScalarSpill` home. Change no ABI, frame-layout rule, runtime service, symbol,
  instruction semantics, or assembly except diagnostic MIR spelling.
- Dump the final-only kind deterministically as
  `normalized-path-activation`; keep existing normalization metrics and pass
  selection stable.
- Add no selectable pass, profile, language feature, runtime behavior, target
  dependency, general provenance subsystem, or dead-carrier deletion.
- Keep implementation behind the existing MIR model, normalization,
  verification, rewriting, dump, and backend facades. Record unrelated or
  materially larger opportunities in the companion discoveries file.

## Progress

- [x] NSR0 — Freeze the storage-phase contract and baseline
- [x] NSR1 — Add the final-only storage representation exhaustively
- [x] NSR2 — Make normalization retain the stable activation role
- [ ] NSR3 — Narrow normalized scalar initialization authority
- [ ] NSR4 — Seal rewrite and analysis handling of the new role
- [ ] NSR5 — Preserve backend behavior and deterministic observation
- [ ] NSR6 — Complete source, profile, and malformed-MIR evidence
- [ ] NSR7 — Harden ownership, documentation, and roadmap closure

## PR-sized implementation sequence

### NSR0 — Freeze the storage-phase contract and baseline

**Purpose:** Establish an auditable before-state and one exhaustive phase
contract before changing the shared storage enum.

- [x] Inventory every `MirStorageKind` producer, match, verifier branch,
  normalizer conversion, rewrite path, dump spelling, analysis classifier, and
  backend layout/lowering consumer in the dedicated
  [NSR0 inventory](NORMALIZATION_STABLE_PATH_ACTIVATION_PROVENANCE_INVENTORY.md).
- [x] Centralize the proof-rich versus normalized storage-legality decision in
  the MIR contract layer rather than scattering stage tests among consumers.
- [x] Add exhaustive tests covering the current kinds, including
  `PathCondition` proof-rich acceptance and normalized rejection.
- [x] Freeze focused baselines for path-activation identity, operation, span,
  storage order, dump, normalization measurements, backend frame behavior, and
  assembly under `none` and the default profile.
- [x] Add synthetic fixtures for ordinary initialized and uninitialized
  `ScalarSpill` declarations so the later verifier tightening has an explicit
  before/after boundary.
- [x] Record any unclassified or duplicate ownership found during the inventory
  without changing the frozen scope.

**Tests:** MIR contract and verifier tests; normalizer and dump baselines;
focused backend assembly equivalence fixture; compiler static checks.

**Exit criteria:** Every storage-kind handling site is inventoried, one
exhaustive stage-legality authority exists, the behavioral baseline is
reproducible, and no runtime or optimization behavior has changed.

### NSR1 — Add the final-only storage representation exhaustively

**Purpose:** Introduce the stable semantic vocabulary and make omissions fail
at the owners which must understand it.

- [x] Add the unit `MirStorageKind::NormalizedPathActivation` variant and its
  narrow semantic query through the MIR model facade.
- [x] Classify it as final-only and `PathCondition` as proof-rich-only; reject
  either kind at the wrong verifier seal.
- [x] Update every exhaustive model, identity traversal, rewrite, inspection,
  analysis, debugging, and backend-facing match identified by NSR0.
- [x] Preserve the new kind without remapping or external side tables through
  storage-preserving dense rewrites.
- [x] Reject source-originated or otherwise invalid construction by contract;
  do not add a public general constructor or payload.
- [x] Add future-variant maintenance tests so a new storage kind cannot bypass
  phase legality or downstream classification.

**Tests:** Exhaustive storage-kind contract tests; wrong-stage and malformed
construction tests; identity traversal and no-op rewrite tests; formatter and
linter checks.

**Exit criteria:** The representation compiles through every existing owner,
has one stable semantic query, is illegal at the proof-rich seal, and is not
yet emitted by production normalization.

### NSR2 — Make normalization retain the stable activation role

**Purpose:** Change the sole production transition atomically while preserving
the normalizer's established proof-consumption transaction.

- [x] Reclassify each validated `PathCondition` storage declaration to
  `NormalizedPathActivation` instead of `ScalarSpill` in the mandatory
  normalizer.
- [x] Require exact one-to-one ownership between consumed path records and
  reclassified activation declarations before mutation.
- [x] Preserve the original `StorageId`, type, generated origin, order, stores,
  loads, lifetime markers, blocks, values, and spans.
- [x] Keep path-rvalue-to-load conversion, logical/path record removal, optional
  proof-transition edits, and storage reclassification in one validated commit.
- [x] Reject stale, foreign, duplicate, missing, already-normalized, wrong-kind,
  or wrong-type inventory entries before publishing any candidate program.
- [x] Keep normalization counters and changed-callable ownership stable; add no
  second provenance metric solely for the renamed final role.

**Tests:** Empty, simple, nested, parented, method, lifecycle, and static-
initializer normalization; optional logical-transition composition; exact
identity/operation/span preservation; malformed and stale inventory rollback;
deterministic repeated normalization.

**Exit criteria:** Every production path activation crosses exactly once to
the final-only kind, no proof identity survives, failed transactions publish no
partial product, and executable normalized MIR is otherwise unchanged.

### NSR3 — Narrow normalized scalar initialization authority

**Purpose:** Remove the broad normalized `ScalarSpill` exception and make the
consumed-proof reliance precise.

- [ ] Run ordinary compiler-owned `ScalarSpill` definite-initialization
  analysis under both proof-rich and normalized contracts.
- [ ] Exempt only `NormalizedPathActivation` from reconstructing erased
  path-sensitive initialization, and only while validating a final product
  carrying private consumed-proof authority.
- [ ] Structurally require each normalized activation to be compiler-generated,
  boolean-typed, locally declared, and free of path/logical proof references.
- [ ] Continue all applicable declaration, place, reference, lifetime,
  instruction, terminator, checked-protocol, cleanup, and ownership checks.
- [ ] Reject an uninitialized ordinary scalar spill in normalized MIR while
  retaining valid normalized short-circuit and conditional programs.
- [ ] Keep the proof-rich verifier's complete path-sensitive acceptance and
  ordinary spill checking unchanged.

**Tests:** Initialized and uninitialized ordinary spills in both stages;
accepted normalized activations; wrong type/origin/stage and leaked proof;
short-circuit, conditional cleanup, optional, array, shared-owner, checked-
protocol, loop, and lifecycle cases.

**Exit criteria:** No stage-wide scalar-spill exemption remains, only the exact
final activation class relies on consumed authority, and all other verifier
obligations remain active.

### NSR4 — Seal rewrite and analysis handling of the new role

**Purpose:** Ensure current transformations preserve the distinction and
future storage transformations cannot silently erase it.

- [ ] Audit final-stage unreachable deletion, empty-block forwarding, block
  merging, dead-pure-definition elimination, reachability, and constant
  consumers against normalized activation declarations and accesses.
- [ ] Make the existing storage-use census and semantic classifiers distinguish
  the new role from ordinary scalar spills without treating it as an eligible
  checked constant carrier.
- [ ] Prove current block/value/CFG edit capabilities cannot create, delete,
  reclassify, or move storage operations as an incidental side effect.
- [ ] Add an explicit default-reject requirement to any capability or edit plan
  which gains storage mutation in the future.
- [ ] Revalidate the kind and its structural invariants after every changed
  final-stage transaction and fresh seal.
- [ ] Use only the storage-kind semantic query; add tests rejecting name-,
  span-, topology-, and stale-set-based classification assumptions.

**Tests:** Every current final pass with live and unreachable path activations;
carrier-census rejection; dense block/value rewrite preservation; stale-plan
and malformed-kind failures; changed and no-op resealing.

**Exit criteria:** Existing passes preserve the role and verify cleanly, no
analysis confuses it with an ordinary scalar spill, and future storage mutation
has an explicit maintenance barrier.

### NSR5 — Preserve backend behavior and deterministic observation

**Purpose:** Make the refined representation visible where useful while proving
that target behavior is unchanged.

- [ ] Render the final-MIR storage kind deterministically as
  `normalized-path-activation` with the reviewed generated-source placeholder.
- [ ] Treat it as the same boolean stack home as the former scalar spill in
  legality, frame planning, place addressing, load/store selection, and
  complete versus retained emission.
- [ ] Keep proof-only `PathCondition` storage rejected at the normalized
  backend boundary and add an exhaustive shared-enum maintenance test.
- [ ] Prove unchanged slot size, alignment, lifetime handling, instruction
  sequence, ABI, symbols, runtime calls, and emitted assembly for focused
  representation-only fixtures.
- [ ] Keep normalization report field names, counts, order, occurrence data,
  checkpoints, and quiet gating unchanged while dumps expose the refined kind.
- [ ] Update deterministic fingerprints and goldens only for intentional MIR
  vocabulary changes, never for target output drift.

**Tests:** MIR dump snapshots; report and checkpoint order; x86-64 legality,
frame, selection, complete/sparse emission, assembler acceptance, byte-for-byte
focused assembly parity, and independent-process determinism.

**Exit criteria:** Users and tests can observe the stable final-MIR role, all
target artifacts remain unchanged for equivalent input, and no backend owner
reconstructs proof history.

### NSR6 — Complete source, profile, and malformed-MIR evidence

**Purpose:** Validate the refined contract over real lowering and the entire
selectable pipeline rather than only synthetic MIR.

- [ ] Add or extend focused source fixtures covering nested `&&`/`||`, `if` and
  `elif`, loops, methods, initializers, destructors, static initialization and
  shutdown, ownership cleanup, optionals, arrays, function values, checked
  failures, and direct panic.
- [ ] Compare default, `none`, logical-folding-disabled, every final CFG cleanup
  disabled individually, reachability-disabled, and all-pass-disabled modes.
- [ ] Pin source result, stdout, stderr, status, failure reason/span, runtime
  trace, lifecycle/destruction order, final MIR kind, and assembly equivalence.
- [ ] Add mutation tests for forged final-only storage, wrong stage, wrong type,
  source origin, leaked proof records/rvalues, invalid place/lifetime use, and
  an uninitialized ordinary scalar spill.
- [ ] Exercise debug, release, repeated in-process, and independent-process
  compilation with deterministic identities, dumps, reports, and artifacts.
- [ ] Confirm zero language, runtime ABI, pass-list, profile, or CLI changes.

**Tests:** Focused compiler suites and optimization/proof-normalization goldens;
default/off parity; debug/release native execution; deterministic artifact
checks; full compiler and golden regressions.

**Exit criteria:** Real programs prove the same success, failure, cleanup,
lifecycle, and target behavior across profiles, while every malformed boundary
case fails at its owning contract.

### NSR7 — Harden ownership, documentation, and roadmap closure

**Purpose:** Finish with one maintainable authority per concern and archive a
complete delivery record.

- [ ] Remove obsolete broad scalar-spill exceptions, migration adapters,
  rollout suppressions, stale comments, and duplicate classifiers.
- [ ] Confirm facade-oriented module ownership: model/contract,
  normalization, scalar initialization, rewrite, dump, and backend concerns
  remain separate with focused colocated tests.
- [ ] Update the living phase, backend, testing, reporting, and optimization
  catalog documentation from proposed to implemented behavior.
- [ ] Resolve the original proof-normalization discovery, retain any unrelated
  findings in this roadmap's discoveries record, and keep FMM-13 explicitly
  unimplemented.
- [ ] Run formatting, lints, compiler tests, golden determinism, release,
  MSRV, and long robustness checks without leaving generated artifacts.
- [ ] Mark every delivered checkbox, archive this roadmap and its resolved
  discoveries, and update both roadmap indices and all incoming links.

**Tests:** `make check`; full golden determinism; release and MSRV gates;
long robustness suite; documentation-link validation; clean-worktree artifact
audit excluding the intentional source and documentation changes.

**Exit criteria:** NSP1 through NSP14 are implemented and documented, every
required gate passes, no transitional owner remains, FMM-13 is still a
separate candidate, and the roadmap can be archived as the durable delivery
record.

## Ordering and completion

NSR0 establishes the complete contract before the enum changes. NSR1 makes the
representation exhaustive; NSR2 makes the mandatory normalizer its sole
producer; NSR3 then safely narrows initialization authority. NSR4 closes
transformation and analysis gaps, NSR5 proves observation and target parity,
NSR6 broadens semantic evidence, and NSR7 removes migration scaffolding and
closes documentation.

A task is complete only when its code, focused tests, documentation, and
checkboxes land together. Discoveries which do not block the frozen design go
to the companion record rather than expanding an active task.
