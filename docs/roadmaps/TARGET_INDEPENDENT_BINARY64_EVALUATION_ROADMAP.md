# Target-Independent Binary64 Evaluation Roadmap

Status: in progress; BE0 and BE1 are complete, and BE2 is next.

This roadmap implements the frozen
[target-independent binary64 evaluation design](TARGET_INDEPENDENT_BINARY64_EVALUATION_DESIGN_PROPOSAL.md).
It introduces an unpublished `skald-binary64` crate around a pinned
`rustc_apfloat` dependency, migrates compile-time decimal literal conversion
to that authority, extends the existing convergent primitive evaluator with
exact floating facts, and folds successful constant checked
`f64`-to-integer protocols through a separate verified pass.

The durable result is one host- and target-independent compile-time binary64
authority behind a narrow Skald-owned facade. The compiler continues to own
source diagnostics, MIR semantics, optimization policy, checked topology, and
atomic rewrites; no APFloat type crosses into those owners.

## Dependencies

- The frozen
  [binary64 design](TARGET_INDEPENDENT_BINARY64_EVALUATION_DESIGN_PROPOSAL.md)
  confirms B64E1 through B64E12, including the crate boundary, public outcome
  model, NaN policy, successful-only checked rewrite, and validation matrix.
- The completed
  [local final-MIR simplification roadmap](../archive/LOCAL_FINAL_MIR_SIMPLIFICATION_ROADMAP.md)
  provides the primitive evaluator, primitive constant-folding pass, exact
  assignment replacement, and repeated default schedule.
- The completed
  [convergent local constant propagation roadmap](../archive/CONVERGENT_LOCAL_CONSTANT_PROPAGATION_ROADMAP.md)
  provides one callable-local solver, checked topology separation, certified
  scalar carriers, whole-callable plans, and independently selectable
  consumers.
- The completed
  [checked integer protocol simplification roadmap](../archive/CHECKED_INTEGER_CONSTANT_PROTOCOL_SIMPLIFICATION_ROADMAP.md)
  provides the successful-only checked rewrite precedent, retained static
  failures, stable measurements, and source-to-native parity requirements.
- The completed
  [selectable final-MIR pipeline roadmap](../archive/SELECTABLE_FINAL_MIR_OPTIMIZATION_PIPELINE_ROADMAP.md)
  provides proof-rich registration, exclusions, schedules, verified mutation,
  inspection, and structured reporting.
- Existing language and compiler contracts already fix binary64 arithmetic,
  unordered comparison, decimal literal rounding, exact raw-bit operations,
  integer conversion, and checked failure semantics. This roadmap changes
  compile-time realization, not source-visible meaning.
- No other active roadmap blocks the remaining work. Any new out-of-scope
  finding should be recorded in a companion discoveries document and indexed
  only when the first actionable finding exists.

## Scope and invariants

- Implement B64E1 through B64E12 without widening their frozen boundary.
- Add `crates/skald-binary64` as an unpublished workspace crate whose only
  numerical implementation dependency is an exactly pinned published
  `rustc_apfloat` release.
- Keep the wrapper independent of `skald-compiler`, source spans,
  diagnostics, HIR, MIR, passes, drivers, targets, and runtime code.
- Expose only Skald-owned `Binary64`, comparison, integer-conversion, and
  decimal-conversion types. No APFloat type, trait, rounding enum, category,
  parser error, or status flag may escape the crate.
- Exchange binary64 values only as exact `u64` bits. Production compile-time
  evaluation must not accept or return Rust `f64`.
- Use nearest-ties-to-even for binary64 arithmetic, integer-to-floating
  conversion, and decimal parsing; use truncation toward zero followed by
  exact target-range validation for floating-to-integer conversion.
- Preserve both zeroes, subnormals, infinities, and raw NaN representations
  wherever the language promises exact bits.
- Let the binary64 crate calculate arithmetic NaNs, but leave
  NaN-producing arithmetic assignments unfolded in the initial optimizer
  profile so raw-bit observations remain equal under `none` and `default`.
- Fold NaN comparisons, conversion to `bool`, identity, sign negation, and
  raw-bit reinterpretation where their outcomes are exact.
