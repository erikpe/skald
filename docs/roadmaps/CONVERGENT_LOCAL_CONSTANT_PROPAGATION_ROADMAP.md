# Convergent Local Constant Propagation Roadmap

Status: in progress; CLR0 through CLR4 are complete and CLR5 is next.

This roadmap implements the frozen
[convergent local constant propagation design](../archive/CONVERGENT_LOCAL_CONSTANT_PROPAGATION_DESIGN_PROPOSAL.md).
It replaces depth-sensitive and protocol-local constant discovery with one
terminating callable-local solver, lets the existing primitive and checked
integer passes consume complete facts independently, and adds selectable
constant-left short-circuit CFG folding at the proof-normalization boundary.

The durable result is an optimizer foundation rather than a one-fixture fix:
supported constant expressions converge regardless of dependency depth,
checked topology is separated from value provenance, private carrier facts
have an explicit proof boundary, and logical proof records are consumed without
creating persistent optimizer provenance.

## Dependencies

- The completed
  [local final-MIR simplification roadmap](../archive/LOCAL_FINAL_MIR_SIMPLIFICATION_ROADMAP.md)
  provides the exact primitive evaluator, block-local facts, primitive folding,
  algebraic simplification, and conservative proof-rich CFG cleanup.
- The completed
  [checked integer constant protocol simplification roadmap](../archive/CHECKED_INTEGER_CONSTANT_PROTOCOL_SIMPLIFICATION_ROADMAP.md)
  provides exact division/remainder/shift evaluators, canonical checked-shape
  discovery, protocol rewrite semantics, and pass-owned metrics.
- The completed
  [proof-provenance normalization roadmap](../archive/PROOF_PROVENANCE_NORMALIZATION_ROADMAP.md)
  provides verified proof-rich and normalized seals, mandatory atomic
  normalization, and final-only CFG cleanup.
- The completed
  [selectable final-MIR pipeline roadmap](../archive/SELECTABLE_FINAL_MIR_OPTIMIZATION_PIPELINE_ROADMAP.md)
  provides the typed registry, schedules, exclusions, occurrence reporting,
  checkpoints, and atomic verified pass runner.
- The completed
  [dense MIR identity rewriting roadmap](../archive/DENSE_MIR_IDENTITY_REWRITING_ROADMAP.md)
  provides exhaustive identity traversal, sparse editing, deterministic dense
  commit, and structured stale-plan failures.
- Permanent whole-world compilation permits closed dependency inventories but
  does not make this solver interprocedural. Single-threaded generated programs
  remove concurrent mutation but do not weaken sequential alias, lifetime,
  ownership, failure, or evaluation-order barriers.
- Niflheim demonstrates the value of deterministic semantic optimization
  sequencing, but Skald's proof-rich MIR, two-seal boundary, checked protocols,
  and lifecycle verification remain authoritative.

## Scope and invariants

- Implement CLP1 through CLP18 without widening their frozen boundary.
- Solve constants to a unique stable result for every finite callable-local
  graph made only from supported primitive operations, certified checked
  carriers, successful checked integer protocols, and constant-selected
  logical relations.
- Use an iterative monotonic worklist with no expression-depth, wave, fuel, or
  pipeline-repetition limit.
- Reuse the implemented primitive and checked evaluators as the only arithmetic
  authorities; do not duplicate target-independent semantics in the solver.
- Separate exact checked and logical structural observations from constant
  availability and mutation plans.
- Propagate through only compiler-owned checked-protocol `ScalarSpill`
  carriers with exhaustive access, unique-write, dominance, type, lifetime,
  authorization, alias, protocol-owner, and seal-local certification.
- Treat every unsupported operation, ambiguous carrier, and statically failing
  evaluated check as a conservative result barrier.
- Do not require a skipped logical RHS to be pure or constant. Preserve left
  evaluation, skip the unselected RHS, and execute a selected RHS exactly once.
