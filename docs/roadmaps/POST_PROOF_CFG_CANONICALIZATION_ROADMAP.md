# Post-Proof CFG Canonicalization Roadmap

Status: in progress; PCR0 through PCR4 are complete and PCR5 is next.

This roadmap implements the frozen
[post-proof CFG canonicalization design](POST_PROOF_CFG_CANONICALIZATION_DESIGN_PROPOSAL.md).
It adds deterministic predecessor-edge facts and two independently selectable
normalized final-MIR passes: empty-block forwarding and basic-block merging.
Together they establish the first reusable post-proof CFG canonicalization
layer beyond unreachable-region deletion, while retaining a narrow mutation
capability and all current language, lifecycle, failure, trace, and ABI
semantics.

Implementation-specific opportunities outside the frozen scope belong in the
[post-proof CFG canonicalization discoveries](POST_PROOF_CFG_CANONICALIZATION_DISCOVERIES.md).
The
[optimization candidate catalog](OPTIMIZATION_CANDIDATE_CATALOG.md) owns
concise cross-domain status for FMC-08 and FMC-09.

## Dependencies

- The completed
  [proof-provenance normalization roadmap](../archive/PROOF_PROVENANCE_NORMALIZATION_ROADMAP.md)
  provides distinct proof-rich and normalized seals, stage-aware execution,
  normalized verification, final-stage capability ownership, and the
  conservative unreachable-block canary.
- The completed
  [dense MIR identity rewriting roadmap](../archive/DENSE_MIR_IDENTITY_REWRITING_ROADMAP.md)
  provides exhaustive identity traversal, private sparse edits, explicit block
  order, atomic transactions, and deterministic dense commit.
- The completed
  [selectable final-MIR pipeline roadmap](../archive/SELECTABLE_FINAL_MIR_OPTIMIZATION_PIPELINE_ROADMAP.md)
  provides registrations, profiles, exclusions, exact internal schedules,
  inspection, measurements, verification, and failure attribution.
- The completed
  [whole-world reachability roadmap](../archive/TARGET_INDEPENDENT_WHOLE_WORLD_REACHABILITY_ROADMAP.md)
  provides seal-bound analysis recomputed after every changed final-stage
  occurrence and the final executable-definition retention pass.
- Completed static-lifecycle certificate and reachability-gated static
  lifecycle work make initialization-exit and cleanup-entry attachments
  explicit permanent semantic roots.
- The open normalized scalar-spill provenance finding is not a dependency.
  This roadmap neither creates, deletes, substitutes, nor reasons through
  storage.
- Niflheim supplies evidence for deterministic transitive empty-block
  forwarding and cycle retention. Skald's normalized MIR, permanent roots,
  dense identities, and separate block-merging pass remain authoritative.

## Scope and invariants

- Implement PCG1 through PCG14 without widening their frozen boundary.
- Extend the shared CFG snapshot with deterministic successor and predecessor
  edge occurrences, preserving multiplicity and exhaustive terminator
  maintenance coverage.
- Keep CFG facts short lived. Do not add persistent edge identities, cross-pass
  caches, preservation declarations, or a global analysis manager.
- Add `post-proof-empty-block-forwarding` and
  `post-proof-basic-block-merging` as distinct `Final` passes with stable
  identities, names, descriptions, metrics, and exclusions.
- Forward only non-entry, unattached, instruction-free goto blocks whose chain
  resolves outside an empty cycle and whose incoming terminators are not
  permanent-attachment barriers.
- Merge only an exact goto predecessor with a distinct non-entry successor
  having one total incoming executable edge occurrence, with neither endpoint
  permanently attached.
- Preserve every executable instruction, effectful terminator, value, storage,
  operation span, checked edge role, and dynamic execution order.
- Preserve body entry identity and treat every static-publication or future
  permanent attachment as a hard mutation barrier.
- Keep candidate selection explicit. Verification is the final trust boundary,
  not a trial-and-error eligibility algorithm.
