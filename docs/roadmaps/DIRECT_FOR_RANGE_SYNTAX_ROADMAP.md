# Direct For-Range Syntax Roadmap

Status: planned; FRS0 is next.

This roadmap restricts concise `lower .. upper` syntax to the direct source of
a `for-in` statement. Reusable range values remain ordinary explicit
`std::range::Range<T>` constructions. The narrower source contract removes a
first-class expression form that is not needed by the language, makes fusion
eligibility structural, and allows the compiler to delete the general
construction-provenance machinery that currently carries range syntax across
unrelated expression consumers.

## Scope and invariants

- The accepted concise form is exactly a direct `for-in` source:

  ```ska
  for (item in lower .. upper) {
  }
  ```

- Explicit canonical ranges remain ordinary expressions and iterables:

  ```ska
  from std::range import Range;

  for (item in Range<u64>(lower, upper)) {
  }
  ```

- `var range = lower .. upper;`, `consume(lower .. upper);`,
  `return lower .. upper;`, and `for (item in (lower .. upper)) {}` are syntax
  errors. Their diagnostics point at `..`, state that concise range syntax is
  allowed only as the direct `for-in` source, and suggest explicit
  `Range<T>(lower, upper)` when a value is required.
- Parentheses around either endpoint remain ordinary endpoint syntax, while
  parentheses around the complete concise range do not make it direct.
  Parenthesized ordinary iterable expressions remain valid.
- The source grammar becomes conceptually:

  ```text
  for-in-statement = "for" "(" identifier [":" storage-type]
                     "in" for-in-source ")" block
  for-in-source    = logical-or-expression
                     [".." logical-or-expression]
  expression       = logical-or-expression
  ```

  The optional `..` branch is represented as a distinct `for-in` source, not
  as an `Expression` variant. It remains non-associative; missing endpoints and
  chains retain bounded, statement-aware recovery.
- Direct range sources preserve the implemented exact same-type endpoint
  rules, half-open ascending semantics, lower-before-upper exactly-once
  evaluation, canonical `Range<T>` specialization, implicit `std::range`
  compiler dependency, class behavior, cleanup order, and diagnostics.
- Direct exact `u8`, `u64`, and `i64` sources retain the implemented scalar
  fusion profile and handwritten-`while` structural performance contract.
  Direct class ranges and specialization-dependent generic ranges retain the
  ordinary canonical `Range<T>` plus `Iterable<T, T>` protocol path.
- Explicit `Range<T>(lower, upper)` remains deliberately ineligible for the
  syntax-owned fusion profile, even when it appears directly after `in`.
- The compiler dependency on `std::range` is acquired only from a successfully
  parsed direct range source. A stray `..` token in an invalid expression must
  not activate the canonical range module or mask the owning syntax error.
- Canonical range evidence remains identity-based and deterministic. Moving it
  onto a dedicated source node must not replace exact template, specialization,
  initializer, bound, iterable, realization, or endpoint-provenance checks
  with spelling or shape inference.
- Lower MIR, backend operations, runtime symbols, ABI version, public standard
  library declarations, range semantics, and the explicit import requirement
  for naming `Range` or `Successor` do not change.
- Inclusive, unbounded, descending, or stepped syntax; overloadable `..`;
  fusion of explicit constructors; implicit conversions; and broader generic
  inference remain out of scope.
- The archived generic-range design and delivery documents remain historical
  records. Living grammar, language, compiler, testing, and debugging
  documentation are revised to describe only the new current contract.

## Progress

- [ ] FRS0 — Enforce the direct-source syntax boundary
- [ ] FRS1 — Make range-loop provenance structural
- [ ] FRS2 — Complete conformance, determinism, and performance closure

## PR-sized implementation sequence

### FRS0 — Enforce the direct-source syntax boundary

**Purpose:** Change the source contract at its owning parser boundary while
keeping the existing resolved and typed range machinery as a temporary
buildable adapter for the following cleanup.

- [ ] Replace the general `Expression::Range`/`RangeExpr` AST form and
  lowest-precedence expression parser tier with a source-shaped
  `ForInSource::{Iterable, Range}` representation. The range source retains
  lower, operator, upper, and complete spans.
- [ ] Parse the shared-prefix `for-in` source once, selecting the range variant
  only when an ungrouped `..` immediately follows the lower
  `logical-or-expression`; preserve the existing ordinary iterable source and
  item-annotation behavior.
- [ ] Reuse or revise `PAR017` as one stable direct-range diagnostic family for
  forbidden expression contexts, missing lower or upper endpoints, and
  chained operators. Recover once at calls, returns, initializers, grouped
  expressions, `for` headers, and following statement boundaries.
- [ ] Pin the exact acceptance distinction between grouped endpoints,
  parenthesized ordinary iterables, and the rejected grouped complete range.
- [ ] Remove range handling from general expression spans, dumps, logical-depth
  accounting, template expression typing, request scanners, and exhaustive
  expression visitors. Visit the two endpoints from the dedicated `for-in`
  source owner instead.