- Retain the stable `primitive-constant-folding` and
  `checked-integer-constant-folding` identities and add independently
  selectable `constant-short-circuit-folding`.
- Keep primitive assignment replacement, complete checked-protocol rewriting,
  and logical path selection under separate mutation owners.
- Add a narrow `ProofTransition` scheduling boundary accepting zero or one
  transition occurrence. Do not expose a generally mutable third MIR product.
- Compose an optional validated logical plan atomically with mandatory proof
  normalization and publish only verified final MIR.
- Keep mandatory normalization active and behaviorally identical under
  `none`, exclusions, and a logical no-op. Optional logical metrics and failures
  remain separately attributable.
- Rebuild every graph and plan from its current verified seal; add no analysis
  manager, preservation declaration, or cross-seal cache.
- Preserve source diagnostics, evaluation order, checked failures, panic spans,
  runtime traces, storage and ownership events, destruction, static activation
  and shutdown, ABI, and backend-visible behavior.
- Preserve deterministic identities, dense compaction, plan order, metrics,
  pass listing, checkpoints, dumps, and assembly.
- Leave newly unreachable regions and dead ordinary definitions to existing
  independently selectable cleanup passes.
- Exclude general load/store propagation, source-local mutation, right-constant
  logical identities with dynamic left operands, calls, floating arithmetic,
  SSA/SCCP, CSE, range analysis, inlining, target-specific optimization, and
  language-contract changes.
- Keep implementation behind concise private facades, colocate private tests
  with their semantic owners, and record unrelated discoveries separately.

## Progress

- [x] CLR0 — Separate structural protocol observations from constant eligibility
- [x] CLR1 — Certify checked-protocol scalar carriers
- [x] CLR2 — Build the convergent callable-local solver
- [x] CLR3 — Migrate primitive-family fact consumers
- [x] CLR4 — Rewrite dependent checked protocols from one solved snapshot
- [ ] CLR5 — Add the proof-consuming transition boundary
- [ ] CLR6 — Implement constant-left logical selection
- [ ] CLR7 — Complete selection, observation, and semantic evidence
- [ ] CLR8 — Harden ownership, documentation, and roadmap closure

## PR-sized implementation sequence

### CLR0 — Separate structural protocol observations from constant eligibility

**Purpose:** Establish exact immutable topology facts before carrier or solver
logic depends on them, while preserving all current optimization behavior.

- [x] Refactor checked-integer discovery so one structural observation records
  the verifier-owned check, success, failure, result store, join, reload,
  operand/result carriers, private load sites, spans, predecessors, and
  protected-root status without requiring literal operand constants.
- [x] Keep existing checked candidate behavior through an adapter which combines
  the structural observation with the current narrow constant-source rule.
- [x] Add a seal-local logical observation over each verified
  `MirLogicalExpression` and matching path condition, including operation,
  left/right/selected results, activation/result storage, split and selection
  predecessors, right entry/exit, short block, join, and spans.
- [x] Give observations deterministic definition/block/record order and owned
  identity values rather than borrowed mutable state.
- [x] Return structured failures for foreign, missing, duplicate, mismatched, or
  malformed identities; ordinary unsupported shapes remain conservative
  rejections.
- [x] Keep topology modules private behind the optimization facade and avoid
  adding mutation, solving, pass registration, or persistent MIR metadata.

**Tests:** Existing checked observer and folding suites unchanged; focused
structural observations for division, remainder, shifts, all logical
operations, nested logical records, methods, lifecycle definitions, and static
initializers; malformed and protected topology; deterministic repeated queries;
`cargo test --locked -p skald-compiler` and formatter/linter checks.

**Exit criteria:** Checked and logical topology can be queried without any
constant fact, current checked folding is behaviorally identical, every
identity is exact and deterministically ordered, and no observer can mutate or
outlive its verified snapshot.

### CLR1 — Certify checked-protocol scalar carriers

