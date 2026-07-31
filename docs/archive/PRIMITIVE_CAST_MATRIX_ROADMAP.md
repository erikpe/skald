# Complete Primitive Cast Matrix Roadmap

Status: complete; CAST0 through CAST8 are complete.

This roadmap implements the sixteen remaining cells of Skald's frozen
[complete explicit primitive cast matrix](../language/TYPES_AND_VALUES.md#frozen-complete-explicit-primitive-cast-matrix)
and migrates the existing nine integer cells into the same cohesive compiler
model. The durable result is one source-to-x86-64 primitive-cast path covering
all twenty-five pairs among `i64`, `u64`, `u8`, `f64`, and `bool`, with
explicit verified control flow only for the three failing `f64`-to-integer
cells.

## Scope and invariants

- The authoritative value, rounding, truncation, failure, evaluation-order,
  representation, target, and ABI contracts are already frozen in the linked
  language and compiler documentation. This roadmap does not reopen them.
- All twenty-five explicit source/target pairs become implemented. The nine
  existing integer-to-integer cells retain their exact modulo and
  two's-complement behavior throughout the migration.
- Primitive casts remain explicit. No initializer, assignment, argument,
  result, arithmetic, comparison, condition, literal, or overload boundary
  gains an implicit conversion or promotion.
- Syntax, resolution, HIR, MIR, facades, dumps, tests, and documentation use
  one cohesive primitive-cast vocabulary. The existing integer-only cast
  vocabulary is migrated atomically rather than retained as a parallel model.
- Same-type casts remain represented operations. In particular, `f64` identity
  preserves every binary64 bit, including signed zero and NaN payload and
  sign.
- The twenty-two non-failing cells are pure rvalues. They evaluate their
  source exactly once and add no block, failure edge, runtime call,
  allocation, ownership action, or cleanup obligation.
- Each `f64`-to-integer cast evaluates and secures its source exactly once,
  checks the mathematical value after truncation toward zero, converts only
  on the success edge, and terminates on failure with
  `floating-point cast out of range`.
- A negative finite fraction greater than `-1.0` is valid for `u64` and `u8`
  and produces zero. NaN, infinities, and post-truncation out-of-range values
  fail. Known constant failures remain runtime failures rather than new
  literal or type-check diagnostics.
- Integer-to-`f64` conversion is correctly rounded to nearest, ties to even,
  using the source signedness. `u64` values above `i64::MAX` cannot be lowered
  through an accidental signed reinterpretation.
- Numeric-to-`bool` conversion is an explicit zero comparison. Floating NaNs,
  infinities, subnormals, and nonzero finite values become `true`; both zeros
  become `false`. Boolean-to-numeric results are canonical zero or one.
- MIR remains target-independent and verification proves the selected
  source/target semantics and checked control-flow shape before backend
  lowering. Target opcodes, registers, and threshold encodings remain private.
- x86-64 realizes every conversion and check inline. No conversion helper,
  link symbol, runtime header change, or runtime ABI version change is added;
  checked failure reuses `ska_rt_panic` and `ska_rt_abi_v6`.
- Niflheim is useful test evidence, not an implementation template. Skald does
  not adopt its conversion runtime helpers, and its tests must specifically
  prevent the pre-truncation unsigned-range inconsistency observed there.
- Implicit, saturating, wrapping floating-to-integer, optional-result,
  recoverable, user-defined, object, optional, array, shared-owner, `Obj`,
  `unit`, and future-primitive conversions remain non-goals.

## Repository gates

Every task runs focused owner tests, `make check`, and `git diff --check`.
Tasks that change Rust phase models, public facades, accepted syntax, or the
backend also run `make msrv-check`. Documentation-only observations use
`make docs-check` and `make docs-test`; this roadmap creates no repository CI
because the Makefile remains the local and external automation interface.

Tests stay with their owners: syntax, resolution, type checking, HIR/MIR,
verification, and backend-unit tests remain colocated; public cross-phase Rust
behavior belongs in the compiler integration-test directory; and complete
compile-failure, stderr, status, assembly, and native observations belong in
the top-level golden corpus. Backend work includes system-assembler acceptance
and deterministic assembly checks. The closing task additionally runs
`make robustness-long` and the full gates from an artifact-free snapshot or
clean checkout.

## Progress

- [x] CAST0 — Establish cohesive primitive-cast front-end vocabulary
- [x] CAST1 — Establish verified pure primitive-cast MIR
- [x] CAST2 — Execute identity and boolean-boundary casts
- [x] CAST3 — Execute integer-to-floating casts
- [x] CAST4 — Enable every non-failing cast from source
- [x] CAST5 — Establish verified checked floating-to-integer control flow
- [x] CAST6 — Execute checked floating-to-integer casts
- [x] CAST7 — Enable checked floating-to-integer casts from source
- [x] CAST8 — Harden and promote the complete matrix

## PR-sized implementation sequence

### CAST0 — Establish cohesive primitive-cast front-end vocabulary

**Purpose:** Replace the integer-only syntax, resolution, and typed-HIR model
before additional semantics create a second cast hierarchy, while preserving
the current nine-cell source availability.

- [x] Generalize primitive cast-target recognition to `i64`, `u64`, `u8`,
      `f64`, and `bool` without disturbing unary precedence, right-associative
      nesting, postfix binding, grouping, recovery, or the common nesting
      budget.
- [x] Keep primitive-keyword casts distinct from nominal and `shared` object
      casts in syntax and resolution. Primitive targets bypass declaration
      lookup; object targets retain their existing lookup and diagnostics.
- [x] Replace public AST and resolved integer-cast nodes and target types with
      cohesive primitive-cast vocabulary, preserving exact target/source/full
      spans and deterministic dumps.
- [x] Replace typed HIR integer-cast vocabulary with an exact primitive source,
      target, and semantic class: identity, integer bits, to `bool`, to `f64`,
      from `bool` to an integer, or checked `f64`-to-integer. Record whether
      the selected class may terminate without encoding backend details.
- [x] Keep type checking as the selection owner and continue constructing HIR
      only for the already implemented nine integer cells. Give syntactically
      valid pending pairs a focused temporary implementation diagnostic so no
      accepted program reaches incomplete MIR.
- [x] Migrate facade exports, public API compile tests, syntax/resolution/HIR
      fixtures, and dump vocabulary in the same change; leave no parallel
      integer-cast public model.
- [x] Update implemented grammar and phase documentation to describe the
      generalized recognized target shape and the still-limited accepted
      matrix without claiming executable new cells.

**Tests:** Primitive/object/shared cast disambiguation; all five primitive
targets; unary, postfix, nested-cast, grouping, recovery, and nesting-limit
cases; AST/resolved/HIR dump determinism; public API compile coverage; all nine
integer regression cells; focused pending-pair diagnostics; repository gates.

**Exit criteria:** Every primitive-keyword cast has one deterministic
front-end shape and one cohesive typed-HIR vocabulary, all existing integer
casts behave unchanged, and none of the remaining sixteen cells can reach
MIR.

### CAST1 — Establish verified pure primitive-cast MIR

**Purpose:** Give all non-failing semantic classes a closed target-independent
representation before any backend or source path depends on them.

- [x] Replace MIR's integer-only cast type and rvalue with one primitive-cast
      operation carrying exact source, target, and selected semantic class.
- [x] Represent the twenty-two non-failing cells as ordinary pure rvalues;
      preserve same-type operations rather than erasing them during lowering.
- [x] Lower each pure typed-HIR class after evaluating its operand once, with
      no block, termination reason, runtime call, storage lifecycle, or
      ownership action introduced by the cast itself.
- [x] Derive control-effect classification from the semantic class: every
      operation in this task remains pure even when its operand contains an
      existing control effect.
- [x] Verify the complete non-failing source/target matrix, semantic-class
      consistency, exact operand/result types, definition-before-use,
      block-local use, and deterministic one-invariant diagnostics.
- [x] Generalize HIR and MIR dump vocabulary, facade exports, direct fixtures,
      and mutation utilities without exposing a target instruction or
      constant-folding decision.
- [x] Keep type-check selection of the thirteen pending non-failing cells
      disabled. Direct HIR/MIR fixtures prove their downstream representation
      while ordinary source compilation remains executable.

**Tests:** Direct HIR/MIR construction for all twenty-two pure pairs; dump
stability; verifier mutations for wrong source, target, semantic class,
operand/result type, definition, and block; exactly-once/control-effect
composition; existing nine source-to-native integer cases; pending-source
rejection; repository gates.

**Exit criteria:** Every non-failing primitive cast can be expressed and
verified as one pure target-independent MIR rvalue, integer behavior is
unchanged, and source availability has not outrun backend support.

### CAST2 — Execute identity and boolean-boundary casts

**Purpose:** Land the ten pure cells that need only bit preservation,
canonical constants, or explicit zero comparison before tackling integer
rounding.

- [x] Extend backend legality and instruction selection for `f64` and `bool`
      identity, all four numeric-to-`bool` casts, and all four
      `bool`-to-numeric casts.
- [x] Preserve complete `f64` identity bits through canonical scalar
      load/store paths, including both zeros, infinities, and representative
      NaN payload/sign patterns.
- [x] Convert integer zero to `false` and every integer nonzero to `true`,
      using signedness-independent equality-to-zero and canonical boolean
      storage.
- [x] Convert both floating zeros to `false` and every other binary64 datum to
      `true`. Make unordered handling explicit so every NaN becomes `true`
      rather than inheriting an ordered-comparison accident.
- [x] Convert `bool` to integer zero/one and exact binary64 `0.0`/`1.0`
      without a helper, branch-dependent uninitialized value, or noncanonical
      result.
- [x] Extend the private machine model only with reusable scalar operations
      justified by these responsibilities; keep semantic class names above
      the target layer.
- [x] Keep source selection of these cells disabled until their complete
      downstream group and integration coverage are ready.

**Tests:** Selector, legality, emission, assembly, and direct-MIR native tests
for all ten cells; integer zero/nonzero and extrema; floating `+0.0`, `-0.0`,
smallest subnormals, finite values, infinities, and multiple NaN bit patterns;
canonical result storage across locals, arguments, results, and comparisons;
absence of runtime calls; repository gates.

**Exit criteria:** All identity and boolean-boundary cells execute from
verified MIR with exact frozen results, deterministic assembly, and no runtime
or ABI change.

### CAST3 — Execute integer-to-floating casts

**Purpose:** Isolate correctly rounded signed and unsigned conversion,
especially the full `u64` domain, behind the already verified pure MIR
boundary.

- [x] Select signed `i64`-to-`f64` conversion with round-to-nearest,
      ties-to-even behavior in the supported floating environment.
- [x] Convert every canonical `u8` exactly without adding a special runtime
      path or weakening its canonical storage invariant.
- [x] Implement full-domain `u64`-to-`f64` inline. Values above `i64::MAX`
      must be converted numerically rather than reinterpreted as negative;
      preserve correct rounding at the high end.
- [x] Extend machine legality, register-use modeling, selection, emission, and
      dumps only as required for the cohesive conversion path.
- [x] Prove the selected algorithm against an independent exact-integer to
      binary64 oracle in tests, including adjacent values around every
      rounding boundary chosen for focused coverage.
- [x] Keep source selection disabled and preserve the unchanged runtime header,
      ABI marker, and reporter surface.

**Tests:** Direct-MIR selector and native matrices for zero, one, signed
negative values, integer extrema, `u8` extrema, values around `2^53`, the
`i64` sign boundary, powers of two, halfway/ties-to-even cases, and values near
`u64::MAX`; exact result-bit assertions; system assembler and deterministic
assembly; absence of helper calls; repository gates.

**Exit criteria:** Each verified integer-to-`f64` operation executes inline
with correctly rounded bits for the complete source domain and no ABI change.

### CAST4 — Enable every non-failing cast from source

**Purpose:** Expose the thirteen newly executable pure cells together so the
language never advertises a partial semantic class or accepts source without a
verified backend path.

- [x] Enable type-check selection of `f64`/`bool` identity, numeric-to-`bool`,
      `bool`-to-numeric, and integer-to-`f64` pairs, producing the semantic
      class already represented in HIR and MIR.
- [x] Remove the temporary pending-pair diagnostic for those thirteen cells;
      retain focused rejection for the three checked `f64`-to-integer cells
      until their control flow is executable.
- [x] Preserve exact source and target typing in every consumer. Do not add
      expected-type inference, contextual literal retyping, or implicit casts.
- [x] Exercise binding initialization, reassignment, arguments, results,
      fields, arrays, optionals, comparisons, arithmetic, conditions after an
      explicit cast to `bool`, and nested primitive casts.
- [x] Prove source evaluation exactly once and ordinary full-expression
      cleanup when operands contain calls, checked operations, selected-path
      expressions, or ownership-bearing effects around the scalar cast.
- [x] Add source-to-native result coverage for all twenty-two now-implemented
      non-failing cells and source diagnostics for nonprimitive sources,
      `unit`, invalid targets, and every implicit-conversion attempt.
- [x] Update grammar, language/compiler overviews, phase/backend descriptions,
      and status wording to identify only checked `f64`-to-integer casts as
      still pending.

**Tests:** Complete twenty-two-cell type-check/HIR/MIR/golden success matrix;
invalid-source and implicit-conversion matrices; precedence, nesting,
evaluation-order, cleanup, consumer, phase-dump, exact-bit, assembly, and
native determinism tests; repository gates.

**Exit criteria:** Every non-failing cell is accepted and executable from
source with the frozen semantics, while each checked cell still stops before
HIR with a focused temporary diagnostic.

### CAST5 — Establish verified checked floating-to-integer control flow

**Purpose:** Encode post-truncation range validity and success-only conversion
as explicit target-independent flow before machine thresholds or conversion
sentinels can obscure the language rule.

- [x] Add the distinct primitive-cast-out-of-range static termination reason
      and map it to the existing exact panic catalog message without changing
      the reporter ABI or link marker.
- [x] Model one checked `f64`-to-integer relation parameterized by exact target
      type. The relation denotes finite source, truncation toward zero, and
      membership of the truncated mathematical integer in the target range;
      it does not expose target comparison constants.
- [x] Lower checked typed HIR by evaluating and securing the source once, then
      emitting the documented range-check diamond. Only the success edge may
      perform conversion, initialize the exact target result carrier, and
      reach the join; failure terminates.
- [x] Mark every checked primitive cast as control-affecting regardless of
      operand purity, and integrate it with nested-expression spill,
      selected-path, loop, return, and cleanup planning.
- [x] Represent the success conversion distinctly enough that verification
      can prove it is dominated by its matching check and cannot be reused for
      another source, target, or block.
- [x] Verify finite/post-truncation range semantics, exact source/target types,
      branch destinations, success-only initialization, matching conversion,
      join reload, terminal failure, block-local definitions, and absence of
      a continuation from failure.
- [x] Add deterministic HIR/MIR vocabulary and malformed-model fixtures while
      keeping source selection of all three checked cells disabled.

**Tests:** Direct HIR/MIR checked shapes for `i64`, `u64`, and `u8`; nested
control-effect and cleanup traces; verifier mutations for mismatched target,
source, relation, operand, result carrier, conversion dominance, branch,
join, failure reason, and illegal successor; dump determinism; static-message
mapping; existing failure regressions; repository gates.

**Exit criteria:** All three checked casts have one explicit verified MIR
diamond whose semantics are post-truncation range membership and whose failure
reaches the existing reporter boundary, with no source exposure yet.

### CAST6 — Execute checked floating-to-integer casts

**Purpose:** Realize the verified checked operation inline on x86-64 without
depending on host-language cast behavior, runtime helpers, or ambiguous target
conversion sentinels.

- [x] Select ordered finite/range checks equivalent to mathematical
      truncation followed by the exact `i64`, `u64`, or `u8` target range.
      Explicitly accept negative fractions greater than `-1.0` for unsigned
      targets and reject NaN and both infinities.
- [x] Perform truncation/conversion only after the matching success check and
      store one canonical exact target result before the join.
- [x] Implement the complete signed and unsigned 64-bit result domains inline;
      do not infer validity solely from a target instruction's sentinel result
      or pass through an unversioned C conversion.
- [x] Lower failure to the existing static-message `ska_rt_panic` path and
      prove it cannot resume, initialize the result, or execute remaining
      source-level cleanup after reporting begins.
- [x] Extend machine legality, flag/register-use modeling, threshold constant
      materialization, selection, emission, and dumps with deterministic
      target-private structure.
- [x] Preserve verified source storage across checks and conversion, including
      nested casts and surrounding control effects, without evaluating or
      reloading an effectful source expression twice.
- [x] Keep ordinary source selection disabled until direct-MIR boundary and
      failure observations are complete.

**Tests:** Direct-MIR selector, legality, assembly, and native success/failure
tests around `-1`, both zeros, `255`/`256`, signed extrema, `2^63`, `2^64`,
adjacent representable binary64 values, large fractions, subnormals,
infinities, quiet/signaling and signed NaNs; exact `-0.5 -> unsigned zero`
regression; status/stderr assertions; success-only result storage; no helper
symbol; system assembler, determinism, and repository gates.

**Exit criteria:** Every verified checked cast either returns the exact
truncated in-range integer or reports the exact frozen failure, inline and
without runtime ABI expansion.

### CAST7 — Enable checked floating-to-integer casts from source

**Purpose:** Complete source availability only after the checked semantic
diamond and all target observations are independently proven.

- [x] Enable type-check selection of `f64` to `i64`, `u64`, and `u8`; remove
      the final temporary implementation diagnostic and accept the complete
      twenty-five-pair matrix.
- [x] Keep nonprimitive sources and targets invalid with focused actual/target
      diagnostics, and keep valid but failing floating constants as runtime
      failures rather than compile-time range errors.
- [x] Exercise each checked result in locals, assignment, arguments, results,
      fields, arrays, optionals, arithmetic, comparisons, nested casts,
      conditions after explicit conversion, loops, and selected-path
      expressions.
- [x] Prove source evaluation exactly once, success-path continuation,
      enclosing-expression ordering, and failure precedence relative to
      operand effects and preexisting checked operations.
- [x] Prove failure does not run later operand effects or promise remaining
      cleanup after the reporter begins, while successful casts retain
      ordinary full-expression cleanup.
- [x] Add source-to-native boundary and failure goldens for all three targets,
      including dynamic values and the representative frozen examples.
- [x] Update living grammar, language/compiler overviews, phase/backend/error
      documents, debugging/testing guidance, and status matrix to describe the
      full matrix as implemented without rollout task codes.

**Tests:** Complete twenty-five-cell source type-check/HIR/MIR matrix; dynamic
and literal success/failure goldens; exact stderr and unsuccessful status;
precedence, nested-control, evaluation-once, failure-order, cleanup, consumer,
phase-dump, assembly, and native determinism coverage; implicit and invalid
conversion regressions; repository gates.

**Exit criteria:** Every frozen primitive cast is accepted from source and
executes with its documented result or failure, documentation claims complete
availability, and no temporary rollout diagnostic remains.

### CAST8 — Harden and promote the complete matrix

**Purpose:** Close the implementation with independent semantic oracles,
optimization parity, current documentation, and no migration-only structure.

- [x] Audit the complete matrix across syntax, resolution, type checking, HIR,
      MIR, verification, control-effect analysis, backend legality, machine
      selection, facade, dump, golden, and documentation owners. Resolve
      high-priority responsibility problems and index lower-priority findings
      separately.
- [x] Add table-driven independent-oracle coverage for all pure conversions
      and checked success/failure decisions, with dense boundary sampling and
      deterministic randomized raw integer/binary64 inputs.
- [x] Confirm constant and dynamically produced operands have identical
      success, result bits, and failure behavior. Exercise both ordinary and
      any optimization/peephole paths so no fold removes effects, changes NaN
      handling, or changes checked-cast success.
- [x] Complete independent-process determinism for diagnostics, AST/resolved/
      HIR/MIR dumps, assembly, stdout, stderr, and process status.
- [x] Confirm generated objects reference no primitive-conversion helper,
      runtime headers and symbols are unchanged, and every artifact retains
      the `ska_rt_abi_v6` marker.
- [x] Remove obsolete integer-only cast names, staging diagnostics, rollout
      comments, and stale pending wording from living code, tests, and
      documentation. Preserve task codes only in roadmap history.
- [x] Mark this roadmap complete, move it to `docs/archive/`, update active and
      archive indexes and incoming links, and leave only separately actionable
      discoveries under `docs/roadmaps/`.

**Tests:** Independent-oracle valid/failure matrices; dense boundaries around
zero, `2^8`, `2^53`, `2^63`, and `2^64`; raw NaN/zero/subnormal/infinity
classes; constant/dynamic and optimization parity; complete compile-failure
and native golden suites; `make check`, `make msrv-check`,
`make robustness-long`, `git diff --check`, documentation/link validation,
runtime ABI inspection, and an artifact-free full gate.

**Exit criteria:** The full primitive cast matrix is independently validated,
deterministic, documented only as current behavior, free of migration-only
vocabulary and ABI expansion, and archived as a completed roadmap.

## Ordering and dependencies

CAST0 is the sole starting task: later work depends on its cohesive public
phase vocabulary. CAST1 then establishes one verified pure MIR boundary shared
by CAST2 and CAST3. Those two backend tasks may proceed independently after
CAST1, but both must finish before CAST4 exposes the non-failing source cells.

CAST5 may begin after CAST0 because it owns the separate checked control-flow
shape, although sequencing it after CAST4 keeps pure-cast migration noise out
of verifier review. CAST6 depends on CAST5's verified relation and diamond;
CAST7 depends on CAST6's executable success and failure paths. CAST8 is the
only broad hardening and closure task and starts after every source cell is
implemented.
