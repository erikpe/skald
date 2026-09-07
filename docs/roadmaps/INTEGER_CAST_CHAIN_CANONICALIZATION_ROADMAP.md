# Integer Cast-Chain Canonicalization Roadmap

Status: in progress; ICC0 is complete and ICC1 is next.

This roadmap implements the integer-only part of FMV-02. It adds one exact,
target-independent final-MIR optimization for arbitrary-length cast chains over
`u8`, `i64`, and `u64`. The durable result is a shared integer-cast algebra, a
read-only chain analysis used by both measurement and transformation, and an
independently selectable proof-rich pass which replaces each eligible live
chain endpoint with its shortest canonical integer-cast recipe.

The work is deliberately justified as optimizer completeness rather than by
current corpus impact. The reviewed redundancy corpus originally found little
remaining cast redundancy, and the current constant solver removes its
constant-shaped examples before final output. Dynamic integer cast chains are
nevertheless total, exactly specified, and small enough to canonicalize
completely without range analysis, floating-point reasoning, or a new MIR.

## Dependencies

- The implemented integer-cast language contract defines same-width bit
  preservation, low-byte narrowing, and zero-extension from `u8` in
  [`../language/TYPES_AND_VALUES.md`](../language/TYPES_AND_VALUES.md#explicit-integer-casts).
- The completed
  [selectable final-MIR pipeline](../archive/SELECTABLE_FINAL_MIR_OPTIMIZATION_PIPELINE_ROADMAP.md)
  supplies stable registration, exact test schedules, exclusions, verified
  pass capabilities, occurrence metrics, and deterministic checkpoints.
- The completed
  [dense callable-local MIR identity rewriting work](../archive/DENSE_MIR_IDENTITY_REWRITING_ROADMAP.md)
  supplies exhaustive value-use classification, guarded same-typed
  substitution, sparse callable edits, and deterministic dense commit.
- The completed
  [local final-MIR simplification work](../archive/LOCAL_FINAL_MIR_SIMPLIFICATION_ROADMAP.md)
  supplies primitive constant folding, guarded value forwarding, dead-pure
  cleanup, and the proof-rich scheduling region where this pass belongs.
- The completed
  [local redundancy measurement](../archive/LOCAL_MIR_REDUNDANCY_MEASUREMENT_ROADMAP.md)
  supplies the cast census and corpus. Its former pairwise composition policy
  must be replaced by the same exact algebra used by the optimization so
  measurement and transformation cannot disagree.

## Scope and invariants

- Canonicalize chains containing only ordinary total casts among `u8`, `i64`,
  and `u64`; chain length has no configured bound.
- Follow one straight transient-value def-use chain within one basic block.
  Cast instructions need not be textually adjacent, and intermediate results
  may have other uses. Cross-block discovery is explicitly deferred.
- Summarize every supported chain as either preservation of all source bits or
  retention of the low eight source bits, together with its root and result
  types. Signedness never changes the stored 64-bit pattern.
- Emit the uniquely selected shortest canonical recipe: zero casts for an
  identity, one direct cast for bit preservation or a final `u8` narrowing,
  and `64-bit source -> u8 -> 64-bit result` for a narrowed-and-widened result.
  Thus every canonical recipe contains at most two casts.
- Prove minimality from the closed three-type cast algebra, not host register
  width, selected examples, or backend behavior.
- Rewrite only when the canonical recipe is strictly shorter than the
  discovered chain. Canonical input is unchanged, making the pass idempotent.
- Preserve endpoint `ValueId`, exact result type, source span, instruction
  position, and all uses when an endpoint cast can be rewritten in place.
- Remove an identity endpoint only by forwarding to a same-typed dominating
  value through the existing ordinary-use boundary. Proof metadata, checked
  protocols, ownership/lifecycle roles, I/O, unknown roles, or uses outside
  the defining block remain forwarding barriers.
- Reuse an existing chain value for the intermediate `u8` result when the
  canonical recipe requires two casts. The pass inserts no values,
  instructions, storage, blocks, edges, or metadata.
- Do not delete shared or newly dead intermediate cast definitions. Existing
  independently selectable dead-pure-definition elimination owns that cleanup.
- Build analysis and mutation plans from one immutable verified seal, validate
  expected instructions and identities before mutation, commit atomically per
  callable/program, and reverify every changed result through the pipeline.
- Preserve operand evaluation count and order, diagnostics, failure behavior,
  proof provenance, ownership/lifecycle events, static activation, ABI,
  runtime traces, deterministic dumps, and native results.
- Treat `bool`, `f64`, checked floating-to-integer conversion, raw-bit
  reinterpretation, non-cast rvalues, malformed cast/type pairs, and
  cross-block provenance as barriers. General primitive-chain canonicalization,
  range-informed rewrites, CSE, storage propagation, backend peepholes, and MIR
  representation changes are non-goals.
- Keep the semantic algebra and chain analysis behind concise private facades;
  production rewriting and the read-only redundancy census must consume those
  owners rather than maintain separate conversion tables.

## Progress

- [x] ICC0 — Establish the complete integer-cast algebra
- [ ] ICC1 — Analyze arbitrary-length same-block chains
- [ ] ICC2 — Implement the selectable guarded rewrite pass
- [ ] ICC3 — Activate and observe canonicalization end to end
- [ ] ICC4 — Harden the boundary and close the roadmap

## PR-sized implementation sequence

### ICC0 — Establish the complete integer-cast algebra

**Purpose:** Create one auditable semantic authority for composing integer
casts and selecting a provably shortest recipe before any production mutation
depends on it.

- [x] Add a private pass-level integer-cast algebra shared by the redundancy
  observer and optimizer, using explicit source/result types and a closed
  `all bits` versus `low eight bits` transformation state.
- [x] Define deterministic composition and canonical-recipe APIs for all nine
  direct integer cast pairs, including same-type identities.
- [x] Encode the zero-, one-, and two-cast recipe selection rules and expose
  the canonical length without exposing mutable analysis state.
- [x] Prove closure under every next integer cast and prove that no zero- or
  one-cast recipe represents either narrowed-and-widened 64-bit transformation.
- [x] Replace the redundancy census's private pairwise integer rules with the
  shared authority while retaining conservative barriers for every other cast
  family.
- [x] Keep the module independent of pipeline registration, MIR mutation,
  reporting, drivers, and target lowering.

**Tests:** Exhaustive root/current/next-type transition tables; all canonical
recipes and lengths; identity and idempotence; every chain of a generated
bounded depth as a check of the finite-state closure; exhaustive `u8` inputs
and representative signed/unsigned 64-bit boundaries; existing cast-census
tests; `cargo test --locked -p skald-compiler` and static checks.

**Exit criteria:** Every integer-only cast sequence has one deterministic
semantic summary and a proven shortest recipe of length zero through two, the
measurement code owns no competing integer composition logic, and executable
MIR behavior is unchanged.

### ICC1 — Analyze arbitrary-length same-block chains

**Purpose:** Turn the algebra into one immutable, deterministic candidate plan
without yet changing compiler output.

- [ ] Build a callable-local index of ordinary primitive-cast assignments from
  one verified snapshot, recording exact block/instruction sites, result and
  operand identities, operation, type, and span.
- [ ] Trace each integer-cast endpoint backward through preceding same-block
  cast definitions with no depth limit, tolerating unrelated intervening
  instructions and shared intermediate uses.
- [ ] Stop exactly at non-integer casts, non-cast definitions, block
  boundaries, unavailable or later definitions, malformed identities/types,
  and repeated identities; return structured failures only for invalid
  verified-state assumptions.
- [ ] Produce a candidate only when the shared canonical recipe is shorter,
  naming the root, endpoint, existing reusable `u8` narrowing where required,
  original depth, canonical depth, and exact expected instruction.
- [ ] Classify identity endpoints separately because deletion requires guarded
  same-typed value forwarding; ordinary endpoint retargeting must not depend on
  intermediate single-use status.
- [ ] Migrate the live redundancy census to the arbitrary-depth analysis and
  update its stable counts, blockers, supporting-entity bounds, examples, and
  measurement-tool projections where the old adjacent-pair model is no longer
  accurate.
- [ ] Keep candidate and example order deterministic by callable, block,
  instruction, and value identity.

**Tests:** Chains of canonical lengths zero, one, and two; redundant chains of
lengths two, three, four, and a generated long depth; nonadjacent instructions;
multiple endpoints and shared intermediates; all root/result type pairs;
boolean, floating, checked, reinterpretation, non-cast, malformed, later-value,
cycle, and cross-block barriers; deterministic repeated analysis; measurement
JSON/human projections and saturation behavior.

**Exit criteria:** The analyzer finds every shorter integer-only chain in its
same-block domain regardless of length, selects an existing value for every
recipe component, reports barriers without mutation, and the measurement
surface describes exactly the production candidate boundary.

### ICC2 — Implement the selectable guarded rewrite pass

**Purpose:** Materialize analyzed recipes through existing verified editing
infrastructure while keeping activation and failure ownership explicit.

- [ ] Add a proof-rich `integer-cast-chain-canonicalization` pass with a unique
  private identity, stable descriptor, private plan/rewrite implementation, and
  registration outside the default profile until focused behavior is complete.
- [ ] Rewrite one-cast endpoints in place to reference the analyzed root and
  exact direct `MirPrimitiveCast`, preserving result identity, type, span, and
  instruction site.
- [ ] Rewrite two-cast endpoints to consume the deterministic existing `u8`
  narrowing result and perform only the required final widening cast.
- [ ] For zero-cast recipes, require same-block dominance and exclusively
  forwarding-safe result uses, substitute the root, then remove only the
  endpoint assignment and value declaration.
- [ ] Revalidate every expected instruction, value type, definition order,
  reusable intermediate, and use-role decision before the first edit; surface
  stale or malformed plans as structured pass failures.
- [ ] Handle overlapping candidates deterministically without depending on
  instruction-index stability or mutation-time rediscovery.
- [ ] Report processed/changed callables, retargeted endpoints, forwarded
  endpoints and uses, removed assignments/values, eliminated cast steps,
  protected rejections, and maximum rewritten chain depth in stable order.
- [ ] Return unchanged without commit or output verification when no safe
  shortening exists; a repeated occurrence over canonical output must be
  unchanged.

**Tests:** Exact schedules for every canonical recipe; mixed and overlapping
candidates; shared intermediates retained; ordinary forwarding consumers;
every protected-use rejection; stable spans/types/identities; stale-plan and
type failures; no inserted MIR entities; exact commit statistics and metrics;
unchanged seal reuse; immediate verification after changes; pass repetition
and dump determinism.

**Exit criteria:** The registered pass safely canonicalizes every planned
integer chain, never lengthens or inserts MIR, preserves unsupported/protected
shapes byte-for-byte, reports deterministic outcomes, and verifies after each
changed occurrence.

### ICC3 — Activate and observe canonicalization end to end

**Purpose:** Put the independently selectable pass into normal compilation only
after its local contract is proven, then demonstrate source-to-native parity
and useful dynamic coverage.

- [ ] Insert one default-profile occurrence after the algebraic simplification
  and its following primitive constant fold, and before checked-protocol
  folding and the later dead-pure cleanup which removes orphaned cast
  definitions.
- [ ] Update registry, default/all-disabled schedules, pass discovery,
  exclusions, known-name diagnostics, checkpoint numbering, schedule
  fingerprints, and aggregate measurement ordering.
- [ ] Add dynamic source fixtures whose parameters prevent constant folding and
  cover every root/result type pair, preserved-bit round trips, narrowing then
  widening, repeated narrowing, long chains, interleaved instructions, and
  shared intermediates.
- [ ] Add focused optimization golden variants for `default`, `none`, this pass
  disabled, dead-pure cleanup disabled, and all passes disabled; pin native
  stdout/stderr/status/runtime-trace equivalence and relevant MIR/report shape.
- [ ] Demonstrate that disabling only this pass retains the original chains,
  while disabling dead-pure cleanup retains harmless orphaned intermediates
  without undoing endpoint canonicalization.
- [ ] Update compiler phase, driver/selection, reporting, testing, debugging if
  useful, pass-list, and optimization-catalog documentation with the current
  behavior and the remaining non-integer FMV-02 boundary.
- [ ] Rerun the reviewed redundancy corpus and record current before/after
  evidence without rewriting the historical archived study.

**Tests:** Registry and policy tests; exact default schedule and all exclusion
combinations; driver listing/errors and structured reports; focused golden
group in debug and release; cross-process checkpoint/metric/dump fingerprints;
`make mir-redundancy-measure`, `make compiler-test`, relevant golden filters,
and `make static-check`.

**Exit criteria:** Normal compilation runs the pass once at the documented
position, selection and reporting remain deterministic, dynamic source tests
exercise every integer canonical form, all optimized and excluded variants are
observably equivalent, and FMV-02 accurately distinguishes its implemented
integer slice from deferred primitive families.

### ICC4 — Harden the boundary and close the roadmap

**Purpose:** Audit completeness and maintainability after integration, resolve
roadmap-local gaps, and publish the implementation as stable current behavior.

- [ ] Add generated semantic differential tests comparing every supported
  chain's direct evaluation with its canonical recipe over exhaustive `u8` and
  boundary-rich deterministic `i64`/`u64` values.
- [ ] Audit chain analysis and rewriting for excessive scans, accidental
  quadratic behavior on long chains, recursion depth, nondeterministic maps,
  duplicated cast semantics, large-file ownership, and unclear facades;
  resolve small roadmap-local issues and record unrelated findings separately.
- [ ] Confirm no task codes or rollout language remain in living code, tests,
  language/compiler documentation, pass names, metrics, or diagnostics.
- [ ] Run the full repository gate from an artifact-free snapshot, the
  supported-toolchain gate, full golden determinism, and release goldens.
- [ ] Mark every completed item, set the roadmap status to complete, move it to
  `docs/archive/`, update active/archive indexes and incoming links, and leave
  only current behavior in living documentation.

**Tests:** Focused compiler and measurement tests; `make check`; `make
msrv-check`; `make golden-determinism-test`; `make golden-release-test`; and
`make docs-check` after archival.

**Exit criteria:** The complete same-block integer domain is canonical and
covered, long chains have bounded iterative analysis cost, all repository gates
pass, living documentation is current, actionable unrelated findings are
indexed separately, and the completed roadmap is archived.

## Ordering and dependencies

ICC0 settles the closed semantic algebra before either measurement or mutation
can depend on it. ICC1 then establishes immutable arbitrary-depth discovery and
keeps the existing evidence tool aligned with the future pass. ICC2 adds
guarded transformation and opt-in registration without changing normal
compilation. ICC3 activates the proven pass at the pipeline point where
constant/algebraic work has exposed chains and dead-pure cleanup can remove
orphans. ICC4 performs broad differential validation and archival only after
the default schedule, observations, and native behavior are stable.

The integer algebra and read-only analysis are prerequisites for all later
tasks. Focused source fixtures may be prepared alongside ICC1, but default
activation and living behavior documentation must wait for ICC2. Extensions to
boolean, binary64, checked, raw-bit, or cross-block chains require a separate
reviewed design and are not allowed to expand this roadmap.