**Purpose:** Create the sole sound bridge for constants crossing the lowered
checked-protocol storage boundary before any broader solver uses storage.

- [x] Add one exhaustive callable-local storage-access census using shared MIR
  identity traversal and classify every read, write, authorization, projection,
  alias, attachment, ownership, call, lifetime, and future storage-bearing role.
- [x] Define an immutable carrier certificate naming the declaration, unique
  ordinary store, source value, exact eligible loads, type, protocol owner, and
  dominance/lifetime evidence.
- [x] Accept only checked-protocol-owned `ScalarSpill` declarations with exact
  base places, one unauthorized write, exact types, dominating store, compatible
  lifetime, no escape, no alias, and no unclassified access.
- [x] Reject generic spills, normalized former path-condition carriers,
  multi-write/read-before-store shapes, projections, authorization, attachments,
  ownership use, and cross-callable or stale identities.
- [x] Reuse canonical local CFG/dominance facts and make every future
  storage-bearing MIR variant fail an exhaustive maintenance test until
  classified.
- [x] Expose narrow read-only certificate queries only to the local constant
  analysis; do not introduce general store-to-load forwarding.

**Tests:** Valid operand/result carriers; same-block order and cross-block
dominance; each rejected access role; multiple and authorized stores;
projection/alias/call/ownership/attachment barriers; lifetime boundaries;
wrong kind/type/callable/seal; unrelated spills; deterministic census and
certificate ordering; full compiler tests plus static checks.

**Exit criteria:** Every propagated storage edge has one complete auditable
certificate, every other storage remains opaque, new storage roles cannot evade
classification, and no executable MIR behavior changes.

### CLR2 — Build the convergent callable-local solver

**Purpose:** Make supported constant discovery complete and terminating before
any production pass changes its rewrite behavior.

- [x] Add a private recursive `local_constant` facade separating graph,
  carrier, logical-transfer, solving, and focused test ownership.
- [x] Build dense `ValueId` and eligible `StorageId` nodes plus deterministic
  reverse dependencies from one verified callable snapshot.
- [x] Model supported literals, primitive rvalues, certified carrier transfers,
  structurally valid checked protocols, and exact logical selected-result
  relations without creating persistent MIR identities.
- [x] Reuse the exact primitive and checked evaluators and require exact result
  types, integer widths, wrapping behavior, boolean canonicalization, floor
  division, divisor-sign remainder, shift flavor, and byte canonicalization.
- [x] Implement the monotonic `Unknown -> Constant` worklist and the separate
  `Unselected -> SelectedShort | SelectedRight` logical state.
- [x] Make logical dependencies conditional: solve fixed short results without
  requiring the RHS, and require the right result only for a selected-right
  constant result.
- [x] Treat unsupported nodes and statically failing evaluated checks as
  barriers while allowing independent facts and failures inside a skipped RHS.
- [x] Detect contradictory derivations, invalid types, identities, or graph
  structure as deterministic internal analysis failures.
- [x] Expose immutable point queries, selection queries, provenance categories,
  and stable plan iteration; expose no queue or mutable graph API.

**Tests:** Primitive chains; alternating primitive/carrier/checked/logical
chains; fan-in/fan-out; seeded and unseeded cycles; unsupported leaves; all
integer widths and boundary semantics; static failures; all four constant-left
rules; selected dynamic RHS; skipped unsupported/failing RHS; generated depths
well beyond normal source nesting; permuted worklist seeds; monotonicity,
termination, deterministic solutions, and no Rust call-stack dependence.

**Exit criteria:** One analysis derives every fact promised by the frozen
closed-domain completeness rule in a single iterative solve, publishes no fact
across a barrier, and remains read-only and seal-local.

### CLR3 — Migrate primitive-family fact consumers

**Purpose:** Make the first production consumer use the shared solution and
retire duplicate constant reasoning without changing unrelated pass authority.

- [x] Migrate `primitive-constant-folding` to build one solution per verified
  callable and plan exact eligible ordinary assignment replacements from it.