- Reuse the existing `PrimitiveConstant`, primitive evaluator, convergent
  local solver, and primitive folding pass. Add no second floating solver,
  lattice, approximate fact, range fact, or optimization IR.
- Rewrite a checked `f64`-to-integer conversion only when a complete verified
  protocol has an exact successful result. Retain NaN, infinity, and finite
  out-of-range protocols unchanged with their runtime failure behavior.
- Preserve operand evaluation exactly once and in source order, all source
  spans, result types and identities, failure reasons, cleanup, ownership,
  static lifecycle, runtime traces, and deterministic phase products.
- Keep floating algebraic identities, static-failure replacement, floating
  remainder, new rounding modes, total ordering, and source-visible floating
  exceptions outside this roadmap.
- Leave the Skald-written runtime `Str.to_f64` and `Str.from_f64` algorithms
  unchanged; they remain ordinary standard-library code and independent
  conformance evidence.
- Add no C/C++ implementation, FFI, system library, generated-program
  dependency, runtime service, runtime ABI revision, or backend semantic
  lookup.
- Raise the workspace MSRV deliberately if the selected dependency requires
  it; update every affected toolchain statement and pin in the same task.
- Maintain facade-oriented recursive Rust modules with private responsibility
  modules and explicit minimal re-exports.

## Progress

- [x] BE0 — Establish the isolated binary64 crate and dependency boundary
- [x] BE1 — Implement exact binary64 arithmetic and comparison
- [ ] BE2 — Implement conversions and decimal parsing
- [ ] BE3 — Migrate compiler literal rounding to the binary64 authority
- [ ] BE4 — Extend primitive constant evaluation and propagation
- [ ] BE5 — Generalize checked-scalar topology, carriers, and solved facts
- [ ] BE6 — Fold successful checked floating-to-integer protocols
- [ ] BE7 — Harden the complete boundary and close the roadmap

## PR-sized implementation sequence

### BE0 — Establish the isolated binary64 crate and dependency boundary

**Purpose:** Create the stable dependency firewall and exact raw-bit value
model before either compiler phase or optimizer behavior depends on it.

- [x] Add `crates/skald-binary64` to the workspace as an unpublished package
      using workspace edition, lint, and toolchain policy.
- [x] Select and exactly pin one published `rustc_apfloat` release; commit the
      lockfile update and record its Apache-2.0 WITH LLVM-exception license in
      the repository's appropriate third-party dependency notice.
- [x] Verify the dependency with the current workspace Rust version; if it is
      incompatible, raise `workspace.package.rust-version` and update all
      affected development guidance and version-sensitive dependency notes.
- [x] Add a documented `Binary64` facade with private raw bits, `from_bits`,
      `to_bits`, `same_bits`, exact sign negation, and the frozen `is_zero`,
      `is_finite`, `is_infinite`, `is_nan`, and `is_negative` predicates.
- [x] Do not implement `PartialEq`, `Eq`, `PartialOrd`, or `Ord`; require
      callers to select bit identity or numeric comparison explicitly.
- [x] Keep `rustc_apfloat` imports private to the new crate and add an
      auditable repository check or focused test that no other production
      source imports it.
- [x] Establish the facade-oriented module layout with small public ownership
      in `lib.rs`, private responsibility modules, and crate-local tests.
- [x] Document the crate as a compiler-only semantic component that is never
      linked into generated programs.

**Tests:** Raw construction and round trip for arbitrary bits; positive and
negative zero; normal, subnormal, infinity, quiet/signaling NaN signs and
payloads; exact sign inversion; every classification boundary; API compile
tests; dependency-isolation search; package and workspace build checks.

**Gates:** `cargo test --locked -p skald-binary64`; `make build-check`;
`make lint`; `make docs-check`; `make msrv-check`; and `git diff --check`.

**Exit criteria:** The workspace contains one tested, documented, dependency-
isolating crate; raw binary64 facts and classification require no Rust `f64`;
no APFloat vocabulary is visible outside the crate; licensing and the actual
supported toolchain are explicit.

### BE1 — Implement exact binary64 arithmetic and comparison

**Purpose:** Complete the non-conversion numerical core and prove its exact
bit behavior before compiler consumers can activate floating folds.

