# Low-Level IR Model, Construction, and Verification Roadmap

Status: planned, 2026-09-17; LI01 is next. Implements LA02 of the
[low-level compiler architecture program](LOW_LEVEL_COMPILER_ARCHITECTURE_DESIGN_PROPOSAL.md).
The [model design](LOW_LEVEL_IR_MODEL_DESIGN_PROPOSAL.md) is accepted, frozen
and promoted as of 2026-09-17.

Planning baseline: `10b5de75`, the committed draft. Implementation baseline:
not started; record the last commit before LI01's first code change here.
Program implementation baseline remains `495debd3`. The preparation closing
commit is `027af94f`; the preserved pre-migration measurement compiler is
`9e3cebb1`. These have different purposes and must not be substituted for one
another. The working tree was clean before this planning change.

This workstream delivers executable low-level models, checked construction and
editing, independent verification, immutable publication and inspection. Its
synthetic target consumers establish the shared selection interface before
native target selection and placement are implemented. Completion supplies
usable contracts and tests to LA03, without changing production emission.

## Scope and invariants

- Implement the frozen design's distinct lowered and selected representations.
  Reuse graph algorithms, not MIR execution enums, physical x86 instruction
  storage or one universal stage enum.
- Bind products to an immutable live context, target and exact snapshot.
  Callable/stage-local typed IDs, checked arena access, verifier-owned seals,
  completion receipts and consuming edits preserve authority.
- Values have one definition; entry inputs, block parameters and simultaneous
  edge arguments express value flow. Semantic MIR storage remains symbolic
  addressable memory, with no value home or concrete frame offset.
- Implement the complete lowered vocabulary and the three finite scalar-domain
  relations, including checked constant evidence. No opaque lifecycle operation,
  general proof engine, semantic SSA conversion or scalar promotion.
- Make logical call roles, service effects, attribution, trace actions and typed
  artifact dependencies explicit. Preserve evaluation/destruction order, scalar
  canonicalization, runtime layout and enabled/omitted trace behavior.
- Preserve final-MIR authority, sparse executable bodies, certified statics,
  stable dispatch and complete/reachable artifact policy. No second semantic
  reachability calculation, absent-body reconstruction or assembly parsing.
- Selected payloads expose opcode-derived operands, timing, ties, clobbers,
  resource views, ABI bindings and bounded bundles/recipes. All CFG edges,
  calls and temporaries are visible before placement.
- Keep the public backend facade, target registry, diagnostics and production
  x86 emission unchanged. Real planning/lowering, x86 opcodes, physical ABI,
  transfers, placement/checking, frames and native emission belong to LA03/LA04.
  Production adoption and legacy removal remain LA05; allocation remains LA06.
- Synthetic targets are permanent test fixtures, not registered backends or
  unchecked emission paths. No AArch64 backend, placeholder placer, public LIR
  importer/cache, generic pass manager or dormant CLI/production switch.

The design remains the authoritative schema and semantic contract. This roadmap
partitions implementation; file names may follow cohesive ownership rather than
the illustrative directory sketch.

## Progress

- [ ] LI01 — Checked contexts, declarations and identity domains
- [ ] LI02 — Lowered scalar/object model and draft construction
- [ ] LI03 — Calls, effects, tracing and terminal operations
- [ ] LI04 — CFG and single-definition verification
- [ ] LI05 — Scalar domains, memory checks and lowered publication
- [ ] LI06 — Generated inventories and complete-program authority
- [ ] LI07 — Selected payload and resource description contracts
- [ ] LI08 — Selected verification and portability witnesses
- [ ] LI09 — Consuming edits, remaps and snapshot-bound analyses
- [ ] LI10 — Immutable inspection and deterministic phase dumps
- [ ] LI11 — Cumulative review, cleanup, downstream handoff and closure

## Owners and durable outputs