- [x] Preserve result `ValueId`, declared type, instruction position, block,
  span, uses, operand evaluation, and current fold-family metrics.
- [x] Permit facts proven through certified carriers, successful checked
  protocols, and exact logical results even when those structural consumers are
  independently disabled.
- [x] Replace `PrimitiveConstantFacts` with the shared solution or a thin
  bounded view for algebraic simplification and conservative CFG cleanup; do
  not retain a second arithmetic/dataflow engine.
- [x] Preserve the reviewed same-block/use-role and proof-root restrictions of
  algebraic and conservative CFG transformations unless separately authorized.
- [x] Keep one immutable plan and at most one atomic callable/program commit per
  selected occurrence; rebuild facts after every changed seal.
- [x] Add stable propagated-provenance metrics without exposing worklist waves,
  queue operations, or graph size as semantic behavior.

**Tests:** Existing primitive/algebraic/CFG suites; arbitrary-depth primitive
and mixed chains; constants crossing eligible carriers and logical selections;
primitive-only selection with checked/logical syntax retained; unsupported and
failure barriers; spans/identities; no-op seal reuse; idempotence; deterministic
metrics and dumps; compiler tests and static checks.

**Exit criteria:** Primitive folding is expression-complete for the supported
graph at one seal, current algebraic/CFG semantics remain unchanged, and no
independent `PrimitiveConstantFacts` engine remains.

### CLR4 — Rewrite dependent checked protocols from one solved snapshot

**Purpose:** Let one checked pass occurrence fold arbitrarily nested successful
protocols without mutating and rediscovering intermediate waves.

- [x] Make checked folding combine structural observations with solver outcomes
  rather than requiring literal carrier-store assignments.
- [x] Plan every eligible successful division, remainder, and shift protocol
  against one immutable verified program snapshot.
- [x] Preserve static-failure and unsupported outcomes, exact failure reasons,
  source locations, evaluation timing, and independently foldable surrounding
  work.
- [x] Revalidate the complete multi-candidate plan, including carrier
  certificates and non-conflicting identity edits, before the first mutation.
- [x] Apply dependent candidates in stable order through one unpublished
  transaction and one deterministic dense commit; publish nothing on failure.
- [x] Preserve operand evaluation, result identity, result carrier/reload,
  source spans, and existing protocol rewrite granularity; leave storage and
  CFG cleanup to their owners.
- [x] Report propagated-operand folds separately from current direct folds while
  retaining established operation/failure and structural metrics.

**Tests:** `((8 / 2) + (7 % 3)) / 2`, `(1 << 2u) << 1u`, deep alternating
checked chains, multiple independent and dependent candidates, all operation
boundaries, inner and selected failures, stale/conflicting plans, rollback,
checked-only and primitive-disabled selection, idempotence, stable plan/metric
order, dense maps, proof verification, and native failure parity.

**Exit criteria:** Every statically successful checked protocol in the solved
supported graph folds in one checked occurrence regardless of nesting depth,
while failures and partial plans retain exact original behavior.

### CLR5 — Add the proof-consuming transition boundary

**Purpose:** Establish a typed place where optional proof-aware transformations
can safely consume logical records without weakening either verified seal.

- [ ] Extend `MirPassStage` and descriptor/listing vocabulary with
  `ProofTransition` between `ProofRich` and `Final`.
- [ ] Extend registrations with a distinct transition callback type and a
  pipeline-owned capability that can inspect `VerifiedProofMirProgram`, accept
  a narrowly typed optional normalization plan, and publish only
  `VerifiedFinalMirProgram`.
- [ ] Partition resolved schedules into proof-rich, zero-or-one transition, and
  final regions; reject repeats, misplaced stages, and callbacks whose declared
  identity/stage disagree.
- [ ] Refactor the runner so mandatory normalization executes exactly once
  through either the selected transition or the unchanged core path.