- [x] Implement binary64 add, subtract, multiply, and divide through
      `rustc_apfloat::ieee::Double` with explicit nearest-ties-to-even
      rounding and exact bit conversion at the private boundary.
- [x] Consume APFloat status results deliberately while keeping overflow,
      underflow, divide-by-zero, and inexact as ordinary binary64 outcomes.
- [x] Return calculated NaN representations from the crate without embedding
      the optimizer's conservative NaN-substitution policy in this layer.
- [x] Add the frozen four-way `Binary64Comparison::{Less, Equal, Greater,
      Unordered}` operation, with both zeroes equal and either NaN unordered.
- [x] Keep numeric comparison distinct from `same_bits`; expose no host
      `f64`, standard comparison trait, or APFloat status/category type.
- [x] Split arithmetic or comparison into private implementation modules only
      when the facade would otherwise mix substantial responsibilities.

**Tests:** Exact and inexact arithmetic; cancellation; signed-zero results;
normal/subnormal boundaries; gradual underflow; overflow to both infinities;
division by both zeroes; zero divided by zero; infinity combinations;
quiet/signaling NaNs with multiple payloads; every ordered relation; either-
operand NaN; fixed bit-vector regressions independent of host `f64`.

**Gates:** Focused crate tests; `cargo test --locked -p skald-binary64`;
`make static-check`; `make msrv-check`; and `git diff --check`.

**Exit criteria:** Every frozen arithmetic and comparison operation has one
host-independent raw-bit implementation and fixed Skald-owned evidence, while
dependency exception state and API types remain private.

### BE2 — Implement conversions and decimal parsing

**Purpose:** Complete the binary64 service required by primitive casts and
source literals without importing compiler types or diagnostics.

- [ ] Add exact nearest-ties-to-even conversion from `i64`, `u64`, `u8`, and
      `bool`, with boolean producing exact positive `0.0` or `1.0`.
- [ ] Add exact `to_bool`, returning false for either zero and true for every
      other binary64 representation, including every NaN.
- [ ] Add `truncating_to_i64`, `truncating_to_u64`, and
      `truncating_to_u8` returning the frozen Skald-owned
      `IntegerConversion::{Value, OutOfRange}` outcome.
- [ ] Validate finiteness, truncate toward zero, and validate the mathematical
      truncated value against the exact target range; accept negative finite
      fractions greater than `-1.0` for unsigned zero and never treat
      `INEXACT` alone as failure.
- [ ] Add nearest-ties-to-even `parse_decimal` over a lexer-validated spelling
      with exact `DecimalConversion::{Finite, Overflow, Invalid}` outcomes.
- [ ] Treat subnormal results and underflow to positive zero as finite, and a
      source spelling rounded to infinity as overflow.
- [ ] Keep raw `f64`/`u64` reinterpretation as direct bit transfer rather than
      an APFloat numeric conversion.
- [ ] Keep dependency parser errors, integer widths, rounding enums, and
      statuses behind the facade.

**Tests:** Values around `2^53`, `2^63`, and `2^64`; integer extrema and
rounding ties; every target integer boundary and adjacent binary64 value;
fractional truncation; negative fractions around zero; signed zero; NaN and
infinity failures; decimal zero, extrema, subnormal, halfway, long-significand,
underflow, overflow, and malformed defensive inputs; fixed raw-bit outcomes.

**Gates:** Focused conversion and decimal tests;
`cargo test --locked -p skald-binary64`; `make static-check`;
`make msrv-check`; and `git diff --check`.

**Exit criteria:** The wrapper exposes the complete frozen compiler-facing API
with exact Skald-owned outcomes, all language-relevant conversion boundaries
are tested without host floating arithmetic, and no compiler policy has leaked
into the crate.

### BE3 — Migrate compiler literal rounding to the binary64 authority

**Purpose:** Make source literals and later optimizer folds share one
compile-time rounding implementation while preserving every source contract.

- [ ] Add the path dependency from `skald-compiler` to `skald-binary64` and
      route validated decimal `f64` spellings through `parse_decimal`.
- [ ] Map `Finite` to the existing `HirExpressionKind::F64Bits` representation
      and `Overflow` to the unchanged `F64_LITERAL_OUT_OF_RANGE` diagnostic.
