# Generic Ranges and Tight Range Loops Roadmap

Status: in progress; RG4 is next.

This roadmap implements the frozen
[language contract](../language/RANGES.md) and
[compiler contract](../compiler/RANGES.md). The
[archived design record](../archive/GENERIC_RANGES_DESIGN_PROPOSAL.md)
preserves alternatives and rationale. This roadmap owns delivery order and
acceptance without reopening the confirmed successor, half-open range, syntax,
HIR provenance, initial fusion-eligibility, or performance decisions.

## Scope and invariants

- `std::range` declares canonical `Successor<Output>` and ordinary
  `Range<T> implements Iterable<T, T>` source.
- Exact `u8`, `u64`, and `i64` successor bounds close to compiler-provided
  existing addition-by-one operations without primitive object conformance.
- Exact classes opt in through ordinary `OpLess<T>` and `Successor<T>` nominal
  implementations and witness dispatch.
- Explicit generic ranges work completely through ordinary construction,
  generic specialization, general iteration, lifecycle, verified MIR, and
  native execution before punctuation is accepted.
- `lower .. upper` is a lowest-precedence, non-associative general expression
  whose exact same-typed endpoints construct canonical `Range<T>`.
- Syntax retains source shape through resolution, then erases to ordinary HIR
  class construction with one non-forgeable canonical range-syntax origin.
- Only an immediately consumed syntax-origin `Range<u8|u64|i64>` may select
  the initial fused plan. Explicit constructors, stored ranges, classes,
  generic parameters, views, and lookalikes remain ordinary iterables.
- Fused lowering preserves left-to-right endpoint evaluation,
  compare-before-yield, advance-before-body, fresh item epochs, loop exits,
  cleanup, and maximum-endpoint safety.
- MIR and backends receive only existing operations. No range MIR opcode,
  backend intrinsic, runtime service, public symbol, allocation rule, or ABI
  revision is introduced.
- Deterministic MIR and assembly structure is the durable performance gate.
  A separate documented reference benchmark must record median range time
  within 10% of matched handwritten `while` before final closure.
- Each task updates living grammar, status, language, compiler, testing, and
  debugging documentation only for behavior actually delivered.
- Inclusive, unbounded, descending, explicitly stepped, and floating ranges;
  structural discovery; overloadable `..`; implicit conversions; broad
  optimization; and fusion beyond immediate integer syntax are excluded.

The implemented generic-interface, operator-protocol, general-iteration,
ordinary construction, lifecycle, optional, MIR verification, x86-64, golden,
and determinism pipelines are the baseline. No other roadmap blocks RG0.

## Progress

- [x] RG0 — Canonical successor protocol and primitive realizations
- [x] RG1 — Explicit generic `Range<T>` values and iteration
- [x] RG2 — Range punctuation, grammar, and resolved canonical construction
- [x] RG3 — Concise range expression HIR and complete ordinary execution
- [ ] RG4 — Immediate primitive range-loop fusion
- [ ] RG5 — Performance evidence, hardening, and release closure

## PR-sized implementation sequence

### RG0 — Canonical successor protocol and primitive realizations

**Purpose:** Establish the source-defined advancement vocabulary and the narrow
primitive-bound realization before `Range<T>` depends on either.

- [x] Add canonical `std/std/range.ska` with the exact public
  `Successor<Output>` interface while keeping `Range<T>` absent or gated until
  RG1.
- [x] Add a request-local range-language-item product that validates canonical
  module, interface-template, parameter, requirement, receiver, and result
  identities from ordinary explicit reachability.
- [x] Keep same-named foreign declarations unrelated and preserve provider,
  visibility, module-cycle, and malformed-canonical diagnostics in stable
  order.
- [x] Extend the static primitive-bound realization boundary with exactly
  `Successor<u8>`, `Successor<u64>`, and `Successor<i64>` mapped to existing
  wrapping addition-by-one operations.