- [ ] Keep `none` and all-disabled schedules free of selectable occurrences but
  still normalized and final-verified exactly once.
- [ ] Define atomic error ownership, occurrence records, transition checkpoint
  labels, the established `after-proof-normalization` checkpoint, and absence
  of any observable partial state between them.
- [ ] Keep core normalization rules/statistics separate and prohibit raw MIR,
  a reusable third seal, general editor access, or multiple transition passes.

**Tests:** Registry and stage exhaustiveness; exact schedule partitions; zero
and one transition; repeated/misordered rejection; callback mismatch; `none`
and exclusion parity; normalization once; no-op pass-through; transition,
normalization, and final-verification failures; checkpoint/record absence on
failure; forged-capability compile-fail coverage; deterministic listing and
occurrence numbering.

**Exit criteria:** The pipeline can host one independently selected
proof-consuming occurrence and still exposes only verified proof-rich or final
products, while every schedule without it is byte-for-byte compatible with the
existing normalization path.

### CLR6 — Implement constant-left logical selection

**Purpose:** Materialize all four frozen short-circuit rules using exact proof
records and the transition capability.

- [ ] Add the stable `constant-short-circuit-folding` implementation and one
  immutable logical selection plan built from a fresh solver solution.
- [ ] For `false && rhs` and `true || rhs`, select the inactive predecessor and
  existing short block without evaluating or requiring purity from the RHS.
- [ ] For `true && rhs` and `false || rhs`, select the active predecessor and
  existing right entry while preserving the entire RHS exactly once.
- [ ] Always preserve left evaluation and the activation/lifetime protocol
  needed until mandatory normalization consumes it.
- [ ] Replace only the exact protocol-owned selected-result load when the
  solution supplies a constant, preserving its result identity, type, and span.
- [ ] Validate nested plans together in stable proof-record order and compose
  all logical edits atomically with the unchanged mandatory normalization plan.
- [ ] Leave unreachable blocks, stores, carrier declarations, lifetime work,
  and dead ordinary definitions to established final cleanup passes.
- [ ] Reject stale, overlapping, malformed, protected, foreign, or inconsistent
  plans with structured pass failure and no published final MIR.

**Tests:** Each rule with literal, derived, checked, and nested left facts;
dynamic/effectful/failing RHS on both selected and skipped paths; constant and
dynamic selected-right results; nested short circuits; methods, statics, and
lifecycle bodies; exact edge/result rewrites; plan conflicts and rollback;
dense identity maps; normalized verification; no persistent logical records;
idempotence through independently repeated compilation.

**Exit criteria:** Every exact constant-left logical record is selected in one
transition occurrence with source-equivalent evaluation and failure behavior,
and no proof-invalid intermediate or retained provenance is observable.

### CLR7 — Complete selection, observation, and semantic evidence

**Purpose:** Make the completed capability usable, independently diagnosable,
and proven from public selection through native execution.

- [ ] Add the logical pass to the production registry, public pass query,
  lexical CLI listing, known-name diagnostics, exclusions, and the default
  schedule immediately before mandatory normalization.
- [ ] Preserve existing pass identities and names, exact proof-rich order, and
  the final suffix; update expected default occurrence positions deliberately.
- [ ] Add logical selection/result metrics split by `&&`/`||` and short/right;
  keep normalization and later deletion metrics under their existing owners.
- [ ] Expose deterministic transition occurrence records and checkpoints plus
  the existing normalization checkpoint over the same final sealed product.
- [ ] Attribute logical analysis/plan/rewrite failures to the pass and unchanged
  core normalization failures to mandatory normalization.
- [ ] Add focused golden sources covering deep mixed expressions, all four
  short-circuit rules, skipped effects/failures, selected effects/failures,
  static initialization/shutdown, ownership/destruction, and function values.
- [ ] Compare default, `none`, each constant consumer disabled, logical-only
  exclusion, cleanup exclusions, and all-pass-disabled configurations.
