# Local Final-MIR Simplification Roadmap

Status: in progress; LSR0 through LSR6 are complete and LSR7 is next.

This roadmap implements the frozen
[local final-MIR simplification design](LOCAL_FINAL_MIR_SIMPLIFICATION_DESIGN_PROPOSAL.md)
and its promoted
[compiler phase](../compiler/PHASES_AND_IR.md#frozen-local-final-mir-simplification-direction),
[driver](../compiler/DRIVER_AND_ARTIFACTS.md#frozen-local-final-mir-simplification-selection-direction),
and
[reporting](../compiler/REPORTING.md#frozen-local-final-mir-simplification-observation-direction)
contracts. It adds exact target-independent primitive evaluation, block-local
constant facts, primitive constant folding, algebraic simplification with
guarded atomic value forwarding, proof-aware local CFG reachability, and
conservative branch/unreachable-block cleanup. It finishes by activating the
frozen repeated default schedule ahead of whole-world retention.

The primary result is a reusable local simplification layer and its safety
boundaries, not an open-ended optimization suite. Each task should directly
resolve small cohesive maintainability problems encountered in its owner.
Larger or unrelated findings belong in the
[local-simplification discoveries record](LOCAL_FINAL_MIR_SIMPLIFICATION_DISCOVERIES.md)
and remain cataloged in the
[optimization register](OPTIMIZATION_CANDIDATE_CATALOG.md) instead of
expanding reviewed scope.

## Dependencies

- The completed
  [static-lifecycle certificate roadmap](../archive/STATIC_LIFECYCLE_CERTIFICATE_ROADMAP.md)
  permits monotone final-MIR realization after effect-removing rewrites.
- The completed
  [dense MIR identity rewriting roadmap](../archive/DENSE_MIR_IDENTITY_REWRITING_ROADMAP.md)
  provides immutable identity observation, sparse callable transactions,
  exhaustive substitution, deletion, deterministic dense commit, and
  immediate resealing.
- The completed
  [selectable final-MIR pipeline roadmap](../archive/SELECTABLE_FINAL_MIR_OPTIMIZATION_PIPELINE_ROADMAP.md)
  provides stable pass registrations, repeated schedules, exclusions,
  per-occurrence measurements, verified checkpoints, and pass-attributed
  failures.
- The completed
  [whole-world reachability roadmap](../archive/TARGET_INDEPENDENT_WHOLE_WORLD_REACHABILITY_ROADMAP.md)
  provides seal-bound program facts and the final definition-retention pass
  that must remain last in the target-independent schedule.
- Current MIR primitive enums, verification, value-use census, callable edit
  facade, static publication attachments, path-condition records, and logical
  records are the owners to extend or consume; this roadmap must not create
  drifting alternatives.

## Scope and invariants

- Preserve source acceptance, source diagnostics, evaluation order,
  exactly-once operand evaluation, checked failure, panic reasons and spans,
  allocation, ownership, aliasing, mutable shared-pointee access, cleanup,
  deterministic destruction, static startup/shutdown, and runtime traces.
- Preserve permanent whole-world compilation and single-threaded generated
  execution without treating either guarantee as proof that mutable memory is
  stable or destruction unobservable.
- Keep `none` as the exact verification-only reference product.
- Keep all transformations target independent and behind the existing final-
  MIR seal, atomic rewrite, invalidation, and immediate reverification
  boundary.
- Add stable pass identities 2, 3, and 4 with names
  `primitive-constant-folding`, `primitive-algebraic-simplification`, and
  `conservative-cfg-cleanup` respectively. Numeric identities remain private.
- Fold only the frozen integer and boolean operation/cast set with explicit
  wrapping and width semantics. Do not fold floating, division, remainder,
  shift, checked conversion, load, call, query, ownership, or failure forms.
- Keep scalar facts instruction-ordered, block-local, pass-local, and invalid
  after every rewrite. Do not infer `StorageId` contents.
- Preserve result identity, type, instruction position, and span when a
  constant or algebraic literal can replace an rvalue in place.
- Forward a value only after exact type, same-block definition order, and
  exhaustive forwarding-safe use roles are proven. Delete the obsolete
  assignment and value declaration atomically; add no copy rvalue.
- Keep proof, checked-protocol, lifecycle, ownership, and unknown use roles as
  forwarding barriers.
- Rewrite only ordinary constant or same-target `Branch` terminators to
  `Goto`. Never rewrite a dedicated checked or multiway terminator.
- Root body entry, static publication/lifecycle attachments, and every block
  named by proof metadata before local executable reachability is solved.
- Remove only unprotected unreachable blocks and transient values defined in
  them. Retain storage declarations, path conditions, logical records, guards,
  and callable attachments.
- Add no proof normalization, empty-block forwarding, block merging, jump
  threading, checked-diamond simplification, floating evaluator, storage
  propagation, alias/effect framework, SSA, inlining, target LIR, or register
  allocation.
- Keep reports structured and compact, verified MIR checkpoints separate,
  passes free of logging and filesystem work, and ordinary disabled
  observation allocation-free where it is today.
- Keep `mod.rs` files as concise facades and tests with their responsibility
  owners.
- Keep the root Makefile as the local and external quality-gate interface; add
  no repository CI.

## Progress

- [x] LSR0 — Implement exact primitive constant semantics
- [x] LSR1 — Add block-local facts and primitive constant folding
- [x] LSR2 — Classify forwarding-safe value uses exhaustively
- [x] LSR3 — Implement primitive algebraic simplification
- [x] LSR4 — Establish proof-aware local CFG reachability
- [x] LSR5 — Implement conservative CFG cleanup
- [x] LSR6 — Activate the repeated selectable default schedule
- [ ] LSR7 — Prove semantic parity, determinism, and optimization value
- [ ] LSR8 — Harden ownership, documentation, and roadmap closure

## PR-sized implementation sequence

### LSR0 — Implement exact primitive constant semantics

**Purpose:** Establish one target-independent semantic owner for the closed
integer and boolean folding set before any production pass can change MIR.

- [x] Add a focused optimizer-private typed constant representation for
      `i64`, `u64`, `u8`, and `bool` without duplicating general `MirType` or
      exposing a public constant-evaluation API.
- [x] Implement explicit wrapping addition, subtraction, multiplication, and
      `i64` negation; width-correct integer bitwise operations and complement;
      boolean not; integer and boolean comparisons; identity casts;
      integer-bit conversions; integer-to-boolean zero testing; and canonical
      boolean-to-integer conversion.
- [x] Canonicalize every `u8` result explicitly and avoid ordinary Rust
      arithmetic whose debug and release behavior differs.
- [x] Return a closed “unsupported” result for every operation/type family
      outside the frozen set rather than panicking or inferring semantics.
- [x] Keep floating literals observable as unsupported inputs; do not convert
      through host `f64` arithmetic.
- [x] Use exhaustive operation matches and add a maintenance test or compile-
      time owner so new primitive variants cannot silently become foldable.
- [x] Organize evaluator, typed constants, and tests behind a concise
      optimization-support facade with no pass, pipeline, driver, reporting,
      or backend dependency.
- [x] Advance the three planned optimization-register entries from
      **Proposed** to **In progress** when this first implementation task
      begins.
- [x] Update living compiler documentation only if implementation details
      sharpen, without expanding the frozen operation set.

**Tests:** Every supported operation/type pair; signed and unsigned extrema;
wrapping overflow; `i64::MIN` negation; all `u8` inputs and practical binary
combinations; signed/unsigned comparison boundaries; canonical booleans;
identity and width casts; explicit division, remainder, shift, floating,
checked-conversion, load, and query rejection; debug/release-independent
expected results.

**Gates:** Focused optimizer-support tests; `cargo test --locked -p
skald-compiler passes::pipeline`; `make fmt-check`; `make lint`; `make
docs-check`; and `git diff --check`.

**Exit criteria:** One small target-independent owner computes exactly the
frozen integer/boolean semantics with exhaustive boundary tests, while MIR,
the production registry, default schedule, reports, and generated artifacts
remain unchanged.

### LSR1 — Add block-local facts and primitive constant folding

**Purpose:** Prove the evaluator and existing rewrite boundary with the first
new independently selectable scalar pass while leaving default behavior
unchanged.

- [x] Add a borrowed linear block-local fact builder that maps only already
      defined transient values to supported typed constants and resets at each
      block boundary.
- [x] Scan instructions in stable order and make an in-place folded result
      immediately available to later instructions in the same block.
- [x] Preserve assignment result identity, declared type, instruction order,
      and source span while replacing only the eligible rvalue kind.
- [x] Inspect verified dense MIR before consuming the rewrite capability so a
      candidate-free occurrence returns unchanged without another
      verification execution.
- [x] Recompute any position-keyed facts after mutation rather than retaining
      stale instruction indices.
- [x] Implement pass-owned processed/changed callable counts and separate
      unary, binary, comparison, and cast fold measurements.
- [x] Register private identity 2 under the stable name
      `primitive-constant-folding`, exposing it through pass listing and exact
      internal schedules while leaving `default` unchanged.
- [x] Keep every checked diamond, terminator, load, storage, proof record,
      lifecycle operation, and unsupported rvalue byte-for-byte unchanged.
- [x] Keep module organization facade-oriented and reuse evaluator fixtures
      rather than rebuilding semantic tables inside pass tests.

**Tests:** Straight-line constant chains; no facts across blocks; every
supported folded kind; unsupported families unchanged; result identity/type/
position/span preservation; deterministic measurements; no-candidate seal and
verification preservation; listing and exact-schedule selection; repeated
occurrences; malformed rewrite attribution; existing dead-pure and reachability
composition.

**Gates:** Focused pass and policy tests; `cargo test --locked -p
skald-compiler passes::pipeline`; `make compiler-test`; `make cli-test`; `make
fmt-check`; `make lint`; `make docs-check`; and `git diff --check`.

**Exit criteria:** The registered constant-folding pass transforms exactly the
frozen local family under internal exact schedules, reports deterministic
facts, preserves unsupported MIR, and is not yet selected by `default`.

### LSR2 — Classify forwarding-safe value uses exhaustively

**Purpose:** Establish the semantic eligibility query required for deleting an
algebraic result without letting the broad substitution mapper decide proof or
protocol safety.

- [x] Extend immutable local-identity observation with a deterministic query
      that enumerates every use site of a selected `ValueId`, not merely its
      count.
- [x] Define focused semantic use roles for ordinary scalar rvalues, casts,
      stores, calls/arguments, returns, branches, checked terminators,
      proof/path/logical metadata, ownership/lifecycle operations, and unknown
      future roles.
- [x] Mark only the frozen ordinary executable roles as forwarding-safe.
- [x] Reject path-condition and logical metadata, dedicated checked
      terminators, proof-coupled success rvalues, lifecycle/ownership state,
      callable attachments, and every unknown role.
- [x] Preserve the existing value census as the compact count/definition API;
      share exhaustive traversal without turning one result type into a
      catch-all analysis manager.
- [x] Prove exact callable ownership, same-block definition/use locality, and
      deterministic site order with structured failures for foreign, unknown,
      deleted, or malformed values.
- [x] Add exhaustive maintenance coverage so every new value-bearing MIR
      variant must choose a role explicitly.
- [x] Keep the query read-only and invalid after any rewrite.

**Tests:** Every currently value-bearing instruction, rvalue, terminator, and
metadata record; multiple ordinary uses; metadata-only uses; mixed safe and
unsafe uses; same-block order; definition-versus-use distinction; foreign,
unknown, deleted, and duplicate definitions; dense and sparse edit parity;
deterministic site ordering.

**Gates:** `cargo test --locked -p skald-compiler mir::rewrite`; `cargo test
--locked -p skald-compiler mir::verify`; focused pipeline tests; `make
fmt-check`; `make lint`; `make docs-check`; and `git diff --check`.

**Exit criteria:** Algebraic passes can ask one exhaustive borrowed query
whether all uses of a result are forwarding-safe, while no production MIR,
pass schedule, or generated program changes.

### LSR3 — Implement primitive algebraic simplification

**Purpose:** Add the frozen reviewed identity catalog and prove atomic value
forwarding/deletion through real production pass machinery.

- [x] Encode the frozen add, subtract, multiply, bitwise, comparison, and unary
      involution rules in one auditable integer/boolean catalog.
- [x] Construct zero, one, and all-ones constants with exact encoded width and
      preserve canonical `u8` and `bool` results.
- [x] For constant-result identities, retain the assignment, result identity,
      declared type, instruction position, and source span.
- [x] For operand-result identities, prove exact type equality, earlier source
      definition in the same block, and that every result use is forwarding-
      safe before mutation.
- [x] Replace uses, delete the obsolete assignment, and delete its value
      declaration in one callable transaction; add no copy rvalue.
- [x] Preserve all operand-producing instructions. Let the existing dead-pure
      pass independently decide whether newly unused producers are removable.
- [x] Select candidates deterministically and rebuild the use and instruction-
      position facts after every structural deletion.
- [x] Return deterministic constant-result, forwarded-use, removed-assignment,
      removed-value, changed-callable, and protected-rejection measurements.
- [x] Register private identity 3 under
      `primitive-algebraic-simplification`, exposing listing and exact internal
      schedules while leaving `default` unchanged.

**Tests:** Every identity for `i64`, `u64`, and `u8`; integer/bool self-
comparisons; unary involutions; width-specific constants; multiple forwarded
uses; mixed safe/unsafe uses; proof and checked barriers; floating exclusion;
operand evaluation retained; dead-pure cleanup composition; dense value
recompaction; repeated occurrence stability; spans and measurement order.

**Gates:** Focused algebraic, rewrite, and pipeline tests; `make
compiler-test`; `make cli-test`; `make fmt-check`; `make lint`; `make
docs-check`; and `git diff --check`.

**Exit criteria:** The registered algebraic pass implements only the frozen
catalog, forwards and deletes only exhaustively safe results, composes with
constant and dead-pure passes under exact schedules, and remains absent from
`default`.

### LSR4 — Establish proof-aware local CFG reachability

**Purpose:** Build the reusable immutable roots, successors, reachability, and
block-owned value facts needed before a pass deletes any CFG structure.

- [x] Define a read-only callable-local CFG view over deterministic block order
      and all current executable successor variants.
- [x] Collect the body entry and every callable-level lifecycle/publication
      attachment block through one narrow definition-variant facade.
- [x] Collect every block named by path-condition, logical-expression, and
      other non-executable local proof metadata through exhaustive identity
      observation.
- [x] Distinguish executable entry roots, protected metadata roots, ordinary
      successor closure, reachable blocks, and protected-but-entry-unreachable
      blocks in focused immutable facts.
- [x] Use the shared value definition census to identify every transient value
      defined by instructions in a selected block without duplicating a list of
      value-producing instruction variants.
- [x] Return structured failures for foreign, unknown, deleted, or malformed
      roots, successors, attachments, values, and definitions.
- [x] Keep facts callable-local and pass-local; add no cached dominators,
      liveness, loop forest, or global analysis manager.
- [x] Provide deterministic focused dumps only if structured test assertions
      cannot make root/reachability failures sufficiently clear.

The structured root sites, ordered adjacency, closure partitions, and typed
rewrite failures proved sufficient for focused assertions, so LSR4 adds no
second textual CFG dump format.

**Tests:** Ordinary function/member entry; static initialization and shutdown
attachments; every path/logical block role; every terminator successor family;
loops and disconnected components; protected unreachable regions; block-owned
assignment/call/I/O and other result values; malformed identities; stable root
and closure order; dense/edit analysis parity.

**Gates:** Focused CFG-fact and rewrite tests; `cargo test --locked -p
skald-compiler mir`; `cargo test --locked -p skald-compiler
passes::static_lifecycle`; `make fmt-check`; `make lint`; `make docs-check`;
and `git diff --check`.

**Exit criteria:** One borrowed deterministic query identifies removable
unreachable blocks and their transient definitions while conservatively
rooting every current proof, publication, and lifecycle attachment; no CFG is
yet changed.

### LSR5 — Implement conservative CFG cleanup

**Purpose:** Exercise safe structural control-flow deletion without introducing
proof normalization or rewriting any checked protocol.

- [x] Detect ordinary `Branch` conditions that resolve to a preceding
      block-local boolean constant and branches whose two targets are equal.
- [x] Reject branch rewriting when the branch block is itself protected by
      proof, lifecycle, or publication metadata.
- [x] Replace eligible ordinary branches with `Goto` to the selected target
      while preserving the original terminator span.
- [x] Leave every dedicated divisor, shift, cast, optional, shared, array,
      ownership, loop, and other checked/multiway terminator unchanged.
- [x] Recompute local CFG reachability after branch edits from the complete
      root set.
- [x] Remove only unprotected unreachable blocks and every transient value
      defined inside them in the same callable transaction.
- [x] Retain all storage declarations, path conditions, logical records,
      guards, attachments, and protected unreachable regions.
- [x] Return deterministic constant-branch, same-target-branch, removed-block,
      removed-value, protected-unreachable, and changed-callable measurements.
- [x] Register private identity 4 under `conservative-cfg-cleanup`, exposing
      listing and exact internal schedules while leaving `default` unchanged.

**Tests:** Constant true/false and same-target branches; branch-result cleanup;
every dedicated terminator excluded; body and attachment roots; path/logical
protection; disconnected loops; removed blocks containing every result
producer family; retained storage; preserved spans; deterministic dense block/
value maps; ordinary, lifetime, ownership, optional, array, static-lifecycle,
and reachability reverification; repeated schedules.

**Gates:** Focused CFG-pass, rewrite, verifier, and pipeline tests; `make
compiler-test`; `make cli-test`; `make fmt-check`; `make lint`; `make
docs-check`; and `git diff --check`.

**Exit criteria:** The registered CFG pass folds only frozen ordinary branches,
removes only unprotected unreachable block/value regions, survives all central
verification, and remains absent from `default`.

### LSR6 — Activate the repeated selectable default schedule

**Purpose:** Make the independently proven passes compose under the frozen
production policy and update all public selection and observation surfaces at
one controlled boundary.

- [x] Change `default` to the exact frozen sequence: dead-pure; primitive
      constant folding; primitive algebraic simplification; primitive constant
      folding; dead-pure; conservative CFG cleanup; dead-pure; whole-world
      reachability.
- [x] Keep `none` empty and preserve the rule that stable-name exclusion removes
      every occurrence of a repeated pass.
- [x] Update registry/profile validation, exact occurrence numbers, public
      pass listing, lexical known-name diagnostics, request equality, CLI
      help/tests, and all-disabled parity.
- [x] Prove that whole-world reachability remains last and observes calls and
      other executable dependencies removed by prior CFG cleanup.
- [x] Integrate every frozen pass-specific measurement with aggregate and trace
      reporting without duplicating structural commit counts.
- [x] Preserve disabled observation behavior, verified checkpoint labels, and
      immediate resealing after each changed occurrence.
- [x] Update living compiler phase, driver, and reporting documentation from
      frozen direction to exact implemented behavior where this task makes it
      current.
- [x] Confirm the three optimization-register entries remain **In progress**
      and link to the active roadmap until roadmap closure.

**Tests:** Exact default schedule and occurrence numbers; lexical listing;
individual and combined exclusions; duplicate exclusions; all-disabled equals
`none`; request/CLI errors before I/O; aggregate counter order; trace event
order; checkpoint labels; changed/unchanged verification counts; failed
occurrence attribution; reachability after CFG removal.

**Gates:** `cargo test --locked -p skald-compiler passes::pipeline`; `make
cli-test`; `make golden-expectations-test`; `make compiler-test`; `make
static-check`; and `git diff --check`.

**Exit criteria:** Every supported compiler adapter selects the frozen default
schedule, every pass remains independently disableable by stable name, reports
and checkpoints identify repetitions exactly, and `none` remains the exact
unoptimized reference.

### LSR7 — Prove semantic parity, determinism, and optimization value

**Purpose:** Establish broad evidence that the enabled suite changes only
permitted MIR/artifact structure and provides measurable simplification across
real programs.

- [ ] Add focused golden programs covering integer/bool folding, algebraic
      forwarding, branch folding, disconnected CFG, proof-protected CFG,
      static initialization/shutdown, and whole-world definitions exposed by
      removed call sites.
- [ ] Assert optimization-off exact MIR parity and deterministic optimized MIR
      dumps, identity compaction, measurements, reports, and assembly across
      repeated and independent-process runs.
- [ ] Prove native stdout, stderr, exit status, panic reason/span, runtime
      trace, allocation, ownership, cleanup, destruction, optional, shared,
      array, function-value, dispatch, and static-lifecycle equivalence.
- [ ] Cover wrapping extrema and `u8` canonicalization in debug and release
      compiler builds.
- [ ] Compare pass-enabled, individually disabled, all-disabled, and `none`
      products without relying on wall-clock timings.
- [ ] Add representative before/after MIR, instruction, block, value, and
      executable-definition measurements sufficient to assess follow-up
      candidates.
- [ ] Record checked diamonds, floating evaluation, proof normalization, load/
      store reasoning, or other out-of-scope opportunities in the discoveries
      record and optimization register rather than extending pass scope.
- [ ] Update compiler test guidance and focused living documentation for the
      new golden/measurement surfaces.

**Tests:** Focused and full debug goldens; release goldens; independent-process
golden determinism; native execution across the existing scalar, control-flow,
class, dispatch, function-value, static, optional, shared, array, panic, and
runtime-trace matrices.

**Gates:** `make test`; `make golden-release-test`; `make
golden-determinism-test`; `make static-check`; `make msrv-check`; and `git
diff --check`.

**Exit criteria:** Broad source-to-native evidence demonstrates semantic
equivalence, all deterministic products are stable, and measured structural
changes explain where later optimization work is likely to pay off.

### LSR8 — Harden ownership, documentation, and roadmap closure

**Purpose:** Complete the maintainability audit and leave only authoritative
current behavior, actionable discoveries, and implemented catalog entries.

- [ ] Audit evaluator, fact, use-site, CFG, pass, registry, schedule,
      measurement, and test modules by responsibility; split only genuine
      mixed owners and keep facades concise.
- [ ] Remove temporary compatibility helpers, duplicated semantic tables,
      stale default-schedule wording, and roadmap codes from living code,
      tests, and non-roadmap documentation.
- [ ] Confirm every frozen exclusion still holds and that no checked, floating,
      storage, proof-normalization, ownership, or target-specific optimization
      entered implicitly.
- [ ] Reconcile discoveries with the optimization register, retaining detailed
      actionable evidence in the discoveries record and concise placement/
      effort/value summaries in the catalog.
- [ ] Advance primitive constant folding, primitive algebraic simplification,
      and conservative CFG cleanup catalog entries to **Implemented** and link
      them to the promoted living contracts.
- [ ] Run the complete repository validation from an artifact-free snapshot or
      clean checkout, plus supported MSRV and independent-process determinism
      gates.
- [ ] Mark every task complete, set roadmap status complete, move the frozen
      proposal and completed roadmap to `docs/archive/`, update active/archive
      indexes, and repair all incoming links.
- [ ] Archive or remove the discoveries record only if no actionable finding
      remains; otherwise keep and index it under `docs/roadmaps/`.

**Tests:** Full repository check, extended deterministic goldens, supported
MSRV, documentation links/indexes, diff hygiene, archive links, and a manual
audit that current registry/profile/report documentation matches code.

**Gates:** `make check`; `make check-long`; `make msrv-check`; `make
docs-check`; `git diff --check`; and repository-status review from an
artifact-free snapshot.

**Exit criteria:** The suite is fully implemented and documented, the catalog
marks its passes implemented, no reviewed exclusion was silently crossed, all
gates pass, completed records are archived, and remaining discoveries have
clear future owners.

## Ordering and dependencies

The evaluator lands before facts and transformations so arithmetic semantics
have one independently testable owner. Constant folding then proves in-place
rewriting before structural value deletion is attempted. Exhaustive use roles
land before algebraic forwarding, and read-only CFG roots/reachability land
before any block is deleted. Each pass registers only after its own semantics
are focused and tested, while the public default remains unchanged until all
three transformations compose under exact internal schedules.

Default activation precedes broad golden hardening so the complete production
path, repeated occurrence identities, reporting, checkpoints, and final
whole-world retention are tested together. Closure comes last because catalog
status, living contracts, archive movement, and discoveries can be accurate
only after implementation and full validation are complete.

LSR0 through LSR5 are sequential at their semantic boundaries. Documentation
fixtures and corpus preparation may proceed alongside focused implementation
only when they do not presume an unimplemented representation or widen the
frozen operation set.