- [x] Reuse cohesive class-witness/primitive-intrinsic specialization machinery
  without generalizing ordinary primitive interface conformance.
- [x] Support definition-site manual successor calls for class and primitive
  specializations while direct primitive member syntax remains invalid.
- [x] Exclude `f64`, `bool`, `unit`, interface views, owners, and noncanonical
  protocols from primitive satisfaction.
- [x] Expose deterministic canonical identities and realization evidence in
  resolved dumps and focused diagnostics.
- [x] Update generic-interface, operator-protocol, range, status, testing, and
  debugging documentation to the delivered protocol-only profile.

**Primary implementation areas:** `std/std/range.ska`, standard-library source
registration and test support, resolved language-item identities, module
dependency evidence, generic-template bound validation and specialization,
bound-member realization, primitive operation registry reuse, dumps, and
focused compiler tests.

**Tests:** Valid installed and replacement protocols; every malformed
declaration component; missing/ambiguous providers and cycles; same-named
lookalikes; exact primitive positive and excluded matrices; class witness and
primitive manual bound calls; direct primitive member rejection; duplicate and
wrong registry mutations; source/provider-order determinism; native generic
successor smoke without range construction.

**Gates:** Focused resolve, generic-interface, operator-realization, module,
and native tests; `make compiler-test`; focused standard-library goldens;
`make docs-check`; `make msrv-check`; and `git diff --check`.

**Exit criteria:** The compiler validates one canonical successor identity,
class bounds retain ordinary witnesses, three integer primitive bounds close
to existing additions, excluded cells remain rejected, and no primitive
object or runtime representation exists.

### RG1 — Explicit generic `Range<T>` values and iteration

**Purpose:** Deliver the complete ordinary library abstraction before new
syntax or optimization can depend on it.

- [x] Add the frozen public `Range<T>` class, final endpoint fields,
  `OpLess<T>` and `Successor<T>` bounds, `Iterable<T, T>` claim, initializer,
  `iter_state`, and `iter_next` implementation to `std::range`.
- [x] Extend canonical validation with exact range class-template,
  initializer, bound, and iterable-claim identities needed by later syntax.
- [x] Compile primitive range specializations through static ordering and
  successor realizations and class specializations through ordinary witnesses.
- [x] Preserve half-open ascending semantics, empty equal/descending ranges,
  maximum primitive endpoints, compare-before-yield, and advance-before-body.
- [x] Exercise ordinary construction, storage, copying where available,
  arguments/results, nesting, generic-bound consumers, and complete item/state
  capabilities without range-specific HIR or MIR.
- [x] Execute exact-class ranges with copy, assignment, destruction, successor
  effects, optional results, and normal/continue/break/return cleanup through
  general iteration.
- [x] Add explicit primitive and representative `BigInteger`-like class range
  goldens, including malformed conformance and capability failures.
- [x] Update the standard-library inventory, range authorities, iteration
  adoption boundary, status, test map, and debugging guidance to the explicit
  implemented profile while keeping `..` rejected.

**Primary implementation areas:** `std/std/range.ska`, standard-library source
provider/test support, canonical range validation, generic specialization,
ordinary construction and iteration tests, lifecycle verification, x86-64
native execution, and golden fixtures.

**Tests:** All three integer types; negative `i64`; equal, descending, and
maximum endpoints; empty and nested loops; break/continue/return; direct,
stored, copied, argument/result, and generic-bound ranges; class ordering and
successor effects; noncopyable/nonassignable failures; optional-layer
correctness; replacement standard library validation; no new MIR/runtime
surface; independent-process phase and assembly determinism.

**Gates:** Focused generic class/interface, operator, iteration, MIR,
static-lifecycle, backend, and standard-library tests;
`make golden-filter GOLDEN_FILTER='ranges/**'`; `make compiler-test`;
`make docs-check`; `make msrv-check`; and `git diff --check`.