- [ ] Pin native stdout/stderr/status, panic reason/location, runtime traces,
  final MIR, assembly-relevant effects, and cross-process deterministic
  fingerprints under debug and release compiler builds.
- [ ] Promote implemented behavior into compiler phase, driver, reporting, and
  testing documentation and update both catalog entries to **Implemented**.

**Tests:** Registry/schedule/CLI/driver/reporting suites; exact pass listing,
descriptions, stages, positions, exclusions, metrics, and checkpoint labels;
focused and full golden suites; debug/release equivalence; repeated independent
processes; compiler, CLI, documentation, and runtime tests.

**Exit criteria:** Users can discover and disable the logical pass independently,
all three consumers demonstrate complete supported facts, every observation has
one stable owner, and source-to-native results match optimization-off semantics.

### CLR8 — Harden ownership, documentation, and roadmap closure

**Purpose:** Finish with one maintainable analysis owner, exhaustive future-
variant defenses, current documentation, and repository-wide evidence.

- [ ] Audit graph, carrier, observation, plan, transition, normalization, and
  pass modules by responsibility; split substantial mixed owners and keep
  public/private facades narrow.
- [ ] Remove obsolete compatibility paths, duplicate constant engines, rollout
  wording, and roadmap codes from living code, tests, and architecture docs.
- [ ] Add exhaustive maintenance tests for every value/storage identity role,
  supported rvalue family, checked terminator, proof record, pass stage,
  callback kind, and transition outcome.
- [ ] Verify no analysis data or pre-commit identity survives a rewrite and no
  optimizer component depends on driver, reporting, filesystem, backend, or
  target state.
- [ ] Recheck deterministic behavior, no-op seal reuse, idempotence, rollback,
  optimization-off parity, lifecycle/static authority, backend input, and
  runtime-trace equivalence across the full repository corpus.
- [ ] Resolve small maintainability issues directly; record larger unrelated
  opportunities in the indexed discoveries file with evidence, owner,
  priority, and bounded later direction.
- [ ] Make living documentation authoritative, complete every roadmap checkbox,
  archive the roadmap, and archive or retain discoveries according to whether
  actionable items remain.
- [ ] Run `make check`, `make golden-determinism-test`,
  `make golden-release-test`, and `make msrv-check` from an artifact-free
  snapshot; run `make robustness-long` because pass policy and Rust compiler
  internals changed.

**Tests:** Every focused suite from earlier tasks; full repository gate; full
debug/release/determinism golden suites; supported-toolchain build; robustness;
documentation links/indexes; formatting and diff hygiene.

**Exit criteria:** All frozen decisions are implemented and documented, every
quality gate passes, the implementation has one clear owner per responsibility,
remaining discoveries are explicitly triaged, and the completed roadmap is
archived with no stale active status.

## Ordering and dependencies

CLR0 separates structural truth from current constant eligibility before CLR1
certifies storage. CLR2 can then build one solver over stable observations and
certificates without importing mutation concerns. CLR3 migrates the simpler
primitive consumers first and removes duplicate fact engines; CLR4 then changes
the more delicate multi-block checked transaction with the solver already
proven.

CLR5 establishes and parity-tests the proof-consuming transition before CLR6
uses it to alter logical CFG. CLR7 changes public selection, the default
schedule, reporting, and source-to-native behavior only after all internal
semantic boundaries are independently tested. CLR8 performs the final ownership
and exhaustive-variant audit and closes the documentation after implementation
evidence exists.

No task should add extra operation families merely because the solver can host
them. New floating, memory, effect, alias, right-constant logical, SSA, or
target-specific work belongs in the optimization candidate catalog or the
roadmap discoveries record.

The ordinary repository gate is `make check`. Tasks that change Rust targets,
manifests, or supported syntax also run `make msrv-check`; the closing task runs
the extended deterministic and release golden gates explicitly. The Makefile
remains the local and external automation interface; this roadmap adds no
repository CI.