- [ ] Derive `std::range` compiler-dependency evidence from successfully parsed
  direct range sources rather than every `DotDot` token. Rename the dependency
  vocabulary from range expression to direct range source and preserve stable
  source-order graph output and canonical provider/cycle diagnostics.
- [ ] Adapt body resolution, generic-template analysis, semantic range-request
  collection, and current fusion selection to consume the new syntax node
  while temporarily producing the existing canonical construction origin.
  Keep this bridge local and explicitly internal so it does not become a new
  public representation contract.
- [ ] Migrate every in-repository value-producing `..` use to an explicit,
  imported `Range<T>(lower, upper)`. Keep direct primitive and class loop
  sources concise, and remove the formerly fusion-transparent whole-range
  grouping cases.
- [ ] Update `GRAMMAR.md`, `RANGES.md`, functions/control-flow, the language
  overview and status matrix, module-system documentation, range golden
  guidance, and user-facing examples in the same change that rejects the old
  forms. Preserve the archived design proposal and completed roadmap as
  history rather than rewriting their decisions.

**Primary implementation areas:** syntax `for-in` and expression AST/parser
facades, syntax dumps and recovery tests, module graph dependency collection,
generic-template body traversal, semantic range request discovery, body
resolution adapters, range goldens, and source-facing living documentation.

**Tests:** Positive syntax/AST/dump cases for direct primitive and class
ranges, grouped endpoints, explicit `Range<T>`, and parenthesized ordinary
iterables; negative cases for local initialization, assignment RHS, call
argument, return, nested call, whole-range grouping, missing endpoints, and
chains; recovery into later statements and nested loops; dependency activation
only for valid direct sources; generic-template endpoint provenance; migrated
native range goldens and bounded robustness seeds.

**Gates:** Focused lexer, syntax, module graph, generic-template, resolution,
robustness, and range golden tests; `make compiler-test`;
`make golden-filter GOLDEN_FILTER='ranges/**'`; `make docs-check`;
`make msrv-check`; and `git diff --check`.

**Exit criteria:** Only an ungrouped direct `for-in` source accepts `..`; every
former value context fails with the focused syntax diagnostic and useful
recovery; explicit `Range<T>` values and all documented direct loops still
resolve and execute; and the repository contains no general syntax range
expression even though the temporary downstream origin adapter still exists.

### FRS1 — Make range-loop provenance structural

**Purpose:** Replace the temporary construction-origin bridge with dedicated
resolved `for-in` source evidence, then remove the span registries and ordinary
construction metadata that were necessary only when `..` could flow through
general expressions.

- [ ] Introduce a dedicated resolved `for-in` source distinction for an
  ordinary iterable expression versus a canonical range-loop source. The range
  source owns the ordered endpoints, exact canonical construction selection,
  protocol evidence, endpoint provenance, and source spans required by either
  execution plan.
- [ ] Make semantic specialization request discovery inspect structurally
  identified range-loop sources in callable and class bodies. Remove the
  global `range_expression_spans` collection, span-containment filtering, and
  resolved-program range-span registry while retaining fixed-point discovery
  for endpoint types exposed by newly materialized specializations.
- [ ] Move definition-site specialization-dependence analysis from the general
  expression visitor into direct range-source analysis. Continue to reject
  fusion whenever either endpoint is specialization-dependent, including
  transitive local and bound-selected producers.
- [ ] Remove `ResolvedConstructionOrigin` from ordinary construction. Validate
  exact canonical identities at creation of the resolved range-loop source,
  and update resolved dumps and mutation tests to expose and challenge that
  structural evidence instead of a forgeable expression-adjacent tag.
- [ ] Select typed execution directly from the resolved source: eligible
  integer sources create `HirPrimitiveRangeIterationPlan`; class and other
  ineligible direct sources construct the canonical range as an ordinary
  receiver and create the existing protocol plan; ordinary iterable sources,
  including explicit `Range<T>`, remain unchanged.
- [ ] Remove `HirConstructionOrigin`, `HirCanonicalRangeOrigin`, general
  range-construction-origin validation, the `TYP052` provenance diagnostic,
  grouping-recursive eligibility recognition, and range-origin dump output.
  Retain only the minimal exact range-loop evidence needed by the primitive
  HIR plan and its verifier assertions.
- [ ] Keep endpoint evaluation, temporary securing, class construction,
  protocol receiver lifetime, advance-before-body order, exits, cleanup, and
  static effects identical on both fused and ordinary paths.
- [ ] Preserve target-independent artifact reachability: fused-only programs
  may prune unused canonical range artifacts, while class direct ranges and
  explicit range construction retain every ordinary method and metadata edge
  they execute.
- [ ] Update the compiler range contract, compiler overview, phase/IR text,
  testing matrix, and debugging workflow to the dedicated-source pipeline and
  origin-free ordinary construction representation.

**Primary implementation areas:** resolved `for-in` and range IR, semantic
range specialization completion, generic-template selections, type-check
iteration planning, ordinary construction HIR, range and iteration dumps,
phase-product verification, artifact reachability, and compiler-facing living
documentation.

