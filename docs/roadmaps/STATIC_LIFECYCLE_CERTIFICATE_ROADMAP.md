# Static-Lifecycle Certificate Redesign Roadmap

Status: in progress; LCR0 is complete and LCR1 is next.

This roadmap implements the frozen
[static-lifecycle certificate design](../archive/STATIC_LIFECYCLE_CERTIFICATE_DESIGN_PROPOSAL.md)
and its promoted
[compiler phase contract](../compiler/PHASES_AND_IR.md#frozen-static-lifecycle-certificate-direction).
It replaces exact cross-phase effect-graph equality with an exact
pre-optimization authority and a monotone final-MIR realization check, then
removes the redundant lifecycle schema and ownership boundaries that would
otherwise burden every future MIR optimization.

The roadmap deliberately lands no production optimization. Its durable result
is a verified final-MIR boundary on which effect-removing, target-narrowing, and
graph-reshaping passes can later rely.

## Scope and invariants

- Keep eager whole-program static activation, deterministic topological order,
  and exact-reverse shutdown unchanged.
- Keep `STA001` self-dependency and `STA002` cycle acceptance, wording, labels,
  notes, witness choice, and deterministic ordering independent of optimization
  selection.
- Continue deriving every call and lifecycle target conservatively from the
  complete closed-world MIR product under analysis.
- Introduce normalized lifecycle effect facts containing target field, access
  kind, root phase, and lifecycle-owned destination status.
- Issue immutable baseline authority with exact root coverage from verified
  preliminary MIR.
- Verify final MIR by requiring each lifecycle root's realized normalized facts
  to be a subset of its baseline authority and safe under the frozen plan.
- Permit removal, target narrowing, inlining, and other call-graph reshaping;
  reject new target-field, access-kind, phase, or lifecycle-owned facts.
- Keep the baseline authority unavailable for mutation by MIR passes.
- Keep function-value candidate discovery as a closed-world analysis input;
  do not use the lifecycle certificate as a callable-retention contract.
- Keep target-independent effect analysis, proof issuance, and realization
  checking out of the x86-64 backend.
- Store one activation order and derive shutdown, lifecycle positions, and
  required dependencies.
- Store only structured activation and destruction regions in executable MIR;
  provide flat transitions only as derived inspection output when useful.
- Move call graphs, candidate inventories, SCC counts, solved summaries, spans,
  and witnesses to pass-owned analysis or planning-report products.
- Preserve publication dominance, post-publication cleanup, destination
  non-escape, ordinary MIR ownership and lifetime verification, and backend ABI
  behavior.
- Preserve deterministic MIR, analysis, and planning dumps, while intentionally
  updating their schema to identify baseline authority separately from
  analysis evidence.
- Replace repeated declaration scans with explicit canonical maps where the
  schema migration already touches construction or verification.
- Keep `mod.rs` files as concise facades and split implementation by analysis,
  planning, verification, synthesis, proof, coordinator, and phase-product
  responsibility.
- Add no runtime guard, lazy initialization, synchronization, atomics, language
  alias restriction, SSA conversion, dense-ID rewrite, optimization-level CLI,
  general pass registry, production transformation, or target-specific
  optimization.
- Keep the root Makefile as the automation interface; add no repository CI.

## Progress

- [x] LCR0 — Establish normalized root-effect analysis
- [ ] LCR1 — Issue exact immutable baseline authority
- [ ] LCR2 — Verify monotone final-MIR realization
- [ ] LCR3 — Separate analysis evidence from executable proof
- [ ] LCR4 — Reorganize lifecycle module ownership
- [ ] LCR5 — Canonicalize planned lifecycle data
- [ ] LCR6 — Canonicalize the executable coordinator
- [ ] LCR7 — Seal phase products and publish the optimization boundary

## PR-sized implementation sequence

### LCR0 — Establish normalized root-effect analysis

**Purpose:** Add the semantic comparison unit and a checker-oriented
root-reachability implementation without changing the current certificate or
pipeline behavior.

- [x] Add a pass-owned normalized fact type whose equality includes target
      field, access kind, propagated root phase, and lifecycle-owned status but
      excludes span, witness, directness, edge kind, and intermediate node.
- [x] Inventory lifecycle roots from static definitions and stored types,
      covering explicit initializers and every implicit class, optional,
      shared-owner, and array operation used by activation or destruction.
- [x] Build a checker-oriented closure over the existing extracted graph that
      derives the normalized facts reachable from each requested lifecycle
      root without trusting `StaticEffectAnalysis` summaries.
- [x] Reuse the exhaustive instruction, terminator, place, cleanup, copy,
      finalization, array, virtual, interface, and indirect-call extraction
      owners; do not add a second MIR scanner.
- [x] Preserve exact-signature function-value target expansion from all
      address formations in the MIR product being analyzed.
- [x] Canonically sort and deduplicate roots and facts, reject foreign
      identities, and keep phase propagation explicit at the first call or
      lifecycle edge from an initializer root.
- [x] Compare the new root facts and derived dependency pairs with the current
      solved summaries across the existing lifecycle fixture matrix, while
      leaving `MirStaticLifecycleCertificate` unchanged.
- [x] Keep derived `Debug` output on normalized roots, facts, and errors for
      actionable test disagreements; a separate user-facing dump is not needed
      while the analysis remains an internal compatibility oracle.

**Tests:** Direct and transitive reads, writes, borrows, replacement, and
destruction; initializer before/after-publication partitioning; lifecycle-owned
destination access; callable recursion and SCCs; constructors, copy,
assignment, finalization, temporary and optional cleanup, shared-owner release,
and nested array lifecycle; virtual, interface, and exact-signature indirect
targets; generic classes and arrays; deterministic order across repeated runs;
equivalence with current dependency pairs for accepted programs.

**Gates:** `cargo test --locked -p skald-compiler static_lifecycle`;
`cargo test --locked -p skald-compiler --test pipeline_determinism`;
`make fmt-check`; `make lint`; `make docs-check`; and `git diff --check`.

**Exit criteria:** Every current lifecycle root has a deterministic normalized
fact set produced independently of stored transitive summaries, all implicit
operation families and dynamic target kinds are covered, and existing
certificate construction, diagnostics, final MIR, and backend output remain
unchanged.

**Completed:** Planning now computes sorted, deduplicated normalized facts for
inventoried initializer and destruction roots by walking the raw extracted
graph independently of solved summaries. Compatibility assertions cover both
root facts and derived dependency pairs while the exact certificate remains
unchanged. Focused tests cover phases, lifecycle-owned access, access kinds,
recursive paths, implicit optional/shared/array destruction, indirect/virtual/
interface targets, malformed identities, and determinism. The existing solver
also keeps lifecycle ownership in its witness representative key so the oracle
cannot collapse distinct semantic facts. All focused gates and the complete
compiler test suite pass.

### LCR1 — Issue exact immutable baseline authority

**Purpose:** Make normalized root facts the exact proof issued by planning
before any final-MIR comparison is relaxed.

- [ ] Add compact MIR-owned authority and per-root records with private
      construction, sorted unique storage, read-only iteration, and no
      optimization-facing mutation API.
- [ ] Have planning construct authority for exactly the lifecycle roots implied
      by every static definition and its stored type.
- [ ] Split certificate verification into an issuance checker that independently
      extracts preliminary MIR, recomputes root facts, and requires exact
      authority equality.
- [ ] Derive required lifetime dependency pairs from authority, definitions,
      and the existing lifecycle destination/published-self rules.
- [ ] Check every derived pair against activation order and retain all current
      self-dependency and cycle decisions from the pre-optimization planner.
- [ ] Temporarily retain the old analysis certificate and stored dependencies
      as a compatibility oracle, requiring both forms to agree until later
      migration removes them.
- [ ] Add malformed-planned-MIR builders local to verifier tests for missing,
      extra, duplicate, foreign-root, foreign-field, wrong-access, wrong-phase,
      and wrong-lifecycle-owned authority mutations.
- [ ] Extend planned dumps with an explicit deterministic baseline-authority
      section without yet removing current analysis sections.
- [ ] Update the compiler contract to mark exact baseline authority issuance as
      implemented while leaving exact final graph equality documented as the
      temporary final boundary.

**Tests:** Exact accepted authority for explicit and zero-default statics;
root coverage for initializer-free destructible fields; every malformed
authority mutation; exact derived dependency agreement; `STA001`/`STA002`
diagnostic and witness parity; stable planned dumps; public read-only authority
inspection; no certificate constructor or mutator exposed publicly.

**Gates:** Focused static-lifecycle plan and verifier tests;
`cargo test --locked -p skald-compiler --test public_api`;
`cargo test --locked -p skald-compiler --test pipeline_determinism`;
`make compiler-test`; `make docs-check`; `make msrv-check`; and
`git diff --check`.

**Exit criteria:** Planned-MIR verification proves exact root authority and
derives every required dependency independently, malformed authority is
rejected, current exact final verification still passes, and source diagnostics
and executable output are unchanged.

### LCR2 — Verify monotone final-MIR realization

**Purpose:** Remove the optimization fence by switching final lifecycle
verification from exact graph identity to the frozen root-effect relation.

- [ ] Add a distinct final realization checker that extracts the final program,
      re-derives closed-world targets, computes lifecycle-root facts, and
      requires each fact set to be a subset of baseline authority.
- [ ] Require exact final root coverage for lifecycle work that remains
      contractually required by the static definitions; do not authorize whole
      static declaration or lifecycle-root pruning in this roadmap.
- [ ] Independently derive realized dependencies and validate them against the
      frozen activation order, self/publication rules, and lifecycle-owned
      destination constraints.
- [ ] Remove exact final equality requirements for direct effects, target
      edges, node inventory, source spans, witnesses, and address-taken
      candidate inventory.
- [ ] Keep ordinary `verify_mir`, coordinator coverage, publication dominance,
      cleanup legality, destination non-escape, and exact-reverse shutdown
      checks unchanged.
- [ ] Add verifier-local transformed fixtures that remove an unreachable
      static access, remove a dead effectful call, narrow dynamic and indirect
      targets, and replace a call path with an equivalent inlined root effect.
- [ ] Add negative transformed fixtures for a new target field, access kind,
      phase, lifecycle-owned destination access, missing surviving indirect
      target, and unsafe realized dependency.
- [ ] Prove that all current unoptimized programs realize authority exactly at
      synthesis, while the same verifier accepts the authorized reductions.
- [ ] Update the compiler contract to make the subset realization relation the
      implemented final-MIR trust boundary.

**Tests:** Positive removal, narrowing, and graph-reshaping mutation tests;
negative unauthorized-fact and unsafe-order tests; direct/virtual/interface/
indirect call cases; pre/post-publication motion; recursive call graphs;
implicit destruction roots; optimized-shaped/unoptimized planned-dump and
diagnostic parity; final verification through the ordinary MIR pipeline and
backend entry.

**Gates:** Focused extraction, verifier, synthesis, pass-pipeline, and backend
tests; `cargo test --locked -p skald-compiler --test pipeline_determinism`;
`make compiler-test`; `make golden-filter GOLDEN_FILTER='static_fields/**'`;
`make docs-check`; `make msrv-check`; and `git diff --check`.

**Exit criteria:** Final MIR may remove or reshape authorized lifecycle effects
without certificate edits, every new normalized root fact is rejected, the
frozen plan is checked directly against realized dependencies, and no
production optimization has been introduced.

### LCR3 — Separate analysis evidence from executable proof

**Purpose:** Retire the transitional exact graph certificate and give analysis,
diagnostics, reporting, and MIR proof data clear owners.

- [ ] Introduce a `StaticLifecyclePlanningReport` or equivalently explicit
      sidecar owning the extracted graph, exact-signature candidate index,
      solved per-node summaries, recursive-component metric, selected
      dependency evidence, and deterministic witnesses.
- [ ] Keep that sidecar in the planned phase product for inspection and drop it
      during synthesis so it never reaches backend-consumable final MIR.
- [ ] Remove `StaticEffectAnalysis`, graph edges, candidate inventories, SCC
      counts, access spans, and cloned witness paths from
      `MirStaticLifecycleCertificate` and replace that type with the compact
      baseline authority or rename it to reflect proof semantics.
- [ ] Remove duplicate dependency evidence from executable MIR; derive proof
      pairs from authority and keep diagnostic paths only in the planning
      report.
- [ ] Update `dump_static_effects`, `dump_static_lifetime_plan`, and
      `dump_planned_mir` to consume the correct analysis, planning, or phase
      product and label authority separately from evidence.
- [ ] Deliberately migrate the repository-internal public inspection API and
      its compile test; do not preserve misleading `planned.effects()` or MIR
      re-exports solely for compatibility.
- [ ] Remove transitional dual-certificate agreement code and mutation hooks
      that no longer represent a production invariant.

**Tests:** Planning-report ownership and deterministic rendering; synthesis
drops analysis-only data; final MIR dump contains compact authority but no SCC
statistics, candidate-retention inventory, or cloned witnesses; public API
compile coverage for intentional facade paths; malformed compact proof tests;
equivalent diagnostics and backend output.

**Gates:** Focused static-lifecycle and MIR dump tests;
`cargo test --locked -p skald-compiler --test public_api`;
`cargo test --locked -p skald-compiler --test pipeline_determinism`;
`make compiler-test`; `make fmt-check`; `make lint`; `make docs-check`;
`make msrv-check`; and `git diff --check`.

**Exit criteria:** Final MIR carries only compact baseline authority, analysis
and evidence have one pass-owned sidecar, public paths reflect those ownership
boundaries, deterministic inspection remains available, and no production
consumer reads retired graph-certificate data.

### LCR4 — Reorganize lifecycle module ownership

**Purpose:** Move the now-separated models to their semantic owners and leave
concise facades before further plan and coordinator representation changes.

- [ ] Move direct effect graphs, nodes, edges, candidate inventories, solved
      summaries, SCC results, access evidence, and witness utilities under the
      static-lifecycle analysis implementation.
- [ ] Keep normalized authority and lifecycle-root identities in the MIR model
      because they cross the planning, optimization, verification, and backend
      phase boundaries.
- [ ] Split the large static-lifecycle MIR model into responsibility-oriented
      `proof`, `plan`, `coordinator`, and phase-product modules.
- [ ] Keep `mir::model::static_lifecycle` and `passes::static_lifecycle` as
      concise facades with minimal intentional re-exports; consumers must not
      depend on private file layout.
- [ ] Remove the circular-looking pass re-exports of MIR-owned analysis types
      and update imports to name the actual semantic owner.
- [ ] Keep deterministic comparison helpers next to the analysis/evidence
      values they order rather than in a generic MIR model facade.
- [ ] Keep substantial unit tests beside their implementation owners and
      cross-phase/public API tests in the crate integration-test directory.
- [ ] Preserve behavior, dumps, verifier errors, public supported paths, and
      phase-product sizes exactly during this physical reorganization.

**Tests:** Static-lifecycle analysis, planning, synthesis, and verification unit
tests after module movement; compile-time public facade coverage; deterministic
analysis and planned dumps; no private module path imported outside its owner;
unchanged pipeline and backend behavior.

**Gates:** `cargo test --locked -p skald-compiler static_lifecycle`;
`cargo test --locked -p skald-compiler --test public_api`;
`cargo test --locked -p skald-compiler --test pipeline_determinism`;
`make fmt-check`; `make lint`; `make docs-check`; `make msrv-check`; and
`git diff --check`.

**Exit criteria:** Analysis, compact proof, planning, coordinator, and phase
products each have one clear module owner behind concise facades, no behavior
or supported public path changed unintentionally, and the subsequent schema
tasks can edit their owner without reopening unrelated analysis modules.

### LCR5 — Canonicalize planned lifecycle data

**Purpose:** Replace the redundant plan, indices, and dependency encodings with
one definition inventory and one activation order before coordinator synthesis
is simplified.

- [ ] Define one canonical lifecycle definition table keyed by stable static
      field identity and require exact unique coverage of declarations.
- [ ] Store one activation-order vector covering definitions exactly once;
      expose shutdown as a reverse iterator or derived view.
- [ ] Remove stored shutdown order from `StaticLifecyclePlan` and eliminate
      stored activation/shutdown indices from static declarations and lifecycle
      definitions.
- [ ] Provide one checked or verifier-local position map for consumers that
      need order comparisons or numeric dump output.
- [ ] Remove stored lifetime dependency vectors from the planned schema and
      derive pairs from authority and definitions at planning verification and
      inspection boundaries.
- [ ] Stop constructing flat activation and shutdown transitions in planned
      MIR; retain source spans on definitions and initializer publication
      boundaries that can derive executable transitions later.
- [ ] Build declaration, definition, initializer, and position indexes once,
      replacing repeated linear `static_fields().find(...)` scans.
- [ ] Preserve deterministic topological tie breaking and exact source
      diagnostics even though the accepted product stores less derived data.
- [ ] Update planned dumps with derived shutdown positions, dependencies, and
      transitions only where those views remain useful.

**Tests:** Missing, duplicate, and foreign definitions/order entries;
activation coverage; reverse shutdown iteration; derived position and
dependency agreement; explicit and zero-default activation spans; final-field
requirements; deterministic order and dumps; unchanged `STA001`/`STA002`
goldens; generic static definitions.

**Gates:** Focused plan/schema/verification tests;
`cargo test --locked -p skald-compiler --test pipeline_determinism`;
`make compiler-test`; `make golden-filter GOLDEN_FILTER='static_fields/**'`;
`make docs-check`; `make msrv-check`; and `git diff --check`.

**Exit criteria:** Planned lifecycle state has one definition inventory and one
activation order, all reverse orders, positions, dependencies, and planned
transitions are derived, construction performs no repeated field lookup loops,
and synthesis still produces byte-for-byte equivalent executable behavior.

### LCR6 — Canonicalize the executable coordinator

**Purpose:** Make structured activation and destruction regions the sole final
executable lifecycle representation and remove synthesis's build-then-reparse
path.

- [ ] Synthesize activation regions directly from canonical definitions,
      activation order, initializer bodies, and publication boundaries.
- [ ] Synthesize destruction regions directly by reverse activation iteration
      and type-selected cleanup construction.
- [ ] Remove stored flat activation/shutdown transition vectors and the code
      that consumes them with positional arithmetic.
- [ ] Keep begin, publish, destruction, and finish transitions inside their
      owning structured regions where they carry executable spans or state
      changes.
- [ ] Make final coordinator verification walk structured regions directly for
      exact field coverage, activation order, reverse destruction order,
      transition legality, publication dominance, cleanup type, and destination
      non-escape.
- [ ] Remove final verification that merely flattens structured regions to
      compare them with a mirrored vector; retain derived flat dump rendering
      if it improves inspection.
- [ ] Preserve initializer CFG identities, storage/value/block identities,
      full-expression cleanup order, backend initializer/finalizer calls, and
      host-wrapper result preservation.
- [ ] Keep zero-static and zero-explicit-initializer programs on their existing
      valid paths without inventing empty executable lifecycle work.

**Tests:** Every static storage and cleanup category; zero-default and explicit
regions; publication-before-cleanup; reverse destruction; malformed region
coverage/order/transition/cleanup mutations; destination escape; generic and
nested arrays; no-static programs; exact assembly and native activation/
shutdown behavior.

**Gates:** Focused synthesis, coordinator-verifier, and x86-64 lifecycle tests;
`cargo test --locked -p skald-compiler --test pipeline_determinism`;
`make compiler-test`; `make golden-filter GOLDEN_FILTER='static_fields/**'`;
`make golden-determinism-test`; `make docs-check`; `make msrv-check`; and
`git diff --check`.

**Exit criteria:** Final MIR stores one structured coordinator representation,
synthesis no longer creates or reparses mirrored transition vectors, verifier
and backend consume the same canonical regions, and generated lifecycle
behavior is unchanged.

### LCR7 — Seal phase products and publish the optimization boundary

**Purpose:** Encode issuance and realization verification in the type flow,
integrate the future pass invalidation contract without unused framework, and
finish the redesign as a safe backend prerequisite.

- [ ] Introduce private draft and sealed verified planned-product boundaries so
      synthesis can consume only a product that passed exact authority
      issuance verification.
- [ ] Make final ordinary-MIR plus lifecycle-realization verification produce a
      read-only verified final product or view required by backend input
      construction.
- [ ] Update driver phase orchestration and structured reporting so planning,
      planned verification, synthesis, MIR-pipeline verification, and backend
      metrics retain truthful boundaries and failure ownership.
- [ ] Remove duplicate planned verification inside synthesis once the consumed
      type proves it, while retaining final verification once before backend
      consumption.
- [ ] Expose the realization checker as the central invalidation target for
      future passes that change static accesses, control-flow reachability,
      lifecycle operations, or possible callees.
- [ ] Document the required future pass classification as preserving or
      changing lifecycle effects; do not add an unused general registry or
      cache before a production transformation needs one.
- [ ] Keep pass-owned statistics flowing through the existing measured MIR
      pipeline and count all verification executions honestly.
- [ ] Replace broad production mutation accessors with verifier-local malformed
      builders or narrow test support where practical.
- [ ] Add an end-to-end test-only sequence exercising effect removal, target
      narrowing, and inlining-shaped rewriting through the same central final
      checker used by the driver.
- [ ] Audit static-lifecycle modules and functions by responsibility, resolve
      remaining high-priority duplication, and record unrelated optimizer or
      dense-identity findings in the indexed optimization discoveries rather
      than expanding this roadmap.
- [ ] Promote the fully implemented schema and trust boundary in compiler,
      debugging, testing, and public API documentation; remove stale exact-
      graph and rollout wording from living docs.

**Tests:** Compile-time inability to construct sealed products through public
APIs; planned/final verifier failure propagation and report-event order;
backend refusal of unverified MIR; direct realization-checker coverage;
test-only transformed pipeline cases; public API compilation; pipeline
statistics; deterministic planned/final dumps; all static lifecycle diagnostics
and native startup/shutdown behavior; default compiler output parity.

**Gates:** Focused driver, reporting, pipeline, verifier, public API, backend,
and lifecycle tests; `make check`; `make golden-determinism-test`;
`make msrv-check`; and `git diff --check` from an artifact-free snapshot or
clean checkout.

**Exit criteria:** Exact authority issuance and subset realization are encoded
in sealed phase products, no backend path accepts unchecked final MIR, no stale
exact graph certificate or redundant lifecycle encoding remains, the full
repository gate passes, and the compiler is ready for a separately reviewed
optimization-pass framework or first transformation.

## Ordering and dependencies

LCR0 establishes semantic fact identity and an independent checker while the
old certificate remains an oracle. LCR1 makes that fact set an exact issued
authority before LCR2 relaxes any final equality; reversing those tasks would
weaken the trust boundary without a replacement.

LCR3 removes analysis-only certificate state only after both issuance and final
realization have focused coverage. LCR4 then moves the separated responsibilities
behind stable facades without changing behavior. LCR5 removes planned mirrors,
and LCR6 removes coordinator mirrors after synthesis has a canonical plan to
consume. Keeping those schema migrations separate makes publication and
destruction regressions reviewable.

LCR7 seals the final type flow after product shapes stabilize. It documents the
future effect-invalidation classification but intentionally avoids a pass
registry with no production transformation. The separate optimization
architecture work can build registration, selection, fixpoint scheduling, and
analysis caching on this verified boundary.

No task may change source lifecycle acceptance, runtime ABI, or language
contracts. If implementation discovers a missing implicit lifecycle root or an
open-world callable path, stop and reopen the frozen design rather than adding
an unsound local exception.