- [ ] Treat `Invalid` after successful lexical/syntax validation as a
      structured internal contract violation without exposing dependency text
      as a source diagnostic or allowing a compiler panic on ordinary input.
- [ ] Preserve literal grammar, exact spans, positive-zero underflow, finite
      subnormals, source-order behavior, deterministic dumps, and unary-minus
      ownership of negative values.
- [ ] Remove production `str::parse::<f64>()` from literal semantic selection;
      retain host `f64` only in tests or tooling where it is not an expected-
      result oracle.
- [ ] Replace host-computed literal expectations in focused tests with exact
      bit constants or `skald-binary64` results where necessary.
- [ ] Update living compiler phase documentation to describe the implemented
      literal authority while retaining the existing language contract.

**Tests:** Existing literal success/failure suites; minimum/maximum finite,
minimum/maximum subnormal, halfway and adjacent decimal cases; underflow to
zero; overflow diagnostic structure; malformed-token ownership; exact HIR/MIR
dumps; repeated independent compilation determinism.

**Gates:** `cargo test --locked -p skald-binary64`;
`make compiler-test`; focused literal goldens; `make docs-check`;
`make msrv-check`; and `git diff --check`.

**Exit criteria:** Every accepted source floating literal reaches HIR through
the software binary64 authority, existing bits and diagnostics remain stable,
and production type checking no longer uses the Rust host parser for semantic
rounding.

### BE4 — Extend primitive constant evaluation and propagation

**Purpose:** Activate exact pure floating folds through the existing primitive
evaluator and convergent solver without creating parallel optimization state.

- [ ] Add `PrimitiveConstant::F64Bits(u64)` and exact conversion back to
      `MirRvalueKind::ConstantF64Bits`, preserving internal bit equality.
- [ ] Extend exhaustive rvalue, graph, view, test-fixture, and future-variant
      classifications so floating constants participate in the existing
      callable-local solution without becoming approximate facts.
- [ ] Add a private floating-evaluation helper mapping MIR types, operations,
      casts, and predicates to the `skald-binary64` facade; keep MIR ownership
      in the existing primitive evaluator.
- [ ] Fold exact sign negation, identity, `f64`-to-`bool`, integer/bool-to-
      `f64`, and `f64`/`u64` raw-bit reinterpretation.
- [ ] Fold all six floating comparisons through the explicit unordered
      outcome, including either-operand NaN, both zeroes, and infinities.
- [ ] Fold add, subtract, multiply, and divide only when the calculated result
      is not NaN; retain every NaN-producing arithmetic assignment unchanged.
- [ ] Preserve assignment identity, result type, instruction position, span,
      operand evaluation, and all existing unsupported-operation barriers.
- [ ] Keep floating algebraic identities excluded from
      `primitive-algebraic-simplification` and its use-forwarding catalog.
- [ ] Extend deterministic primitive-fold measurements and reporting only
      where the current schema requires family visibility; update living
      phase, reporting, testing, and optimization-catalog documentation with
      implemented pure-fold behavior.
- [ ] Demonstrate propagation through arbitrary supported expression depth
      and independence from pass repetition, iteration order, host build mode,
      and optimization profile selection.

**Tests:** Every supported operation/type pair and malformed mismatch; raw NaN
facts; exact NaN comparisons and boolean conversion; NaN arithmetic retention;
signed zero; subnormal, infinity, and rounding boundaries; raw-bit
reinterpretation; deep and permuted solver graphs; no-op/idempotence/rollback;
exact MIR dumps; `none`/`default` native and raw-bit parity.

**Gates:** Focused primitive evaluator, local-constant, folding, pipeline,
reporting, and backend tests; binary64 golden matrix; VM benchmark golden;
`make check`; `make golden-determinism-test`; `make msrv-check`; and
`git diff --check`.

**Exit criteria:** The existing primitive pass and solver fold the complete
frozen pure binary64 matrix from one exact fact domain, observable results
match the optimization-off native path, and NaN-producing arithmetic plus
floating algebraic identities remain conservative barriers.

### BE5 — Generalize checked-scalar topology, carriers, and solved facts

**Purpose:** Establish reusable proof-rich structural and fact foundations for
checked floating casts before adding their mutation owner.

