# Target-Independent Whole-World Reachability Roadmap

Status: in progress; WRR0 through WRR6 are complete and WRR7 is next.

This roadmap implements the frozen
[target-independent whole-world reachability design](TARGET_INDEPENDENT_WHOLE_WORLD_REACHABILITY_DESIGN_PROPOSAL.md)
and its promoted
[compiler phase](../compiler/PHASES_AND_IR.md#frozen-target-independent-whole-world-reachability-direction),
[backend](../compiler/BACKEND.md#frozen-target-independent-reachability-boundary),
[driver](../compiler/DRIVER_AND_ARTIFACTS.md#frozen-whole-world-reachability-selection),
and
[reporting](../compiler/REPORTING.md#frozen-whole-world-reachability-observation)
contracts. It establishes reusable target-independent execution-dependency
and root analysis, binds deterministic reachability facts to verified final
MIR, permits stable-identity sparse executable definitions, moves semantic
retention ahead of target lowering, and finishes by enabling one conservative
definition-pruning pass.

The primary result is whole-program compiler infrastructure rather than a
large optimization suite. Each task should remove small adjacent duplication,
unclear ownership, or panic-prone handling when that cleanup is cohesive with
the task. Larger findings belong in the
[reachability discoveries record](TARGET_INDEPENDENT_WHOLE_WORLD_REACHABILITY_DISCOVERIES.md)
instead of expanding reviewed scope.

## Dependencies

- The completed
  [static-lifecycle certificate roadmap](../archive/STATIC_LIFECYCLE_CERTIFICATE_ROADMAP.md)
  provides immutable baseline authority and monotone realization after
  effect-removing transformations.
- The completed
  [dense callable-local MIR identity rewriting roadmap](../archive/DENSE_MIR_IDENTITY_REWRITING_ROADMAP.md)
  provides stable program-level identities, atomic executable-body ownership
  transfer, and immediate resealing after local edits.
- The completed
  [selectable final-MIR optimization pipeline roadmap](../archive/SELECTABLE_FINAL_MIR_OPTIMIZATION_PIPELINE_ROADMAP.md)
  provides typed registration, deterministic schedules, pass capabilities,
  verified checkpoints, structured failures, and measurement ownership.
- Existing static-effect extraction, function-value candidate analysis,
  virtual families, interface conformances, lifecycle metadata, backend
  dispatch planning, and target-private artifact retention are owners to
  refactor or consume; the roadmap must not create drifting alternatives.

## Scope and invariants

- Preserve source acceptance, diagnostics, evaluation order, checked failure,
  panic behavior, allocation, ownership, aliasing, mutable shared-pointee
  access, and deterministic destruction independently of optimization policy.
- Preserve permanent whole-world compilation and single-threaded generated
  execution. Compiler implementation parallelism may not change roots, graph
  order, facts, dumps, schedules, or artifacts.
- Keep all static activation and reverse shutdown rooted and observable.
- Keep preliminary MIR definition-complete. Only verified final MIR may omit
  unreachable executable definitions.
- Keep program-level modules, declarations, functions, classes, interfaces,
  members, fields, static fields, types, virtual families, lifecycle
  authority, literals, source spans, and IDs stable during initial retention.
- Represent callable and implicit class/array lifecycle dependencies through
  one target-independent exhaustive extraction boundary.
- Keep root collection, dependency extraction, closure solving, analysis
  queries, program retention, final verification, and backend planning as
  separate responsibilities behind concise facades.
- Bind immutable reachability facts to exactly one verified final-MIR product.
  Every changed pass invalidates those facts and central verification rebuilds
  them before another consumer runs.
- Independently recompute reachability during final verification and require a
  valid retained definition for every reachable internal executable target.
- Validate every physically retained definition even when it is unreachable.
- Use conservative full-family and full-conformance dynamic target expansion
  initially; record narrower rapid-type or points-to analysis as later work.
- Scope function-value targets to address formations in reachable execution
  nodes and exact function types; retain every exactly formed callable address.
- Add a narrow atomic definition-retention capability rather than exposing
  mutable program tables or expanding callable-local rewrite into global ID
  compaction.
- Keep the existing target-private assembly artifact walk. It remains the
  final safety net for symbols and helpers introduced after MIR.
- Register one stable `whole-world-reachability` pass. Keep `none` empty and
  eventually place the pass after `dead-pure-definition-elimination` in
  `default` only after broad hardening succeeds.
- Keep reports compact and structured, dumps separate, passes free of logging,
  and the disabled observation path allocation-free where it is today.
- Add no language change, static-lifecycle replanning, declaration/metadata ID
  compaction, SSA, local CFG optimization, devirtualization, inlining, alias or
  effect framework, global analysis manager, target LIR, dynamic pass plugin,
  public arbitrary schedule, or repository CI.
- Keep `mod.rs` files as concise facades and tests with their responsibility
  owners.
- Keep the root Makefile as the local and external quality-gate interface.

## Progress

- [x] WRR0 — Establish the execution-dependency contract
- [x] WRR1 — Centralize possible-target and lifecycle dependency extraction
- [x] WRR2 — Implement deterministic root closure and analysis queries
- [x] WRR3 — Bind reachability facts to verified final MIR
- [x] WRR4 — Verify sparse final executable definitions
- [x] WRR5 — Add atomic stable-identity definition retention
- [x] WRR6 — Make backend planning consume the retained domain
- [ ] WRR7 — Register and observe whole-world reachability pruning
- [ ] WRR8 — Activate, harden, and close whole-world reachability

## PR-sized implementation sequence

### WRR0 — Establish the execution-dependency contract

**Purpose:** Introduce the neutral typed vocabulary and ownership boundaries
needed by both reachability and existing correctness analysis before changing
target expansion or compiler behavior.

- [x] Add a target-independent reachability/dependency facade under `passes`
      with an initial focused model/test owner and document where root,
      extraction, target, closure, and dump responsibilities will land as they
      become real; expose no production transformation yet.
- [x] Define stable typed execution-node identities for callables, class copy
      construction, class copy assignment, complete class finalization, array
      default, array copy, array assignment, and array destruction.
- [x] Define exhaustive dependency-edge kinds and whole-program root reasons
      without including static-effect phases, target symbols, or presentation
      text in semantic identity.
- [x] Establish the distinction between executable nodes, root reasons,
      runtime-entity references, semantic declarations, and physically retained
      definitions in code documentation and focused types.
- [x] Reuse or carefully generalize the existing static-lifecycle node identity
      so there is not a second independently exhaustive lifecycle-node enum;
      preserve public/static-certificate compatibility where required.
- [x] Add deterministic canonical comparison keys for nodes, edge kinds,
      spans, and root reasons; do not expose private storage representation.
- [x] Build shared fixture helpers for functions, members, static initializers,
      virtual/interface dispatch, function values, and implicit lifecycle work
      without cloning large existing test builders.
- [x] Document the maintenance rule that every executable MIR operation or
      lifecycle variant must update dependency extraction and coverage in the
      same change.

**Tests:** Node and edge identity ordering; root-reason ordering; stable span
ordering; complete callable/lifecycle node taxonomy; compatibility with static-
lifecycle root identities; fixture determinism; compile-time exhaustive-match
maintenance points.

**Gates:** `cargo test --locked -p skald-compiler passes`;
`cargo test --locked -p skald-compiler mir`; `make fmt-check`; `make lint`;
`make docs-check`; and `git diff --check`.

**Exit criteria:** One concise owner defines deterministic target-independent
execution nodes, edges, roots, and runtime-entity vocabulary without changing
static-effect results, final MIR, pass schedules, backend work, or artifacts.

Completed on 2026-08-30. MIR now owns one neutral exhaustive execution-node
identity while the prior static-lifecycle names remain compatibility aliases.
The crate-private reachability facade owns the closed dependency, root,
runtime-entity, semantic-declaration, and retained-definition vocabulary plus
canonical ordering and shared focused fixtures. Static-effect analysis reuses
the shared span key. No extraction, closure, seal integration, transformation,
schedule, backend, or artifact behavior changed.

### WRR1 — Centralize possible-target and lifecycle dependency extraction

**Purpose:** Create the single exhaustive semantic dependency source used by
reachability and static-effect analysis before either relies on a new closure
solver.

- [x] Implement a read-only executable-definition view that uniformly scans
      ordinary functions, member definitions, and final or preliminary static
      initializer bodies without snapshotting MIR.
- [x] Centralize direct function, static method, direct instance method,
      virtual-family, interface-conformance, callable-address, and exact-
      signature indirect target selection behind typed deterministic queries.
- [x] Centralize implicit initializer, user copy, synthesized copy, copy
      assignment, user destructor, field/base finalizer, optional, shared-owner,
      and array lifecycle dependency selection.
- [x] Preserve external and intrinsic calls as typed leaf dependencies rather
      than internal execution nodes with invented bodies.
- [x] Inventory callable-address formations by containing execution node,
      exact function type, exact target, and stable evidence span so later
      closure can scope candidates to reachable formations.
- [x] Use exhaustive no-wildcard matches for every instruction, rvalue,
      terminator, type/lifecycle plan, dispatch target, and cleanup form that
      can select executable work.
- [x] Migrate static-effect extraction to consume the shared target/lifecycle
      resolver while retaining its separate static-access evidence, phases,
      witnesses, root-effect authority, diagnostics, dumps, and solved results.
- [x] Remove superseded duplicate target-selection helpers only after exact
      static-effect parity is proven.
- [x] Return structured extraction errors for malformed identities instead of
      introducing new panics in analysis code.

**Tests:** Exact direct/static/member targets; full virtual families; complete
interface requirement implementations; external/intrinsic leaves; callable
addresses and exact function types; every explicit and implicit lifecycle
family; malformed target identities; exact static-effect analysis/dump parity;
deterministic edge order.

**Gates:** `cargo test --locked -p skald-compiler passes::static_lifecycle`;
`cargo test --locked -p skald-compiler mir::verify`;
`cargo test --locked -p skald-compiler passes`; `make compiler-test`;
`make fmt-check`; `make lint`; `make docs-check`; and `git diff --check`.

**Exit criteria:** Reachability and static lifecycle can obtain every current
possible executable target from one exhaustive service, existing lifecycle
authority/diagnostic behavior is unchanged, and no reachability pruning runs.

Completed on 2026-08-31. `passes::reachability` now owns a borrowed executable-
definition view, structured extraction failures, deterministic call/dispatch
resolution, scoped callable-address and indirect-call inventories, and the
canonical class/optional/shared/array lifecycle walk. Static-effect extraction
maps the neutral edges back to its private phases and witnesses; exact existing
analysis, planning, realization, and dump tests pass after removal of its old
target, function-value, and lifecycle helper modules. No closure is computed
and no MIR definition is removed.

### WRR2 — Implement deterministic root closure and analysis queries

**Purpose:** Build the reusable immutable whole-program analysis on complete
final MIR before changing seals, definition validity, or backend behavior.

- [x] Collect explicit roots for the program entry, every lifecycle
      coordinator activation obligation, and every reverse-shutdown cleanup
      using typed reasons; do not root imported external declarations.
- [x] Compute iterative deterministic least-fixed-point reachability over
      execution nodes without recursive compiler-stack traversal.
- [x] Implement the coupled function-value fixed point: only address
      formations in reached execution nodes populate exact-signature indirect
      candidates, while every reached formation retains its exact target.
- [x] Scan all structurally retained blocks of a reached callable
      conservatively; do not infer local CFG reachability in this roadmap.
- [x] Record reachable callables, used virtual families, used interface
      requirements, function-value signatures/targets, and the initial runtime-
      entity references needed by backend planning.
- [x] Provide borrowed deterministic queries for roots, reachability,
      outgoing possible targets, reachable callables, dispatch use, runtime
      entities, and canonical first-witness explanation.
- [x] Reuse the existing deterministic graph algorithms where appropriate and
      keep internal sets/maps private so representation can be tuned later.
- [x] Add a deterministic reachability dump separate from MIR dumps and
      reporting; include roots, reachable nodes, edge kinds, candidates,
      witnesses, and summary counts without target data.
- [x] Expose analysis for focused compiler tests/tools without registering a
      pass or adding a public driver option.

**Tests:** Entry-only and transitive chains; unreachable definitions; self and
mutual recursion; multiple roots; virtual/interface calls; callable addresses;
indirect signatures; address formations only in unreachable callables;
static activation/shutdown; lifecycle cycles; deterministic queries, witnesses,
and dumps across repeated and independent-process runs.

**Gates:** `cargo test --locked -p skald-compiler passes::reachability`;
`cargo test --locked -p skald-compiler passes::static_lifecycle`;
`make compiler-test`; `make fmt-check`; `make lint`; `make docs-check`; and
`git diff --check`.

**Exit criteria:** Complete valid final MIR can produce one deterministic,
queryable target-independent reachability product whose results do not mutate
MIR, affect pass policy, or enter target lowering.

Completed on 2026-08-31. `passes::reachability` now collects typed entry,
static-activation, and reverse-shutdown roots and computes an iterative
least-fixed-point closure over the shared WRR1 dependency inventory. The
closure couples reached callable-address formations with reached exact-
signature indirect sites, records runtime and dispatch use, retains canonical
first explanations, and exposes only borrowed deterministic queries over
private sorted storage. A separate target-independent dump and focused tests
cover dead code, structural blocks, recursion, dispatch, function values,
static lifecycle, lifecycle cycles, external leaves, repeated analysis, and
independent-process determinism. Analysis remains test/tool-only: it is not
seal-bound, registered as a pass, consumed by the backend, or allowed to
mutate MIR.

### WRR3 — Bind reachability facts to verified final MIR

**Purpose:** Give future passes, verification, and backends one coherent
program-plus-analysis product with conservative invalidation.

- [x] Extend the private representation of `VerifiedFinalMirProgram` to retain
      reachability facts derived from exactly its `MirProgram`, while
      preserving its read-only public program view and unforgeable seal.
- [x] Order central final verification so ordinary structure and lifecycle
      realization make complete current MIR safe before reachability facts are
      published; attribute analysis failure structurally.
- [x] Add a crate-private read-only reachability query on the verified product
      for pass capabilities and backend input without exposing mutable facts or
      construction.
- [x] Preserve reachability facts and avoid recomputation for unchanged pass
      outcomes.
- [x] Invalidate program and reachability facts together for every changed
      pass occurrence, then rebuild both only through immediate central final
      verification.
- [x] Ensure verified checkpoint inspection observes the correct seal-bound
      facts at input, after-occurrence, and final boundaries without changing
      existing checkpoint labels or MIR dump bytes.
- [x] Extend verification/pipeline accounting only with already-known
      deterministic counts; do not add a global analysis cache or pass-declared
      preservation protocol.
- [x] Add compile-fail/public-API coverage proving external code cannot forge,
      detach, replace, or mutate reachability facts.

**Tests:** Program/fact coherence; unchanged seal preservation; changed
recomputation before a later pass/checkpoint/backend; failure cutoff; cloned
verified products; exact schedule repetition; compile-fail capability and seal
visibility; unchanged optimization-off MIR and dumps.

**Gates:** `cargo test --locked -p skald-compiler passes`;
`cargo test --locked -p skald-compiler --test public_api`;
`make compiler-test`; `make fmt-check`; `make lint`; `make docs-check`;
`make msrv-check`; and `git diff --check`.

**Exit criteria:** Every verified final-MIR seal owns deterministic facts for
exactly its program, every transformation invalidates them, and no definition
or backend behavior has yet changed.

Completed on 2026-08-31. Central final verification now derives reachability
only after ordinary and static-lifecycle verification and owns the result in
the same opaque seal as its exact MIR. Passes can borrow facts through the
crate-private verified query; unchanged outcomes carry the coherent product
without verification, while changed outcomes discard MIR and facts together
and rebuild both before later passes, checkpoints, or backend input. Focused
tests cover coherence, cloning, changed call-target closure, unchanged and
repeated schedules, checkpoint facts and stable MIR dumps, structured analysis
failure attribution, exact verification counts, backend handoff, and external
seal/fact privacy. No definition retention, backend behavior, global cache,
preservation declaration, or new report accounting was introduced.

### WRR4 — Verify sparse final executable definitions

**Purpose:** Redefine final-only completeness so stable declarations may
survive without unreachable bodies while central verification still rejects
every semantically executable missing definition.

- [x] Separate preliminary producer completeness from final retained-
      definition completeness in verifier ownership and diagnostics.
- [x] Keep preliminary MIR requiring every internal source definition and
      lifecycle member body produced by lowering.
- [x] In final MIR, validate every physically present function, member, and
      static-initializer definition against its declaration and full ordinary
      body invariants without requiring all declarations to have bodies.
- [x] Recompute roots and reachability independently, then require a retained
      body for every reachable internal callable selected by entry, direct,
      dynamic, indirect, callable-address, copy, assignment, destruction,
      optional, shared, array, or static-lifecycle work.
- [x] Permit virtual families and interface conformances to name declared
      methods whose bodies are absent only when no reachable operation can
      select them.
- [x] Preserve exact static-lifecycle root coverage and baseline-authority
      subset realization with sparse unrelated definitions.
- [x] Produce deterministic missing-target errors containing callable and
      dependency category without exposing source diagnostics or relying on
      pass claims.
- [x] Add narrow test-only sparse-definition fixture construction; do not add
      production mutable table access.
- [x] Update final-MIR architecture documentation when the implemented
      distinction becomes current, while leaving the production compiler
      definition-complete until retention lands.

**Tests:** Acceptance of absent unreachable functions and every member kind;
rejection of missing entry, direct/static/member, virtual, interface, indirect,
addressed, initializer, copy, assignment, destructor, optional/shared/array,
and static-root targets; retained malformed bodies; preliminary completeness;
lifecycle authority; deterministic error ordering.

**Gates:** `cargo test --locked -p skald-compiler mir::verify`;
`cargo test --locked -p skald-compiler passes::static_lifecycle`;
`cargo test --locked -p skald-compiler passes::reachability`;
`make compiler-test`; `make fmt-check`; `make lint`; `make docs-check`; and
`git diff --check`.

**Exit criteria:** Central final verification accepts stable declarations with
absent unreachable bodies and independently rejects every absent reachable
body, while preliminary MIR remains definition-complete and production output
is still unchanged.

Completed on 2026-08-31. Ordinary final-MIR verification now validates every
retained body and its declaration/metadata relationships without treating a
missing declaration slot as malformed. Preliminary verification has an
explicit producer-completeness mode that still requires every internal
function and every declared initializer, copy operation, destructor, and
method body. Central final verification derives fresh whole-world reachability
after structural and static-lifecycle checks, then deterministically rejects
each reachable callable with no retained body and names the selecting root or
dependency category. Declaration nodes remain in the graph even when their
bodies are absent, so virtual/interface selection and static activation cannot
silently disappear. Focused tests cover sparse unreachable functions and all
member kinds, every callable-selection family, optional/shared/array and
static activation/shutdown lifecycle work, malformed retained bodies,
preliminary completeness, lifecycle authority, and stable error ordering. A
narrow test-only body-removal helper builds sparse fixtures; production tables
remain immutable and the compiler still emits definition-complete final MIR
until WRR5 introduces atomic retention.

### WRR5 — Add atomic stable-identity definition retention

**Purpose:** Provide the sole safe program-level ownership operation that can
physically remove unreachable bodies without turning global retention into
general mutable MIR access.

- [x] Add a concise `mir` retention facade separate from callable-local
      `mir::rewrite`, with cohesive function, member, lifecycle-initializer,
      result, summary, and structured-error owners.
- [x] Consume a verified final-MIR product and its seal-bound reachability
      facts through one pipeline-owned capability; do not accept a caller-
      constructed retained-ID set as proof.
- [x] Preserve dense function declarations and sparse definition-slot
      positions, member declarations and keys, static lifecycle coordinator,
      all global identities, spans, metadata, proof authority, and every
      retained body byte-for-byte.
- [x] Remove unreachable ordinary function and member definitions in canonical
      identity order; assert that all current static initializer bodies remain
      rooted and retained.
- [x] Publish rebuilt definition containers only after every retention
      precondition succeeds, and expose no partially filtered `MirProgram` on
      error.
- [x] Return stable removed callable IDs plus examined, retained, and removed
      counts by callable kind without logging, verification, dumping, or
      reporting.
- [x] Add an explicit unchanged result that preserves the verified seal and
      avoids another final verification execution when no definition is
      removed.
- [x] Route changed results through the existing immediate central
      reverification boundary and refresh reachability facts before exposure.
- [x] Keep arbitrary declaration deletion, global compaction, target narrowing,
      metadata rewriting, and caller-supplied retention predicates outside the
      facade.

**Tests:** Functions and each member definition kind; retained static
initializers; sparse function holes; canonical member order; stable declarations
and IDs; byte-identical retained bodies; exact summaries; unchanged seal;
changed reverification; atomic failure; lifecycle authority immutability;
repeated retention idempotence.

**Gates:** `cargo test --locked -p skald-compiler mir::retain`;
`cargo test --locked -p skald-compiler passes`;
`cargo test --locked -p skald-compiler mir::verify`;
`make compiler-test`; `make fmt-check`; `make lint`; `make docs-check`; and
`git diff --check`.

**Exit criteria:** A private pipeline capability can atomically make final
definition tables equal the verified reachable callable set without changing
semantic declarations or IDs, and every changed result is independently
resealed.

Completed on 2026-08-31. `mir::retain` now prepares one opaque retention plan
directly from seal-bound reachability facts while borrowing the exact verified
program. Preparation inventories ordinary functions, every member kind, and
static initializer bodies in canonical identity order, returns stable removed
IDs and per-kind examined/retained/removed counts, and rejects an unrooted
static initializer before consuming any container. An unchanged plan preserves
the original verified product. Only a validated changed plan may invalidate
the seal, move retained function/member bodies into rebuilt sparse/ordered
containers, and publish complete raw MIR for immediate central
reverification. The operation leaves declarations, global identities,
metadata, spans, retained bodies, coordinator regions, and lifecycle authority
untouched and exposes no predicate or mutable table API. Focused tests cover
all definition kinds, static roots, existing holes, canonical order, exact
metadata/body preservation, summaries, atomic failure, unchanged verification
accounting, fresh changed facts, and repeated idempotence. The capability is
not yet registered as a production optimization; backend preparation remains
the next task.

### WRR6 — Make backend planning consume the retained domain

**Purpose:** Realize the phase-placement benefit by preventing absent
unreachable bodies from entering target legality and lowering while preserving
required dispatch/runtime metadata and final machine-artifact pruning.

- [x] Expose only the verified reachability query needed by `BackendInput`;
      keep analysis representation and pass policy private to target-
      independent owners.
- [x] Make x86-64 legality, signature checks, runtime-trace activation, frame
      planning, and instruction selection iterate physically retained
      executable definitions rather than dense declarations.
- [x] Make dispatch planning distinguish declared slots from virtual families
      and interface requirements reachable MIR can select; require bodies only
      for verified reachable selections.
- [x] Permit unreachable declaration-only methods to occupy unused semantic
      metadata without causing target errors or invented null-call behavior on
      a reachable path.
- [x] Consume required runtime-entity facts for class/array/optional-box
      dispatch metadata, literals, and static data where needed; retain extra
      target metadata conservatively if removal is not required for body
      pruning correctness.
- [x] Add instrumentation or focused test hooks proving pruned definitions do
      not enter target legality, layout, trace, frame, or instruction-selection
      work.
- [x] Preserve complete-emission behavior over every physically present
      definition for direct backend diagnostics/tests; do not resurrect absent
      bodies.
- [x] Keep the target-private exported-symbol artifact walk after lowering and
      preserve its target-generated helper, dispatch, literal, panic, and trace
      dependencies.
- [x] Update the implemented backend contract as each retained-domain behavior
      becomes current.

**Tests:** Sparse verified functions and members through backend input;
reachable and unused virtual/interface slots; function values; static startup
and shutdown; optional/shared/array helpers; runtime traces; complete versus
retained assembly modes; no visit of pruned bodies; target-generated helper
survival; deterministic assembly; structured missing-required-entity errors.

**Gates:** `cargo test --locked -p skald-compiler backend`;
`cargo test --locked -p skald-compiler passes::reachability`;
`make compiler-test`; `make cli-test`; `make fmt-check`; `make lint`;
`make docs-check`; and `git diff --check`.

**Exit criteria:** Every backend accepts verified sparse final MIR, lowers only
retained executable definitions and required runtime metadata, and still
performs final target-private artifact retention without changing current
complete-program source behavior.

Completed on 2026-08-31. `BackendInput` now projects only canonical required-
runtime-entity and used-dispatch queries from its sealed reachability product;
the x86-64 backend never receives the analysis representation or pass policy.
Required entity identities are defensively validated before target planning,
while the first implementation conservatively retains additional declaration
metadata. Array and general legality, ABI signature checks, trace activation,
frame planning, and instruction selection consume the physical executable-
definition iterator. Class layout remains declaration-driven and performs no
callable-body walk.

Dispatch planning retains dense ABI slot positions but requires executable
bodies only for used virtual families and interface requirements. A physically
present implementation remains in complete-emission metadata; an absent body
may produce a null entry only in a verified-unused slot, so no reachable
dispatch path gains invented null-call behavior. Focused observer tests prove
that sparse functions and members never enter callable-oriented backend
phases, while direct complete emission still visits every present body.
Additional tests cover unused virtual/interface declarations, enabled runtime
traces, deterministic complete versus artifact-retained assembly, callable-
address survival through the unchanged machine-symbol walk, and sparse static
startup, reverse shutdown, and array-helper execution. The final target-
private artifact-retention walk remains unchanged and WRR7 can now register
the first selectable semantic pruning client.

### WRR7 — Register and observe whole-world reachability pruning

**Purpose:** Add the first selectable production client of the reusable
analysis and retention foundation without changing the supported default
schedule yet.

- [ ] Add one cohesive optimization module with typed identity, stable name
      `whole-world-reachability`, description, and transformation entry point.
- [ ] Register the descriptor exactly once and expose it through
      `available_mir_passes`, `--list-mir-passes`, unknown-name diagnostics,
      and crate-private exact schedule resolution.
- [ ] Implement the pass solely by consuming the verified reachability product
      through atomic definition retention; do not rescan MIR or mutate
      declarations/metadata in the pass module.
- [ ] Return unchanged when no body is removed and changed only after atomic
      retention; report examined, reachable, removed, and conservative target
      counts by stable owner/counter names.
- [ ] Extend occurrence/aggregate reporting without adding pass logging,
      graph dumps to report events, live-duration determinism assertions, or
      report-only MIR traversal.
- [ ] Add deterministic reachability inspection alongside existing verified
      MIR checkpoints for focused compiler tools/tests, not as new CLI dump
      publication policy.
- [ ] Keep `none` empty and keep production `default` containing only the
      existing dead-pure-definition elimination pass during this task.
- [ ] Prove crate-private schedules containing reachability once, repeatedly,
      before/after the canary, and after a synthetic edge-removing pass.
- [ ] Update driver, reporting, and phase documentation for the registered but
      not yet default-enabled pass.

**Tests:** Registry uniqueness and lexical listing; description/help output;
exact schedule selection, exclusion, repetition, and occurrence numbering;
unchanged/changed accounting; pass-attributed retention and output-verification
failures; checkpoint order; deterministic dumps; current default exact parity.

**Gates:** `cargo test --locked -p skald-compiler passes`;
`cargo test --locked -p skald-compiler reporting`;
`cargo test --locked -p skald-compiler driver`; `make cli-test`;
`make compiler-test`; `make fmt-check`; `make lint`; `make docs-check`;
`make msrv-check`; and `git diff --check`.

**Exit criteria:** The pass is registered, listable, selectable by exact
compiler schedules, measurable, inspectable, repeatable, and fully verified,
while the supported default compiler still runs only the existing canary.

### WRR8 — Activate, harden, and close whole-world reachability

**Purpose:** Prove broad equivalence and deterministic savings, enable the pass
in the supported default schedule, resolve maintainability debt, and promote
the implementation from frozen direction to current architecture.

- [ ] Place `whole-world-reachability` after
      `dead-pure-definition-elimination` in `default`; keep `none` empty and
      preserve deterministic explicit order independent of registry order.
- [ ] Prove disabling reachability from `default` retains the prior complete
      final MIR and that disabling both passes matches `none` exactly.
- [ ] Add source-to-MIR, assembly, and native fixtures containing unreachable
      functions, recursion, every member kind, direct/dynamic/indirect calls,
      static lifecycle, ownership, optional, shared, array, panic, trace, and
      literal dependencies.
- [ ] Verify native behavior, stdout/stderr, exit status, panic behavior,
      static activation/shutdown, destruction timing, and runtime traces are
      equivalent across `none`, default, and reachability-disabled modes.
- [ ] Demonstrate deterministic reductions in retained MIR definitions,
      backend-visited callables, and emitted artifacts on representative
      fixtures without promising benchmark-specific runtime improvements.
- [ ] Run repeated-process determinism for analysis dumps, optimized MIR,
      measurements excluding elapsed time, assembly, and native observations.
- [ ] Audit target selection, lifecycle extraction, final verification,
      retention, pass execution, and backend planning for duplicate walkers,
      oversized owners, broad mutation, stale terminology, and avoidable
      panics; resolve high-priority issues within the roadmap.
- [ ] Record lower-priority or materially broader findings with evidence,
      likely owner, priority, and bounded follow-up in the discoveries record.
- [ ] Update living compiler, backend, driver, reporting, debugging, and test
      documentation to current implemented behavior and remove rollout
      language or roadmap codes outside roadmap/archive records.
- [ ] Run the full repository gate from an artifact-free snapshot, plus the
      supported-toolchain gate, confirm every task and exit criterion, archive
      the completed roadmap/design record, and index any remaining discoveries.

**Tests:** Full Rust unit/integration suite; CLI list/profile/disable matrix;
complete golden corpus; native equivalence; runtime traces; malformed MIR;
optimization-off parity; independent-process determinism; documentation links;
supported MSRV.

**Gates:** Focused reachability, verifier, pipeline, backend, driver, and
reporting suites; artifact-free `make check`; `make golden-determinism-test`;
`make msrv-check`; `make docs-check`; and `git diff --check`.

**Exit criteria:** The default compiler performs target-independent whole-world
definition pruning before backend lowering, `none` remains the exact
unoptimized reference, every reachable executable dependency is independently
verified, backend target-artifact retention remains intact, living docs are
authoritative, and no high-priority roadmap-owned maintainability issue
remains.

## Ordering and dependencies

The order settles semantic vocabulary before extraction, extraction before
closure, and closure before it becomes part of the final-MIR trust token.
Sparse definition validity follows only after the seal owns independently
derived facts. Atomic retention then has a verifier capable of checking its
result. Backend planning changes before a production pass can hand sparse MIR
to ordinary compilation. Registration and observation precede supported
default activation, leaving broad parity and determinism as an explicit final
gate rather than assumptions made during infrastructure work.

The target resolver and static-effect migration are the highest-risk shared
refactor and should not be parallelized with closure semantics. Focused backend
preparation may be investigated after the runtime-entity query contract is
stable, but production integration depends on sparse-definition verification
and atomic retention. No later task may weaken roots or target expansion merely
to make an earlier test pass.

## Discoveries and deferred work

The dedicated
[discoveries record](TARGET_INDEPENDENT_WHOLE_WORLD_REACHABILITY_DISCOVERIES.md)
owns findings that are useful but not necessary for WWR0 through WRR8. Likely
examples include declaration/metadata compaction, rapid-type analysis,
call-site points-to precision, reachability preservation proofs, broader
interprocedural analyses, and target metadata reduction beyond what sparse
definition consumption requires. Those are not roadmap commitments unless the
roadmap is explicitly revised and reviewed.