- Extend `MirFinalCfgEdit` only with guarded compound operations authorized by
  current normalized CFG facts. Do not expose raw mutable MIR or the general
  callable editor to final passes.
- Commit each pass through one atomic whole-program rewrite and publish no
  partial result after any failure.
- Resolve all eligible forwarding chains in one occurrence and merge
  deterministically to a local fixed point. Each pass is independently
  idempotent.
- Run the default final suffix as unreachable-block deletion, empty-block
  forwarding, basic-block merging, and whole-world reachability.
- Recompute normalized verification and seal-bound reachability after each
  changed pass; unchanged passes retain their seal.
- Preserve deterministic dense compaction, ordering, dumps, measurements,
  listing, checkpoints, and assembly.
- Preserve optimization-off acceptance, diagnostics, native output, exit
  status, panic text, runtime traces, static lifecycle, ownership,
  destruction, and ABI behavior.
- Exclude proof-rich canonicalization, branch folding, jump threading,
  duplication, critical-edge splitting, protocol rewrites, loop transforms,
  storage changes, scalar propagation, SSA, inlining, target layout, and
  repository CI.
- Keep Rust module facades concise and implementation-private tests beside
  their semantic owner.

## Progress

- [x] PCR0 — Add deterministic predecessor-edge CFG facts
- [x] PCR1 — Define normalized canonicalization candidates
- [x] PCR2 — Add guarded final-CFG compound edits
- [x] PCR3 — Implement selectable empty-block forwarding
- [x] PCR4 — Implement selectable basic-block merging
- [ ] PCR5 — Freeze and prove default pass composition
- [ ] PCR6 — Complete inspection and reporting ownership
- [ ] PCR7 — Prove source-level semantic and target equivalence
- [ ] PCR8 — Harden ownership, documentation, and roadmap closure

## PR-sized implementation sequence

### PCR0 — Add deterministic predecessor-edge CFG facts

**Purpose:** Give deletion, forwarding, merging, and future CFG consumers one
canonical structural view before new mutation is authorized.

- [x] Add a closed callable-local edge fact containing source, target, and a
  stable successor occurrence or equivalent exhaustive edge role.
- [x] Extend every block fact with ordered successor and predecessor edge
  occurrences while retaining existing value-definition, entry, root,
  reachability, and unreachable queries.
- [x] Preserve duplicate successor occurrences rather than collapsing them to
  predecessor-block sets.
- [x] Derive dense-definition and sparse-edit facts through one shared builder
  in source block and successor occurrence order.
- [x] Expose entry and permanent-attachment classification plus the minimum
  immutable instruction-count and terminator-shape facts required by candidate
  analysis.
- [x] Keep proof-rich and normalized root contracts explicit; normalized facts
  continue rejecting consumed proof roots.
- [x] Add exhaustive maintenance coverage requiring every future terminator
  successor form and permanent block attachment to receive a classification.
- [x] Preserve current CFG consumers and diagnostics unless the edge vocabulary
  provides a strictly clearer error.

**Tests:** Focused `mir::rewrite::cfg` dense/edit parity tests; every goto,
branch, checked, optional, array, loop, return, panic, and terminate shape;
duplicate branch targets; deterministic predecessor order; entry and static-
publication roots; malformed references and consumed-proof failures; existing
proof-rich and post-proof CFG pass suites; formatter and linter.

**Exit criteria:** Every live executable edge occurs exactly once in ordered
CFG facts, predecessor multiplicity is exact and deterministic, dense and
sparse queries agree, new MIR variants cannot evade classification, and no
mutating behavior has changed.

### PCR1 — Define normalized canonicalization candidates

**Purpose:** Make the frozen eligibility proof independently reviewable and
measurable before connecting it to structural edits.

- [x] Add immutable empty-forwarding candidate and resolved-plan vocabulary
  scoped to normalized CFG facts.