| Responsibility | Starting owner / authority | Durable output |
| --- | --- | --- |
| Public verified input and trace-source policy | [Backend facade](../../crates/skald-compiler/src/backend/mod.rs), [backend guide](../compiler/BACKEND.md) | Unchanged public paths; narrow private checked fact views |
| Graph and value flow | [Frozen model design](LOW_LEVEL_IR_MODEL_DESIGN_PROPOSAL.md#cfg-and-value-flow-rules) | Shared traversal/dominance utilities over separate stage storage |
| Lowered execution and checks | [Operation/domain contract](LOW_LEVEL_IR_MODEL_DESIGN_PROPOSAL.md#lowered-values-operations-and-memory) | Closed model, checked builders and independent verifier/seals |
| Generated artifacts and receipts | [Inventory contract](LOW_LEVEL_IR_MODEL_DESIGN_PROPOSAL.md#inventories-and-generated-work) | Canonical worklist, finalized stage inventories and snapshot-bound completion |
| Selected operands and resources | [Selected interface](LOW_LEVEL_IR_MODEL_DESIGN_PROPOSAL.md#selected-graph-and-target-facing-structural-interface) | Borrowed structural descriptions, verifier and test-only target adapters |
| Phase dependencies | [Boundary tests](../../crates/skald-compiler/tests/phase_boundaries.rs) | Narrow maintained guards for new graph/model/verifier scopes |
| Inspection and model tests | New private phase owners; [testing guide](../development/TESTING.md) | Read-only views, deterministic dumps and colocated malformed fixtures |
| Migration accounting | [Program coverage/handoff](LOW_LEVEL_COMPILER_MIGRATION_COVERAGE.md) | Implemented model evidence distinguished from pending native delivery |

Keep `mod.rs` files as concise facades with selective visibility. Shared graph,
model and verification owners cannot import frontend state, MIR executable enums,
source lookup or physical target types. Nominal IDs and source spans may provide
attribution without granting semantic/source database access. Reuse typed arena
and test support facilities where their invariants fit; do not add a parallel
backend runner, corpus or dependency scanner.

## Checkpoints and quality gates

1. **Common contract readiness (LI01):** map the frozen signature, inventory,
   operand/resource/timing, flags/edge, object/scratch and inspection/error
   agreements to the inherited x86/AArch64 walkthroughs. Record common schema
   choices and the concrete decisions still owned by LA03. A representable
   witness permits model work; a mismatch blocks only its dependent work until
   an explicit owning-design amendment resolves it. Do not choose x86 defaults.
2. **Lowered publication readiness (LI05/LI06):** publish only after all mandatory
   per-callable checks pass; complete-program authority additionally requires
   finalized inventories and matching receipts. Earlier tasks may implement
   private draft storage and checker components, but cannot mint provisional
   seals, dummy receipts or successful results from incomplete verification.
3. **Selected interface readiness (LI08):** both structural and synthetic target
   checks must pass the worked portability cases. If the schema cannot express a
   required constraint, amend it explicitly before publication/downstream work.
   Passing synthetic tests does not clear native ABI, recipe or placement checks.
4. **Closure readiness (LI11):** reconcile the full implementation with the frozen
   design and ledger, run the final gates and update the program handoff.
   A substantial missing contract needs an explicit implementation task before
   closure; an independent improvement becomes a discovery.

These are correctness/readiness checkpoints, not performance experiments.
LA02 has no cost-based abandonment decision and cannot clear foundation adoption.
At a failed checkpoint, leave its checkbox open, describe the evidence and fix
or amend the affected contract. Never hide a failure behind legacy fallback.

For each implementation task, run focused owner tests, `make check` and
`make msrv-check`. The ordinary gate covers formatting, all-target build/lint,
documentation, workspace/runtime tests and source goldens; the MSRV gate covers
supported compilation of the new Rust APIs. Run additional focused tests on
the supported toolchain where syntax/API behavior warrants it. Use the existing
Makefile interface rather than creating CI or another quality-gate runner.

Malformed implementation-private fixtures stay with the model/verifier owner.
Public boundary/privacy tests use existing integration or compile-fail patterns.
Keep source observations in their owning goldens and reusable non-Rust corpora
under the top-level test tree. Tests must challenge independently constructed
invalid products, not merely repeat builder decisions.

Production is unchanged here. If a task unexpectedly changes production
lowering, ABI, tracing or artifacts, first review the scope change and require
the applicable extended native/determinism/release gates before acceptance.
The [measurement procedure](../development/LOW_LEVEL_COMPILER_MEASUREMENTS.md)
and baseline remain intact; eleven short compile timing gates are inconclusive.
Future old/new comparisons need one compatible harness, and LA05 owns cost
acceptance. Do not benchmark test-only models as evidence of native adoption.

## PR-sized implementation sequence

### LI01 — Checked contexts, declarations and identity domains

**Purpose:** establish the immutable execution facts and ownership boundaries
needed by every model consumer.

- [ ] Record the implementation baseline from current history before changing
  code. Inspect prior preparation commits and protect unrelated user changes.
- [ ] Record the common contract readiness result in the program handoff,
  covering all six [joint questions](LOW_LEVEL_IR_MODEL_DESIGN_PROPOSAL.md#joint-review-and-promotion-checkpoint).
  Distinguish accepted common schemas from pending concrete target decisions.
- [ ] Implement private target/profile and policy facts, execution scalar types,
  checked layout/signature component declarations, typed callable/artifact keys
  and distinct lowered/selected local ID domains.
- [ ] Supply borrowed narrow context views, checked `usize` arenas and live
  context identity checks. Fixture plans validate supplied facts; do not claim
  a production plan has been projected from final MIR.
- [ ] Exercise declarations and lookup with real checked fixture contexts;
  extend maintained phase guards for the scopes actually introduced.
- [ ] Document implemented facts and ownership without announcing a running
  LIR pipeline. Add modules only when they contain a tested responsibility.

**Tests:** same facts in two simultaneously live contexts; wrong context/target,
foreign owner/stage, unknown/category-mismatched artifact, invalid layout,
signature roles and overflow/range rejection. Cover unit/metadata disposition,
hidden destinations and receiver/origin roles from the inherited witnesses.
Run the task quality gates.

**Exit criteria:** checked declarations and identity lookups have genuine
consumers, deterministic ordering and no physical location dependency; the
common readiness result explicitly lists LA03's remaining obligations.

### LI02 — Lowered scalar/object model and draft construction

**Purpose:** make executable computation and addressable storage distinct and
constructible without assigning a stack home.

- [ ] Implement callable/block/value/object storage, entry inputs, definition
  sites, ordered results, explicit edge occurrences and the draft builder.
- [ ] Implement the closed scalar/address/load/store/lifetime schemas and
  finite check/evidence records; reuse the neutral comparison descriptor.
- [ ] Reserve forward blocks/parameters, append typed instructions and terminate
  blocks with checked construction errors for duplicate/post-termination writes.
- [ ] Preserve explicit widths, alignments, binary64 bits and object roles.
  Represent zero-size addressable objects and elided unit/metadata separately.
- [ ] Provide private malformed-fixture construction for independent verifier
  testing. Keep drafts distinct from verified products; no seal exists yet.

**Tests:** scalar/cast schema cells, byte/boolean canonical constants, signed
wrapping semantics, negative zero/NaN bit identity, nonzero entry IDs,
reservations, duplicate definitions, width/stride overflow and lifetime sites.
Construct straight-line and loop/diamond drafts. Run the task quality gates.

**Exit criteria:** the scalar/object vocabulary and meaningful drafts are
represented without MIR executable types, physical registers or offsets.
Guard records are obligations, not assumed successful proofs.

### LI03 — Calls, effects, tracing and terminal operations

**Purpose:** complete observable executable meaning before verifying publication.

- [ ] Implement direct and signature-typed indirect calls, logical components,
  six attribution meanings and shared entry/call/return signature rules.
- [ ] Implement conservative checked service records, mandatory effects and
  address provenance. Optional annotations may widen effects only.
- [ ] Implement ordered trace actions/records and explicit reported/nonreturning
  service termination versus hard trap, with no implicit unwind or cleanup.
- [ ] Enforce builder-level component/policy checks while retaining independent
  verifier obligations for malformed fixtures.
- [ ] Construct the shared-release acceptance graph from ordinary memory,
  branches, finalizer/free calls and the original header value. Include enabled
  and omitted tracing drafts, not an opaque helper opcode.

**Tests:** aggregate destination with no scalar result, receiver/origin triples,
indirect signature mismatch, secured earlier operands across later calls,
unknown aliasing, narrowed store/call effects, all attribution categories and
omitted-policy trace references. Run the task quality gates.

**Exit criteria:** all frozen lowered instruction/terminator families have
explicit schemas and meaningful call/effect/trace consumers; no operation
recovers its executable meaning from MIR or a service-name string.

### LI04 — CFG and single-definition verification

**Purpose:** establish independently checked graph/value structure shared by
both stages.

- [ ] Implement checked structural traversal, reachability and dominance with
  one verification session rather than repeated graph reconstruction per use.
- [ ] Validate ownership/bounds/reservations before indexing, exact definition
  sites/results/types, terminators, entry rules and ordered edge signatures.
- [ ] Check same-block ordering, reachable dominance and predecessor edge-use
  positions. Preserve simultaneous swaps/cycles and distinct edge occurrences.
- [ ] Check unreachable structure/local ordering; return unknown for analysis
  cross-block dominance queries involving unreachable blocks.
- [ ] Emit canonical structured stage/callable/local failures. Keep graph checks
  as components of full verification, not a public verified-phase constructor.

**Tests:** independent malformed tables, duplicate/unresolved definitions,
foreign/dangling IDs, wrong parameter arity/type, successor-only uses,
nondominating values, unreachable and newly reachable uses, loops, same-target
branch edges and critical edges. Run the task quality gates.

**Exit criteria:** both sound and invalid graph fixtures exercise one shared
algorithm owner; graph success alone cannot publish a lowered/selected seal.

### LI05 — Scalar domains, memory checks and lowered publication

**Purpose:** complete per-callable verification and introduce genuine immutable
lowered authority.

- [ ] Verify scalar/cast legality and canonical forms, each finite check relation,
  exact operand identity, success-edge protection and rooted unreachable-region
  obligations. Validate direct constant evidence independently.
- [ ] Check memory widths/alignments, known object extents/provenance, mandatory
  effects, signatures/service contracts and trace/reference associations.
- [ ] Compose all mandatory checks before consuming a draft into a verified
  callable. Keep seal construction private to verification; add exact snapshot
  witnesses and verifier-produced completion receipts.
- [ ] Provide ordered failures and private diagnostic conversion preserving the
  existing public `BackendError` shape and source/generated identity distinction.
  Test conversion without wiring a fabricated production phase.
- [ ] Remove any construction/verification transitions due here and document
  the actual publication API and its structural/upstream/native proof boundary.

**Tests:** secured divisor versus reload, bypass/failure-path use, disconnected
and unreachable guard regions, wrong width/target, stale evidence; constants at
zero/nonzero, 7/8, 63/64 and independently computed truncated-float range
boundaries, NaNs/infinities. Challenge narrowed effects, out-of-object accesses,
fake unit results and omitted trace data. Add seal/read-only privacy checks and
deterministic reason/location assertions. Run the task quality gates.

**Exit criteria:** every mandatory per-callable lowered check is active.
Verified meaningful graphs and genuine receipts exist; invalid inputs fail
without unchecked constructors, indexing panics or runtime/source-error fallback.

### LI06 — Generated inventories and complete-program authority

**Purpose:** extend valid individual callables to a finalized, context-bound
program without retaining every intermediate body.

- [ ] Implement canonical declared/building/verified worklists and staged
  inventory finalization over already checked declaration catalogs.
- [ ] Predeclare recursive helper keys; prohibit reentering a building body,
  conflicting definitions and synthesizing absent semantic bodies.
- [ ] Reconcile required definitions, typed dependencies and exact snapshot-bound
  completion/derivation receipts at complete-program publication.
- [ ] Implement parent-bound target declaration extension/freeze rules. Existing
  layout/signature facts cannot be replaced; new facts require replanning.
  LI07 adds resource/ABI catalogs and LI08 completes selected publication;
  do not introduce placeholder descriptors or a provisional selected seal here.
- [ ] Preserve authorized external/null dispatch dispositions and existing
  complete-mode helper roots. Demonstrate streaming completion bookkeeping
  without a requirement to retain every predecessor-stage body.
- [ ] Update the program handoff with model inventory evidence and pending
  production planner/helper/target-discovery ownership.

**Tests:** mutual recursion, duplicate/conflicting/unfinished requests, unknown
keys, absent-body references, category/addend errors, legal externals/null slots,
equal numeric IDs in foreign contexts, stale edited-body receipts, wrong input
witness and target extensions attempting to overwrite parent facts. Randomize
request arrival to challenge canonical inventories. Run the task quality gates.

**Exit criteria:** complete-program authority proves actual inventory closure
with genuine receipts. Test catalogs exercise the contract; production discovery
and helper generation remain explicitly pending.

### LI07 — Selected payload and resource description contracts

**Purpose:** expose target requirements through a narrow shared structural view.

- [ ] Implement separate selected storage/builders and fresh IDs using the
  shared graph utilities; preserve origins through explicit stage remaps.
- [ ] Implement borrowed opcode-derived operand/tie/clobber/effect/reference
  descriptions, checked representations, resource banks/views/overlap units,
  reservations and partial-preservation descriptors.
- [ ] Implement frozen early/late event ordering, component ABI bindings,
  symbolic ABI areas and atomic/bounded recipe descriptors with declared scratch.
- [ ] Add test-only two-address and three-address payloads with call, fixed,
  early-clobber and flag-bundle cases. Derive descriptions from their payloads;
  do not separately edit use/def lists or introduce real x86 opcodes.
- [ ] Record readiness against each joint counterpart and extend boundary guards
  for the new shared/target-facing owners.

**Tests:** ordered descriptors, overlapping narrow/full views across banks,
reserved resources, call clobber then result definition, tied distinct value IDs,
hidden destination/receiver ABI roles and bounded scratch/bundle edge exposure.
Run the task quality gates.

**Exit criteria:** structural consumers can inspect meaningful synthetic selected
graphs without matching target enums or reading MIR. No selected seal is minted
until both verification layers are available in LI08.

### LI08 — Selected verification and portability witnesses

**Purpose:** verify the selected interface independently before native adapters
or placement rely on it.

- [ ] Compose shared context/graph/value/effect checks with descriptor/resource/
  representation/ABI/bundle consistency checks and a target-verifier hook.
- [ ] Require both shared and target checks for immutable selected publication;
  bind input snapshot, selection extension and generated-thunk receipts.
- [ ] Complete all six frozen worked cases in synthetic targets: live tied input,
  guarded diamond with correction blocks, loop/swap/critical edges, hidden
  destination and mixed pressure, release/trace omission, resource extensions.
- [ ] Challenge descriptor mistakes independently of normal payload constructors.
  Expose target-created CFG and temporaries, and reject leftover lower-stage IDs.
- [ ] Record selected readiness and the exact real x86/AArch64 obligations still
  owned by LA03. Do not claim structural tests prove physical preservation.

**Tests:** malformed slots/ties/timing/resource footprints, incompatible fixed
views, invalid ABI roles, missing bundle references/scratch, omitted-trace
dependencies, wrong target/context and failed target verification. Test a live
input after a destructive tie and partial-width preservation descriptions with
two distinct synthetic target shapes. Run the task quality gates.

**Exit criteria:** synthetic selected products satisfy both verification layers;
the shared interface has portability witnesses and no target-specific defaults.
No placeholder placement/checker or production target registration exists.

### LI09 — Consuming edits, remaps and snapshot-bound analyses

**Purpose:** support safe transformation without stale publication or analysis.

- [ ] Implement consuming edit/rebuild APIs for both stage products: operand
  replacement, edge/argument redirection, block splitting and explicit remaps.
- [ ] Update definitions, check evidence, references and origins coherently,
  then require full reverification before downstream use.
- [ ] Consume affected complete-program authority when replacing/editing bodies;
  produce fresh receipts and reconcile finalization with the replacement.
- [ ] Provide read-only phase-local analysis views borrowing the exact immutable
  snapshot. No global cache, generic pass manager or placement stub.

**Tests:** positive splits/swaps/correction-block remaps and independently broken
remaps, dangling deleted IDs, a reachable formerly dead block, changed guarded
operand and stale complete-program receipt. Use compile-fail/privacy patterns to
challenge mutation and reuse of consumed authority. Run the task quality gates.

**Exit criteria:** transformations either republish fully verified new snapshots
or fail; old analyses/receipts cannot certify changed bodies.

### LI10 — Immutable inspection and deterministic phase dumps

**Purpose:** make the verified model inspectable through its owning phase.

- [ ] Implement read-only visitors and canonical lowered/selected text renderers
  with stage/schema/profile headers and complete ordered structural facts.
- [ ] Print float bits, widths, effects, guards, attribution, edge occurrences,
  resource/timing constraints and typed artifacts; exclude host/context tokens,
  timing values and unordered output.
- [ ] Keep best-effort malformed-draft rendering visibly separate and panic-free.
- [ ] Test independent-process dump determinism with existing child-process
  testing patterns and owner fixtures; avoid new public CLI or importer plumbing.
- [ ] Update living backend/inspection/test guidance for implemented private
  APIs and add linked model documentation as needed. Keep actual production
  phase events, public dump adapters and requested metrics delivery pending.

**Tests:** canonical dumps of the six worked cases, binary64 special bits,
generated inventories with varied insertion order, equivalent distinct live
contexts, malformed draft rendering and independent process repetitions.
Confirm views cannot grant mutable, MIR or source-database access.
Run the task quality gates.

**Exit criteria:** genuine phase-owned consumers inspect both verified models
deterministically; documentation distinguishes available APIs from pending
production execution and observation.

### LI11 — Cumulative review, cleanup, downstream handoff and closure

**Purpose:** assess the complete model as one change and prepare its closing
commit for the user.

- [ ] Review implementation-baseline-to-`HEAD` diff, stat and name-status plus
  staged, unstaged and untracked work. Inspect earlier task commits and relevant
  program changes from `495debd3`; a clean tree is not cleanup evidence.
- [ ] Reconcile every artifact-ledger entry against source/history, using symbol
  searches and `git log -S` where useful. Search beyond the ledger for duplicate
  graph/check logic, aliases, escape hatches, unused abstractions, broad exports,
  allowances and stale rollout/task names.
- [ ] Remove expired scaffolding and make small final coherence fixes. Resolve
  substantial missing model contracts before closure; record unrelated
  discoveries with evidence, owner, priority and later implementation boundary.
- [ ] Confirm meaningful consumers and negative fixtures cover every frozen
  operation, verification layer and worked case. Audit snapshot identity,
  immutable payloads and program receipts across task boundaries.
- [ ] Update the program handoff with actual APIs, test evidence, readiness
  results and explicit LA03 target/streaming/placement obligations. Transfer any
  justified future-removal artifacts with exact symbols and owners; keep the
  program coverage record active.
- [ ] Reconcile living docs, parent design, A22/audit and indexes. Keep native
  migration/adoption/allocation pending. Do not change baseline measurements
  or grant cost clearance.
- [ ] After fixups, run `make check` and `make msrv-check` from an artifact-free
  snapshot/clean checkout containing the final uncommitted work. Record source
  identity, commands/results, baseline/endpoint and residual changes awaiting
  the user's commit.
- [ ] Mark completion only after all gates/exit criteria pass; archive this
  roadmap and the frozen child design, update both indexes and repair inbound
  relative links. Keep the parent program and migration coverage active.
- [ ] Check the final cumulative/closing diffs, documentation and whitespace.
  Leave committing to the user.

**Tests:** final artifact-free ordinary and supported-toolchain gates after all
fixups, plus documentation/link/whitespace checks after archival.

**Exit criteria:** the accepted model is implemented, tested and reviewed
cumulatively; expired scaffolds are removed, justified retained artifacts have
explicit continuing purposes, and LA03 has an accurate executable-model handoff.
No unsupported claim of a native LIR pipeline or allocator is made.

## Ordering and dependencies

LI01 settles fact/identity ownership before construction. LI02/LI03 complete
schemas before LI04/LI05 compose full per-callable verification. LI06 then adds
program/inventory authority. LI07/LI08 build and verify separate selected
products, followed by consuming edits and inspection in LI09/LI10. LI11 closes
the cumulative change. Keep this order; shared graph helpers may be reused
across stages without granting either stage the other's payload or authority.

The LA03 design may proceed alongside model work against these frozen contracts,
but its dependent implementation needs the relevant checked APIs and readiness
results. It must freeze concrete opcode/resource/ABI/transfers/frame contracts
and supply a real target verifier, independently checked baseline placement and
end-to-end native pilots. A target mismatch requires an owning-design amendment,
not a shared-core x86 dependency or an emitter-only workaround.

## Temporary artifacts and discoveries

No compiler scaffolding is introduced by this planning change. Maintain this
ledger during implementation, including committed artifacts:

| File / symbol | Introducing task / commit | Removal or transfer owner | Final disposition / retention criterion |
| --- | --- | --- | --- |
| None yet | Planning only | — | — |

Ledger draft-only adapters, exploratory fixtures, aliases, gates, instrumentation
and lint allowances as they arise. Genuine draft builders and synthetic
malformed/portability fixtures are durable APIs/tests, not provisional verifier
success paths. Any retained fixture must protect a named final contract.

The shared model should compile in ordinary builds; only synthetic targets and
malformed fixture support are test-only. Do not gate the whole model under
`cfg(test)` or broaden public visibility to suppress pre-consumer warnings.
If production-unwired private APIs require temporary dead-code allowances,
limit them to the necessary symbols, justify and ledger them, and assign removal
to LA03's first real consumer. LI11 transfers outstanding obligations to the
program handoff rather than forgetting them at child archival. No blanket lint
allowance or empty placeholder API is justified.

Use `LOW_LEVEL_COMPILER_ARCHITECTURE_DISCOVERIES.md` for the first substantial
new independent finding, and index it when created; do not create an empty
discoveries file. Implement small maintainability fixes directly within the
responsible task when they preserve the reviewed scope.
