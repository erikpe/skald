# Checked Integer Constant Protocol Simplification Roadmap

Status: in progress; CIR0 is complete and CIR1 is next.

This roadmap implements optimization-catalog candidates FMC-01 and FMC-02 as
one independently selectable target-independent final-MIR pass. It folds fully
constant, statically successful integer division, remainder, and shift
protocols while preserving Skald's exact arithmetic, operand evaluation,
failure, source-span, identity, lifecycle, verification, and deterministic
pipeline contracts.

The durable result is larger than three constant-evaluation cases. It adds a
narrow reusable model for observing and atomically rewriting one verified
checked scalar protocol without turning ordinary block-local facts into a
general storage analysis or requiring proof-provenance normalization.

Implementation-specific findings that do not belong in this reviewed scope go
in the
[checked-integer protocol discoveries record](CHECKED_INTEGER_CONSTANT_PROTOCOL_SIMPLIFICATION_DISCOVERIES.md).
Candidate placement and status remain authoritative in the
[optimization candidate catalog](OPTIMIZATION_CANDIDATE_CATALOG.md).

## Dependencies

- The completed
  [local final-MIR simplification roadmap](../archive/LOCAL_FINAL_MIR_SIMPLIFICATION_ROADMAP.md)
  provides exact typed primitive constants, block-local folding, the repeated
  scalar schedule, conservative CFG cleanup, and structural measurements.
- The completed
  [dense MIR identity rewriting roadmap](../archive/DENSE_MIR_IDENTITY_REWRITING_ROADMAP.md)
  provides sparse callable transactions, instruction-list and terminator
  replacement, value deletion, deterministic dense commit, and immediate
  resealing.
- The completed
  [selectable final-MIR pipeline roadmap](../archive/SELECTABLE_FINAL_MIR_OPTIMIZATION_PIPELINE_ROADMAP.md)
  provides stable pass registration, exact profiles and exclusions,
  pass-attributed failures, measurements, and verified checkpoints.
- Existing checked integer lowering and verification are authoritative for the
  divisor-check and shift-count-check protocol shapes. The optimizer must
  consume those MIR semantics rather than infer source syntax or duplicate a
  looser checked representation.
- Niflheim's backend constant folder is useful evidence for arithmetic cases
  and pass placement, but its register SSA-like input has no equivalent of
  Skald's proof-bearing carrier diamond. Skald therefore requires its own
  protocol-aware eligibility and rewrite boundary.

## Scope and invariants

- Add one stable selectable final-MIR pass named
  `checked-integer-constant-folding`; its numeric identity remains private.
- Fold quotient and remainder protocols only when both exact integer operands
  are constants and the divisor is nonzero.
- Fold shift protocols only when the exact integer left operand and `u64`
  count are constants and the count is below the selected operand width.
- Cover `i64`, `u64`, and canonical `u8`; distinguish quotient from remainder,
  left from right shift, arithmetic from logical right shift, and widths 64
  and 8.
- Implement signed floor quotient and divisor-sign remainder exactly,
  including the defined `i64::MIN / -1 == i64::MIN` and
  `i64::MIN % -1 == 0` pair. Host debug/release overflow behavior must never
  select a result.
- Preserve left-to-right, exactly-once completion of both operands and all
  pre-existing operand effects or failures. Optimization may remove only the
  checked operation and structural work proven private to its successful
  protocol.
- Preserve the checked-expression span on the replacement constant and on the
  ordinary successor edge replacing the check. Do not move failures to compile
  time or change runtime-trace location identities.
- Recover constants only through the checked terminator's verified
  `ScalarSpill` carriers and their unique dominating stores. This is a narrow
  immutable protocol query, not general load/store propagation, alias
  analysis, or a persistent dataflow framework.
- Require the exact verifier-owned check, success, failure, result-store, and
  join relationship. Reject malformed, stale, shared, protected, or otherwise
  noncanonical topology instead of partially rewriting it.
- Rewrite one successful checked protocol atomically: replace the dedicated
  check by an ordinary `Goto` to its success block, replace its checked
  operation by an exact constant while retaining that result identity and
  span, and delete only now-unused protocol-private operand-load values.
- Initially retain operand and result carrier declarations, their stores,
  lifecycle instructions, the success block, and the result reload. Existing
  cleanup passes may remove the newly unreachable failure block. Carrier
  elimination or block merging requires separately proven storage and CFG
  rules.