- [x] Identify only non-entry, unattached, zero-instruction goto blocks whose
  target is distinct and whose incoming edge sources are not permanent-root
  mutation barriers.
- [x] Resolve chains deterministically to the first non-forwardable target and
  retain self-loops, cycles, and chains entering cycles with explicit reasons.
- [x] Add immutable block-merge candidates for exact goto pairs with one total
  incoming successor occurrence and neither endpoint permanently attached.
- [x] Count all live edges, including entry-unreachable regions, when proving
  merge uniqueness.
- [x] Select the first merge candidate by current block order and define
  deterministic rescan behavior after a future edit.
- [x] Return stable opportunity and barrier counts without retaining raw MIR
  references or mutable state.
- [x] Keep target-converged branch folding, protocol simplification,
  duplication, and storage reasoning outside both queries.

**Tests:** Single and transitive forwarding; multiple predecessors; duplicate
edges; body entry; permanent roots and incoming permanent-root edges;
instruction-bearing blocks; self-loops and cycles; linear merge chains; branch
predecessors; entry successor; two-block loops; unreachable regions;
deterministic repeated queries; no mutation.

**Exit criteria:** Read-only normalized facts classify every reviewed candidate
and barrier deterministically, forwarding terminates without choosing cycle
representatives, merge selection proves one incoming edge occurrence, and
neither query changes MIR.

**Implementation evidence:** `mir::rewrite::cfg::canonicalization` owns the
immutable forwarding and merge analyses behind the existing CFG facade.
`MirFinalCfgFacts` is an opaque normalized-only wrapper, so proof-rich facts
cannot authorize post-proof candidates accidentally. Focused tests cover
transitive plans, explicit cycle barriers, permanent attachments, duplicate
and unreachable incoming edges, valid two-block loops, deterministic rescan,
repeatability, and absence of MIR mutation.

### PCR2 — Add guarded final-CFG compound edits

**Purpose:** Authorize exactly the two frozen transformations without turning
the final-stage capability into general mutable MIR access.

- [x] Add a guarded operation accepting one complete resolved forwarding plan
  and verify its exact current snapshot before mutation.
- [x] Redirect every incoming successor occurrence to its resolved target while
  preserving terminator kind, operands, role, and span.
- [x] Remove only planned instruction-free blocks and prove no entry,
  attachment, executable edge, or transient value still refers to them.
- [x] Add a guarded operation which rechecks one merge pair, appends successor
  instructions, transfers its exact terminator, and removes the successor.
- [x] Preserve all value and storage declarations and references; add only the
  minimum private editor primitive needed to move complete block contents.
- [x] Reject stale facts, changed shapes or roots, invalid cycles, and foreign
  or deleted identities with structured rewrite failures.
- [x] Keep raw programs, general callable edits, sparse slots, and unrestricted
  instruction or terminator replacement hidden from final passes.
- [x] Prove failure publishes no partial callable/program and success compacts
  blocks deterministically.

**Tests:** Capability access boundaries; successful forwarding and merging;
exact span and terminator preservation; block-local value movement; body-entry
retention; static-publication barriers; stale and foreign identities; induced
commit failure; atomic rollback; dense maps; deterministic order; normalized
verification.

**Exit criteria:** The capability performs only the two reviewed compound
edits, rechecks every barrier, preserves executable contents, fails atomically,
and exposes no general mutable MIR surface.

**Implementation evidence:** The private final-CFG canonicalization capability
accepts only a complete analyzed forwarding plan or one analyzed merge
candidate. It validates every supplied identity, requires exact current
normalized facts, and recomputes the complete candidate analysis before any
mutation. Forwarding reuses exhaustive successor-role rewriting; merging uses
one private sparse-editor primitive which moves complete successor contents
without changing value or storage declarations. Focused tests cover transitive
and duplicate-role redirection, spans and terminators, instruction ordering,
entry and permanent-publication barriers, stale, foreign, deleted, incomplete,
and cyclic inputs, dense maps, normalized verification, and atomic rollback
after an induced commit failure.

