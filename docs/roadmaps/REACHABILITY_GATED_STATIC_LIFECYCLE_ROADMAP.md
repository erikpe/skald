# Reachability-Gated Static Lifecycle Roadmap

Status: in progress; RSR0 and RSR1 are complete and RSR2 is next.

This roadmap implements the frozen
[reachability-gated static lifecycle design](REACHABILITY_GATED_STATIC_LIFECYCLE_DESIGN_PROPOSAL.md)
and its promoted
[language](../language/STATIC_FIELDS.md#frozen-reachability-gated-activation-direction),
[compiler phase](../compiler/PHASES_AND_IR.md#frozen-reachability-gated-static-lifecycle-direction),
[backend](../compiler/BACKEND.md#frozen-reachability-gated-static-lifecycle-boundary),
[driver](../compiler/DRIVER_AND_ARTIFACTS.md#frozen-static-activation-orchestration),
and [reporting](../compiler/REPORTING.md#frozen-static-activation-observation)
contracts. It changes static lifetime from declaration-wide eager execution to
one exact field-grained activation closure while preserving complete
whole-world checking, eager-before-entry execution for active fields, and
exact-reverse normal-return shutdown.

The semantic change is deliberately late in the sequence. Shared extraction,
shadow analysis, subset-capable proof, and independent final safety checks land
first. Each task should make small cohesive maintainability improvements that
reduce future optimization cost; larger findings belong in the
[discoveries record](REACHABILITY_GATED_STATIC_LIFECYCLE_DISCOVERIES.md).

## Dependencies

- The completed
  [static-lifecycle certificate roadmap](../archive/STATIC_LIFECYCLE_CERTIFICATE_ROADMAP.md)
  supplies compact baseline authority, independent issuance and realization
  verification, and immutable lifecycle order across optimization.
- The completed
  [target-independent whole-world reachability roadmap](../archive/TARGET_INDEPENDENT_WHOLE_WORLD_REACHABILITY_ROADMAP.md)
  supplies exhaustive execution/lifecycle dependency extraction, deterministic
  closure, sparse final definitions, seal-bound facts, and backend retained-
  domain queries.
- The completed
  [selectable final-MIR pipeline roadmap](../archive/SELECTABLE_FINAL_MIR_OPTIMIZATION_PIPELINE_ROADMAP.md)
  supplies optimization profiles, pass selection, changed-product resealing,
  checkpoint inspection, and structured measurement ownership.
- Stable `StaticFieldId`, initializer identities, preliminary initializer
  bodies, field-grained lifecycle plans, dense callable-local rewriting, and
  target-private machine-artifact retention remain foundations rather than
  replacement targets.

## Scope and invariants

- Keep Skald compilation permanently whole-world and generated execution
  single-threaded. Compiler implementation parallelism must not change active
  sets, witnesses, diagnostics, dumps, ordering, or artifacts.
- Keep every declared static, initializer expression, callable, class,
  interface, type, source span, and stable identity present through ordinary
  source checking and verified preliminary MIR.
- Compute one exact least fixed point after preliminary MIR verification and
  before static-lifecycle planning; do not make activation a selectable pass.
- Root activation at the selected entry and use every structurally present
  block plus the frozen full virtual-family, interface-conformance, lifecycle,
  and exact-function-type target rules.
- Treat ordinary static reads, writes, replacements, and borrows as activation
  accesses. Exclude only the lifecycle-owned unpublished destination of the
  field's own initializer.
- Add initializer and eventual-destruction execution for each newly active
  field, and iterate execution nodes and fields to one deterministic fixed
  point.
- Plan, diagnose, prove, synthesize, and execute lifecycle work for exactly the
  active subset. Preserve current dependency order, publication rules,
  ownership, failure behavior, and exact-reverse normal-return shutdown within
  that subset.
- Bind a canonically sorted exact active-field set and active lifecycle-root
  authority into the immutable certificate. Reports and witnesses are not
  proof identity.
- Independently recompute activation at issuance and require every reachable
  final static access to target an active field. Retained unreachable bodies
  may still mention inactive declarations.
- Keep active fields active after final-MIR effect removal. No optimization
  profile, pass exclusion, target, or backend heuristic may replan activation.
- Apply `STA001` and `STA002` only to the active dependency graph while keeping
  every ordinary syntax, resolution, type, ownership, and preliminary-MIR
  diagnostic for inactive declarations.
- Emit no inactive initializer or destructor call. Backend slot planning may
  initially remain conservative only when private unreferenced artifacts are
  removed by the existing machine-artifact closure.
- Add no eager-static syntax, module initializer, registration hook, retention
  annotation, runtime lazy initialization, access guard, initialized-state
  byte, atomics, locking, thread-local storage, ABI revision, or public symbol.
- Keep `mod.rs` files concise facades, reuse the shared dependency vocabulary,
  and do not add a second call/lifecycle walker or a global analysis cache.
- Keep the root Makefile as the repository automation interface; add no CI.

## Progress

- [x] RSR0 — Establish activation vocabulary and behavioral baselines
- [x] RSR1 — Centralize preliminary static-access extraction
- [ ] RSR2 — Compute and inspect the shadow activation closure
- [ ] RSR3 — Generalize lifecycle proof and schema for an active subset
- [ ] RSR4 — Verify final access against exact activation authority
- [ ] RSR5 — Switch lifecycle planning and synthesis to reachable activation
- [ ] RSR6 — Align backend planning and artifact retention
- [ ] RSR7 — Publish activation observation and migration diagnostics
- [ ] RSR8 — Harden the language transition and close the roadmap

## PR-sized implementation sequence

### RSR0 — Establish activation vocabulary and behavioral baselines

**Purpose:** Give the new semantic analysis one cohesive owner and pin the
current eager behavior before any extraction or lifecycle product changes.

- [x] Add a private responsibility-oriented activation-analysis module behind
      the static-lifecycle facade with typed field, execution-node, trigger,
      edge, witness, and count vocabulary but no closure solver or behavior
      change.
- [x] Define canonical comparison keys and immutable borrowed queries up front;
      keep stable identities and source spans rather than source-name lookup.
- [x] Add reusable focused fixtures for direct access, every stored family,
      dynamic and indirect calls, implicit lifecycle work, inactive-only
      dependencies, self-dependencies, cycles, and deterministic ordering.
- [x] Add source-to-native/golden baselines showing that an imported unused
      explicit initializer currently executes, while recording preliminary,
      planned, final, assembly, stdout/stderr, status, and shutdown observations.
- [x] Inventory the current analysis, planner, proof, synthesis, verifier,
      driver, backend-slot, dump, reporting, and test owners in module comments
      or living architecture only where that ownership is durable.
- [x] Preserve current source acceptance, diagnostics, MIR, assembly, runtime
      behavior, public facade paths, and runtime ABI exactly.

**Tests:** New activation model ordering/query unit tests; existing static-
lifecycle plan, synthesis, verifier, backend, and native tests; a focused
imported-unused-static golden baseline; public-API compilation and deterministic
dump checks.

**Gates:** `cargo test --locked -p skald-compiler passes::static_lifecycle`;
`make compiler-test`; focused golden selection; `make fmt-check`; `make lint`;
`make docs-check`; and `git diff --check`.

**Exit criteria:** Activation has one clear internal vocabulary and reusable
fixtures, the old eager semantics are observably pinned, and production
compilation is unchanged.

Completed on 2026-08-31. The private activation owner now supplies typed,
canonically ordered, immutable model vocabulary and focused source and identity
fixtures without participating in production compilation. An imported-unused
static golden case pins preliminary, planned, final, assembly, process, and
reverse-shutdown behavior under default, optimization-disabled, and
reachability-disabled selection. The durable compiler ownership inventory is
published in the living phase documentation; current eager semantics, public
facades, and the runtime ABI are unchanged.

### RSR1 — Centralize preliminary static-access extraction

**Purpose:** Make activation and existing static-effect inference consume one
exhaustive read-only source of ordinary static accesses and executable
dependencies before either analysis solves a graph.

- [x] Extend the shared preliminary dependency inventory with typed direct
      static-place accesses containing source node, target field, access kind,
      phase, span, and lifecycle-owned destination classification.
- [x] Cover reads, writes, replacement, immutable and mutable borrows, calls,
      every rvalue/terminator form, initializer publication, and current class,
      optional, shared-owner, and array lifecycle expansion through exhaustive
      matches.
- [x] Keep a field's own unpublished initializer destination distinct from an
      ordinary pre-publication self-access; do not let one hide the other.
- [x] Retain structurally present accesses without local CFG pruning and keep
      target resolution target-independent and deterministic.
- [x] Migrate static-effect inference to the shared access inventory, prove
      exact diagnostic/report/dump parity, and remove the superseded direct
      scanner only after parity passes.
- [x] Return structured extraction failures from verified preliminary inputs
      instead of adding assertions or panic-prone lookup paths.
- [x] Split the oversized shared lifecycle extractor only if the behavior-
      preserving facade refactor remains cohesive with this change; otherwise
      record it in discoveries.

**Tests:** One focused case per static access and lifecycle form; ordinary
versus lifecycle-owned destination; structural constant-false blocks; direct,
virtual, interface, function-value, copy, assignment, destruction, optional,
shared, and array targets; malformed identities; exact old static-effect and
planning parity; deterministic extraction order.

**Gates:** `cargo test --locked -p skald-compiler passes::reachability`;
`cargo test --locked -p skald-compiler passes::static_lifecycle`;
`make compiler-test`; `make fmt-check`; `make lint`; `make docs-check`; and
`git diff --check`.

**Exit criteria:** One exhaustive service supplies preliminary executable and
static-access dependencies to both consumers, with no activation closure and
no behavior change.

Completed on 2026-08-31. The shared reachability traversal now emits immutable,
canonically ordered direct static-access records beside executable and
lifecycle dependencies, with borrowed whole-inventory and per-source queries.
Typed ordinary versus lifecycle-owned destination evidence preserves exact
source node, target field, access kind, structural phase, and span. Static-
effect analysis consumes that inventory without owning a MIR body scanner; its
former place, instruction, and control-flow walkers were removed after exact
direct-effect, plan, diagnostic, dump, and realization parity passed. Invalid
field and lifecycle-destination identities use structured extraction failures.
The independent lifecycle extractor split remains deferred under the existing
indexed discovery because moving its class, optional/shared, and array owners
was not cohesive with this semantic extraction change. Production activation
and declaration-wide eager behavior are unchanged.

### RSR2 — Compute and inspect the shadow activation closure

**Purpose:** Prove the exact field-grained semantic set beside current eager
planning before that set can affect execution or diagnostics.

- [ ] Implement an iterative deterministic least fixed point over two queues:
      activation-reachable execution nodes and active `StaticFieldId`s.
- [ ] Root the execution domain at the selected entry, follow the frozen full
      direct/dynamic/lifecycle/function-value target rules, and scan every
      structural block of each reached definition.
- [ ] Activate fields for ordinary access records; add each active field's
      explicit initializer and eventual-destruction lifecycle nodes; exclude
      only its lifecycle-owned unpublished destination.
- [ ] Keep callable-address candidates scoped to activation-reachable address
      formations and exact function types, matching the frozen whole-world
      reachability rule.
- [ ] Produce one immutable analysis with sorted active/inactive fields,
      outgoing dependencies, conservative target counts, canonical first
      triggers, witness paths, and summary counts.
- [ ] Add a deterministic focused activation dump separate from MIR dumps and
      report events.
- [ ] Run the analysis in shadow mode at the mandatory post-preliminary-
      verification boundary while continuing to plan and execute all declared
      statics.
- [ ] Assert that repeated runs, declaration/provider discovery permutations,
      and independent compiler processes select identical sets and witnesses.

**Tests:** Empty and direct roots; transitive calls; recursion; direct and
transitive static dependencies; static initializer and shutdown discovery;
constant-false access; virtual/interface/indirect targets; unreachable address
formations; sibling fields and generic specializations; deterministic query and
dump output; imported-unused decimal parsing as an inactive shadow result.

**Gates:** Focused activation-analysis and reachability suites;
`cargo test --locked -p skald-compiler --test pipeline_determinism`;
`make compiler-test`; focused goldens; `make fmt-check`; `make lint`;
`make docs-check`; and
`git diff --check`.

**Exit criteria:** Every successful compilation can produce a deterministic,
queryable exact activation result from verified preliminary MIR, but current
eager lifecycle planning, diagnostics, final MIR, and native behavior remain
unchanged.

### RSR3 — Generalize lifecycle proof and schema for an active subset

**Purpose:** Remove the complete-declaration assumption from lifecycle products
and verifiers while production still uses the current all-declared set.

- [ ] Extend the compact lifecycle proof with one canonically sorted exact
      active-field authority and expose only read-only membership/count queries.
- [ ] Define declared fields separately from active lifecycle definitions;
      require definitions, activation order, root authority, and coordinator
      regions to cover exactly the certified active set.
- [ ] Derive shutdown, positions, dependency pairs, and transition views from
      the active plan without compacting `StaticFieldId` or any program-level
      declaration table.
- [ ] Generalize planning, planned verification, synthesis, final realization
      verification, MIR dumping, cloning, and malformed-product fixtures for
      empty and sparse active subsets.
- [ ] Independently verify sorted field authority, definitions, lifecycle roots,
      and coordinator coverage for subset fixtures; production continues to
      certify its current all-declared field set in this compatibility slice.
- [ ] Keep source-rich triggers, witnesses, SCCs, and counts in the planning
      report rather than certificate identity.
- [ ] Keep production orchestration on the all-declared lifecycle set until
      final reachable-access safety is implemented; no source behavior or
      lifecycle diagnostic changes in this task.
- [ ] Preserve public API privacy and unforgeable planned/final seals.

**Tests:** Empty, one-field, sparse, and complete active plans; malformed active
authority and every missing/extra proof/coordinator component; exact schema
verification; active dependency order; reverse shutdown; inactive preliminary
initializer retention; stable declaration identities; dump and clone
determinism; external inability to forge authority or seals.

**Gates:** Static-lifecycle plan/synthesis/verification suites; MIR verifier and
public-API tests; `make compiler-test`; `make fmt-check`; `make lint`;
`make docs-check`; `make msrv-check`; and `git diff --check`.

**Exit criteria:** Lifecycle products and independent verifiers correctly
represent any exact active subset while the production driver still emits the
same all-declared eager program.

### RSR4 — Verify final access against exact activation authority

**Purpose:** Establish the independent final safety condition required before
inactive lifecycle definitions may disappear from production MIR.

- [ ] Extend final reachability facts or their verifier-owned extraction with
      exact reachable static-place accesses and deterministic selecting
      evidence.
- [ ] Seal the exact shadow activation result at the post-preliminary boundary
      and independently recompute it without trusting its solved summaries,
      canonical witnesses, or planning report; keep production eager until the
      next task consumes the seal.
- [ ] Require every static access in an execution-reachable final definition to
      name a field in the lifecycle certificate's active authority.
- [ ] Continue fully validating every physically retained definition while
      permitting an unreachable retained body to mention an inactive declared
      field.
- [ ] Require every active field's storage, initializer when explicit,
      destruction dependencies, lifecycle roots, and coordinator regions to be
      reachable and present under existing sparse-definition rules.
- [ ] Reject a transformed product that introduces a reachable inactive access,
      expands active root effects beyond baseline authority, or loses required
      active lifecycle work.
- [ ] Prove changed passes discard and rebuild final MIR, reachability facts,
      and static realization together; unchanged outcomes preserve only one
      coherent seal.
- [ ] Keep verification independent of pass claims, schedules, profiles,
      backend slot planning, and shadow-analysis witnesses.

**Tests:** Reachable versus unreachable inactive access; missing active storage,
initializer, destructor, and coordinator roots; newly introduced access after a
changed pass; removed final access to an already-active field; sparse retained
bodies; exact deterministic failures; `none`, selective-disable, and default
seal behavior.

**Gates:** MIR verification, reachability, static realization, retention, and
pipeline suites; public-API tests; `make compiler-test`; `make fmt-check`;
`make lint`; `make docs-check`; `make msrv-check`; and `git diff --check`.

**Exit criteria:** Central final verification independently rejects every
reachable inactive static access and missing active obligation, while current
all-declared production behavior remains unchanged.

### RSR5 — Switch lifecycle planning and synthesis to reachable activation

**Purpose:** Make the frozen source-visible semantics current only after exact
analysis, subset products, and final safety checks are all independently
established.

- [ ] Replace all-declared production planning input with the verified exact
      activation authority from the mandatory preliminary-MIR boundary.
- [ ] Build the dependency graph, planning report, definitions, authority, and
      activation order for exactly active fields; derive exact-reverse shutdown
      from that order.
- [ ] Restrict `STA001` and `STA002` to active graph components while retaining
      all ordinary source and preliminary-MIR errors in inactive declarations
      and initializer bodies.
- [ ] Move only active explicit initializer bodies into final MIR and synthesize
      only active zero-default transitions and active destruction regions.
- [ ] Preserve pre-publication self-access rules, post-publication cleanup,
      stored-value ownership, replacement, panic, allocation, and non-unwinding
      shutdown behavior for active fields.
- [ ] Ensure `none`, `default`, pass exclusions, repeated schedules, and every
      backend target receive the same active authority and runtime lifecycle.
- [ ] Remove compatibility-only all-declared adapters and shadow-only plumbing
      once the verified production path owns the result.
- [ ] Update language status, static-field/error contracts, compiler phases,
      and all text that still calls declaration-wide eager activation current.

**Tests:** Direct read/write/borrow activation; unused explicit and zero-default
fields; active/inactive sibling and generic fields; active transitive
initializer/destructor dependencies; inactive cycle acceptance and the same
cycle becoming deterministic `STA001`/`STA002`; source errors in inactive
initializers; exact startup/output/failure/shutdown under every profile.

**Gates:** All static-lifecycle and MIR pipeline tests; driver profile tests;
focused compile-failure and native goldens; `make compiler-test`;
`make golden-test`; `make fmt-check`; `make lint`; `make docs-check`;
`make msrv-check`; and `git diff --check`.

**Exit criteria:** Reachability-gated activation is the documented and executed
language contract, inactive lifecycle work is absent from final MIR, and all
optimization policies preserve the same observable static behavior.

### RSR6 — Align backend planning and artifact retention

**Purpose:** Make target lowering consume the certified active domain and prove
that inactive lifecycle work does not survive as ordinary emitted artifacts.

- [ ] Expose one backend-facing active-static query from verified final MIR;
      do not expose planning reports, mutable sets, or preliminary bodies.
- [ ] Restrict program initializer/finalizer emission and all lifecycle helper
      visits to active coordinator regions.
- [ ] Drive static-slot and target metadata planning from active storage where
      safe; retain a conservative private slot only when a physically retained
      unreachable body still requires an addressable symbol.
- [ ] Require the existing target-private generated-symbol closure to remove
      unreferenced inactive slots, initializer bodies, lifecycle helpers,
      literals, trace metadata, and transitive machine artifacts.
- [ ] Keep static symbols private and preserve layout, relocations, host entry,
      calling conventions, result preservation, runtime ABI version, and the
      final target-specific artifact walk.
- [ ] Add backend visit counters or focused test observers that distinguish
      declared, active, conservatively planned, retained, and emitted static
      entities without entering source semantics.
- [ ] Prove the imported-but-unused decimal parser case omits
      `_EiselPowers._words` activation, allocation, cleanup, and table-only
      artifacts while an actual floating parse retains them.

**Tests:** Empty/sparse/full active slots; unreachable retained-body fallback;
initializer/finalizer body visits; helper and literal retention; deterministic
assembly; runtime-trace behavior; native active lifecycle; decimal parse used
versus merely imported; target legality and public ABI checks.

**Gates:** Focused x86-64 backend, artifact-retention, static-field, and native
tests; affected golden groups; `make compiler-test`; `make golden-test`;
`make fmt-check`; `make lint`; `make docs-check`; and `git diff --check`.

**Exit criteria:** Backend work and emitted artifacts agree with the certified
active lifecycle, conservative private fallback cannot create observable
lifetime effects, and target/runtime contracts are unchanged.

### RSR7 — Publish activation observation and migration diagnostics

**Purpose:** Make the semantic decision explainable and measurable without
mixing analysis logging, deterministic dumps, reports, or source diagnostics.

- [ ] Promote the activation dump to the request-local inspection boundary with
      active/inactive counts, exact triggers, edges, conservative targets,
      witnesses, activation order, and derived shutdown order.
- [ ] Add typed already-known report metrics for declared, active, inactive,
      explicit, zero-default, and inactive-explicit counts plus activation
      graph/target totals at their owning phase.
- [ ] Keep analysis and passes observer-free; let the driver/pipeline adapt
      immutable facts to existing structured events and detail filtering.
- [ ] Preserve quiet default behavior and keep detailed activation dumps out of
      report event text, MIR checkpoint bytes, source diagnostics, request
      identity, and generated artifacts.
- [ ] Add no permanent warning for an inactive explicit initializer. Document
      the migration rule that intentional side effects require ordinary
      reachable code until a separate eager/module-init design exists.
- [ ] Update debugging and testing guidance with the shortest route from a
      source field to its activation witness, certificate, coordinator, final
      reachability, backend slot, and native observation.
- [ ] Keep event and dump ordering deterministic under independent processes
      and any future compiler-internal parallelism.

**Tests:** Typed metric ownership/order/detail gating; quiet-default parity;
activation dump snapshots and inspector labels; report-writer failure; source
diagnostic separation; zero enabled-observation formatting work; repeated and
cross-process deterministic facts.

**Gates:** Reporting, driver, pipeline-inspection, dump, and CLI tests; public-
API tests; `make compiler-test`; focused binary tests; `make fmt-check`;
`make lint`; `make docs-check`; and `git diff --check`.

**Exit criteria:** A developer can deterministically explain every active field
and measure the transition without new logging, warnings, or default output.

### RSR8 — Harden the language transition and close the roadmap

**Purpose:** Audit real code, complete cross-phase regression coverage, remove
rollout scaffolding, and leave only current behavior in living documentation.

- [ ] Audit the standard library, samples, and golden corpus for static
      initializers used solely for registration or side effects; convert only
      intentional supported behavior to ordinary reachable operations.
- [ ] Complete stored-family, access-kind, call/dispatch, function-value,
      lifecycle, ownership, panic, diagnostic, generic-specialization,
      determinism, and imported-unused large-table matrices.
- [ ] Compare `none`, `default`, reachability-disabled, pass-exclusion, repeated
      schedules, and independent processes for identical active sets and native
      static observations while allowing intended code-size differences.
- [ ] Audit module size and ownership after the change; resolve small cohesive
      duplication, assertions, or facade leakage and record larger findings in
      the discoveries document with evidence and a bounded first step.
- [ ] Remove shadow/compatibility terminology, temporary adapters, roadmap
      codes, and stale declaration-wide eager wording from living code, tests,
      dumps, diagnostics, and documentation.
- [ ] Confirm grammar and runtime ABI remain unchanged and that eager statics,
      module initialization, lazy initialization, retention annotations,
      post-optimization replanning, and identity compaction remain excluded.
- [ ] Run the full artifact-free repository, extended, MSRV, documentation,
      golden determinism, and diff gates.
- [ ] Mark all roadmap work complete, archive the frozen design and roadmap,
      repair links/indexes, and keep only actionable discoveries active.

**Tests:** Complete compiler and binary suites; all golden variants including
compile-failure, native, panic/runtime-trace, control-flow lifecycle, and
whole-world reachability groups; standard-library integration; independent-
process determinism; artifact-free rerun.

**Gates:** `make check`; `make check-long`; `make msrv-check`;
`make docs-check`; `git diff --check`; and an artifact-free rerun of the
repository's documented complete gate.

**Exit criteria:** The implementation, source contract, compiler architecture,
backend behavior, tests, and active indexes all describe one hardened
reachability-gated static lifecycle, with no required work left in this
roadmap.

## Ordering and dependencies

RSR0 through RSR2 are additive and preserve eager behavior: vocabulary and
baselines precede shared extraction, and extraction precedes the coupled
closure. RSR3 makes lifecycle products subset-capable without changing the
production set. RSR4 establishes final reachable-access safety while every
field is still active. Only RSR5 performs the behavior-breaking semantic
cutover.

RSR6 follows the cutover because backend pruning must consume verified active
authority rather than predict it. RSR7 adds stable observation after the
facts and ownership are final. RSR8 is the broad migration and closure audit.
Backend fallback investigation and documentation preparation may proceed
alongside earlier tasks, but no target heuristic, reporting policy, or
optimization pass may select or alter the semantic active set.