- Keep statically failing zero-divisor and out-of-range-shift protocols
  unchanged. Direct failure folding is a later candidate because it must prove
  continuation, cleanup, panic-span, and trace equivalence.
- Keep a dynamic operation unchanged when only its divisor or count is a known
  valid constant. Removing that check requires a verified unchecked/proven
  operation representation or proof normalization and is outside FMC-01 and
  FMC-02.
- Do not fold checked binary64-to-integer conversion, floating arithmetic,
  type tests, optional or array checks, memory operations, calls, ownership,
  or target-specific instructions.
- Do not delete or normalize path conditions, logical-expression records,
  guards, static-publication attachments, or lifecycle metadata. Reject a
  candidate whose protocol blocks are protected by those owners.
- Keep `none` byte-for-byte equivalent to the verification-only path. Stable
  per-pass exclusion removes every occurrence and restores the existing
  unchecked-optimization baseline for this feature.
- Keep the pass target independent, deterministic, free of logging and
  filesystem work, and mediated by the existing verified capability and
  atomic rewrite transaction.
- Keep `mod.rs` files concise facades and tests beside the semantic owner.
  The root Makefile remains the repository and external quality-gate surface;
  add no repository CI.

## Progress

- [x] CIR0 — Implement exact checked-integer constant evaluation
- [ ] CIR1 — Model verified checked-protocol candidates
- [ ] CIR2 — Add atomic successful-protocol rewriting
- [ ] CIR3 — Fold constant integer division and remainder protocols
- [ ] CIR4 — Fold constant integer shift protocols
- [ ] CIR5 — Register selection and structured measurements
- [ ] CIR6 — Activate and compose the default schedule
- [ ] CIR7 — Prove semantic parity, determinism, and optimization value
- [ ] CIR8 — Harden ownership, documentation, and roadmap closure

## PR-sized implementation sequence

### CIR0 — Implement exact checked-integer constant evaluation

**Purpose:** Establish one optimizer-private semantic owner for successful
division, remainder, and shift results before any CFG protocol can change.

- [x] Reuse the implemented typed `PrimitiveConstant` representation without
      widening the ordinary total-rvalue evaluator to control-affecting
      operations.
- [x] Add a focused checked-integer evaluator whose outcome distinguishes an
      exact successful constant, a statically failing check, and an unsupported
      type/operation pair.
- [x] Implement unsigned quotient and remainder for `u64` and canonical `u8`.
- [x] Implement `i64` floor quotient and divisor-sign remainder without host
      overflow, including every sign combination and the signed-minimum pair.
- [x] Implement wrapping left shift, arithmetic `i64` right shift, logical
      `u64`/`u8` right shift, exact 64/8-bit count validation, and explicit
      `u8` canonicalization.
- [x] Keep zero divisors and excessive counts represented as static failure
      outcomes rather than panics, Rust arithmetic traps, or folded values.
- [x] Keep evaluator inputs and outputs independent of MIR topology, target
      instructions, spans, reporting, and filesystem state.

**Tests:** Exhaustive operation/type matrices; signed quotient/remainder sign
quadrants; zero, one, extrema, the signed-minimum pair, and wrapping results;
shift counts at zero, width minus one, width, and `u64::MAX`; right-shift
signedness; `u8` canonicalization; unsupported-pair rejection; debug/release
agreement; `cargo test --locked -p skald-compiler`; and the repository
formatting check.

**Exit criteria:** Every in-scope successful constant result and every
out-of-scope or statically failing input has one deterministic, independently
tested evaluator outcome with no dependency on checked CFG shape.

**Completion evidence:** The optimizer-private evaluator reuses typed
primitive constants and the existing byte canonicalization rule. Its focused
matrix covers exact success, exact static failure, unsupported inputs, signed
extrema, every byte quotient/remainder input, every valid byte shift, and
boundary shift counts in both debug and release builds. The full compiler test
suite and static checks accept the new semantic owner without changing the
active optimization schedule.

### CIR1 — Model verified checked-protocol candidates

**Purpose:** Recover fully constant operands and exact protocol topology
without introducing general storage propagation or duplicating mutable MIR.

- [ ] Add one immutable optimizer-private protocol-candidate model with
      division/remainder and shift variants and exact check, success, failure,
      and join block identities.
- [ ] Record the dedicated terminator, operation, carrier storages, result
      storage, protocol-private operand loads, checked result assignment, store,
      successor edge, and source spans required for a later atomic rewrite.