- [ ] Add an immutable exact topology observation for the verified
      `PrimitiveCastRangeCheck` source carrier, success-only conversion,
      result carrier, failure terminator, join reload, spans, and blocks.
- [ ] Keep floating-cast topology and evaluation separate from integer
      division/shift topology while sharing only genuinely common site,
      snapshot, and error vocabulary.
- [ ] Generalize checked-carrier ownership and certification around explicit
      checked-scalar protocol families, preserving exhaustive storage-use,
      authorization, exact-base, unique-write, dominance, lifetime, type, and
      protocol-use requirements.
- [ ] Add exact checked floating-to-integer evaluation using
      `skald-binary64`, distinguishing successful constants, retained static
      failures, and unsupported/mismatched inputs.
- [ ] Extend the convergent graph and solver so certified floating source
      carriers and successful checked result carriers publish exact facts;
      failures publish no result fact and retain their observation.
- [ ] Preserve all existing integer protocol certificates, solver facts,
      metrics, topology order, and rejection behavior without weakening them
      through a permissive generic abstraction.
- [ ] Return owned deterministic observations and structured malformed-shape
      outcomes; retain no verified-MIR borrow or local identity across a
      commit.
- [ ] Add no production rewrite or pass registration in this task.

**Tests:** Exact topology success for all three integer targets; every missing,
duplicate, reordered, mismatched, protected, aliased, projected, unauthorized,
or lifetime-invalid site; source facts through nested ordinary and checked
expressions; successful result propagation; retained NaN/infinity/range
failures; permutation, convergence, and existing checked-integer regression
coverage.

**Gates:** Focused topology, carrier, graph, solver, and checked-evaluation
tests; `make compiler-test`; `make docs-check`; `make msrv-check`; and
`git diff --check`.

**Exit criteria:** One conservative checked-scalar foundation can prove exact
floating-cast and existing integer protocols without conflating their
semantics; the solver derives every successful supported fact and no failing
fact; no MIR is yet mutated by the new family.

### BE6 — Fold successful checked floating-to-integer protocols

**Purpose:** Add the independently selectable, atomic mutation owner for the
remaining checked constant-conversion family and activate it only with full
semantic evidence.

- [ ] Implement a whole-callable immutable plan for successful constant
      `f64`-to-`i64`, `f64`-to-`u64`, and `f64`-to-`u8` protocols using one
      fresh convergent solution and exact topology/certification evidence.
- [ ] Revalidate the program, callable, topology, carriers, constants, types,
      spans, and absence of conflicting edits before the first mutation.
- [ ] Preserve operand evaluation and the target-typed result while replacing
      the successful checked protocol atomically with its exact integer
      constant and ordinary successor flow.
- [ ] Remove only obsolete protocol-private transient values proven by the
      plan; retain storage/lifetime work until independently eligible cleanup
      owns its removal.
- [ ] Retain NaN, infinity, finite out-of-range, unsupported, malformed, and
      insufficiently proven candidates unchanged with the exact existing
      runtime failure behavior.
- [ ] Register the proof-rich pass as
      `checked-f64-to-integer-constant-folding` with a unique identity,
      description, implementation-stage match, exclusions, inspection labels,
      and deterministic metrics.
- [ ] Place it after primitive folding in the default proof-rich schedule and
      before repeated primitive/dead/CFG cleanup needed to consume its result
      and unreachable failure region.
- [ ] Preserve `none`, all-pass exclusion parity, independent selection,
      no-op seal reuse, atomic rollback, immediate reverification, and stable
      scheduling of every existing pass.
- [ ] Update driver, reporting, testing, compiler phase, architecture, and
      optimization-catalog documentation with the implemented behavior.

**Tests:** All target boundaries and adjacent values; negative fractional
unsigned zero; signed zero; fractional truncation; NaN, infinity, and finite
failure retention; nested checked casts and checked siblings; propagated
sources and results; protected topology; planning conflicts; stale snapshots;
registry/schedule/exclusion/measurement/checkpoint matrices; MIR reduction;
native failure/success traces; `none`/`default` equivalence.

**Gates:** Focused plan, rewrite, registry, runner, driver, reporting, and
backend tests; checked-conversion goldens; VM benchmark golden;
`make check`; `make golden-determinism-test`; `make golden-release-test`;
`make msrv-check`; and `git diff --check`.