### PCR3 — Implement selectable empty-block forwarding

**Purpose:** Deliver FMC-08 and exercise controlled edge redirection over
normalized final MIR.

- [x] Add a cohesive module with a stable identity, exact
  `post-proof-empty-block-forwarding` name, `Final` stage, and frozen
  description.
- [x] Scan borrowed verified definitions first and preserve the seal when no
  candidate exists.
- [x] Apply each callable's complete resolved plan through the guarded
  capability and one atomic program transaction.
- [x] Resolve every eligible chain in one occurrence and prove independent
  idempotence when block merging is absent.
- [x] Report processed/changed callables, removed forwarding blocks, redirected
  occurrences, retained cycles, and permanent-root barriers.
- [x] Register and list the pass, add it after post-proof unreachable deletion
  in default, and support stable-name disabling.
- [x] Confirm changed output is normalized-reverified with fresh reachability
  while unchanged output retains its seal.
- [x] Update current pipeline and driver documentation without claiming block
  merging is implemented.

**Tests:** Productive/no-op cases; transitive chains; ordinary and protocol
predecessor roles; multiple predecessors; loops and cycles; static initializers;
entry-unreachable regions with the canary disabled; metrics; listing; default
and disabled schedules; checkpoints; idempotence; failure attribution.

**Exit criteria:** The pass removes exactly eligible forwarding blocks in one
occurrence, never changes a permanent-root terminator, reports deterministic
reasons, reseals every change, and is independently selectable in default.

**Implementation evidence:** The registered
`post-proof-empty-block-forwarding` final-stage pass snapshots every executable
callable through borrowed normalized facts, resolves complete plans before
invalidating the seal, and applies them through one atomic guarded CFG rewrite.
Changed products are normalized-reverified with fresh reachability; global
no-op execution retains the seal. The pass reports removed blocks, redirected
successor occurrences, cycle retention, and permanent-attachment barriers and
runs between post-proof unreachable deletion and whole-world reachability.
Focused tests cover transitive and duplicate edges, multiple predecessors,
checked protocol roles and spans, functions, members, static initializers,
entry-unreachable regions, ordinary loop latches, cycles, stable selection,
checkpoints, metrics, and idempotence. Full-suite testing found and directly
fixed an additional exact eligibility barrier: verifier-significant empty
array-loop bodies remain attached rather than being forwarded.

### PCR4 — Implement selectable basic-block merging

**Purpose:** Deliver FMC-09 and validate ordered movement of executable block
contents under the normalized contract.

- [x] Add a cohesive module with a stable identity, exact
  `post-proof-basic-block-merging` name, `Final` stage, and frozen description.
- [x] Scan borrowed verified definitions first and preserve the seal on a
  no-candidate result.
- [x] Repeatedly select the first current pair, apply the guarded merge,
  rebuild facts, and stop when no pair remains.
- [x] Prove termination by one deletion per step and independent idempotence
  when forwarding is absent.
- [x] Preserve instruction order, successor terminator/span, value/storage
  declarations, and block-local use-before-definition.
- [x] Report processed/changed callables, merged pairs, moved instructions,
  removed blocks, multiple-edge barriers, and permanent-root barriers.
- [x] Register and list the pass, insert it after forwarding in default, and
  support independent stable-name disabling.
- [x] Update pipeline and driver documentation for both passes.

**Tests:** One pair and maximal chains; instruction-bearing successors;
body-entry predecessor; return, panic, terminate, checked, optional, array,
ownership, and cleanup sequences; branch/duplicate-edge barriers; multiple
predecessors; publication roots; two-block loops; unreachable regions;
metrics, listing, selection, idempotence, compaction, and verification.

**Exit criteria:** The pass deterministically merges all eligible pairs to a
local fixed point, preserves executable and value/storage invariants, and is
independently selectable after forwarding in default.