- [ ] Add a narrow carrier-source query that accepts only exact
      `ScalarSpill` storage with one dominating store whose source is an exact
      typed constant assignment after ordinary primitive folding.
- [ ] Require matching operand/result types, verifier-owned success shape,
      unique predecessors/writes, terminal failure reason, and result reload.
- [ ] Consult existing local CFG roots and reject protocols whose relevant
      blocks are protected by proof, logical, lifecycle, or publication
      metadata.
- [ ] Keep query results immutable and seal-local; recompute them after every
      rewrite instead of caching or attaching them to MIR.
- [ ] Return structured non-candidate reasons for static failure, dynamic
      operand, noncanonical topology, protected topology, and unsupported
      operation without treating an ordinary miss as compiler failure.

**Tests:** Read-only candidate discovery for all operation/type variants;
direct and primitive-folded constants; dynamic and partially constant
operands; nested/control-affecting operands; duplicate or nondominating carrier
writes; mismatched success/failure shapes; protected blocks; deterministic
candidate order; unchanged input MIR; and focused malformed-identity errors.

**Exit criteria:** A borrowed verified callable deterministically identifies
only exact, fully constant, statically successful protocols and supplies every
identity needed by rewriting, with no general memory fact escaping the query.

### CIR2 — Add atomic successful-protocol rewriting

**Purpose:** Make the multi-entity transformation one coherent operation that
cannot expose an unchecked division or shift to verification.

- [ ] Add a pass-private rewrite transaction shared by division/remainder and
      shift candidates; extend the generic callable edit facade only if one
      genuinely reusable structural primitive is missing.
- [ ] Revalidate the candidate against live sparse edit state before mutation
      and return a structured rewrite error for a stale or mismatched snapshot.
- [ ] Replace the dedicated checked terminator with a span-preserving `Goto`
      to the existing success block.
- [ ] Rewrite the checked success assignment in place to the exact constant,
      preserving its result `ValueId`, declared type, instruction position, and
      checked-expression span.
- [ ] Remove the two now-unused success operand-load instructions and matching
      value declarations in the same callable transaction.
- [ ] Retain the result store, join reload, storage declarations and lifecycle,
      and all proof/lifecycle records; do not merge blocks or delete the
      failure region in this transaction.
- [ ] Commit once per changed callable, compact all local identities
      deterministically, invalidate seal-bound facts, and rely on central
      immediate final-MIR and lifecycle-realization verification.

**Tests:** Atomic success and failure; retained result identity semantics;
removed load-value declarations; dense remapping of later values and blocks;
foreign/deleted/stale identity errors; unchanged storage/proof/lifecycle
records; deterministic commit maps; and verifier acceptance only after the
complete transaction.

**Exit criteria:** One exact candidate can become verifier-valid ordinary MIR
without any intermediate unchecked operation, dangling identity, or partial
checked protocol.

### CIR3 — Fold constant integer division and remainder protocols

**Purpose:** Deliver FMC-01 over all Skald integer types using the shared
evaluation, candidate, and rewrite owners.

- [ ] Implement the division/remainder transformation over deterministic
      callable and block order.
- [ ] Fold `i64`, `u64`, and `u8` quotient protocols with nonzero constant
      divisors.
- [ ] Fold the matching remainder protocols with exact unsigned or
      divisor-sign semantics.
- [ ] Cover the defined signed-minimum quotient/remainder pair without
      producing a failure or depending on a host division instruction.
- [ ] Leave zero-divisor protocols, one-dynamic-operand protocols, noncanonical
      or protected protocols, and all other checked families byte-for-byte
      unchanged by this transformation.
- [ ] Preserve operand computation and carrier-store order even when their
      values become structurally redundant.

**Tests:** Focused MIR fixtures for all six operation/type combinations and
sign/extrema cases; folded constants exposed by the earlier primitive pass;
zero-divisor and partially constant exclusions; side-effecting/failing operand
order; nested checked expressions; exact spans; compacted identities;
verification; and repeat-run idempotence.

**Exit criteria:** Every eligible constant quotient and remainder protocol
folds exactly once, while every failure-bearing or insufficiently proven case
retains the original checked behavior.

### CIR4 — Fold constant integer shift protocols

**Purpose:** Deliver FMC-02 through the same checked-protocol boundary without
duplicating division-specific machinery.

- [ ] Implement left and right shift transformation for `i64`, `u64`, and
      `u8` with an exact constant `u64` count.