**Tests:** Exact resolved source evidence for primitive, class, generic, and
explicit ranges; fixed-point specialization discovery without global span
containment; missing/mismatched canonical identity and endpoint-provenance
mutations; positive and negative fusion matrices; ordinary class lifecycle and
explicit-range protocol traffic; absence of construction-origin vocabulary;
fused-only pruning and ordinary-path retention; deterministic resolved and HIR
dumps.

**Gates:** Focused resolver, specialization, type/HIR, iteration, MIR,
static-lifecycle, reachability, backend, and pipeline-determinism tests;
`make compiler-test`; range goldens; `make docs-check`; `make msrv-check`; and
`git diff --check`.

**Exit criteria:** Direct range syntax is represented structurally from AST
through typed loop-plan selection; ordinary resolved and HIR constructions
carry no range origin; global range-expression span provenance is gone; fused
and protocol execution retain their exact semantics and artifact behavior.

### FRS2 — Complete conformance, determinism, and performance closure

**Purpose:** Prove the narrower contract and simplified architecture across
the complete pipeline, confirm that tight-loop quality did not regress, and
promote the roadmap only after living documentation and repository-wide
evidence agree.

- [ ] Complete one explicit conformance matrix covering every accepted direct
  primitive/class form, explicit/stored/argument/result `Range<T>` replacement,
  grouped endpoint distinction, rejected expression context, malformed source,
  loop exit, lifecycle, effect, and runtime-failure case.
- [ ] Update range goldens so concise syntax demonstrates direct consumption
  only. Add focused compile-failure cases for all four contract examples and
  ensure each diagnostic suggests explicit construction without requiring a
  generic argument the compiler cannot reliably infer in the message.
- [ ] Refresh bounded range mutation seeds and deep-source cases around the new
  `for-in` grammar and recovery boundary; remove seeds that encode obsolete
  first-class range values.
- [ ] Extend independent-process determinism coverage across tokens, AST,
  module graph, resolved program, HIR, preliminary/final MIR, diagnostics,
  assembly, and published artifacts for direct primitive, direct class, and
  explicit range sources.
- [ ] Re-run target-independent operation counts and x86-64 shape comparisons
  for all three fused integer types against the matched handwritten `while`
  loops. Run and record the existing range-loop benchmark only if deterministic
  shape changed or timing evidence falls outside the documented acceptance
  band; wall time remains outside correctness gates.
- [ ] Confirm no runtime symbol, ABI, canonical standard-library declaration,
  or lower-MIR vocabulary changed and that invalid out-of-context `..` never
  reaches resolution, type checking, or artifact publication.
- [ ] Audit for stale range-expression terms, origin plumbing, dead adapters,
  obsolete tests, and duplicate documentation. Make small cohesive removals
  directly; record larger unrelated findings in an indexed discoveries file.
- [ ] Promote all remaining living range, iteration, grammar, module,
  compiler, testing, debugging, benchmark, and fixture documentation to the
  implemented direct-source contract. Run closure gates, mark this roadmap
  complete, archive it, and repair roadmap indexes and incoming links.

**Primary implementation areas:** range conformance goldens, diagnostic
fixtures, generative robustness, pipeline determinism, MIR/assembly structural
tests, range-loop benchmark records when required, runtime/ABI snapshots,
documentation search and link validation, and roadmap archival.

**Tests:** Complete valid/invalid source matrix; direct versus explicit native
equivalence; primitive/class effects and cleanup; malformed recovery and
bounded robustness; provider/source-order and process determinism; fused MIR
and assembly parity; artifact retention/pruning; unchanged runtime symbols and
ABI.

**Gates:** Focused range and performance procedures; `make check`;
`make msrv-check`; `make golden-determinism-test`; appropriate bounded
robustness coverage; `make docs-check`; and `git diff --check` from an
artifact-free snapshot or clean checkout.

**Exit criteria:** The complete repository accepts only direct concise range
sources, all reusable values use explicit `Range<T>`, the obsolete expression
and construction-origin architecture is absent, deterministic fused loops
retain handwritten-`while` structure, living documentation is current, and
the completed roadmap is archived with no unowned high-priority finding.

## Ordering and dependencies

FRS0 changes the user-visible grammar first and uses a narrow adapter into the
existing semantic representation, so the breaking source migration is
reviewable without simultaneously rewriting fusion. FRS1 can then rely on the
fact that canonical syntax has exactly one structural consumer and remove the
general-expression span and construction-origin machinery in one cohesive
semantic change. FRS2 follows only after representations stabilize, allowing
determinism, reachability, native behavior, and performance evidence to test
the final architecture rather than a transitional one.

The implemented generic-range, general-iteration, generic-interface,
operator-overloading, whole-world reachability, and selectable final-MIR
optimization contracts are the baseline. No other active roadmap blocks FRS0.
Future stepped or inclusive range syntax, if designed, should extend the
dedicated `for-in` source family without making range punctuation a general
expression again.