**Exit criteria:** Imported `Range<T>(start, end)` is a complete ordinary exact
class and iterable for supported primitives and conforming classes, every
documented lifecycle and boundary case verifies and executes, `..` remains
invalid, and no range-specific lower IR or runtime support exists.

### RG2 — Range punctuation, grammar, and resolved canonical construction

**Purpose:** Add the concise source surface and settle all frontend identities
and diagnostics before executable HIR consumes it.

- [x] Add longest-match `..` tokenization before `.`, preserving member access,
  decimal literals, no-whitespace integer forms, token dumps, and source spans.
- [x] Add the lowest-precedence non-associative range-expression grammar and a
  dedicated source AST node with lower, upper, operator, and complete spans.
- [x] Diagnose missing endpoints and ungrouped chains once and recover at
  expression, statement, `for` header, argument, initializer, and body
  boundaries.
- [x] Include both endpoints in nesting limits, source scanners, template
  request discovery, visitors, dumps, and every expression-containing syntax
  position without reducing existing depth budgets.
- [x] Add compiler-dependency evidence from successful `..` syntax to canonical
  `std::range` without introducing an import binding.
- [x] Resolve endpoints in source order, require one exact static type, close
  canonical `Range<T>` bounds, and select exact initializer, ordering,
  successor, result, and realization identities.
- [x] Reject mixed endpoints, missing bounds, unsupported primitive cells,
  lookalike declarations, structural methods, conversions, and expected-type
  filtering with focused ordered diagnostics.
- [x] Retain deterministic resolved range evidence and dumps while gating the
  expression before completed HIR until RG3.
- [x] Update the implemented grammar, range language contract, module/compiler
  contracts, status, testing, and debugging documents to the accepted
  frontend-only maturity.

**Primary implementation areas:** lexer tokens/scanner/dumps, syntax AST and
expression parser facade, recovery and nesting tests, compiler-dependency
collection, source request scanner, resolved expression IR/dumps, range
language-item selection, and resolution diagnostics.

**Tests:** `1..3`, spaced forms, decimal/member punctuation, precedence against
all existing tiers, grouping, one invalid chain, every missing endpoint and
recovery boundary, deep/malformed generation, module activation and
replacement providers, exact primitive/class endpoint pairs, every mismatch
and missing bound, generic source requests, AST/resolved determinism, and an
explicit later-phase gate.

**Gates:** Focused lexer, syntax, resolution, module, specialization-request,
robustness, and dump tests; range diagnostic goldens; `make compiler-test`;
`make docs-check`; `make msrv-check`; and `git diff --check`.

**Exit criteria:** Every valid `..` expression has deterministic source and
resolved canonical construction evidence, every malformed or mistyped form
fails at its owning frontend boundary, ordinary grammar outside the new tier
is unchanged, and no unresolved range reaches HIR.

### RG3 — Concise range expression HIR and complete ordinary execution

**Purpose:** Make `..` a first-class exact `Range<T>` value in every ordinary
consumer while preserving the provenance required by later fusion.

- [x] Lower resolved range syntax to existing exact class-construction HIR with
  ordinary initializer, argument, destination, result, effect, ownership, and
  cleanup plans.
- [x] Add one non-forgeable `CanonicalRangeSyntax` construction origin carrying
  exact operator span, range template/class/initializer, endpoint type,
  ordering, and successor identities.
- [x] Validate complete correspondence between the origin and construction;
  never label explicit `Range<T>(...)` or lookalike construction as syntax.
- [x] Preserve origin through semantically transparent grouping only and erase
  fusion eligibility across storage, copy, call, argument/result, alias,
  owner, optional, or interface-view boundaries.
- [x] Lower every non-immediate or nonprimitive use through ordinary
  construction, `HirForIn`, interface, optional, lifecycle, MIR, and backend
  paths.
- [x] Prove lower-before-upper exactly-once evaluation, initialization after
  both endpoints, result security, reverse cleanup, and class effects and
  failures in arbitrary expression consumers.