- [ ] Accept counts `0..=63` for 64-bit operands and `0..=7` for `u8`.
- [ ] Preserve wrapping left-shift semantics, arithmetic signed right shift,
      logical unsigned right shift, and canonical `u8` results.
- [ ] Leave count-at-width, larger-count, partially constant, noncanonical,
      protected, and other checked protocols byte-for-byte unchanged by this
      transformation.
- [ ] Share protocol discovery, rewrite, outcome, and accounting vocabulary
      with division/remainder instead of introducing a second pass framework.

**Tests:** All six direction/type combinations; zero and maximum valid counts;
high-bit signed and unsigned right shifts; discarded high bits; `u8`
canonicalization; count-at-width and `u64::MAX` exclusions; operand effects;
nested checks; spans; verification; deterministic rewriting; and idempotence.

**Exit criteria:** Every eligible constant shift protocol folds through the
shared transaction with exact fixed-width semantics and no statically failing
count is optimized away.

### CIR5 — Register selection and structured measurements

**Purpose:** Expose the completed transformation through the existing modular
pipeline without changing public optimization categories.

- [ ] Register stable name `checked-integer-constant-folding` with one private
      identity and a concise description in the canonical production registry.
- [ ] Keep division, remainder, and shift as one pass because they share the
      checked scalar protocol boundary, while retaining separate internal
      evaluators/matchers where their semantics differ.
- [ ] Report deterministic counts for processed and changed callables, folded
      quotient, remainder, and shift protocols, removed protocol-load values,
      and retained statically failing candidates.
- [ ] Keep commit statistics authoritative for total entity retention/removal;
      do not reconstruct generic rewrite totals in pass-owned metrics.
- [ ] Update pass listing, lexical known-name diagnostics, per-pass disabling,
      exact-schedule tests, reporting, checkpoint, and public descriptor
      expectations.
- [ ] Prove the disabled pass performs no protocol observation or rewrite work
      beyond ordinary registry/schedule resolution.

**Tests:** Registry uniqueness and ordering; `--list-mir-passes`; public pass
descriptors; unknown-name diagnostics; exact-name disabling; pass-attributed
failure; metric names/order/counts; checkpoint labels; quiet/detail/trace
allocation boundaries; `make cli-test`; and focused compiler tests.

**Exit criteria:** Tools can discover, select, exclude, inspect, and attribute
the checked-integer pass through the existing stable pipeline interfaces, with
no new request field, CLI category, or dynamic pass API.

### CIR6 — Activate and compose the default schedule

**Purpose:** Place checked protocol folding where ordinary scalar folding can
expose constants and existing cleanup can consume the resulting ordinary CFG.

- [ ] Insert one checked-integer pass occurrence after the second primitive
      constant-folding occurrence and before the following dead-pure and
      conservative CFG cleanup occurrences.
- [ ] Keep whole-world reachability last and retain all existing relative
      ordering among dead-pure, primitive folding, algebraic simplification,
      CFG cleanup, and whole-world retention.
- [ ] Prove the earlier scalar passes expose direct constant carrier sources to
      checked folding, then conservative CFG cleanup removes the now-
      unreachable failure block.
- [ ] Prove exact `none`, checked-pass-disabled, every existing local-pass-
      disabled, reachability-disabled, and all-six-passes-disabled behavior.
- [ ] Keep repeated occurrence numbering, checkpoint labels, verification
      counts, measurement aggregation, and deterministic schedule dumps exact.
- [ ] Update living compiler, driver, reporting, and testing documentation in
      the same change that activates the default.

**Tests:** Exact default schedule; selective and all-pass exclusions; `none`
parity; per-occurrence outcomes and numbering; after-pass/final verified dumps;
composition with dead-pure, scalar folding, CFG cleanup, and reachability;
structured failure cutoffs; and deterministic report ordering.

**Exit criteria:** The default profile folds eligible checked constants and
cleans their failure blocks in the intended order, while every supported
exclusion produces a verified deterministic product.

### CIR7 — Prove semantic parity, determinism, and optimization value

**Purpose:** Demonstrate that eliminating checked protocols changes neither
observable source behavior nor permanent whole-world/single-threaded lifecycle
semantics and produces a measurable structural win.

- [ ] Add a focused checked-integer optimization golden covering successful
      `i64`, `u64`, and `u8` division, remainder, left shift, and right shift
      under default, `none`, checked-pass-disabled, CFG-disabled, and all-pass-
      disabled variants.