**Exit criteria:** Users can discover and disable the checked floating-cast
pass independently; every fully proven successful constant protocol is
rewritten once; every static failure retains runtime semantics; subsequent
passes consume the exact result without a second evaluator.

### BE7 — Harden the complete boundary and close the roadmap

**Purpose:** Finish with one maintainable dependency owner, exhaustive
behavioral evidence, current living documentation, and no rollout residue.

- [ ] Audit the binary64 facade, private modules, primitive evaluator, solver,
      topology, carrier, plan, pass, policy, and reporting owners by
      responsibility; split substantial mixed modules and avoid needless tiny
      files.
- [ ] Prove by repository search that only `skald-binary64` imports
      `rustc_apfloat` and production compile-time binary64 semantics no longer
      use host `f64` arithmetic, parsing, comparisons, or casts.
- [ ] Audit the wrapper API for accidental APFloat, MIR, diagnostic, target,
      or runtime vocabulary and remove unused generality or compatibility
      paths.
- [ ] Recheck fixed vectors against the selected APFloat release and native
      x86-64 behavior, treating native comparison as target evidence rather
      than the semantic definition.
- [ ] Recheck raw-bit equality across `none`, `default`, exclusions, debug,
      release, repeated processes, and deterministic compilation, especially
      every retained NaN-producing expression.
- [ ] Verify unchanged language grammar, diagnostics, runtime string
      conversion, runtime ABI, ownership/lifecycle behavior, proof
      normalization, backend legality, and whole-world retention.
- [ ] Resolve small maintainability issues directly; put larger unrelated
      findings in an indexed discoveries record with evidence, likely owner,
      priority, and bounded later direction.
- [ ] Remove roadmap codes and rollout wording from living code and docs;
      promote implemented details into their authoritative compiler,
      reporting, testing, development, and catalog locations.
- [ ] Complete every roadmap checkbox, archive the design and roadmap, update
      active/archive indexes and incoming links, and retain a discoveries
      record only while actionable work remains.
- [ ] Run the complete ordinary and extended repository gates from an
      artifact-free snapshot and record their evidence before closure.

**Tests:** Every focused suite from earlier tasks; complete wrapper and compiler
tests; full debug/release/determinism goldens; VM benchmark across configured
profiles; supported-toolchain checks; robustness; documentation links/indexes;
dependency isolation; formatting and diff hygiene.

**Gates:** `make check`; `make check-long`; `make golden-determinism-test`;
`make golden-release-test`; `make msrv-check`; `make robustness-long`; and
`git diff --check`.

**Exit criteria:** One narrow unpublished crate owns every production
compile-time binary64 calculation; all pure and successful checked folds are
implemented and independently evidenced; conservative exclusions remain
explicit; living documentation is authoritative; the completed records are
archived with no unresolved high-priority roadmap finding.

## Ordering and dependencies

BE0 establishes the crate, dependency, raw-bit model, license, and actual
toolchain floor before semantic code is built on it. BE1 proves arithmetic and
comparison independently; BE2 completes conversion and decimal parsing on the
same facade. These crate-only steps can be reviewed without MIR mutation.

BE3 migrates the simpler early-phase consumer first and proves that source
literal behavior remains unchanged. BE4 then extends the existing primitive
fact and fold path, including the conservative NaN policy, before checked CFG
work depends on floating constants.

BE5 separates and proves checked floating topology, storage certification,
evaluation, and solved facts without mutation. BE6 can consequently add one
small independently selectable rewrite owner over already tested evidence.
BE7 performs the dependency leak, host-float, future-variant, parity,
documentation, and repository-wide audits only after the entire path exists.

No task should add floating algebraic identities, static-failure replacement,
new floating operations, broader range analysis, or target-specific folding
merely because the binary64 crate can calculate more results. Those remain
separate optimization candidates or later designs.

The ordinary repository gate is `make check`. Manifest and toolchain changes
run `make msrv-check`; behavior-changing optimizer tasks run focused native
and determinism goldens; the closing task runs the full extended gate. The
Makefile remains the local and external automation interface, and this roadmap
adds no repository CI.