- [x] Add HIR/resolved mutation tests for forged identity, wrong endpoint,
  wrong initializer, wrong result, and invalid primitive/class realization.
- [x] Add concise primitive and class native goldens matching explicit range
  semantics, including stored ranges and loop exits.
- [x] Promote status and living documentation to complete concise range
  expression execution while retaining ordinary loop performance until RG4.

**Primary implementation areas:** type checking for range expressions and
construction, HIR construction origin and dumps, construction lowering,
expression consumers and ownership plans, general-iteration selection,
preliminary/final MIR verification, backend legality, and goldens.

**Tests:** Every owning and effectful endpoint family admitted by exact
construction; arbitrary expression storage/argument/result/field consumers;
immediate and stored primitive/class loops; evaluation/failure suppression;
normal/continue/break/return cleanup; forged-origin mutations; explicit versus
syntax HIR distinction; native equivalence; no range-specific MIR/runtime
symbols; cross-process complete-pipeline determinism.

**Gates:** Focused type/HIR, construction, iteration, MIR, lifecycle, backend,
and pipeline-determinism tests; `make golden-filter GOLDEN_FILTER='ranges/**'`;
`make compiler-test`; `make docs-check`; `make msrv-check`; and
`git diff --check`.

**Exit criteria:** `lower .. upper` produces an ordinary exact `Range<T>` in
every consumer with one verified canonical syntax origin, explicit and stored
ranges remain ordinary, evaluation and cleanup match construction semantics,
and all supported forms execute natively without fusion.

### RG4 — Immediate primitive range-loop fusion

**Purpose:** Remove protocol overhead only where typed canonical syntax proves
the exact scalar loop while retaining the ordinary range path as reference.

- [ ] Extend structured `HirForIn` execution planning with an immediate
  primitive range variant selected only from exact canonical syntax origin.
- [ ] Require exact `u8`, `u64`, or `i64`, compiler-provided comparison and
  successor realizations, canonical `Range<T>: Iterable<T, T>`, and no
  observable intervening boundary.
- [ ] Keep explicit `Range<T>(...)`, stored ranges, classes, generic parameters,
  interface views, inherited claims, and lookalikes on the ordinary protocol
  plan, with negative eligibility tests for each.
- [ ] Retain ordered endpoint evaluation, hidden current/end scalars, fresh item
  epochs, primitive compare and increment operations, advance-before-body,
  loop identities, exits, cleanup depths, effects, and spans in HIR.
- [ ] Lower the plan directly to existing scalar storage, comparison,
  assignment, branch, jump, and cleanup MIR without constructing a range or
  optional result.
- [ ] Verify equal/descending first exit, maximum-endpoint safety, normal body,
  continue, break, return, nested fused/unfused loops, and panic attribution.
- [ ] Add mutation tests for wrong origin/type/operation, missing endpoint or
  item epochs, update-after-body, invalid loop targets, cleanup imbalance, and
  forbidden interface/optional traffic.
- [ ] Preserve ordinary static effects, phase determinism, backend legality,
  runtime symbol set, and ABI version.
- [ ] Update range, iteration, phase, backend, status, testing, and debugging
  documents to the implemented fusion profile.

**Primary implementation areas:** `HirForIn` execution plans and dumps,
range-origin eligibility, type-check control effects, HIR-to-MIR iteration
lowering, MIR verification and mutation fixtures, static lifecycle, x86-64
control flow, and fused/unfused goldens.

**Tests:** Positive matrix for three integer types and transparent grouping;
negative matrix for every excluded boundary and type; equal, descending,
maximum, nested, and mixed loops; exactly-once endpoint effects; all exits;
ordinary versus fused native observations; absence of range/interface/optional
MIR; malicious plan mutations; reordered full-phase determinism.