- [ ] Cover sign quadrants, zero, extrema, the signed-minimum pair, shift width
      boundaries, high-bit right shifts, and canonical byte results through
      native execution.
- [ ] Retain zero-divisor, remainder-by-zero, and excessive-shift panic
      fixtures with exact status, stderr, reason, source span, runtime trace,
      operand order, and optimization-on/off equivalence.
- [ ] Include effectful, failing, dynamic, partially constant, nested, logical,
      static-initializer, ownership, and destruction contexts that must either
      preserve operand behavior or remain unoptimized.
- [ ] Pin deterministic before/after block, instruction, value, and checked-
      terminator counts plus pass-owned reason metrics and final MIR dumps.
- [ ] Prove the optimized backend input contains no folded integer
      division/shift operation or dedicated check and that disabled/`none`
      products retain them.
- [ ] Run focused debug, release, and independent-process deterministic
      goldens, followed by the complete ordinary and extended repository
      gates.

**Tests:** Focused unit/integration/golden suites; the `optimizations/**`
golden filter; focused full determinism; `make check`; `make
golden-release-test`; `make golden-determinism-test`; `make static-check`;
`make msrv-check`; and `git diff --check`.

**Exit criteria:** Native behavior and failure observations are identical
across optimization settings, deterministic products are stable across
processes and build modes, and structural measurements show the intended
checks, operations, load values, and failure blocks disappearing.

### CIR8 — Harden ownership, documentation, and roadmap closure

**Purpose:** Finish with cohesive semantic owners, authoritative current
documentation, reconciled discoveries, and an artifact-free validation record.

- [ ] Audit checked evaluation, protocol observation, rewriting, pass,
      registry, schedule, measurement, and test modules by responsibility;
      split only genuine mixed owners and keep facades concise.
- [ ] Remove temporary compatibility helpers, duplicated arithmetic or
      protocol tables, stale schedule wording, and roadmap codes from living
      code, tests, and non-roadmap documentation.
- [ ] Confirm every exclusion still holds: static failure, one-dynamic-operand
      check removal, floating/checked casts, proof normalization, general
      storage reasoning, ownership, and target-specific optimization.
- [ ] Reconcile the roadmap discoveries with the optimization candidate
      catalog, keeping detailed actionable evidence in the discoveries record
      and concise status/placement/effort/value summaries in the catalog.
- [ ] Advance FMC-01 and FMC-02 to **Implemented** and link them to promoted
      living compiler, driver, reporting, and testing contracts.
- [ ] Run the complete repository validation from an artifact-free snapshot or
      clean checkout, plus supported MSRV and independent-process determinism.
- [ ] Mark every task complete, set roadmap status complete, move this roadmap
      to `docs/archive/`, update active/archive indexes, and repair every
      incoming link.
- [ ] Archive or remove the discoveries record only if no actionable finding
      remains; otherwise keep it indexed under `docs/roadmaps/`.

**Tests:** Full repository check, extended deterministic and release goldens,
supported MSRV, documentation links/indexes, diff hygiene, archive links,
repository-status review, and a manual comparison of current registry,
schedule, reporting, and exclusion documentation against code.

**Gates:** `make check`; `make check-long`; `make msrv-check`; `make
docs-check`; `git diff --check`; and repository-status review from an
artifact-free snapshot.

**Exit criteria:** The pass is fully implemented and documented, catalog
status and living contracts are current, no excluded semantic family entered
implicitly, every gate passes, completed records are archived, and remaining
discoveries have clear future owners.

## Ordering and dependencies

Checked arithmetic semantics land before MIR observation so protocol code
cannot become the accidental arithmetic authority. Immutable candidate
discovery lands before mutation, and the shared atomic transaction lands before
either operation family uses it. Division/remainder then exercises the
transaction and its most subtle signed arithmetic; shifts follow through the
same boundary with independent width and signedness cases.

Registration waits until both catalog candidates are implemented so a named
production pass never exposes a half-supported family. Default activation
waits until selection, reporting, verification, and exclusion tests are
complete. Broad goldens and closure follow activation so they exercise the
real schedule rather than a synthetic exact-pass harness.

Direct failure folding, dynamic operation check removal, carrier/storage
elimination, proof normalization, and block merging are deliberately
independent later projects. Their potential value does not justify widening
this roadmap's first checked-terminator rewrite.