**Implementation evidence:** The registered
`post-proof-basic-block-merging` final-stage pass first inspects borrowed
normalized definitions and retains the existing seal for a global no-op. A
productive occurrence repeatedly rebuilds normalized CFG facts, selects the
first current pair, and invokes the guarded compound edit until every callable
converges, then publishes one atomic program rewrite. Each step removes one
block while preserving instruction order, exact operation and terminator
spans, and all value/storage identities. Stable measurements cover merged
pairs, moved instructions, removed blocks, multiple incoming edges, and
permanent attachments. Focused tests cover maximal chains, unreachable
regions, two-block loops, functions, members, static initializers, selection,
listing, checkpoints, metrics, dense compaction, fresh reachability, and
repeated-occurrence idempotence. Full-suite verification identified and
directly encoded one additional exact form of frozen rule 7: checked scalar,
optional-shared unwrap, and array-loop successor blocks whose complete shape
is verifier-significant remain structural protocol attachments rather than
being merged with their following block.

### PCR5 — Freeze and prove default pass composition

**Purpose:** Freeze the complete final-stage schedule and prove both passes
remain modular when composed, excluded, or repeated.

- [ ] Freeze default final order as post-proof unreachable deletion,
  forwarding, merging, and whole-world reachability.
- [ ] Prove each pass alone through exact schedules and each disabled
  independently from default.
- [ ] Cover both disabled, the unreachable canary disabled, reachability
  disabled, duplicate exclusions, and all final passes disabled.
- [ ] Confirm disabling every default name is equivalent to `none` while
  mandatory normalization still runs once.
- [ ] Demonstrate forwarding exposes merging without hidden shared state.
- [ ] Verify no frozen-rule alternating case requires another default
  occurrence; amend the design if contrary evidence appears.
- [ ] Prove repeated exact occurrences are unchanged after convergence and do
  not trigger redundant verification.
- [ ] Ensure whole-world reachability consumes remaining call sites and stays
  the last target-independent pass.

**Tests:** Registry and schedule suites; exact occurrence identities/stages;
all exclusion combinations; normalization and verification counts; fresh
reachability binding; checkpoints; productive composition fixtures;
independent-process schedule determinism.

**Exit criteria:** One deterministic default schedule owns composition, both
passes remain independently selectable and convergent, no hidden fixed-point
loop exists, and reachability sees canonicalized CFG.

### PCR6 — Complete inspection and reporting ownership

**Purpose:** Make structural and semantic reasons observable without
duplicating rewrite accounting or exposing unverified MIR.

- [ ] Freeze both descriptors in the public query and
  `--list-mir-passes` output in stable-name order with `Final` stage.
- [ ] Emit deterministic productive and barrier metrics in stable first-owner
  and counter order.
- [ ] Keep generic rewrite summaries authoritative for entity changes and
  avoid conflicting duplicate counters.
- [ ] Add stage-bearing after-pass checkpoints containing only verified final
  products and exact occurrence identities.
- [ ] Attribute analysis, stale-plan, rewrite, and output-verification failures
  to the exact pass, identity, stage, schedule position, and occurrence.
- [ ] Preserve deterministic dumps and reports across processes and selection
  combinations.
- [ ] Update reporting, driver, phase, and testing documentation with exact
  metrics, checkpoints, listing, and selection behavior.

**Tests:** CLI listing and unknown-name inventory; request adapters; details and
trace reporting; zero/productive/barrier counters; rewrite summaries;
checkpoint labels/seals; synthetic failures; cross-process output; docs links.

**Exit criteria:** Users can distinguish forwarding from merging, understand
productive and retained candidates, inspect only verified products, and
reproduce stable reports and dumps.

### PCR7 — Prove source-level semantic and target equivalence

**Purpose:** Validate the equivalence argument through real lowering, static
lifecycle, tracing, backend lowering, and native execution.

- [ ] Add golden fixtures producing transitive empty chains and single-entry
  instruction-bearing merge chains after normalization.