**Gates:** Focused iteration, range HIR, MIR, verifier, static-lifecycle,
backend, and determinism tests; range goldens; `make compiler-test`;
`make docs-check`; `make msrv-check`; and `git diff --check`.

**Exit criteria:** Every eligible immediate integer `..` loop emits verified
ordinary scalar MIR with the frozen evaluation and cleanup order, every
ineligible form retains ordinary `Iterable` execution, and no target or
runtime range mechanism exists.

### RG5 — Performance evidence, hardening, and release closure

**Purpose:** Prove that the fused plan meets the handwritten-`while` target and
close the feature with complete deterministic, compatibility, and living
documentation evidence.

- [ ] Add target-independent structural tests proving one comparison, one
  same-typed induction increment, no call/optional/owner/runtime operation,
  and no loop-carried range aggregate in representative fused loops.
- [ ] Add x86-64 assembly-shape comparisons against matched handwritten
  `while` without freezing registers, labels, offsets, or incidental complete
  instruction sequences.
- [ ] Create `tests/benchmarks/range_loop` with matched `u8`, `u64`, and `i64`
  range/while programs, deterministic work and checksum, documented build and
  measurement procedure, code-size and hot-loop inspection, and repeated
  median timing.
- [ ] Record a reference result whose range median is within 10% of the matched
  `while` median; investigate structural or fixed overhead when it is not.
- [ ] Keep wall time outside `make check`; make deterministic MIR, assembly,
  native-result, and documentation checks the repository gates.
- [ ] Complete the source-to-native positive, failure, effect, lifecycle,
  primitive/class, explicit/syntax, fused/unfused, boundary, nesting, and exit
  conformance matrix.
- [ ] Harden bounded malformed/deep source, malicious resolved/HIR/MIR
  mutations, provider/source/import reordering, independent-process dumps and
  artifacts, runtime-symbol snapshots, and ABI neutrality.
- [ ] Audit range-owned compiler modules and functions for cohesive facades and
  record out-of-scope findings in an indexed discovery document rather than
  extending closure.
- [ ] Promote every living document to the implemented profile, remove stale
  rollout wording and roadmap codes outside historical files, run full gates,
  then archive this completed roadmap and repair indexes and links.

**Primary implementation areas:** MIR structural assertions, x86-64 assembly
tests, `tests/benchmarks/range_loop`, range goldens and conformance map,
generative robustness, pipeline determinism, runtime/ABI snapshots, living
documentation, and archive indexes.

**Tests:** Complete range matrix plus exact structural and native comparison
fixtures; repeated reference benchmark; independent-process token through
artifact determinism; replacement standard libraries; bounded malformed and
deep inputs; malicious phase-product mutations; no new public runtime symbols
or metadata forms.

**Gates:** Focused range and benchmark procedures, `make check`,
`make msrv-check`, `make golden-determinism-test`, appropriate bounded
robustness coverage, `make docs-check`, and `git diff --check` from an
artifact-free snapshot or clean checkout.

**Exit criteria:** Structural hot-loop requirements pass for all three integer
types, the documented reference median is within 10% of handwritten `while`,
the complete range contract is implemented and deterministic without ABI
change, living documentation describes only current behavior, and the roadmap
is archived with no unowned high-priority finding.

## Ordering and dependencies

RG0 settles the only new generic-bound realization before standard-library
source depends on it. RG1 then proves the ordinary explicit abstraction and
all class/primitive lifecycle semantics without punctuation. RG2 adds source
and resolved identities while retaining an explicit HIR gate, preventing
frontend work from silently inventing executable ownership rules. RG3 erases
syntax into ordinary construction and completes every value consumer before
optimization exists.

RG4 can therefore compare one narrow fused plan against a complete ordinary
semantic reference and cannot accidentally broaden range eligibility. RG5
lands measurement only after stable executable shape exists, then owns broad
hardening and closure. Global inlining, devirtualization, stepped ranges, and
fusion of explicit or stored values may proceed later without changing any
earlier source meaning.