- [ ] Cover functions, methods, static initializers, loops, cleanup, checked
  success/failure, optional, array, shared ownership, calls, return, panic, and
  hard termination.
- [ ] Cover joins, empty cycles, body entry, and publication boundaries which
  must remain unchanged.
- [ ] Add variants for default, `none`, forwarding disabled, merging disabled,
  both disabled, and post-proof unreachable deletion disabled.
- [ ] Compare native output, status, panic text/location, destruction order,
  static startup/shutdown, and runtime-trace rows.
- [ ] Assert productive MIR block/jump and relevant assembly reduction without
  making target spelling the semantic oracle.
- [ ] Measure corpus opportunities and reductions and record only supported
  conclusions.
- [ ] Update backend and testing contracts with implemented equivalence and
  coverage.

**Tests:** Focused goldens/variants; native and traced runs; panic/termination;
MIR and assembly observations; existing simplification, normalization,
reachability, lifecycle, backend, and runtime suites; full debug golden run.

**Exit criteria:** Both passes reach the backend productively, barrier fixtures
remain valid, all selections are natively equivalent, traces and failures
match, and corpus measurements are evidence-backed.

### PCR8 — Harden ownership, documentation, and roadmap closure

**Purpose:** Audit the delivered layer as reusable infrastructure and close the
roadmap without stale status, hidden mutation, or high-priority debt.

- [ ] Audit CFG facts, candidate analysis, editing, final capability, passes,
  policy, execution, inspection, reporting, driver, and backend by
  responsibility; split owners where that materially improves maintenance.
- [ ] Confirm no final pass gained raw mutable MIR, general callable editing,
  storage/proof mutation, or unrestricted instruction/terminator replacement.
- [ ] Remove stale claims that post-proof CFG work only deletes unreachable
  blocks or that reachability immediately follows normalization.
- [ ] Ensure maintenance tests catch new terminator edges, permanent block
  attachments, and trace events requiring a design decision.
- [ ] Remove roadmap/decision codes from living source, tests, diagnostics,
  dumps, metrics, and public compiler documentation.
- [ ] Resolve small findings and record larger follow-ups with evidence,
  impact, owner, priority, and bounded direction in discoveries.
- [ ] Promote implemented status in the catalog and living contracts and
  remove superseded discovery wording.
- [ ] Run complete repository and supported-toolchain gates from an artifact-
  free snapshot.
- [ ] Mark all tasks complete, archive roadmap/design, update indexes and
  incoming links, and leave only actionable discoveries active.

**Tests:** Every focused prior suite; `make check`; full golden/native tests;
independent-process determinism; release golden tests; docs links/indexes;
formatter; linter; Rust MSRV; artifact-free snapshot.

**Exit criteria:** PCG1 through PCG14 are authoritative, FMC-08/FMC-09 are
implemented, every invariant is covered, only actionable follow-ups remain,
planning records are archived, and the worktree contains only intentional
delivered changes.

## Ordering and dependencies

PCR0 establishes exact edge multiplicity and roots before PCR1 states
candidates. PCR1 keeps eligibility read only before PCR2 grants mutation. PCR2
establishes capability and atomicity once, so PCR3 and PCR4 remain small pass
owners rather than duplicating structural checks.

PCR3 delivers forwarding first because it removes arbitrary-predecessor empty
indirection and exposes linear edges. PCR4 then merges maximal single-incoming
chains. PCR5 freezes composition after both passes exist. PCR6 settles
observation against the final schedule, PCR7 proves end-to-end behavior, and
PCR8 performs ownership and artifact-free closure rather than a documentation-
only tail.

Candidate and guarded-edit fixtures may be prepared together within PCR1/PCR2,
but mutation must not precede capability checks. The passes share facts and
safe operations, never hidden state. Reporting assertions may grow alongside
each pass, but PCR6 freezes the complete vocabulary.

The root Makefile remains the repository and external automation interface.
This roadmap adds no repository CI.
